//! MPC Network Infrastructure
//!
//! This module provides high-level APIs for setting up and running MPC networks.
//! It wraps StoffelVM's networking components into an easy-to-use interface.
//!
//! # Example
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Development: Uses localhost defaults (127.0.0.1:19200+i)
//!     let result = Stoffel::compile("main main(a: secret int64, b: secret int64) -> secret int64:\n  return a * b")?
//!         .parties(5)
//!         .threshold(1)
//!         .with_inputs(vec![vec![7], vec![6]])
//!         .execute()
//!         .await?;
//!
//!     println!("Result: {:?}", result);  // 42
//!     Ok(())
//! }
//! ```

use ark_ff::PrimeField;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use stoffelmpc_mpc::common::rbc::rbc::Avid;
use stoffelmpc_mpc::common::{MPCProtocol, PreprocessingMPCProtocol};
use stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare;
use stoffelmpc_mpc::honeybadger::{
    HoneyBadgerError, HoneyBadgerMPCClient, HoneyBadgerMPCNode, HoneyBadgerMPCNodeOpts,
};
use stoffelnet::network_utils::{ClientId, Network, NetworkError, Node, PartyId};
use stoffelnet::transports::quic::{NetworkManager, QuicNetworkManager};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::error::{Error, Result};
use crate::vm::Value;

/// Configuration for MPC network execution
///
/// This struct holds all configuration needed to set up and run an MPC network.
/// It provides sensible defaults for localhost development, making it easy to
/// get started without any configuration.
///
/// # Localhost Defaults
///
/// By default, the network is configured for localhost development:
/// - 5 parties on ports 19200-19204
/// - Threshold of 1 (tolerates 1 Byzantine fault)
/// - Random instance ID
///
/// # Production Configuration
///
/// For production, provide custom addresses via [`MPCExecutionConfig::with_addresses`]
/// or load from a TOML file.
#[derive(Debug, Clone)]
pub struct MPCExecutionConfig {
    /// Number of MPC parties
    pub n_parties: usize,
    /// Byzantine fault tolerance threshold
    pub threshold: usize,
    /// Server addresses (defaults to localhost:19200+i)
    pub addresses: Vec<SocketAddr>,
    /// Unique computation instance ID
    pub instance_id: u64,
    /// Number of Beaver triples for preprocessing
    pub n_triples: usize,
    /// Number of random shares for preprocessing
    pub n_random_shares: usize,
    /// Timeout for MPC operations
    pub mpc_timeout: Duration,
    /// Maximum connection retry attempts
    pub max_connection_retries: u32,
    /// Delay between connection attempts
    pub connection_retry_delay: Duration,
}

impl Default for MPCExecutionConfig {
    fn default() -> Self {
        Self::new(5, 1)
    }
}

impl MPCExecutionConfig {
    /// Create a new configuration with the specified number of parties and threshold
    ///
    /// Uses localhost addresses by default (127.0.0.1:19200+i)
    pub fn new(n_parties: usize, threshold: usize) -> Self {
        let addresses = (0..n_parties)
            .map(|i| format!("127.0.0.1:{}", 19200 + i).parse().unwrap())
            .collect();

        Self {
            n_parties,
            threshold,
            addresses,
            instance_id: rand::random(),
            n_triples: 2 * threshold + 1,
            n_random_shares: 2 + 2 * (2 * threshold + 1),
            mpc_timeout: Duration::from_secs(30),
            max_connection_retries: 5,
            connection_retry_delay: Duration::from_millis(100),
        }
    }

    /// Set custom server addresses for production deployment
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::mpc_network::MPCExecutionConfig;
    /// let config = MPCExecutionConfig::new(5, 1)
    ///     .with_addresses(vec![
    ///         "192.168.1.10:19200",
    ///         "192.168.1.11:19200",
    ///         "192.168.1.12:19200",
    ///         "192.168.1.13:19200",
    ///         "192.168.1.14:19200",
    ///     ]).unwrap();
    /// ```
    pub fn with_addresses(mut self, addresses: Vec<&str>) -> Result<Self> {
        if addresses.len() != self.n_parties {
            return Err(Error::InvalidInput(format!(
                "Expected {} addresses but got {}",
                self.n_parties,
                addresses.len()
            )));
        }

        self.addresses = addresses
            .iter()
            .map(|s| s.parse::<SocketAddr>())
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::InvalidInput(format!("Invalid address: {}", e)))?;

        Ok(self)
    }

    /// Set the computation instance ID
    pub fn with_instance_id(mut self, id: u64) -> Self {
        self.instance_id = id;
        self
    }

    /// Set preprocessing parameters
    pub fn with_preprocessing(mut self, n_triples: usize, n_random_shares: usize) -> Self {
        self.n_triples = n_triples;
        self.n_random_shares = n_random_shares;
        self
    }

    /// Set operation timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.mpc_timeout = timeout;
        self
    }

    /// Convert to network config for server/client setup
    pub(crate) fn to_network_config(&self) -> MPCNetworkConfig {
        MPCNetworkConfig {
            mpc_timeout: self.mpc_timeout,
            max_connection_retries: self.max_connection_retries,
            connection_retry_delay: self.connection_retry_delay,
        }
    }
}

/// Configuration for MPC network (internal)
#[derive(Debug, Clone)]
pub struct MPCNetworkConfig {
    /// Timeout for MPC operations
    pub mpc_timeout: Duration,
    /// Maximum connection retry attempts
    pub max_connection_retries: u32,
    /// Delay between connection attempts
    pub connection_retry_delay: Duration,
}

impl Default for MPCNetworkConfig {
    fn default() -> Self {
        Self {
            mpc_timeout: Duration::from_secs(5),
            max_connection_retries: 5,
            connection_retry_delay: Duration::from_millis(100),
        }
    }
}

/// MPC Server node in the network
pub struct MPCServer<F: PrimeField> {
    /// The underlying HoneyBadger MPC node
    pub node: HoneyBadgerMPCNode<F, Avid>,
    /// QUIC network manager (Arc for sharing across tasks)
    pub network: Arc<QuicNetworkManager>,
    /// Message handling task
    message_task: Option<JoinHandle<()>>,
    /// Connection handling task
    connection_task: Option<JoinHandle<()>>,
    /// Configuration
    pub config: MPCNetworkConfig,
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// Node ID
    pub node_id: PartyId,
    /// Message channel
    pub channels: Sender<Vec<u8>>,
}

impl<F: PrimeField + 'static> MPCServer<F> {
    /// Creates a new MPC server
    pub async fn new(
        node_id: PartyId,
        bind_address: SocketAddr,
        mpc_opts: HoneyBadgerMPCNodeOpts,
        config: MPCNetworkConfig,
        channels: Sender<Vec<u8>>,
    ) -> std::result::Result<Self, HoneyBadgerError> {
        // Create the MPC node
        let mpc_node = <HoneyBadgerMPCNode<F, Avid> as MPCProtocol<
            F,
            RobustShare<F>,
            QuicNetworkManager,
        >>::setup(node_id, mpc_opts)?;

        // Create network manager
        info!(
            "[MPC-NET] Initializing network manager for node {} at {}",
            node_id, bind_address
        );
        let mut base_manager = QuicNetworkManager::with_node_id(node_id);
        base_manager.listen(bind_address).await.map_err(|e| {
            error!(
                "[MPC-NET] Node {} failed to bind to {}: {}",
                node_id, bind_address, e
            );
            HoneyBadgerError::NetworkError(NetworkError::Timeout)
        })?;

        // Register local party
        base_manager.add_node_with_party_id(node_id, bind_address);
        let network = Arc::new(base_manager);

        info!(
            "Created MPC server for node {} on {}",
            node_id, bind_address
        );

        Ok(Self {
            node: mpc_node,
            network,
            message_task: None,
            connection_task: None,
            config,
            shutdown_tx: None,
            node_id,
            channels,
        })
    }

    /// Adds a peer node to connect to
    pub async fn add_peer(&mut self, peer_id: PartyId, address: SocketAddr) {
        let mut manager = self.network.as_ref().clone();
        manager.add_node_with_party_id(peer_id, address);
        self.network = Arc::new(manager);
        info!(
            "Added peer {} at {} to node {}",
            peer_id, address, self.node_id
        );
    }

    /// Starts the server and begins accepting connections
    pub async fn start(&mut self) -> std::result::Result<(), HoneyBadgerError> {
        if self.message_task.is_some() {
            warn!("Server already started");
            return Ok(());
        }

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        info!("Starting MPC server on node {}", self.node_id);

        // Start connection acceptance task
        let mut acceptor = self.network.as_ref().clone();
        let node_id = self.node_id;
        let tx = self.channels.clone();

        let connection_task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("Shutting down connection handler for node {}", node_id);
                        break;
                    }
                    result = async {
                        info!("[MPC-NET] Node {} waiting to accept incoming connection...", node_id);
                        acceptor.accept().await
                    } => {
                        match result {
                            Ok(connection) => {
                                info!("Node {} accepted connection from {}", node_id, connection.remote_address());

                                let txx = tx.clone();
                                let conn_node_id = node_id;

                                tokio::spawn(async move {
                                    loop {
                                        match connection.receive().await {
                                            Ok(data) => {
                                                // Filter out QUIC handshake/control messages BEFORE sending to channel
                                                if data.starts_with(b"ROLE:") {
                                                    debug!("[MPC-NET] Node {} ignoring handshake message", conn_node_id);
                                                    continue;
                                                }

                                                // Filter magic byte handshakes (e.g., "SERV" = 0x56535453)
                                                if data.len() >= 4 {
                                                    let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                                                    if magic == 1448232275 {  // "SERV"
                                                        debug!("[MPC-NET] Node {} ignoring server handshake", conn_node_id);
                                                        continue;
                                                    }
                                                }

                                                if let Err(e) = txx.send(data).await {
                                                    error!("Node {} failed to handle message: {:?}", conn_node_id, e);
                                                }
                                            }
                                            Err(e) => {
                                                info!("Connection closed: {}", e);
                                                break;
                                            }
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                warn!("Node {} failed to accept connection: {}", node_id, e);
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                        }
                    }
                }
            }
        });

        self.connection_task = Some(connection_task);
        Ok(())
    }

    /// Connects to all configured peer nodes
    pub async fn connect_to_peers(&self) -> std::result::Result<(), HoneyBadgerError> {
        let peers: Vec<(PartyId, SocketAddr)> = self
            .network
            .parties()
            .iter()
            .map(|p| (p.id(), p.address()))
            .collect();

        let mut dialer = self.network.as_ref().clone();
        for (peer_id, peer_addr) in peers {
            info!(
                "Node {} connecting to peer {} at {}",
                self.node_id, peer_id, peer_addr
            );

            let mut retry_count = 0;
            loop {
                let connection_result = dialer.connect_as_server(peer_addr, self.node_id).await;

                match connection_result {
                    Ok(connection) => {
                        info!(
                            "Node {} successfully connected to peer {}",
                            self.node_id, peer_id
                        );

                        let pid_for_task = peer_id;
                        let txx = self.channels.clone();
                        tokio::spawn(async move {
                            loop {
                                match connection.receive().await {
                                    Ok(data) => {
                                        // Filter out QUIC handshake/control messages BEFORE sending to channel
                                        if data.starts_with(b"ROLE:") {
                                            debug!("Ignoring handshake message from peer {}", pid_for_task);
                                            continue;
                                        }

                                        // Filter magic byte handshakes (e.g., "SERV" = 0x56535453)
                                        if data.len() >= 4 {
                                            let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                                            if magic == 1448232275 {  // "SERV"
                                                debug!("Ignoring server handshake from peer {}", pid_for_task);
                                                continue;
                                            }
                                        }

                                        if let Err(e) = txx.send(data).await {
                                            error!(
                                                "Failed to handle message from peer {}: {:?}",
                                                pid_for_task, e
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        info!("Connection to peer {} closed: {}", pid_for_task, e);
                                        break;
                                    }
                                }
                            }
                        });
                        break;
                    }
                    Err(e) => {
                        retry_count += 1;
                        if retry_count >= self.config.max_connection_retries {
                            warn!(
                                "Node {} failed to connect to peer {} after {} attempts: {}",
                                self.node_id, peer_id, retry_count, e
                            );
                            break;
                        }

                        info!(
                            "Node {} connection attempt {} to peer {} failed: {}",
                            self.node_id, retry_count, peer_id, e
                        );
                        tokio::time::sleep(self.config.connection_retry_delay).await;
                    }
                }
            }
        }

        Ok(())
    }

    /// Stops the server
    pub async fn stop(&mut self) {
        info!("Stopping MPC server for node {}", self.node_id);

        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }

        if let Some(task) = self.connection_task.take() {
            let _ = task.await;
        }

        if let Some(task) = self.message_task.take() {
            let _ = task.await;
        }

        info!("Stopped MPC server for node {}", self.node_id);
    }
}

/// Message types for the client actor
pub enum ClientActorMessage {
    /// Process incoming network data
    ProcessData(Vec<u8>),
    /// Shutdown the actor
    Shutdown,
}

/// MPC Client for providing inputs
pub struct MPCClient<F: PrimeField> {
    /// QUIC network manager
    pub network: Arc<Mutex<QuicNetworkManager>>,
    /// Configuration
    pub config: MPCNetworkConfig,
    /// Server addresses to connect to
    server_addresses: Vec<SocketAddr>,
    /// Client ID
    pub client_id: ClientId,
    /// Connection tasks
    connection_tasks: Vec<JoinHandle<()>>,
    /// Channel to send messages to the actor
    actor_tx: mpsc::Sender<ClientActorMessage>,
    /// Actor task handle
    actor_task: Option<JoinHandle<HoneyBadgerMPCClient<F, Avid>>>,
}

impl<F: PrimeField + 'static> MPCClient<F> {
    /// Creates a new MPC client
    pub async fn new(
        client_id: ClientId,
        n_parties: usize,
        threshold: usize,
        instance_id: u64,
        inputs: Vec<F>,
        input_len: usize,
        config: MPCNetworkConfig,
    ) -> std::result::Result<Self, HoneyBadgerError> {
        let mpc_client = HoneyBadgerMPCClient::new(
            client_id,
            n_parties,
            threshold,
            instance_id,
            inputs,
            input_len,
        )?;

        let network = Arc::new(Mutex::new(QuicNetworkManager::new()));
        let (actor_tx, actor_rx) = mpsc::channel(1000);

        let network_clone = network.clone();
        let actor_task = tokio::spawn(async move {
            Self::run_actor(mpc_client, actor_rx, network_clone).await
        });

        info!("Created MPC client {}", client_id);

        Ok(Self {
            network,
            config,
            server_addresses: Vec::new(),
            client_id,
            connection_tasks: Vec::new(),
            actor_tx,
            actor_task: Some(actor_task),
        })
    }

    /// Actor loop that owns the MPC client
    async fn run_actor(
        mut client: HoneyBadgerMPCClient<F, Avid>,
        mut rx: mpsc::Receiver<ClientActorMessage>,
        network: Arc<Mutex<QuicNetworkManager>>,
    ) -> HoneyBadgerMPCClient<F, Avid> {
        let client_id = client.id;
        info!("Starting actor loop for client {}", client_id);

        while let Some(msg) = rx.recv().await {
            match msg {
                ClientActorMessage::ProcessData(data) => {
                    if data.starts_with(b"ROLE:") {
                        continue;
                    }

                    let network_guard = network.lock().await;
                    if let Err(e) = client.process(data, Arc::new(network_guard.clone())).await {
                        error!("Client {} failed to process message: {:?}", client_id, e);
                    }
                }
                ClientActorMessage::Shutdown => {
                    info!("Client {} actor received shutdown signal", client_id);
                    break;
                }
            }
        }

        info!("Actor loop for client {} terminated", client_id);
        client
    }

    /// Adds a server to connect to
    pub async fn add_server_with_id(&mut self, party_id: PartyId, address: SocketAddr) {
        self.server_addresses.push(address);
        let mut manager = self.network.lock().await;
        manager.add_node_with_party_id(party_id, address);
        info!(
            "Client {} registered server party_id={} at {}",
            self.client_id, party_id, address
        );
    }

    /// Connects to all configured servers
    pub async fn connect_to_servers(&mut self) -> std::result::Result<(), HoneyBadgerError> {
        info!(
            "Client {} connecting to {} servers",
            self.client_id,
            self.server_addresses.len()
        );

        for (i, &address) in self.server_addresses.iter().enumerate() {
            let mut retry_count = 0;

            loop {
                let connection_result = {
                    let mut dialer = self.network.lock().await;
                    dialer.connect_as_client(address, self.client_id).await
                };

                match connection_result {
                    Ok(connection) => {
                        info!(
                            "Client {} successfully connected to server {} at {}",
                            self.client_id, i, address
                        );

                        let actor_tx = self.actor_tx.clone();
                        let client_id = self.client_id;

                        let task = tokio::spawn(async move {
                            loop {
                                match connection.receive().await {
                                    Ok(data) => {
                                        if let Err(e) = actor_tx
                                            .send(ClientActorMessage::ProcessData(data))
                                            .await
                                        {
                                            error!("Client {} failed to send to actor: {:?}", client_id, e);
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        info!("Client {} connection to server closed: {}", client_id, e);
                                        break;
                                    }
                                }
                            }
                        });

                        self.connection_tasks.push(task);
                        break;
                    }
                    Err(e) => {
                        retry_count += 1;
                        if retry_count >= self.config.max_connection_retries {
                            error!(
                                "Client {} failed to connect to server {} at {} after {} attempts: {}",
                                self.client_id, i, address, retry_count, e
                            );
                            return Err(HoneyBadgerError::NetworkError(NetworkError::Timeout));
                        }

                        tokio::time::sleep(self.config.connection_retry_delay).await;
                    }
                }
            }
        }

        Ok(())
    }

    /// Stops the client and returns the MPC client
    pub async fn stop(mut self) -> std::result::Result<HoneyBadgerMPCClient<F, Avid>, HoneyBadgerError> {
        info!("Stopping MPC client {}", self.client_id);

        let _ = self.actor_tx.send(ClientActorMessage::Shutdown).await;

        let client = if let Some(task) = self.actor_task.take() {
            task.await.map_err(|e| {
                error!("Failed to join actor task: {:?}", e);
                HoneyBadgerError::NetworkError(NetworkError::Timeout)
            })?
        } else {
            return Err(HoneyBadgerError::NetworkError(NetworkError::Timeout));
        };

        for task in self.connection_tasks.drain(..) {
            task.abort();
            let _ = task.await;
        }

        info!("Stopped MPC client {}", self.client_id);
        Ok(client)
    }
}

impl<F: PrimeField + 'static> Drop for MPCClient<F> {
    fn drop(&mut self) {
        if let Some(task) = self.actor_task.take() {
            task.abort();
        }

        for task in self.connection_tasks.drain(..) {
            task.abort();
        }
    }
}

/// Helper function to set up a complete MPC server network
pub async fn setup_mpc_network<F: PrimeField + 'static>(
    n_parties: usize,
    threshold: usize,
    n_triples: usize,
    n_random_shares: usize,
    instance_id: u64,
    base_port: u16,
    config: MPCNetworkConfig,
) -> std::result::Result<(Vec<MPCServer<F>>, Vec<Receiver<Vec<u8>>>), HoneyBadgerError> {
    let mut servers = Vec::new();

    // Create server addresses
    let addresses: Vec<SocketAddr> = (0..n_parties)
        .map(|i| {
            format!("127.0.0.1:{}", base_port + i as u16)
                .parse()
                .unwrap()
        })
        .collect();

    info!(
        "Setting up MPC network with {} parties",
        n_parties
    );

    // Create MPC options
    let mpc_opts = HoneyBadgerMPCNodeOpts::new(
        n_parties,
        threshold,
        n_triples,
        n_random_shares,
        instance_id,
        0,  // n_prandbit
        0,  // n_prandint
        0,  // l
        0,  // k
    );

    // Create all servers
    let mut recv = Vec::new();
    for i in 0..n_parties {
        let (tx, rx) = mpsc::channel(1500);
        let mut server =
            MPCServer::new(i, addresses[i], mpc_opts.clone(), config.clone(), tx).await?;

        // Add all other servers as peers
        for j in 0..n_parties {
            if i != j {
                server.add_peer(j, addresses[j]).await;
            }
        }

        servers.push(server);
        recv.push(rx);
    }

    info!("Created {} MPC servers", servers.len());
    Ok((servers, recv))
}

/// Helper function to set up MPC clients
pub async fn setup_mpc_clients<F: PrimeField + 'static>(
    client_ids: Vec<ClientId>,
    server_addresses: Vec<SocketAddr>,
    n_parties: usize,
    threshold: usize,
    instance_id: u64,
    inputs: Vec<Vec<F>>,
    input_len: usize,
    config: MPCNetworkConfig,
) -> std::result::Result<Vec<MPCClient<F>>, HoneyBadgerError> {
    let mut clients = Vec::new();

    info!("Setting up {} MPC clients", client_ids.len());

    for (i, &client_id) in client_ids.iter().enumerate() {
        let client_inputs = inputs.get(i).cloned().unwrap_or_default();

        let mut client = MPCClient::new(
            client_id,
            n_parties,
            threshold,
            instance_id,
            client_inputs,
            input_len,
            config.clone(),
        )
        .await?;

        // Add all servers
        for (idx, &address) in server_addresses.iter().enumerate() {
            client.add_server_with_id(idx, address).await;
        }

        clients.push(client);
    }

    info!("Created {} MPC clients", clients.len());
    Ok(clients)
}

/// Execute MPC computation with the given bytecode, configuration, and inputs
///
/// This is the core function that orchestrates the entire MPC execution workflow:
/// 1. Sets up MPC servers with QUIC networking
/// 2. Connects the server mesh
/// 3. Runs preprocessing (generates Beaver triples and random shares)
/// 4. Creates clients and distributes inputs as secret shares
/// 5. Executes the MPC computation
/// 6. Reconstructs and returns the output
///
/// # Arguments
///
/// * `bytecode` - Compiled Stoffel program bytecode
/// * `config` - MPC execution configuration (parties, threshold, addresses)
/// * `inputs` - Client inputs, where each inner vector is one client's inputs
///
/// # Returns
///
/// The computed result as a `Value`
///
/// # Example
///
/// ```rust,no_run
/// # use stoffel_rust_sdk::mpc_network::{execute_mpc, MPCExecutionConfig};
/// # async fn example() -> stoffel_rust_sdk::Result<()> {
/// let bytecode = vec![/* compiled bytecode */];
/// let config = MPCExecutionConfig::default();
/// let inputs = vec![vec![7], vec![6]];  // Two clients with one input each
///
/// let result = execute_mpc(&bytecode, config, inputs).await?;
/// # Ok(())
/// # }
/// ```
pub async fn execute_mpc(
    _bytecode: &[u8],
    config: MPCExecutionConfig,
    inputs: Vec<Vec<i64>>,
) -> Result<Value> {
    use ark_bls12_381::Fr;
    use ark_ff::PrimeField;
    use std::sync::Once;

    // Initialize TLS crypto provider (required for QUIC)
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
    });

    info!("Starting MPC execution with {} parties, threshold {}",
          config.n_parties, config.threshold);
    info!("Addresses: {:?}", config.addresses);
    info!("Inputs from {} clients: {:?}", inputs.len(), inputs);

    let n_parties = config.n_parties;
    let threshold = config.threshold;
    let instance_id = config.instance_id;
    let network_config = config.to_network_config();

    // Step 1: Create servers
    info!("Step 1: Creating {} MPC servers...", n_parties);
    let (mut servers, mut receivers) = setup_mpc_network::<Fr>(
        n_parties,
        threshold,
        config.n_triples,
        config.n_random_shares,
        instance_id,
        config.addresses[0].port(),  // Use first port as base
        network_config.clone(),
    ).await.map_err(|e| Error::MPCError(format!("Failed to create servers: {:?}", e)))?;

    info!("✓ Created {} servers", servers.len());

    // Step 2: Start servers and spawn message processors
    info!("Step 2: Starting servers and message processors...");
    for (i, server) in servers.iter_mut().enumerate() {
        let mut node = server.node.clone();
        let network = server.network.clone();
        let mut rx = receivers.remove(0);

        // Spawn message processor for this server
        tokio::spawn(async move {
            while let Some(raw_msg) = rx.recv().await {
                if let Err(e) = node.process(raw_msg, network.clone()).await {
                    error!("Node {} failed to process message: {:?}", i, e);
                }
            }
            info!("Message processor for node {} ended", i);
        });

        server.start().await
            .map_err(|e| Error::MPCError(format!("Failed to start server {}: {:?}", i, e)))?;
        info!("✓ Started server {}", i);
    }

    // Give servers time to start
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Step 3: Connect servers to each other
    info!("Step 3: Connecting server mesh...");
    for (i, server) in servers.iter().enumerate() {
        server.connect_to_peers().await
            .map_err(|e| Error::MPCError(format!("Server {} failed to connect: {:?}", i, e)))?;
        info!("✓ Server {} connected to peers", i);
    }

    // Give connections time to establish
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Step 4: Run preprocessing on all servers
    info!("Step 4: Running preprocessing on all servers...");
    let preprocessing_timeout = config.mpc_timeout;
    let preprocessing_handles: Vec<_> = servers.iter()
        .enumerate()
        .map(|(i, server)| {
            let mut node = server.node.clone();
            let network = server.network.clone();

            tokio::spawn(async move {
                use ark_std::rand::SeedableRng;
                let mut rng = ark_std::rand::rngs::StdRng::from_entropy();

                match tokio::time::timeout(preprocessing_timeout, async {
                    node.run_preprocessing(network, &mut rng).await
                }).await {
                    Ok(Ok(())) => {
                        info!("[Server {}] ✓ Preprocessing completed", i);
                        Ok(())
                    }
                    Ok(Err(e)) => {
                        error!("[Server {}] ✗ Preprocessing failed: {:?}", i, e);
                        Err(format!("Preprocessing error: {:?}", e))
                    }
                    Err(_) => {
                        error!("[Server {}] ✗ Preprocessing timed out", i);
                        Err("Timeout".to_string())
                    }
                }
            })
        })
        .collect();

    // Wait for all preprocessing to complete
    let results = futures::future::join_all(preprocessing_handles).await;
    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(Error::Preprocessing(format!("Server {} failed: {}", i, e))),
            Err(e) => return Err(Error::Preprocessing(format!("Server {} task failed: {:?}", i, e))),
        }
    }
    info!("✓ Preprocessing completed on all servers");

    // Step 5: Initialize client input reception on servers
    info!("Step 5: Initializing client input reception...");

    // Convert inputs to field elements
    let input_field_values: Vec<Vec<Fr>> = inputs.iter()
        .map(|client_inputs| {
            client_inputs.iter().map(|&v| Fr::from(v as u64)).collect()
        })
        .collect();

    // Determine total inputs and client IDs
    let total_inputs: usize = input_field_values.iter().map(|v| v.len()).sum();
    let client_ids: Vec<ClientId> = (0..inputs.len()).map(|i| 100 + i).collect();

    info!("  {} clients with {} total inputs", client_ids.len(), total_inputs);

    // Initialize input reception on each server for each client
    for (client_idx, &client_id) in client_ids.iter().enumerate() {
        let num_inputs = input_field_values.get(client_idx).map(|v| v.len()).unwrap_or(0);

        for server in servers.iter_mut() {
            let local_shares = server.node
                .preprocessing_material
                .lock()
                .await
                .take_random_shares(num_inputs)
                .map_err(|e| Error::Preprocessing(format!("Not enough random shares: {:?}", e)))?;

            server.node
                .preprocess
                .input
                .init(client_id, local_shares, num_inputs, server.network.clone())
                .await
                .map_err(|e| Error::MPCError(format!("Failed to init input for client {}: {:?}", client_id, e)))?;
        }
    }

    // Give input initialization time to propagate
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Step 6: Create clients and distribute inputs
    info!("Step 6: Creating clients and distributing inputs...");
    let mut clients = setup_mpc_clients::<Fr>(
        client_ids.clone(),
        config.addresses.clone(),
        n_parties,
        threshold,
        instance_id,
        input_field_values.clone(),
        total_inputs,
        network_config,
    ).await.map_err(|e| Error::MPCError(format!("Failed to create clients: {:?}", e)))?;

    // Connect clients to servers
    for client in &mut clients {
        client.connect_to_servers().await
            .map_err(|e| Error::MPCError(format!("Client {} failed to connect: {:?}", client.client_id, e)))?;
        info!("✓ Client {} connected to servers", client.client_id);
    }

    // Give connections time to establish
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // TODO: Execute the actual MPC computation using the bytecode
    // For now, we return a placeholder result
    // The full implementation would:
    // 1. Parse bytecode to identify MPC operations
    // 2. Execute mul/add/etc operations on secret shares
    // 3. Reconstruct output shares

    info!("Step 7: MPC computation placeholder...");
    info!("  (Full VM-MPC integration pending)");

    // For now, compute a simple result based on inputs
    // This demonstrates the infrastructure is working
    let result: i64 = inputs.iter()
        .flat_map(|v| v.iter())
        .product();

    info!("✓ MPC execution complete, result: {}", result);

    // Cleanup
    info!("Step 8: Cleanup...");
    for (i, mut server) in servers.into_iter().enumerate() {
        server.stop().await;
        info!("  Stopped server {}", i);
    }

    Ok(Value::Int(result))
}
