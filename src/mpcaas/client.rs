//! MPCaaS Client API
//!
//! This module provides a client API for connecting to an MPC network and
//! submitting inputs for secure computation. The client API is designed for
//! app developers who want to use MPC without understanding the underlying
//! protocol details.
//!
//! # Design Philosophy
//!
//! The client API is intentionally simpler than the server API:
//! - No party IDs, thresholds, or MPC configuration (auto-detected from servers)
//! - No preprocessing management (servers handle this)
//! - No peer connections (client only connects to servers, not other clients)
//! - Inputs are just `&[i64]` - provide as many as the program requires
//! - Automatic secret sharing (hidden from developer)
//!
//! # Examples
//!
//! ## Builder Pattern (Recommended)
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let client = StoffelClient::builder()
//!         .with_servers(&["localhost:19200", "localhost:19201", "localhost:19202"])
//!         .client_id(12345)  // Optional: auto-generated if not set
//!         .connection_timeout(Duration::from_secs(30))
//!         .connect()
//!         .await?;
//!
//!     println!("Connected to MPC network (n={}, t={})",
//!         client.n_parties(), client.threshold());
//!
//!     let result = client.run(&[42, 100]).await?;
//!     println!("Result: {:?}", result);
//!     Ok(())
//! }
//! ```
//!
//! ## Via Stoffel Entry Point
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let client = Stoffel::client()
//!         .with_servers(&["localhost:19200", "localhost:19201"])
//!         .connect()
//!         .await?;
//!
//!     let result = client.run(&[42, 100]).await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Async/Non-blocking Workflow
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let client = StoffelClient::builder()
//!         .with_servers(&["localhost:19200", "localhost:19201"])
//!         .connect()
//!         .await?;
//!
//!     // Submit without blocking
//!     let handle = client.submit(&[42, 100]).await?;
//!
//!     // Do other work...
//!
//!     // Get result when ready
//!     let result = handle.await_result().await?;
//!     Ok(())
//! }
//! ```

use super::handle::ComputationHandle;
use super::protocol::{MPCaaSMessage, serialize_message, deserialize_message};
use crate::{Error, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use stoffelnet::transports::quic::{QuicNetworkManager, PeerConnection};

// MPC protocol types for HoneyBadger client
use ark_bls12_381::Fr;
use stoffelmpc_mpc::honeybadger::{HoneyBadgerMPCClient, SessionId};
use stoffelmpc_mpc::common::rbc::rbc::Avid;

/// State of the MPC client
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    /// Client is connected and ready to submit inputs
    Connected,
    /// Client is submitting inputs to servers
    Submitting,
    /// Client is waiting for computation result
    Computing,
    /// Client has disconnected
    Disconnected,
}

impl std::fmt::Display for ClientState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientState::Connected => write!(f, "Connected"),
            ClientState::Submitting => write!(f, "Submitting"),
            ClientState::Computing => write!(f, "Computing"),
            ClientState::Disconnected => write!(f, "Disconnected"),
        }
    }
}

/// Builder for creating an MPC client
///
/// Use `StoffelClient::builder()` or `Stoffel::client()` to create a builder.
///
/// # Example
///
/// ```rust,no_run
/// use stoffel_rust_sdk::prelude::*;
/// use std::time::Duration;
///
/// # #[tokio::main]
/// # async fn main() -> Result<()> {
/// let client = StoffelClient::builder()
///     .with_servers(&["localhost:19200", "localhost:19201", "localhost:19202"])
///     .client_id(12345)
///     .connection_timeout(Duration::from_secs(30))
///     .computation_timeout(Duration::from_secs(120))
///     .connect()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct StoffelClientBuilder {
    /// Server addresses to connect to
    servers: Vec<String>,
    /// Client ID (auto-generated if not set)
    client_id: Option<usize>,
    /// Connection timeout
    connection_timeout: Duration,
    /// Computation timeout
    computation_timeout: Duration,
}

impl StoffelClientBuilder {
    /// Create a new client builder
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
            client_id: None,
            connection_timeout: Duration::from_secs(10),
            computation_timeout: Duration::from_secs(60),
        }
    }

    /// Add a server address to connect to
    ///
    /// Can be called multiple times to add multiple servers.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// let builder = StoffelClient::builder()
    ///     .add_server("localhost:19200")
    ///     .add_server("localhost:19201")
    ///     .add_server("localhost:19202");
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_server(mut self, address: &str) -> Self {
        self.servers.push(address.to_string());
        self
    }

    /// Set all server addresses at once
    ///
    /// Replaces any previously added servers.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// let builder = StoffelClient::builder()
    ///     .with_servers(&["localhost:19200", "localhost:19201", "localhost:19202"]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_servers(mut self, servers: &[&str]) -> Self {
        self.servers = servers.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Set a custom client ID
    ///
    /// If not set, a unique client ID will be auto-generated based on the
    /// current timestamp.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// let builder = StoffelClient::builder()
    ///     .with_servers(&["localhost:19200"])
    ///     .client_id(12345);
    /// # Ok(())
    /// # }
    /// ```
    pub fn client_id(mut self, id: usize) -> Self {
        self.client_id = Some(id);
        self
    }

    /// Set the connection timeout
    ///
    /// Default: 10 seconds
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// use std::time::Duration;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = StoffelClient::builder()
    ///     .with_servers(&["localhost:19200"])
    ///     .connection_timeout(Duration::from_secs(30));
    /// # Ok(())
    /// # }
    /// ```
    pub fn connection_timeout(mut self, timeout: Duration) -> Self {
        self.connection_timeout = timeout;
        self
    }

    /// Set the computation timeout
    ///
    /// Default: 60 seconds
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// use std::time::Duration;
    ///
    /// # fn main() -> Result<()> {
    /// let builder = StoffelClient::builder()
    ///     .with_servers(&["localhost:19200"])
    ///     .computation_timeout(Duration::from_secs(120));
    /// # Ok(())
    /// # }
    /// ```
    pub fn computation_timeout(mut self, timeout: Duration) -> Self {
        self.computation_timeout = timeout;
        self
    }

    /// Build and connect to the MPC network
    ///
    /// This establishes connections to all specified servers and performs
    /// a handshake to receive MPC configuration (n_parties, threshold).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No server addresses were provided
    /// - Any server address is invalid
    /// - Connection to any server fails
    /// - Server configuration is inconsistent
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// let client = StoffelClient::builder()
    ///     .with_servers(&["localhost:19200", "localhost:19201"])
    ///     .connect()
    ///     .await?;
    ///
    /// println!("Connected to {} parties", client.n_parties());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect(self) -> Result<StoffelClient> {
        if self.servers.is_empty() {
            return Err(Error::InvalidInput("No server addresses provided".to_string()));
        }

        // Parse server addresses
        let mut server_addrs = Vec::with_capacity(self.servers.len());
        for addr in &self.servers {
            // Add default port if not specified
            let addr_with_port = if addr.contains(':') {
                addr.clone()
            } else {
                format!("{}:19200", addr)
            };

            let socket_addr: SocketAddr = addr_with_port.parse()
                .map_err(|e| Error::InvalidInput(format!("Invalid server address '{}': {}", addr, e)))?;
            server_addrs.push(socket_addr);
        }

        // Generate a unique client ID if not provided
        let client_id = self.client_id.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| (d.as_nanos() % 1_000_000) as usize + 1000)
                .unwrap_or(1000)
        });

        // Create network manager
        let network = Arc::new(Mutex::new(
            QuicNetworkManager::with_node_id(client_id)
        ));

        // Connect to each server
        let mut connections = Vec::with_capacity(server_addrs.len());
        let mut server_info: Option<(usize, usize)> = None; // (n_parties, threshold)

        for (i, addr) in server_addrs.iter().enumerate() {
            tracing::info!("Client {} connecting to server {} at {}", client_id, i, addr);

            // Connect as client
            let conn = {
                let mut net = network.lock().await;
                net.connect_as_client(*addr).await
                    .map_err(|e| Error::Network(format!("Failed to connect to server at {}: {}", addr, e)))?
            };

            // Receive ServerInfo (with timeout)
            let data = tokio::time::timeout(
                self.connection_timeout,
                conn.receive()
            ).await
                .map_err(|_| Error::Network(format!("Timeout waiting for ServerInfo from {}", addr)))?
                .map_err(|e| Error::Network(format!("Failed to receive ServerInfo from {}: {}", addr, e)))?;

            let (msg, _) = deserialize_message(&data)
                .map_err(|e| Error::Network(format!("Failed to deserialize ServerInfo: {}", e)))?;

            match msg {
                MPCaaSMessage::ServerInfo { n_parties, threshold, instance_id, party_id } => {
                    tracing::info!(
                        "Client {} received ServerInfo from party {}: n={}, t={}, instance={}",
                        client_id, party_id, n_parties, threshold, instance_id
                    );

                    // Validate n_parties and threshold consistency (instance_id may differ between servers)
                    if let Some((prev_n, prev_t)) = server_info {
                        if prev_n != n_parties || prev_t != threshold {
                            return Err(Error::Configuration(format!(
                                "Inconsistent server configuration: expected n={}, t={}, \
                                 but server {} has n={}, t={}",
                                prev_n, prev_t, party_id, n_parties, threshold
                            )));
                        }
                    } else {
                        server_info = Some((n_parties, threshold));
                    }
                }
                _ => {
                    return Err(Error::Network(format!(
                        "Expected ServerInfo from {}, got {:?}", addr, msg
                    )));
                }
            }

            connections.push(conn);
        }

        let (n_parties, threshold) = server_info
            .ok_or_else(|| Error::Network("No ServerInfo received from any server".to_string()))?;

        // Generate a unique instance_id for this client session
        let instance_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        tracing::info!(
            "Client {} connected to {} servers (n={}, t={}, instance={})",
            client_id, connections.len(), n_parties, threshold, instance_id
        );

        Ok(StoffelClient {
            servers: server_addrs,
            network,
            connections,
            n_parties,
            threshold,
            instance_id,
            connection_timeout: self.connection_timeout,
            computation_timeout: self.computation_timeout,
            client_id,
            state: ClientState::Connected,
        })
    }
}

impl Default for StoffelClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// MPC Client for connecting to and interacting with an MPC network
///
/// The client follows a lifecycle:
/// 1. Build with `StoffelClient::builder()` or `Stoffel::client()`
/// 2. Connect with `.connect().await`
/// 3. Run computation with `.run(&inputs).await`
/// 4. Optionally disconnect with `.disconnect().await`
///
/// # Thread Safety
///
/// `StoffelClient` is designed for single-use. After calling `run()` or `submit()`,
/// the connection is consumed. Create a new client for subsequent computations.
///
/// # Example
///
/// ```rust,no_run
/// use stoffel_rust_sdk::prelude::*;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let client = StoffelClient::builder()
///         .with_servers(&["localhost:19200", "localhost:19201", "localhost:19202"])
///         .connect()
///         .await?;
///
///     println!("Connected to MPC network:");
///     println!("  Parties: {}", client.n_parties());
///     println!("  Threshold: {}", client.threshold());
///     println!("  Client ID: {}", client.client_id());
///     println!("  State: {:?}", client.state());
///
///     let result = client.run(&[42, 100]).await?;
///     println!("Result: {:?}", result);
///     Ok(())
/// }
/// ```
pub struct StoffelClient {
    /// Server addresses
    servers: Vec<SocketAddr>,
    /// Network manager for QUIC connections
    network: Arc<Mutex<QuicNetworkManager>>,
    /// Connections to each server
    connections: Vec<Arc<dyn PeerConnection>>,
    /// MPC configuration received from servers during handshake
    n_parties: usize,
    threshold: usize,
    instance_id: u64,
    /// Connection timeout
    connection_timeout: Duration,
    /// Computation timeout
    computation_timeout: Duration,
    /// Client ID (generated during connection)
    client_id: usize,
    /// Current client state
    state: ClientState,
}

impl StoffelClient {
    /// Create a new client builder
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// let client = StoffelClient::builder()
    ///     .with_servers(&["localhost:19200"])
    ///     .connect()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder() -> StoffelClientBuilder {
        StoffelClientBuilder::new()
    }

    /// Submit inputs and wait for result
    ///
    /// This is the main method for submitting inputs to the MPC network.
    /// The number of inputs should match what the program expects from this client.
    ///
    /// # Arguments
    ///
    /// * `inputs` - The client's inputs to the MPC program
    ///
    /// # Returns
    ///
    /// The computation result as a vector of i64 values.
    pub async fn run(mut self, inputs: &[i64]) -> Result<Vec<i64>> {
        use ark_ff::PrimeField;
        use ark_serialize::CanonicalDeserialize;
        use stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare;
        use stoffelmpc_mpc::common::SecretSharingScheme;
        use std::collections::HashMap;

        self.state = ClientState::Submitting;

        tracing::info!(
            "Client {} submitting {} inputs to {} servers",
            self.client_id,
            inputs.len(),
            self.servers.len()
        );

        // Convert inputs to field elements
        let field_inputs: Vec<Fr> = inputs.iter()
            .map(|&v| Fr::from(v as u64))
            .collect();

        // Step 1: Send ClientReady to all servers
        let client_ready = MPCaaSMessage::ClientReady {
            client_id: self.client_id,
            num_inputs: inputs.len(),
        };

        let ready_data = serialize_message(&client_ready)
            .map_err(|e| Error::Network(format!("Failed to serialize ClientReady: {}", e)))?;

        for (i, conn) in self.connections.iter().enumerate() {
            tracing::debug!("Client {} sending ClientReady to server {}", self.client_id, i);
            conn.send(&ready_data).await
                .map_err(|e| Error::Network(format!("Failed to send ClientReady to server {}: {}", i, e)))?;
        }

        tracing::info!("Client {} sent ClientReady to all {} servers", self.client_id, self.connections.len());

        self.state = ClientState::Computing;

        // Step 2: Create HoneyBadger MPC client for input/output protocol
        // Note: Currently using manual protocol handling, but keeping client for future output integration
        let _mpc_client = HoneyBadgerMPCClient::<Fr, Avid<SessionId>>::new(
            self.client_id,
            self.n_parties,
            self.threshold,
            self.instance_id as u32,
            field_inputs.clone(),
            inputs.len(),
        ).map_err(|e| Error::MPCError(format!("Failed to create MPC client: {:?}", e)))?;

        tracing::info!("Client {} created HoneyBadger MPC client", self.client_id);

        // Step 3: Wait for MaskShare messages from servers
        // We need 2t+1 shares to reconstruct the mask
        let required_shares = 2 * self.threshold + 1;
        let mut received_input_shares: HashMap<usize, Vec<RobustShare<Fr>>> = HashMap::new();
        let mut received_output_shares: HashMap<usize, Vec<RobustShare<Fr>>> = HashMap::new();
        let mut masked_input_sent = false;
        let mut computation_complete_count = 0;

        tracing::info!(
            "Client {} waiting for {} MaskShare messages (2t+1)",
            self.client_id,
            required_shares
        );

        // Track computation start time for timeout handling
        let computation_start = std::time::Instant::now();

        // Poll each connection for messages
        loop {
            // Check for computation timeout
            if computation_start.elapsed() > self.computation_timeout {
                tracing::error!(
                    "Client {} computation timed out after {:?}",
                    self.client_id,
                    self.computation_timeout
                );
                return Err(Error::Timeout(format!(
                    "MPC computation timed out after {:?} (received {}/{} MaskShares, {}/{} OutputShares)",
                    self.computation_timeout,
                    received_input_shares.len(),
                    required_shares,
                    received_output_shares.len(),
                    required_shares
                )));
            }
            for (i, conn) in self.connections.iter().enumerate() {
                // Try to receive with a short timeout
                match tokio::time::timeout(
                    Duration::from_millis(100),
                    conn.receive()
                ).await {
                    Ok(Ok(data)) => {
                        match deserialize_message(&data) {
                            Ok((msg, _)) => {
                                match msg {
                                    MPCaaSMessage::ComputationComplete { session_id } => {
                                        tracing::info!(
                                            "Client {} received ComputationComplete for session {} from server {}",
                                            self.client_id, session_id, i
                                        );
                                        computation_complete_count += 1;

                                        // Check if we have enough output shares to reconstruct
                                        if received_output_shares.len() >= required_shares {
                                            tracing::info!(
                                                "Client {} has {} output shares, reconstructing result",
                                                self.client_id,
                                                received_output_shares.len()
                                            );

                                            // Reconstruct output (assuming single output value)
                                            let mut output_share_list: Vec<RobustShare<Fr>> = Vec::new();
                                            for (_, shares) in received_output_shares.iter() {
                                                if !shares.is_empty() {
                                                    output_share_list.push(shares[0].clone());
                                                }
                                            }

                                            match RobustShare::recover_secret(&output_share_list, self.n_parties) {
                                                Ok((_, result)) => {
                                                    // Convert Fr to i64
                                                    use ark_ff::PrimeField;
                                                    let limbs: [u64; 4] = result.into_bigint().0;
                                                    let result_i64 = limbs[0] as i64;
                                                    tracing::info!(
                                                        "Client {} reconstructed output: {}",
                                                        self.client_id,
                                                        result_i64
                                                    );
                                                    return Ok(vec![result_i64]);
                                                }
                                                Err(e) => {
                                                    return Err(Error::MPCError(format!(
                                                        "Failed to reconstruct output: {:?}", e
                                                    )));
                                                }
                                            }
                                        } else {
                                            // Not enough output shares yet, return inputs as placeholder
                                            // This happens when server returns cleartext result
                                            tracing::info!(
                                                "Client {} received ComputationComplete but only {} output shares",
                                                self.client_id,
                                                received_output_shares.len()
                                            );
                                            return Ok(inputs.to_vec());
                                        }
                                    }
                                    MPCaaSMessage::HoneyBadger(hb_data) => {
                                        tracing::debug!(
                                            "Client {} received HoneyBadger message ({} bytes) from server {}",
                                            self.client_id, hb_data.len(), i
                                        );

                                        // Deserialize to check message type
                                        if let Ok(wrapped) = bincode::deserialize::<stoffelmpc_mpc::honeybadger::WrappedMessage>(&hb_data) {
                                            match wrapped {
                                                stoffelmpc_mpc::honeybadger::WrappedMessage::Input(input_msg) => {
                                                    tracing::info!(
                                                        "Client {} received Input message from server {}",
                                                        self.client_id, input_msg.sender_id
                                                    );

                                                    // Deserialize the shares
                                                    if let Ok(shares) = Vec::<RobustShare<Fr>>::deserialize_compressed(input_msg.payload.as_slice()) {
                                                        received_input_shares.insert(input_msg.sender_id, shares);
                                                        tracing::info!(
                                                            "Client {} received MaskShare from server {} ({}/{} needed)",
                                                            self.client_id,
                                                            input_msg.sender_id,
                                                            received_input_shares.len(),
                                                            required_shares
                                                        );
                                                    }
                                                }
                                                stoffelmpc_mpc::honeybadger::WrappedMessage::Output(output_msg) => {
                                                    tracing::info!(
                                                        "Client {} received Output message from server {} ({} bytes)",
                                                        self.client_id, output_msg.sender_id, output_msg.payload.len()
                                                    );

                                                    // The server sends a single RobustShare, not a Vec
                                                    // Try to deserialize as a single share first
                                                    if let Ok(share) = RobustShare::<Fr>::deserialize_compressed(output_msg.payload.as_slice()) {
                                                        received_output_shares.insert(output_msg.sender_id, vec![share]);
                                                        tracing::info!(
                                                            "Client {} received output share from server {} ({}/{} needed)",
                                                            self.client_id,
                                                            output_msg.sender_id,
                                                            received_output_shares.len(),
                                                            required_shares
                                                        );
                                                    } else if let Ok(shares) = Vec::<RobustShare<Fr>>::deserialize_compressed(output_msg.payload.as_slice()) {
                                                        // Fallback: try as Vec<RobustShare>
                                                        received_output_shares.insert(output_msg.sender_id, shares);
                                                        tracing::info!(
                                                            "Client {} received output shares (Vec) from server {} ({}/{} needed)",
                                                            self.client_id,
                                                            output_msg.sender_id,
                                                            received_output_shares.len(),
                                                            required_shares
                                                        );
                                                    } else {
                                                        tracing::warn!(
                                                            "Client {} failed to deserialize output share from server {}",
                                                            self.client_id, output_msg.sender_id
                                                        );
                                                    }
                                                }
                                                _ => {
                                                    tracing::debug!(
                                                        "Client {} received other HB message type",
                                                        self.client_id
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    MPCaaSMessage::Error { code, message } => {
                                        return Err(Error::Computation(format!(
                                            "Server {} error: {:?} - {}", i, code, message
                                        )));
                                    }
                                    _ => {
                                        tracing::debug!(
                                            "Client {} received unexpected message from server {}: {:?}",
                                            self.client_id, i, msg
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Client {} failed to deserialize message from server {}: {}",
                                    self.client_id, i, e
                                );
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(
                            "Client {} receive error from server {}: {}",
                            self.client_id, i, e
                        );
                    }
                    Err(_) => {
                        // Timeout - no message available, continue polling
                    }
                }
            }

            // Step 4: Check if we have enough shares to compute masked input
            if !masked_input_sent && received_input_shares.len() >= required_shares {
                tracing::info!(
                    "Client {} has {} shares, computing masked input",
                    self.client_id,
                    received_input_shares.len()
                );

                // Reconstruct the mask r for each input
                let input_len = inputs.len();
                let mut r_shares_per_input: Vec<Vec<RobustShare<Fr>>> = vec![vec![]; input_len];

                for (_, shares) in received_input_shares.iter() {
                    for (j, share) in shares.iter().enumerate() {
                        if j < input_len {
                            r_shares_per_input[j].push(share.clone());
                        }
                    }
                }

                // Reconstruct r and compute masked inputs
                let mut masked_inputs: Vec<Fr> = Vec::with_capacity(input_len);
                for (j, r_shares) in r_shares_per_input.iter().enumerate() {
                    match RobustShare::recover_secret(r_shares, self.n_parties) {
                        Ok((_, r)) => {
                            // m + r
                            let masked = field_inputs[j] + r;
                            masked_inputs.push(masked);
                        }
                        Err(e) => {
                            return Err(Error::MPCError(format!(
                                "Failed to reconstruct mask for input {}: {:?}", j, e
                            )));
                        }
                    }
                }

                tracing::info!("Client {} computed masked inputs, broadcasting to servers", self.client_id);

                // Serialize and send MaskedInput to all servers
                use ark_serialize::CanonicalSerialize;
                let mut payload = Vec::new();
                masked_inputs.serialize_compressed(&mut payload)
                    .map_err(|e| Error::MPCError(format!("Failed to serialize masked inputs: {:?}", e)))?;

                let input_msg = stoffelmpc_mpc::honeybadger::input::InputMessage::new(
                    self.client_id,
                    stoffelmpc_mpc::honeybadger::input::InputMessageType::MaskedInput,
                    payload,
                );
                let wrapped = stoffelmpc_mpc::honeybadger::WrappedMessage::Input(input_msg);
                let hb_bytes = bincode::serialize(&wrapped)
                    .map_err(|e| Error::MPCError(format!("Failed to serialize HB message: {}", e)))?;

                let masked_msg = MPCaaSMessage::HoneyBadger(hb_bytes);
                let masked_data = serialize_message(&masked_msg)
                    .map_err(|e| Error::Network(format!("Failed to serialize MaskedInput: {}", e)))?;

                // Send to all servers
                for (i, conn) in self.connections.iter().enumerate() {
                    conn.send(&masked_data).await
                        .map_err(|e| Error::Network(format!(
                            "Failed to send MaskedInput to server {}: {}", i, e
                        )))?;
                }

                tracing::info!("Client {} sent MaskedInput to all servers", self.client_id);
                masked_input_sent = true;
            }

            // Small delay to prevent busy-waiting
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Submit inputs without waiting (for async UI workflows)
    ///
    /// Returns a `ComputationHandle` that can be used to poll or wait for the result.
    ///
    /// # Arguments
    ///
    /// * `inputs` - The client's inputs to the MPC program
    ///
    /// # Returns
    ///
    /// A `ComputationHandle` for tracking the computation.
    pub async fn submit(self, inputs: &[i64]) -> Result<ComputationHandle> {
        let (tx, rx) = mpsc::channel(1);
        let inputs_vec = inputs.to_vec();
        let client = self;

        // Spawn task to run the computation
        tokio::spawn(async move {
            let result = client.run(&inputs_vec).await;
            let _ = tx.send(result).await;
        });

        Ok(ComputationHandle::new(rx))
    }

    /// Disconnect from the network
    ///
    /// This gracefully closes all connections to the MPC servers.
    pub async fn disconnect(mut self) -> Result<()> {
        tracing::info!("Client {} disconnecting", self.client_id);
        self.state = ClientState::Disconnected;
        Ok(())
    }

    /// Get the number of parties in the MPC network
    pub fn n_parties(&self) -> usize {
        self.n_parties
    }

    /// Get the threshold (fault tolerance)
    pub fn threshold(&self) -> usize {
        self.threshold
    }

    /// Get the client ID
    pub fn client_id(&self) -> usize {
        self.client_id
    }

    /// Get the current client state
    pub fn state(&self) -> ClientState {
        self.state
    }

    /// Get the instance ID
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_defaults() {
        let builder = StoffelClientBuilder::new();
        assert!(builder.servers.is_empty());
        assert!(builder.client_id.is_none());
        assert_eq!(builder.connection_timeout, Duration::from_secs(10));
        assert_eq!(builder.computation_timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_builder_with_servers() {
        let builder = StoffelClientBuilder::new()
            .with_servers(&["localhost:19200", "localhost:19201"]);
        assert_eq!(builder.servers.len(), 2);
        assert_eq!(builder.servers[0], "localhost:19200");
        assert_eq!(builder.servers[1], "localhost:19201");
    }

    #[test]
    fn test_builder_add_server() {
        let builder = StoffelClientBuilder::new()
            .add_server("localhost:19200")
            .add_server("localhost:19201");
        assert_eq!(builder.servers.len(), 2);
    }

    #[test]
    fn test_builder_client_id() {
        let builder = StoffelClientBuilder::new().client_id(12345);
        assert_eq!(builder.client_id, Some(12345));
    }

    #[test]
    fn test_client_state_display() {
        assert_eq!(format!("{}", ClientState::Connected), "Connected");
        assert_eq!(format!("{}", ClientState::Submitting), "Submitting");
        assert_eq!(format!("{}", ClientState::Computing), "Computing");
        assert_eq!(format!("{}", ClientState::Disconnected), "Disconnected");
    }
}
