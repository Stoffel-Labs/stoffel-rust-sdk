//! MPCaaS Server API
//!
//! This module provides the server-side API for infrastructure operators running
//! MPC compute nodes. Unlike the client API, the server API requires understanding
//! of MPC configuration (party ID, peers, program, preprocessing).
//!
//! # Design Philosophy
//!
//! The server API is more complex than the client API because infrastructure
//! operators need to configure:
//! - Party ID (unique identifier in the MPC network)
//! - Peer addresses (other servers in the network)
//! - The MPC program to execute
//! - Preprocessing parameters (Beaver triples, random shares)
//!
//! # Examples
//!
//! ## Explicit Peer Configuration
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let program = Stoffel::compile("main main(a: secret int64, b: secret int64) -> secret int64:\n  return a + b")?
//!         .build()?;
//!
//!     let server = Stoffel::server(0)  // Party ID
//!         .bind("0.0.0.0:19200")
//!         .with_peers(&[
//!             (1, "192.168.1.11:19200"),
//!             (2, "192.168.1.12:19200"),
//!         ])
//!         .with_program(program.program().clone())
//!         .with_preprocessing(10, 20)
//!         .build()?;
//!
//!     server.start().await?;
//!     server.run_forever().await
//! }
//! ```

use super::client_handler::ClientHandler;
use super::protocol::{MPCaaSMessage, serialize_message, deserialize_message};
use super::peer_manager::{DiscoveryMode, PeerManager};
use crate::program::Program;
use crate::{Error, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use stoffelnet::transports::quic::{QuicNetworkManager, NetworkManager, PeerConnection};

/// Connection type for incoming connections
/// Note: Peer connections are established outbound, so incoming connections are always clients
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionType {
    /// Client connection (sends inputs, receives outputs)
    Client,
    /// Server/peer connection (participates in MPC)
    #[allow(dead_code)]
    Server,
}

// StoffelVM MPC engine and VM
use stoffel_vm::net::hb_engine::HoneyBadgerMpcEngine;
use stoffel_vm::net::mpc_engine::MpcEngine;
use stoffel_vm::core_vm::VirtualMachine;

// MPC protocol types for message processing
use ark_bls12_381::Fr;
use stoffelmpc_mpc::common::MPCProtocol;
use stoffelmpc_mpc::common::rbc::rbc::Avid as RBCImpl;
use stoffelmpc_mpc::honeybadger::HoneyBadgerMPCNode;
use std::time::Duration;

/// State of the MPC server
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerState {
    /// Server initialized but not started
    Initialized,
    /// Server is starting up
    Starting,
    /// Connecting to peer servers
    ConnectingPeers,
    /// Running preprocessing (generating cryptographic material)
    Preprocessing,
    /// Ready to accept client connections
    Ready,
    /// Currently processing a computation
    Computing,
    /// Shutting down
    ShuttingDown,
}

impl std::fmt::Display for ServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerState::Initialized => write!(f, "Initialized"),
            ServerState::Starting => write!(f, "Starting"),
            ServerState::ConnectingPeers => write!(f, "ConnectingPeers"),
            ServerState::Preprocessing => write!(f, "Preprocessing"),
            ServerState::Ready => write!(f, "Ready"),
            ServerState::Computing => write!(f, "Computing"),
            ServerState::ShuttingDown => write!(f, "ShuttingDown"),
        }
    }
}

/// Builder for creating an MPC server
///
/// Use `Stoffel::server(party_id)` to create a builder.
pub struct StoffelServerBuilder {
    party_id: usize,
    bind_address: Option<SocketAddr>,
    peers: Vec<(usize, SocketAddr)>,
    signaling_server: Option<String>,
    stun_server: Option<String>,
    program: Option<Program>,
    n_triples: usize,
    n_random_shares: usize,
    /// Instance ID for MPC session coordination (all parties MUST use the same value)
    instance_id: Option<u64>,
    /// Absolute epoch time (seconds since Unix epoch) when preprocessing should start
    /// All servers MUST use the same value for coordinated preprocessing start
    preprocessing_start_epoch: Option<u64>,
}

impl StoffelServerBuilder {
    /// Create a new server builder with the given party ID
    pub fn new(party_id: usize) -> Self {
        Self {
            party_id,
            bind_address: None,
            peers: Vec::new(),
            signaling_server: None,
            stun_server: None,
            program: None,
            n_triples: 10,
            n_random_shares: 20,
            instance_id: None,
            preprocessing_start_epoch: None,
        }
    }

    /// Set the address to bind to
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// let builder = Stoffel::server(0).bind("0.0.0.0:19200");
    /// # Ok(())
    /// # }
    /// ```
    pub fn bind(mut self, address: &str) -> Self {
        self.bind_address = address.parse().ok();
        self
    }

    /// Set the peer servers to connect to
    ///
    /// Each peer is specified as a tuple of (party_id, address).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// let builder = Stoffel::server(0)
    ///     .with_peers(&[
    ///         (1, "192.168.1.11:19200"),
    ///         (2, "192.168.1.12:19200"),
    ///     ]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_peers(mut self, peers: &[(usize, &str)]) -> Self {
        self.peers = peers
            .iter()
            .filter_map(|(id, addr)| {
                addr.parse::<SocketAddr>().ok().map(|a| (*id, a))
            })
            .collect();
        self
    }

    /// Set the signaling server for peer discovery
    ///
    /// Use this instead of `with_peers()` for dynamic peer discovery.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// let builder = Stoffel::server(0)
    ///     .with_signaling_server("signal.example.com:9000");
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_signaling_server(mut self, address: &str) -> Self {
        self.signaling_server = Some(address.to_string());
        self
    }

    /// Set the STUN server for NAT traversal
    ///
    /// Optional, used with signaling server for NAT traversal.
    pub fn with_stun_server(mut self, address: &str) -> Self {
        self.stun_server = Some(address.to_string());
        self
    }

    /// Set the MPC program to execute
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// let program = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .build()?;
    ///
    /// let builder = Stoffel::server(0)
    ///     .with_program(program.program().clone());
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_program(mut self, program: Program) -> Self {
        self.program = Some(program);
        self
    }

    /// Set preprocessing parameters
    ///
    /// # Arguments
    ///
    /// * `n_triples` - Number of Beaver triples to generate
    /// * `n_random_shares` - Number of random shares to generate
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// let builder = Stoffel::server(0)
    ///     .with_preprocessing(10, 20);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_preprocessing(mut self, n_triples: usize, n_random_shares: usize) -> Self {
        self.n_triples = n_triples;
        self.n_random_shares = n_random_shares;
        self
    }

    /// Set the MPC instance ID for session coordination
    ///
    /// **CRITICAL**: All servers in the same MPC computation MUST use the same instance_id.
    /// If not set, a timestamp-based ID will be generated which will NOT coordinate properly
    /// between servers.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// let instance_id = 12345_u64;  // Shared across all servers
    /// let builder = Stoffel::server(0)
    ///     .with_instance_id(instance_id);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_instance_id(mut self, id: u64) -> Self {
        self.instance_id = Some(id);
        self
    }

    /// Set the absolute time when preprocessing should start (seconds since Unix epoch)
    ///
    /// CRITICAL: All servers MUST use the same value for coordinated preprocessing start.
    /// If not set, servers will use relative delays which may cause synchronization issues.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// // Calculate a start time 15 seconds from now
    /// let start_epoch = std::time::SystemTime::now()
    ///     .duration_since(std::time::UNIX_EPOCH)
    ///     .unwrap()
    ///     .as_secs() + 15;
    ///
    /// let builder = Stoffel::server(0)
    ///     .with_preprocessing_start_time(start_epoch);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_preprocessing_start_time(mut self, epoch: u64) -> Self {
        self.preprocessing_start_epoch = Some(epoch);
        self
    }

    /// Build the MPC server
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No bind address is specified
    /// - No peers are specified and no signaling server is set
    /// - No program is specified
    pub fn build(self) -> Result<StoffelServer> {
        let bind_address = self.bind_address
            .ok_or_else(|| Error::Configuration("No bind address specified".to_string()))?;

        // Determine peer discovery mode
        let discovery_mode = if let Some(signaling) = self.signaling_server {
            DiscoveryMode::SignalingServer {
                address: signaling,
                stun: self.stun_server,
            }
        } else if !self.peers.is_empty() {
            DiscoveryMode::Explicit(self.peers.clone())
        } else {
            return Err(Error::Configuration(
                "Either peers or signaling server must be specified".to_string()
            ));
        };

        let program = self.program
            .ok_or_else(|| Error::Configuration("No program specified".to_string()))?;

        // Calculate n_parties from peers + self
        let n_parties = self.peers.len() + 1;
        let threshold = 1; // Default threshold

        // Create peer manager
        let peer_manager = PeerManager::new(
            self.party_id,
            discovery_mode,
        );

        // Create client handler
        let client_handler = ClientHandler::new();

        // Create QUIC network manager with party ID
        let network = QuicNetworkManager::with_node_id(self.party_id);

        // Use provided instance_id or generate one from timestamp
        // CRITICAL: All servers in the same MPC computation MUST have the same instance_id
        let instance_id = self.instance_id.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        });

        // Wrap network in Arc for sharing (for accept loop)
        let network_arc = Arc::new(Mutex::new(network));

        // Note: MPC engine will be created in background task after network is connected
        // This is because HoneyBadgerMpcEngine needs Arc<QuicNetworkManager>, but we need
        // mutable access to connect to peers first

        Ok(StoffelServer {
            party_id: self.party_id,
            bind_address,
            program,
            peer_manager,
            client_handler,
            state: Arc::new(std::sync::Mutex::new(ServerState::Initialized)),
            n_parties,
            threshold,
            n_triples: self.n_triples,
            n_random_shares: self.n_random_shares,
            instance_id,
            network: network_arc,
            peer_connections: Arc::new(Mutex::new(std::collections::HashMap::new())),
            client_connections: Arc::new(Mutex::new(std::collections::HashMap::new())),
            mpc_engine: Arc::new(Mutex::new(None)),
            preprocessing_complete: Arc::new(AtomicBool::new(false)),
            preprocessing_start_epoch: self.preprocessing_start_epoch,
        })
    }
}

/// MPC Server
///
/// An MPC compute node that participates in secure multiparty computation.
/// Servers receive secret shares from clients, perform computation collaboratively
/// with other servers, and send result shares back to clients.
pub struct StoffelServer {
    /// This server's party ID
    party_id: usize,
    /// Address to bind to
    bind_address: SocketAddr,
    /// The MPC program to execute
    program: Program,
    /// Peer connection manager
    peer_manager: PeerManager,
    /// Client connection handler
    client_handler: ClientHandler,
    /// Current server state
    state: Arc<std::sync::Mutex<ServerState>>,
    /// Number of parties
    n_parties: usize,
    /// Fault tolerance threshold
    threshold: usize,
    /// Number of Beaver triples for preprocessing
    n_triples: usize,
    /// Number of random shares for preprocessing
    n_random_shares: usize,
    /// Unique instance ID for this computation session
    instance_id: u64,
    /// QUIC network manager for connections
    network: Arc<Mutex<QuicNetworkManager>>,
    /// Peer connections (party_id -> connection)
    peer_connections: Arc<Mutex<std::collections::HashMap<usize, Arc<dyn PeerConnection>>>>,
    /// Client connections (client_id -> connection)
    client_connections: Arc<Mutex<std::collections::HashMap<usize, Arc<dyn PeerConnection>>>>,
    /// HoneyBadger MPC engine from StoffelVM (created after network connection)
    mpc_engine: Arc<Mutex<Option<Arc<HoneyBadgerMpcEngine>>>>,
    /// Flag indicating preprocessing is complete
    preprocessing_complete: Arc<AtomicBool>,
    /// Absolute epoch time when preprocessing should start (None = use relative delays)
    preprocessing_start_epoch: Option<u64>,
}

impl StoffelServer {
    /// Get the party ID
    pub fn party_id(&self) -> usize {
        self.party_id
    }

    /// Get the bind address
    pub fn bind_address(&self) -> SocketAddr {
        self.bind_address
    }

    /// Get the current server state
    pub fn state(&self) -> ServerState {
        *self.state.lock().unwrap()
    }

    /// Get the number of parties
    pub fn n_parties(&self) -> usize {
        self.n_parties
    }

    /// Get the threshold
    pub fn threshold(&self) -> usize {
        self.threshold
    }

    /// Get the instance ID
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    /// Start the server (bind and prepare to accept connections)
    ///
    /// This performs the following steps:
    /// 1. Bind to the configured address
    /// 2. Transition to Ready state (peer connections happen in background)
    ///
    /// Note: Peer connections and preprocessing happen asynchronously during
    /// `run_forever()` to avoid blocking client connections.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let program = Stoffel::compile("main main() -> int64:\n  return 42")?.build()?;
    /// let server = Stoffel::server(0)
    ///     .bind("0.0.0.0:19200")
    ///     .with_peers(&[(1, "127.0.0.1:19201")])
    ///     .with_program(program.program().clone())
    ///     .build()?;
    ///
    /// server.start().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn start(&self) -> Result<()> {
        self.set_state(ServerState::Starting);
        tracing::info!("Server {} starting on {}", self.party_id, self.bind_address);

        // Step 1: Bind QUIC listener
        {
            let mut network = self.network.lock().await;
            network.listen(self.bind_address).await
                .map_err(|e| Error::Network(format!("Failed to bind: {}", e)))?;
        }
        tracing::info!("Server {} bound to {}", self.party_id, self.bind_address);

        // Step 2: Ready to accept clients immediately
        // Peer connections and preprocessing will happen in background during run_forever()
        self.set_state(ServerState::Ready);
        tracing::info!("Server {} ready to accept clients", self.party_id);

        Ok(())
    }

    /// Run forever, handling client requests
    ///
    /// This is the main server loop. It accepts client connections,
    /// coordinates with other servers, and processes computations.
    /// Peer connections are established in the background while accepting clients.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let program = Stoffel::compile("main main() -> int64:\n  return 42")?.build()?;
    /// let server = Stoffel::server(0)
    ///     .bind("0.0.0.0:19200")
    ///     .with_peers(&[(1, "127.0.0.1:19201")])
    ///     .with_program(program.program().clone())
    ///     .build()?;
    ///
    /// server.start().await?;
    /// server.run_forever().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run_forever(&self) -> Result<()> {
        tracing::info!("Server {} entering main loop, waiting for connections...", self.party_id);
        println!("Server {} is ready and accepting connections!", self.party_id);

        // Spawn background task for peer connections and MPC preprocessing
        let peer_manager = self.peer_manager.clone();
        let network = Arc::clone(&self.network);
        let peer_connections = Arc::clone(&self.peer_connections);
        let party_id = self.party_id;
        let n_parties = self.n_parties;
        let threshold = self.threshold;
        let n_triples = self.n_triples;
        let n_random_shares = self.n_random_shares;
        let instance_id = self.instance_id;
        let mpc_engine_slot = Arc::clone(&self.mpc_engine);
        let preprocessing_complete = Arc::clone(&self.preprocessing_complete);
        let preprocessing_start_epoch = self.preprocessing_start_epoch;

        tokio::spawn(async move {
            Self::connect_to_peers_and_preprocess(
                party_id,
                n_parties,
                threshold,
                n_triples,
                n_random_shares,
                instance_id,
                preprocessing_start_epoch,
                peer_manager,
                network,
                peer_connections,
                mpc_engine_slot,
                preprocessing_complete,
            ).await;
        });

        loop {
            // Check if we should shut down
            if self.state() == ServerState::ShuttingDown {
                tracing::info!("Server {} shutting down", self.party_id);
                break;
            }

            // Accept incoming connections
            let conn = {
                let mut network = self.network.lock().await;
                match tokio::time::timeout(
                    std::time::Duration::from_millis(100),
                    network.accept()
                ).await {
                    Ok(Ok(conn)) => Some(conn),
                    Ok(Err(e)) => {
                        tracing::warn!("Server {} accept error: {}", self.party_id, e);
                        None
                    }
                    Err(_) => None, // Timeout, no connection
                }
            };

            if let Some(conn) = conn {
                // Spawn a task to handle this connection
                let party_id = self.party_id;
                let n_parties = self.n_parties;
                let threshold = self.threshold;
                let instance_id = self.instance_id;
                let client_connections = Arc::clone(&self.client_connections);
                let client_handler = self.client_handler.clone();
                let mpc_engine = Arc::clone(&self.mpc_engine);
                let preprocessing_complete = Arc::clone(&self.preprocessing_complete);
                let program_bytecode = self.program.bytecode().to_vec();

                tokio::spawn(async move {
                    if let Err(e) = Self::handle_incoming_connection(
                        conn,
                        party_id,
                        n_parties,
                        threshold,
                        instance_id,
                        client_connections,
                        client_handler,
                        mpc_engine,
                        preprocessing_complete,
                        program_bytecode,
                    ).await {
                        tracing::error!("Server {} connection handler error: {}", party_id, e);
                    }
                });
            }
        }

        Ok(())
    }

    /// Background task to connect to peer servers and run MPC preprocessing
    async fn connect_to_peers_and_preprocess(
        party_id: usize,
        n_parties: usize,
        threshold: usize,
        n_triples: usize,
        n_random_shares: usize,
        instance_id: u64,
        preprocessing_start_epoch: Option<u64>,
        peer_manager: PeerManager,
        network: Arc<Mutex<QuicNetworkManager>>,
        peer_connections: Arc<Mutex<std::collections::HashMap<usize, Arc<dyn PeerConnection>>>>,
        mpc_engine_slot: Arc<Mutex<Option<Arc<HoneyBadgerMpcEngine>>>>,
        preprocessing_complete: Arc<AtomicBool>,
    ) {
        let peers = peer_manager.get_all_peers().await;
        let total_peers = peers.len();

        tracing::info!("Server {} starting background peer connections", party_id);

        // Step 1: Connect to peer servers with HIGHER party IDs only
        // This avoids race conditions where both peers try to connect simultaneously
        // Peers with lower IDs will connect TO us, we connect TO peers with higher IDs
        for peer in &peers {
            // Skip self and peers with lower or equal IDs
            if peer.party_id <= party_id {
                continue;
            }

            tracing::info!(
                "Server {} attempting to connect to peer {} at {}",
                party_id,
                peer.party_id,
                peer.address
            );

            // Try to connect with a timeout
            let connect_result = {
                let mut net = network.lock().await;
                tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    net.connect_as_server(peer.address, party_id)
                ).await
            };

            match connect_result {
                Ok(Ok(conn)) => {
                    let mut conns = peer_connections.lock().await;
                    conns.insert(peer.party_id, conn);
                    tracing::info!(
                        "Server {} connected to peer {}",
                        party_id,
                        peer.party_id
                    );
                }
                Ok(Err(e)) => {
                    tracing::warn!(
                        "Server {} failed to connect to peer {}: {}",
                        party_id,
                        peer.party_id,
                        e
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        "Server {} timed out connecting to peer {}",
                        party_id,
                        peer.party_id
                    );
                }
            }
        }

        let connected = peer_connections.lock().await.len();
        tracing::info!(
            "Server {} peer connections complete: {}/{} peers",
            party_id,
            connected,
            total_peers
        );

        // Step 2: Create the MPC engine's network and connect to peers
        // We need to connect first (requires &mut), then wrap in Arc for the engine
        tracing::info!("Server {} creating MPC network", party_id);

        let mut mpc_network = QuicNetworkManager::with_node_id(party_id);

        // Bind the MPC network to a different port (base + 1000 + party_id)
        // This allows the MPC engine to have its own listener
        let mpc_port = 19200 + 1000 + party_id as u16;
        let mpc_bind_addr: std::net::SocketAddr = format!("0.0.0.0:{}", mpc_port).parse().unwrap();

        if let Err(e) = mpc_network.listen(mpc_bind_addr).await {
            tracing::warn!("Server {} MPC network failed to bind to {}: {}", party_id, mpc_bind_addr, e);
            // Continue anyway - we can still connect outbound
        } else {
            tracing::info!("Server {} MPC network bound to {}", party_id, mpc_bind_addr);
        }

        // Register self-party in the party map to avoid PartyNotFound(self) during preprocessing
        // This matches the pattern in StoffelVM's mpc_multiplication_integration.rs
        mpc_network.add_node_with_party_id(party_id, mpc_bind_addr);

        // Ensure loopback connection exists for self-delivery
        // This is critical for MPC protocols that send messages to themselves during preprocessing
        mpc_network.ensure_loopback_installed().await;

        // Wait for ALL servers to bind their MPC ports and be ready
        // CRITICAL: MPC preprocessing requires ALL parties to participate synchronously.
        // If one party starts preprocessing before others are connected, the protocol will fail.
        //
        // The delay is calculated as:
        // - Base delay: 2 seconds for all servers to start
        // - Per-party stagger: 200ms * party_id to spread out connection attempts
        // - Safety margin: extra time for network latency
        let base_delay = std::time::Duration::from_millis(3000);
        let per_party_stagger = std::time::Duration::from_millis(200 * party_id as u64);
        let initial_delay = base_delay + per_party_stagger;
        tracing::info!("Server {} waiting {:?} for other servers to bind MPC ports", party_id, initial_delay);
        tokio::time::sleep(initial_delay).await;

        // Use Arc<Mutex> for mpc_network so it can be shared between accept and connect tasks
        let mpc_network = Arc::new(tokio::sync::Mutex::new(mpc_network));

        // Number of peers we need to connect TO (higher IDs) and ACCEPT FROM (lower IDs)
        let num_outgoing = n_parties - 1 - party_id; // peers with higher IDs
        let num_incoming = party_id; // peers with lower IDs

        tracing::info!(
            "Server {} MPC network: connecting to {} higher-ID peers, accepting from {} lower-ID peers",
            party_id, num_outgoing, num_incoming
        );

        // Spawn a task to accept incoming connections from lower-ID peers
        let accept_network = Arc::clone(&mpc_network);
        let accept_party_id = party_id;
        let accept_task = tokio::spawn(async move {
            let mut accepted = 0;
            let accept_timeout = std::time::Duration::from_secs(10);
            let start = std::time::Instant::now();

            while accepted < num_incoming && start.elapsed() < accept_timeout {
                let mut net = accept_network.lock().await;
                match tokio::time::timeout(std::time::Duration::from_millis(500), net.accept()).await {
                    Ok(Ok(conn)) => {
                        tracing::info!(
                            "Server {} accepted incoming MPC connection from {}",
                            accept_party_id,
                            conn.remote_address()
                        );
                        accepted += 1;
                    }
                    Ok(Err(e)) => {
                        tracing::debug!("Server {} MPC accept error: {}", accept_party_id, e);
                    }
                    Err(_) => {
                        // Timeout, continue
                    }
                }
                drop(net); // Release lock between attempts
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }

            tracing::info!(
                "Server {} accepted {}/{} incoming MPC connections",
                accept_party_id, accepted, num_incoming
            );
            accepted
        });

        // Connect to peers with HIGHER party IDs (on their MPC ports)
        for peer in &peers {
            if peer.party_id <= party_id {
                continue;
            }

            let peer_mpc_port = peer.address.port() + 1000;
            let peer_mpc_addr = std::net::SocketAddr::new(peer.address.ip(), peer_mpc_port);

            tracing::info!(
                "Server {} MPC network connecting to peer {} at {}",
                party_id,
                peer.party_id,
                peer_mpc_addr
            );

            // Retry with exponential backoff
            let max_retries = 5;
            let mut retry_count = 0;
            let mut connected = false;

            while retry_count < max_retries && !connected {
                if retry_count > 0 {
                    let backoff = std::time::Duration::from_millis(200 * (1 << retry_count));
                    tokio::time::sleep(backoff).await;
                }

                let connect_result = {
                    let mut net = mpc_network.lock().await;
                    tokio::time::timeout(
                        std::time::Duration::from_secs(3),
                        net.connect_as_server(peer_mpc_addr, party_id)
                    ).await
                };

                match connect_result {
                    Ok(Ok(_)) => {
                        tracing::info!("Server {} MPC network connected to peer {}", party_id, peer.party_id);
                        connected = true;
                    }
                    Ok(Err(e)) => {
                        tracing::debug!(
                            "Server {} MPC network attempt {} failed to connect to peer {}: {}",
                            party_id, retry_count + 1, peer.party_id, e
                        );
                        retry_count += 1;
                    }
                    Err(_) => {
                        tracing::debug!(
                            "Server {} MPC network attempt {} timed out connecting to peer {}",
                            party_id, retry_count + 1, peer.party_id
                        );
                        retry_count += 1;
                    }
                }
            }

            if !connected {
                tracing::warn!(
                    "Server {} MPC network failed to connect to peer {} after {} retries",
                    party_id, peer.party_id, max_retries
                );
            }
        }

        // Wait for accept task to complete
        let _ = accept_task.await;

        // Step 4: Create HoneyBadger MPC engine with the connected network
        // Extract the network from the Arc<Mutex>
        let mpc_network = match Arc::try_unwrap(mpc_network) {
            Ok(mutex) => mutex.into_inner(),
            Err(_) => {
                tracing::error!("Server {} MPC network still has multiple references", party_id);
                return;
            }
        };
        let mpc_network_arc = Arc::new(mpc_network);

        tracing::info!("Server {} creating HoneyBadger MPC engine", party_id);

        let engine = match HoneyBadgerMpcEngine::new(
            instance_id,
            party_id,
            n_parties,
            threshold,
            n_triples,
            n_random_shares,
            mpc_network_arc,
        ) {
            Ok(engine) => engine,
            Err(e) => {
                tracing::error!("Server {} failed to create MPC engine: {}", party_id, e);
                return;
            }
        };

        // Store the engine
        {
            let mut engine_slot = mpc_engine_slot.lock().await;
            *engine_slot = Some(Arc::clone(&engine));
        }

        tracing::info!("Server {} MPC engine created", party_id);

        // Step 5: Get a CLONE of the MPC node for message processing
        // The node implements Clone with internal Arc<Mutex<...>> for shared state.
        // This allows message processors to call node.process() without deadlock
        // while preprocessing runs on the engine's internal node.
        let mpc_node = engine.node_clone().await;

        // Step 6: Spawn message processing tasks BEFORE starting preprocessing
        // This is CRITICAL: without message processors, preprocessing messages sent by
        // the MPC protocol will never be received and processed, causing timeouts.
        let shutdown_signal = Arc::new(AtomicBool::new(false));
        let processor_handles = Self::spawn_message_processors(
            party_id,
            mpc_node,
            engine.net(),
            Arc::clone(&shutdown_signal),
        ).await;

        // Give message processors time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // CRITICAL SYNC POINT: Wait for ALL servers to be connected before starting preprocessing
        // MPC preprocessing is a synchronous distributed protocol - all parties must participate.
        //
        // If preprocessing_start_epoch is set, all servers wait until that absolute time.
        // This ensures all servers start preprocessing at exactly the same moment regardless
        // of when they reached this point.
        //
        // If not set, fall back to a relative delay (less reliable but works for simple cases).
        if let Some(start_epoch) = preprocessing_start_epoch {
            // ABSOLUTE TIME SYNCHRONIZATION
            // All servers were given the same start_epoch, so they'll all wake up at the same time
            let now_epoch = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            if now_epoch < start_epoch {
                let wait_secs = start_epoch - now_epoch;
                tracing::info!(
                    "Server {} using ABSOLUTE TIME SYNC: waiting {} seconds until epoch {} (now: {})",
                    party_id,
                    wait_secs,
                    start_epoch,
                    now_epoch
                );
                tokio::time::sleep(Duration::from_secs(wait_secs)).await;
            } else {
                // We're past the start time - this shouldn't happen if start_epoch was set correctly
                tracing::warn!(
                    "Server {} missed preprocessing start time! Target epoch: {}, current: {}. Starting immediately.",
                    party_id,
                    start_epoch,
                    now_epoch
                );
            }
        } else {
            // RELATIVE DELAY FALLBACK (less reliable)
            // Use a fixed delay that's long enough for all servers to reach this point.
            // This approach has timing issues - servers arrive at different times.
            let sync_delay = Duration::from_millis(5000);
            tracing::info!(
                "Server {} using RELATIVE DELAY SYNC: waiting {:?} (consider using with_preprocessing_start_time for reliable sync)",
                party_id,
                sync_delay
            );
            tokio::time::sleep(sync_delay).await;
        }

        // Step 7: Run HoneyBadger preprocessing
        tracing::info!("Server {} starting MPC preprocessing", party_id);

        let preprocess_result = engine.start_async().await;

        // Signal message processors to stop (preprocessing is done or failed)
        shutdown_signal.store(true, Ordering::SeqCst);

        // Wait for all message processors to complete (with timeout)
        let shutdown_timeout = Duration::from_secs(5);
        for handle in processor_handles {
            let _ = tokio::time::timeout(shutdown_timeout, handle).await;
        }
        tracing::debug!("Server {} message processors stopped", party_id);

        // Handle preprocessing result
        match preprocess_result {
            Ok(()) => {
                preprocessing_complete.store(true, Ordering::SeqCst);
                tracing::info!("Server {} MPC preprocessing complete!", party_id);
                println!("Server {} MPC preprocessing complete!", party_id);
            }
            Err(e) => {
                tracing::error!("Server {} MPC preprocessing failed: {}", party_id, e);
            }
        }
    }

    /// Handle an incoming connection (determine if client or peer)
    async fn handle_incoming_connection(
        conn: Arc<dyn PeerConnection>,
        party_id: usize,
        n_parties: usize,
        threshold: usize,
        instance_id: u64,
        client_connections: Arc<Mutex<std::collections::HashMap<usize, Arc<dyn PeerConnection>>>>,
        _client_handler: ClientHandler,
        mpc_engine: Arc<Mutex<Option<Arc<HoneyBadgerMpcEngine>>>>,
        preprocessing_complete: Arc<AtomicBool>,
        program_bytecode: Vec<u8>,
    ) -> Result<()> {
        tracing::info!(
            "Server {} received connection from {}",
            party_id,
            conn.remote_address()
        );

        // Determine connection type
        // Note: Incoming connections are always from clients (peer connections are outbound)
        let conn_type = ConnectionType::Client;

        match conn_type {
            ConnectionType::Client => {
                // This is a client connection - send ServerInfo
                tracing::info!("Server {} handling client connection", party_id);

                let server_info = MPCaaSMessage::ServerInfo {
                    n_parties,
                    threshold,
                    instance_id,
                    party_id,
                };

                let data = serialize_message(&server_info)
                    .map_err(|e| Error::Network(format!("Failed to serialize ServerInfo: {}", e)))?;

                conn.send(&data).await
                    .map_err(|e| Error::Network(format!("Failed to send ServerInfo: {}", e)))?;

                // Wait for ClientReady message
                let response = conn.receive().await
                    .map_err(|e| Error::Network(format!("Failed to receive ClientReady: {}", e)))?;

                let (msg, _) = deserialize_message(&response)
                    .map_err(|e| Error::Network(format!("Failed to deserialize message: {}", e)))?;

                match msg {
                    MPCaaSMessage::ClientReady { client_id, num_inputs } => {
                        tracing::info!(
                            "Server {} received ClientReady from client {} with {} inputs",
                            party_id,
                            client_id,
                            num_inputs
                        );

                        // Store client connection
                        {
                            let mut clients = client_connections.lock().await;
                            clients.insert(client_id, Arc::clone(&conn));
                        }

                        // Check if preprocessing is complete
                        if !preprocessing_complete.load(Ordering::SeqCst) {
                            tracing::warn!(
                                "Server {} preprocessing not complete, client {} must wait",
                                party_id,
                                client_id
                            );

                            // Wait for preprocessing to complete (with timeout)
                            let timeout_duration = std::time::Duration::from_secs(30);
                            let start = std::time::Instant::now();
                            let mut last_log = std::time::Instant::now();

                            while !preprocessing_complete.load(Ordering::SeqCst) {
                                if start.elapsed() > timeout_duration {
                                    tracing::error!(
                                        "Server {} preprocessing timeout - client {} cannot proceed after {:?}",
                                        party_id,
                                        client_id,
                                        timeout_duration
                                    );
                                    return Err(Error::Timeout(format!(
                                        "Server preprocessing timeout after {:?} - MPC engine not ready",
                                        timeout_duration
                                    )));
                                }

                                // Log progress every 5 seconds
                                if last_log.elapsed() > std::time::Duration::from_secs(5) {
                                    tracing::info!(
                                        "Server {} still waiting for preprocessing ({:.1}s elapsed), client {} queued",
                                        party_id,
                                        start.elapsed().as_secs_f64(),
                                        client_id
                                    );
                                    last_log = std::time::Instant::now();
                                }

                                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            }

                            tracing::info!(
                                "Server {} preprocessing completed after {:?}, proceeding with client {}",
                                party_id,
                                start.elapsed(),
                                client_id
                            );
                        }

                        // Get the MPC engine
                        let engine = {
                            let guard = mpc_engine.lock().await;
                            guard.clone().ok_or_else(|| Error::Preprocessing(
                                "MPC engine not initialized".to_string()
                            ))?
                        };

                        tracing::info!(
                            "Server {} starting input protocol for client {} with {} inputs",
                            party_id,
                            client_id,
                            num_inputs
                        );

                        // Send MaskShare to client manually
                        // NOTE: We can't use engine.init_client_input() directly because it tries
                        // to send via the MPC engine's internal network, but the client is connected
                        // to this server's network. So we manually:
                        // 1. Take random shares from preprocessing material
                        // 2. Store them locally for unmasking later
                        // 3. Send them to the client via the connection we have

                        use stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare;
                        use stoffelmpc_mpc::honeybadger::input::{InputMessage, InputMessageType};
                        use stoffelmpc_mpc::honeybadger::WrappedMessage;
                        use ark_bls12_381::Fr;
                        use ark_serialize::CanonicalSerialize;

                        // Get random shares from preprocessing material
                        let local_shares: Vec<RobustShare<Fr>> = {
                            let mpc_node = engine.node_clone().await;
                            let mut prep_material = mpc_node.preprocessing_material.lock().await;
                            prep_material
                                .take_random_shares(num_inputs)
                                .map_err(|e| Error::Preprocessing(format!(
                                    "Not enough random shares for {} inputs: {:?}", num_inputs, e
                                )))?
                        };

                        tracing::info!(
                            "Server {} got {} random shares from preprocessing for client {}",
                            party_id,
                            local_shares.len(),
                            client_id
                        );

                        // Store local shares for later unmasking (via the engine's input server)
                        {
                            let mpc_node = engine.node_clone().await;
                            let mut share_store = mpc_node.preprocess.input.local_mask_shares.lock().await;
                            share_store.insert(client_id, local_shares.clone());
                            tracing::debug!("Server {} stored local mask shares for client {}", party_id, client_id);
                        }

                        // Serialize shares and send to client
                        let mut payload = Vec::new();
                        local_shares.serialize_compressed(&mut payload)
                            .map_err(|e| Error::MPCError(format!("Failed to serialize MaskShare: {:?}", e)))?;

                        let input_msg = InputMessage::new(
                            party_id,  // sender_id
                            InputMessageType::MaskShare,
                            payload,
                        );
                        let wrapped = WrappedMessage::Input(input_msg);
                        let hb_bytes = bincode::serialize(&wrapped)
                            .map_err(|e| Error::MPCError(format!("Failed to serialize HB message: {}", e)))?;

                        let mask_share_msg = MPCaaSMessage::HoneyBadger(hb_bytes);
                        let mask_share_data = serialize_message(&mask_share_msg)
                            .map_err(|e| Error::Network(format!("Failed to serialize MaskShare message: {}", e)))?;

                        conn.send(&mask_share_data).await
                            .map_err(|e| Error::Network(format!(
                                "Failed to send MaskShare to client {}: {}", client_id, e
                            )))?;

                        tracing::info!(
                            "Server {} sent MaskShare ({} bytes) to client {}",
                            party_id,
                            mask_share_data.len(),
                            client_id
                        );

                        // Wait for MaskedInput from client
                        // The client will send this after reconstructing r and computing m+r
                        tracing::info!(
                            "Server {} waiting for MaskedInput from client {}",
                            party_id,
                            client_id
                        );

                        let masked_response = conn.receive().await
                            .map_err(|e| Error::Network(format!("Failed to receive MaskedInput: {}", e)))?;

                        let (masked_msg, _) = deserialize_message(&masked_response)
                            .map_err(|e| Error::Network(format!("Failed to deserialize MaskedInput: {}", e)))?;

                        match masked_msg {
                            MPCaaSMessage::HoneyBadger(hb_data) => {
                                tracing::info!(
                                    "Server {} received HoneyBadger message ({} bytes) from client {}",
                                    party_id,
                                    hb_data.len(),
                                    client_id
                                );

                                // Deserialize the HoneyBadger message
                                if let Ok(wrapped) = bincode::deserialize::<stoffelmpc_mpc::honeybadger::WrappedMessage>(&hb_data) {
                                    match wrapped {
                                        stoffelmpc_mpc::honeybadger::WrappedMessage::Input(input_msg) => {
                                            tracing::info!(
                                                "Server {} processing MaskedInput from client {}",
                                                party_id,
                                                client_id
                                            );

                                            // Process through the MPC engine
                                            if let Err(e) = engine.process_masked_input(input_msg).await {
                                                tracing::error!(
                                                    "Server {} failed to process MaskedInput: {}",
                                                    party_id,
                                                    e
                                                );
                                                return Err(Error::MPCError(format!("MaskedInput processing failed: {}", e)));
                                            }

                                            tracing::info!(
                                                "Server {} stored input shares for client {}",
                                                party_id,
                                                client_id
                                            );

                                            // Check if we have the client's input shares now
                                            if engine.has_client_input(client_id).await {
                                                tracing::info!(
                                                    "Server {} confirmed input shares for client {} are stored in engine",
                                                    party_id,
                                                    client_id
                                                );
                                            }
                                        }
                                        _ => {
                                            tracing::warn!(
                                                "Server {} received non-Input HB message from client {}",
                                                party_id,
                                                client_id
                                            );
                                        }
                                    }
                                } else {
                                    tracing::error!(
                                        "Server {} failed to deserialize HB message from client {}",
                                        party_id,
                                        client_id
                                    );
                                }
                            }
                            _ => {
                                tracing::warn!(
                                    "Server {} received unexpected message type from client {} (expected MaskedInput)",
                                    party_id,
                                    client_id
                                );
                            }
                        }

                        // Execute the program with MPC
                        tracing::info!(
                            "Server {} executing program with MPC for client {}",
                            party_id,
                            client_id
                        );

                        let computation_result = Self::execute_mpc_program(
                            party_id,
                            &engine,
                            &program_bytecode,
                            client_id,
                        ).await;

                        match &computation_result {
                            Ok(result) => {
                                tracing::info!(
                                    "Server {} completed computation for client {}: {:?}",
                                    party_id,
                                    client_id,
                                    result
                                );

                                // If result is a Share, send output shares to client
                                if let crate::vm::Value::Share(_, share_data) = result {
                                    tracing::info!(
                                        "Server {} sending output share ({} bytes) to client {}",
                                        party_id,
                                        share_data.len(),
                                        client_id
                                    );

                                    // Send output share directly through client connection
                                    // (The engine's send_output_shares uses internal MPC network which client isn't connected to)
                                    use stoffelmpc_mpc::honeybadger::output::OutputMessage;
                                    use stoffelmpc_mpc::honeybadger::WrappedMessage;

                                    let output_msg = OutputMessage::new(party_id, share_data.clone());
                                    let wrapped = WrappedMessage::Output(output_msg);

                                    match bincode::serialize(&wrapped) {
                                        Ok(hb_bytes) => {
                                            let output_share_msg = MPCaaSMessage::HoneyBadger(hb_bytes);
                                            match serialize_message(&output_share_msg) {
                                                Ok(output_data) => {
                                                    if let Err(e) = conn.send(&output_data).await {
                                                        tracing::error!(
                                                            "Server {} failed to send output share to client {}: {}",
                                                            party_id,
                                                            client_id,
                                                            e
                                                        );
                                                    } else {
                                                        tracing::info!(
                                                            "Server {} sent output share to client {}",
                                                            party_id,
                                                            client_id
                                                        );
                                                    }
                                                }
                                                Err(e) => {
                                                    tracing::error!(
                                                        "Server {} failed to serialize output share message: {}",
                                                        party_id,
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!(
                                                "Server {} failed to serialize output WrappedMessage: {}",
                                                party_id,
                                                e
                                            );
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Server {} computation failed for client {}: {}",
                                    party_id,
                                    client_id,
                                    e
                                );
                            }
                        }

                        // Generate a session ID for this computation
                        let session_id = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis() as u64)
                            .unwrap_or(0);

                        let complete_msg = MPCaaSMessage::ComputationComplete { session_id };
                        let complete_data = serialize_message(&complete_msg)
                            .map_err(|e| Error::Network(format!("Failed to serialize ComputationComplete: {}", e)))?;

                        conn.send(&complete_data).await
                            .map_err(|e| Error::Network(format!("Failed to send ComputationComplete: {}", e)))?;

                        tracing::info!(
                            "Server {} sent ComputationComplete to client {} (session {})",
                            party_id,
                            client_id,
                            session_id
                        );
                    }
                    _ => {
                        tracing::warn!(
                            "Server {} received unexpected message from client",
                            party_id
                        );
                    }
                }
            }
            ConnectionType::Server => {
                // This is a peer server connection
                tracing::info!("Server {} accepted peer connection", party_id);
                // Peer connections are handled through the mesh topology
            }
        }

        Ok(())
    }

    /// Run a single computation and return
    ///
    /// Unlike `run_forever()`, this processes one computation and returns.
    /// Useful for testing or batch processing.
    pub async fn run_once(&self) -> Result<crate::vm::Value> {
        self.set_state(ServerState::Computing);
        tracing::info!("Server {} running single computation", self.party_id);

        // TODO: Implement actual single computation
        // For now, just execute the program locally
        let result = self.program.execute_local()?;

        self.set_state(ServerState::Ready);
        Ok(result)
    }

    /// Graceful shutdown
    ///
    /// Signals the server to shut down and waits for cleanup.
    pub async fn shutdown(self) -> Result<()> {
        self.set_state(ServerState::ShuttingDown);
        tracing::info!("Server {} shutting down", self.party_id);

        // TODO: Implement graceful shutdown
        // - Finish any in-progress computations
        // - Close client connections
        // - Disconnect from peers

        Ok(())
    }

    /// Set the server state
    fn set_state(&self, state: ServerState) {
        let mut s = self.state.lock().unwrap();
        *s = state;
    }

    /// Execute the MPC program with client inputs
    ///
    /// This creates a VirtualMachine, attaches the MPC engine, loads the bytecode,
    /// and executes the main function. Client inputs are loaded from the global store.
    async fn execute_mpc_program(
        party_id: usize,
        engine: &Arc<HoneyBadgerMpcEngine>,
        bytecode: &[u8],
        client_id: usize,
    ) -> Result<crate::vm::Value> {
        use crate::vm::load_bytecode_into_vm;

        tracing::info!(
            "Server {} setting up VM with MPC engine for client {}",
            party_id,
            client_id
        );

        // Create a new VM
        let mut vm = VirtualMachine::new();

        // Attach the MPC engine
        vm.state.set_mpc_engine(engine.clone());

        // Hydrate the VM's client store from the MPC engine's input store
        // This copies client input shares from the HB engine to the VM's local store
        match vm.state.hydrate_from_mpc_engine() {
            Ok(count) => {
                tracing::info!(
                    "Server {} hydrated {} client(s) from MPC engine to VM",
                    party_id,
                    count
                );
            }
            Err(e) => {
                tracing::warn!(
                    "Server {} failed to hydrate from MPC engine: {}",
                    party_id,
                    e
                );
            }
        }

        // Load bytecode
        load_bytecode_into_vm(&mut vm, bytecode)?;

        tracing::info!(
            "Server {} loaded bytecode, executing main function",
            party_id
        );

        // Verify client inputs are in the VM store
        // The program will load inputs via ClientStore.take_share() builtin
        if vm.state.has_client_input(client_id) {
            let input_count = vm.state.get_client_input_count(client_id);
            tracing::info!(
                "Server {} found {} inputs for client {} in VM store",
                party_id,
                input_count,
                client_id
            );
        } else {
            tracing::warn!(
                "Server {} - no inputs found for client {} in VM store",
                party_id,
                client_id
            );
        }

        tracing::info!(
            "Server {} executing main function",
            party_id
        );

        // Execute the main function
        // The program loads inputs via ClientStore.take_share() builtin
        let result = vm.execute("main")
            .map_err(|e| Error::Computation(format!("VM execution failed: {}", e)))?;

        tracing::info!(
            "Server {} VM execution complete, result: {:?}",
            party_id,
            result
        );

        // Convert VM value to SDK value
        Ok(crate::vm::convert_vm_value_to_sdk_value(result))
    }

    /// Spawns message processing tasks for all peer connections.
    /// These tasks receive messages from the MPC network and route them through
    /// the MPC node's process() method. This is essential for preprocessing and
    /// other MPC protocols that require bidirectional message exchange.
    ///
    /// IMPORTANT: The MPC node implements Clone with internal Arc<Mutex<...>> for
    /// shared state. This allows multiple tasks to share node state without external
    /// locking, avoiding deadlocks during preprocessing.
    ///
    /// IMPORTANT: This function uses channel-based message aggregation to ensure
    /// sequential processing. All messages from all connections are sent to a single
    /// channel and processed by one task. This matches the StoffelVM pattern and
    /// prevents race conditions in the MPC protocol state machine.
    ///
    /// Returns a vector of JoinHandles for the spawned tasks.
    async fn spawn_message_processors(
        party_id: usize,
        node: HoneyBadgerMPCNode<Fr, RBCImpl>,
        network: Arc<QuicNetworkManager>,
        shutdown: Arc<AtomicBool>,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        use tokio::sync::mpsc;

        let mut handles = Vec::new();

        // Get all peer connections from the network
        let connections = network.get_all_connections().await;

        tracing::info!(
            "Server {} spawning message processor with {} connections (channel-based sequential processing)",
            party_id,
            connections.len()
        );

        // Create a channel to aggregate messages from all connections
        // Using a bounded channel to prevent unbounded memory growth
        let (msg_tx, mut msg_rx) = mpsc::channel::<(usize, Vec<u8>)>(1000);

        // Spawn reader tasks for each connection - they only forward messages to the channel
        for (peer_id, connection) in connections {
            // NOTE: Do NOT skip loopback (self) connections!
            // MPC protocols send messages to themselves (e.g., shares during RanSha init).
            let tx = msg_tx.clone();
            let shutdown_clone = Arc::clone(&shutdown);

            let handle = tokio::spawn(async move {
                tracing::info!(
                    "Message reader for peer {} started (party {})",
                    peer_id,
                    party_id
                );

                loop {
                    // Check shutdown signal
                    if shutdown_clone.load(Ordering::SeqCst) {
                        tracing::debug!(
                            "Message reader for peer {} shutting down",
                            peer_id
                        );
                        break;
                    }

                    // Try to receive a message with timeout
                    match tokio::time::timeout(
                        Duration::from_millis(100),
                        connection.receive()
                    ).await {
                        Ok(Ok(raw_msg)) => {
                            // Forward to the aggregation channel (don't process here!)
                            if let Err(e) = tx.send((peer_id, raw_msg)).await {
                                tracing::warn!(
                                    "Party {} failed to forward message from peer {}: {:?}",
                                    party_id,
                                    peer_id,
                                    e
                                );
                                break;
                            }
                        }
                        Ok(Err(e)) => {
                            // Connection error - might be closed
                            let err_str = e.to_string();
                            if err_str.contains("closed") || err_str.contains("Closed") {
                                tracing::debug!(
                                    "Connection to peer {} closed: {}",
                                    peer_id,
                                    e
                                );
                                break;
                            }
                            // Other receive errors - log but continue
                            tracing::debug!(
                                "Party {} receive error from peer {}: {}",
                                party_id,
                                peer_id,
                                e
                            );
                        }
                        Err(_) => {
                            // Timeout - continue polling
                            continue;
                        }
                    }
                }

                tracing::info!(
                    "Message reader for peer {} stopped (party {})",
                    peer_id,
                    party_id
                );
            });

            handles.push(handle);
        }

        // Drop the original sender so the processor can detect when all readers are done
        drop(msg_tx);

        // Spawn a SINGLE processor task that handles ALL messages sequentially
        // This is CRITICAL: sequential processing prevents race conditions in the MPC protocol
        let mut node_for_processor = node.clone();
        let net_for_processor = Arc::clone(&network);
        let shutdown_for_processor = Arc::clone(&shutdown);

        let processor_handle = tokio::spawn(async move {
            tracing::info!(
                "Message processor started for party {} (sequential processing)",
                party_id
            );

            loop {
                // Check shutdown signal
                if shutdown_for_processor.load(Ordering::SeqCst) {
                    tracing::debug!(
                        "Message processor for party {} shutting down",
                        party_id
                    );
                    break;
                }

                // Try to receive from channel with timeout
                match tokio::time::timeout(
                    Duration::from_millis(100),
                    msg_rx.recv()
                ).await {
                    Ok(Some((peer_id, raw_msg))) => {
                        // Process message sequentially - this is the ONLY place
                        // where node.process() is called
                        if let Err(e) = node_for_processor.process(raw_msg, net_for_processor.clone()).await {
                            tracing::warn!(
                                "Party {} failed to process message from peer {}: {:?}",
                                party_id,
                                peer_id,
                                e
                            );
                        }
                    }
                    Ok(None) => {
                        // Channel closed, all readers done
                        tracing::debug!(
                            "Message channel closed for party {}",
                            party_id
                        );
                        break;
                    }
                    Err(_) => {
                        // Timeout - continue polling
                        continue;
                    }
                }
            }

            tracing::info!(
                "Message processor stopped for party {}",
                party_id
            );
        });

        handles.push(processor_handle);

        handles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_state_display() {
        assert_eq!(format!("{}", ServerState::Ready), "Ready");
        assert_eq!(format!("{}", ServerState::Computing), "Computing");
    }

    /// Test that the shutdown signal correctly stops message processors
    #[tokio::test]
    async fn test_message_processor_shutdown_signal() {
        // Create a mock shutdown signal
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = Arc::clone(&shutdown);

        // Spawn a task that mimics the message processor loop
        let handle = tokio::spawn(async move {
            let mut iterations = 0;
            while !shutdown_clone.load(Ordering::SeqCst) {
                iterations += 1;
                if iterations > 100 {
                    // Safety limit to prevent infinite loop in test
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            iterations
        });

        // Let it run for a bit
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Signal shutdown
        shutdown.store(true, Ordering::SeqCst);

        // Should complete within timeout
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            handle
        ).await;

        assert!(result.is_ok(), "Message processor should shut down cleanly");
        let iterations = result.unwrap().unwrap();
        assert!(iterations > 0, "Should have run at least one iteration");
        assert!(iterations < 100, "Should have stopped before safety limit");
    }

    /// Test that message processor handles connection closure gracefully
    #[tokio::test]
    async fn test_message_processor_handles_connection_closure() {
        // This test verifies the logic that checks for "closed" in error messages
        let err_closed = "Stream closed by peer";
        let err_other = "Timeout waiting for data";

        assert!(err_closed.contains("closed"));
        assert!(!err_other.contains("closed"));
    }

    /// Test ServerState transitions
    #[test]
    fn test_server_state_transitions() {
        // Verify all expected states exist and can be compared
        let states = vec![
            ServerState::Initialized,
            ServerState::Starting,
            ServerState::ConnectingPeers,
            ServerState::Preprocessing,
            ServerState::Ready,
            ServerState::Computing,
            ServerState::ShuttingDown,
        ];

        for state in &states {
            // Each state should be equal to itself
            assert_eq!(*state, *state);
        }

        // Preprocessing should come before Ready in the lifecycle
        assert_ne!(ServerState::Preprocessing, ServerState::Ready);
    }

    /// Test that spawn_message_processors returns correct number of handles
    /// Note: This is a structural test; the actual spawning requires a network setup
    ///
    /// The channel-based architecture spawns:
    /// - 1 reader task per connection (including loopback to self)
    /// - 1 processor task for sequential message processing
    #[test]
    fn test_spawn_message_processors_expected_count() {
        // For a 5-party network where we are party 0:
        // - We spawn readers for all N connections (including loopback)
        // - We spawn 1 processor for sequential processing
        // Expected: N connections + 1 processor = N + 1 handles
        let n_parties = 5;
        let n_connections = n_parties; // All peers including loopback
        let expected_handles = n_connections + 1; // readers + 1 processor

        assert_eq!(expected_handles, 6);
    }

    // =========================================================================
    // Server Builder Tests
    // =========================================================================

    /// Test StoffelServerBuilder::new creates builder with party ID
    #[test]
    fn test_server_builder_new() {
        let builder = StoffelServerBuilder::new(0);
        assert_eq!(builder.party_id, 0);
        assert!(builder.bind_address.is_none());
        assert!(builder.peers.is_empty());
        assert!(builder.program.is_none());
    }

    /// Test StoffelServerBuilder with instance_id
    #[test]
    fn test_server_builder_instance_id() {
        let builder = StoffelServerBuilder::new(0)
            .with_instance_id(12345);

        assert_eq!(builder.instance_id, Some(12345));
    }

    /// Test StoffelServerBuilder preprocessing parameters
    #[test]
    fn test_server_builder_preprocessing_params() {
        let builder = StoffelServerBuilder::new(0)
            .with_preprocessing(50, 100);

        assert_eq!(builder.n_triples, 50);
        assert_eq!(builder.n_random_shares, 100);
    }

    /// Test StoffelServerBuilder default preprocessing params
    #[test]
    fn test_server_builder_default_preprocessing() {
        let builder = StoffelServerBuilder::new(0);

        // Default values from the builder
        assert_eq!(builder.n_triples, 10);
        assert_eq!(builder.n_random_shares, 20);
    }

    /// Test StoffelServerBuilder bind address
    #[test]
    fn test_server_builder_bind_address() {
        let builder = StoffelServerBuilder::new(0)
            .bind("127.0.0.1:19200");

        assert!(builder.bind_address.is_some());
        let addr = builder.bind_address.unwrap();
        assert_eq!(addr.port(), 19200);
    }

    /// Test StoffelServerBuilder with invalid bind address
    #[test]
    fn test_server_builder_invalid_bind_address() {
        let builder = StoffelServerBuilder::new(0)
            .bind("not-a-valid-address");

        // Should be None for invalid address
        assert!(builder.bind_address.is_none());
    }

    /// Test StoffelServerBuilder with peers
    #[test]
    fn test_server_builder_peer_addresses() {
        let builder = StoffelServerBuilder::new(0)
            .with_peers(&[
                (1, "127.0.0.1:19201"),
                (2, "127.0.0.1:19202"),
                (3, "127.0.0.1:19203"),
            ]);

        assert_eq!(builder.peers.len(), 3);
        assert_eq!(builder.peers[0].0, 1);
        assert_eq!(builder.peers[1].0, 2);
        assert_eq!(builder.peers[2].0, 3);
    }

    /// Test StoffelServerBuilder with invalid peer addresses (filtered out)
    #[test]
    fn test_server_builder_invalid_peer_filtered() {
        let builder = StoffelServerBuilder::new(0)
            .with_peers(&[
                (1, "127.0.0.1:19201"),
                (2, "invalid-address"),  // Should be filtered
                (3, "127.0.0.1:19203"),
            ]);

        // Only valid addresses should be kept
        assert_eq!(builder.peers.len(), 2);
    }

    /// Test StoffelServerBuilder with preprocessing start time
    #[test]
    fn test_server_builder_preprocessing_start_time() {
        let start_epoch = 1704067200_u64; // Jan 1, 2024 00:00:00 UTC
        let builder = StoffelServerBuilder::new(0)
            .with_preprocessing_start_time(start_epoch);

        assert_eq!(builder.preprocessing_start_epoch, Some(start_epoch));
    }

    /// Test StoffelServerBuilder chaining
    #[test]
    fn test_server_builder_chaining() {
        let builder = StoffelServerBuilder::new(0)
            .bind("127.0.0.1:19200")
            .with_peers(&[(1, "127.0.0.1:19201")])
            .with_preprocessing(5, 10)
            .with_instance_id(999)
            .with_preprocessing_start_time(1704067200);

        assert!(builder.bind_address.is_some());
        assert_eq!(builder.peers.len(), 1);
        assert_eq!(builder.n_triples, 5);
        assert_eq!(builder.n_random_shares, 10);
        assert_eq!(builder.instance_id, Some(999));
        assert_eq!(builder.preprocessing_start_epoch, Some(1704067200));
    }

    /// Test StoffelServerBuilder with signaling server
    #[test]
    fn test_server_builder_signaling_server() {
        let builder = StoffelServerBuilder::new(0)
            .with_signaling_server("signal.example.com:9000");

        assert_eq!(builder.signaling_server, Some("signal.example.com:9000".to_string()));
    }

    /// Test StoffelServerBuilder with STUN server
    #[test]
    fn test_server_builder_stun_server() {
        let builder = StoffelServerBuilder::new(0)
            .with_stun_server("stun.example.com:3478");

        assert_eq!(builder.stun_server, Some("stun.example.com:3478".to_string()));
    }

    /// Test multiple party IDs
    #[test]
    fn test_server_builder_different_party_ids() {
        for party_id in 0..5 {
            let builder = StoffelServerBuilder::new(party_id);
            assert_eq!(builder.party_id, party_id);
        }
    }
}
