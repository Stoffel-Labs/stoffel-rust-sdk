//! MPC Client implementation
//!
//! This module provides the MPCClient abstraction for the Stoffel SDK, which represents clients
//! that send private inputs to an MPC network and receive computation results.
//!
//! In Stoffel's architecture, MPC participants are created from a `StoffelRuntime`, which combines
//! a compiled program with MPC infrastructure configuration.
//!
//! # Architecture
//!
//! The SDK provides three types of MPC participants:
//!
//! - **[`MPCClient`]** (this module): Clients that secret share inputs, send them to the MPC network,
//!   and reconstruct outputs locally. Does not participate in computation.
//! - **[`MPCServer`]** (see `server` module): MPC servers that receive secret-shared inputs, execute
//!   the Stoffel program collaboratively, and distribute output shares to clients.
//! - **[`MPCNode`]** (see `session` module): Full participants that both provide inputs AND participate
//!   in computation. Useful for collaborative multi-party scenarios.
//!
//! # Usage
//!
//! All MPC participants must be created from a `StoffelRuntime`. This ensures that
//! clients and servers are always associated with a specific program and MPC configuration.
//!
//! ## Basic Example
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! # fn main() -> Result<()> {
//! // Compile program with MPC configuration to create a runtime
//! let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .parties(5)
//!     .threshold(1)
//!     .build()?;
//!
//! // Create MPC server (performs computation)
//! let server = runtime.server(0).build()?;
//!
//! // Create MPC client (provides private inputs)
//! let client = runtime.client(100).with_inputs(vec![42]).build()?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Network-Based Operation
//!
//! MPC clients are network-based by design. They:
//! 1. Generate secret shares of private inputs locally
//! 2. Connect to MPC servers via QUIC networking
//! 3. Send input shares to servers
//! 4. Receive output shares from servers
//! 5. Reconstruct final outputs locally
//!
//! ### Example: Connecting to MPC Network
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! # async fn example() -> Result<()> {
//! let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .parties(5)
//!     .threshold(1)
//!     .build()?;
//!
//! // Create client with private inputs
//! let mut client = runtime.client(100)
//!     .with_inputs(vec![10, 20])
//!     .build()?;
//!
//! // Add server addresses
//! client.add_server(0, "127.0.0.1:19200".parse().unwrap());
//! client.add_server(1, "127.0.0.1:19201".parse().unwrap());
//! client.add_server(2, "127.0.0.1:19202".parse().unwrap());
//!
//! // Connect to all servers
//! let rx = client.connect_to_servers().await?;
//!
//! // Client can now send inputs and receive outputs over the network
//! # Ok(())
//! # }
//! ```
//!
//! # MPC Protocol and Secret Sharing Configuration
//!
//! ## Default Configuration
//!
//! The Stoffel SDK uses secure defaults that work together:
//!
//! - **MPC Protocol**: HoneyBadger (Byzantine fault-tolerant, asynchronous)
//! - **Secret Sharing**: RobustShare (error correction, required for HoneyBadger)
//! - **Reliable Broadcast**: AVID (efficient asynchronous verifiable information dispersal)
//!
//! HoneyBadger provides:
//! - Byzantine fault tolerance (handles malicious parties)
//! - Asynchronous communication (no timing assumptions)
//! - Optimal resilience (tolerates up to t faulty parties where n >= 3t+1)
//!
//! RobustShare provides:
//! - Reed-Solomon error correction
//! - Robust reconstruction even when shares are corrupted
//! - Required for HoneyBadger's Byzantine fault tolerance
//!
//! ## Advanced: Custom RBC Protocols
//!
//! Advanced users can implement custom RBC protocols by implementing the `RBC` trait from
//! `stoffelmpc_mpc::common::RBC`. The SDK currently uses:
//! - **Field Type**: `ark_bls12_381::Fr` (BLS12-381 scalar field)
//! - **RBC Protocol**: `Avid` (default for HoneyBadger)
//!
//! Alternative RBC implementations available in `mpc-protocols`:
//! - `Avid`: Asynchronous Verifiable Information Dispersal (default, recommended for HoneyBadger)
//! - `Bracha`: Classic Bracha's reliable broadcast (simpler, but less efficient for large messages)
//!
//! To use a different RBC protocol, you would need to directly use the types from
//! `stoffelmpc_mpc` with your chosen RBC implementation.

use crate::{Error, Result, ProtocolType};
use std::sync::Arc;
use ark_ff::BigInteger;

/// Shared MPC configuration from StoffelRuntime
///
/// This struct serves as the single source of truth for MPC network configuration.
/// It is created by `StoffelRuntime` and passed to all MPC participants (clients,
/// servers, nodes) to ensure consistency across the network.
///
/// # Architecture
///
/// Instead of duplicating configuration fields in each MPC participant struct,
/// all configuration flows from `StoffelRuntime` through `MPCConfig`:
///
/// ```text
/// StoffelRuntime
///   ├─> Creates MPCConfig
///   ├─> MPCClient (receives config)
///   ├─> MPCServer/MPCNode (receives config)
///   └─> MPCSession (receives config)
/// ```
///
/// This design ensures that:
/// - Configuration is never duplicated or inconsistent
/// - The protocol type is accessible to all participants
/// - The instance ID, party count, and threshold are guaranteed to match
///
/// # Example
///
/// ```rust,no_run
/// # use stoffel_rust_sdk::prelude::*;
/// # fn example() -> Result<()> {
/// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
///     .parties(5)
///     .threshold(1)
///     .build()?;
///
/// // All participants automatically receive the same MPCConfig from runtime
/// let client = runtime.client(100).build()?;
/// let server = runtime.server(0).build()?;
///
/// // Both have identical configuration
/// assert_eq!(client.config().n_parties, server.config().n_parties);
/// assert_eq!(client.config().protocol_type, server.config().protocol_type);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct MPCConfig {
    /// Number of parties in the MPC network
    pub n_parties: usize,
    /// Threshold for fault tolerance (max faulty parties)
    pub threshold: usize,
    /// Instance ID for this computation
    pub instance_id: u64,
    /// MPC protocol type (e.g., HoneyBadger)
    pub protocol_type: ProtocolType,
}

/// Type alias for MPCConfig (backward compatibility)
pub type ProtocolConfig = MPCConfig;

// Re-export key types from mpc-protocols for convenience
pub use stoffelmpc_mpc::honeybadger::{
    HoneyBadgerError, HoneyBadgerMPCClient, HoneyBadgerMPCNode, HoneyBadgerMPCNodeOpts,
};

// AVID is the default RBC (Reliable Broadcast Channel) protocol for HoneyBadger
// Advanced users can substitute other RBC implementations that implement the RBC trait
pub use stoffelmpc_mpc::common::rbc::rbc::Avid;

//==============================================================================
// MPCClient - For clients sending inputs to MPC network
//==============================================================================

/// Network-based client for sending private inputs to an MPC network
///
/// `MPCClient` is a network-based abstraction that handles:
/// - **Secret sharing**: Generates secret shares of private inputs locally
/// - **Network connectivity**: Establishes QUIC connections to MPC servers
/// - **Input distribution**: Sends secret shares to servers over the network
/// - **Output reconstruction**: Receives result shares and reconstructs final outputs locally
///
/// The client does not participate in the actual computation - it only provides inputs
/// and receives outputs. The MPC network ensures that individual servers never learn the
/// client's private inputs.
///
/// **Note**: Clients are network-based by design. The network manager is created automatically
/// during client construction and connectivity is established via `add_server()` and
/// `connect_to_servers()`.
///
/// # Architecture: Configuration from StoffelRuntime
///
/// `MPCClient` receives its configuration (`MPCConfig`) from the `StoffelRuntime` that
/// creates it. This ensures the client uses the correct protocol type, instance ID,
/// party count, and threshold without duplication:
///
/// ```text
/// StoffelRuntime --creates--> MPCClientBuilder --builds--> MPCClient
///       |                           |                          |
///       |                           |                          |
///    MPCConfig ─────────────────────┴──────────────────────────┘
///                    (protocol, instance_id, n_parties, threshold)
/// ```
///
/// # Usage
///
/// Always create clients from a `StoffelRuntime`:
///
/// ```rust,no_run
/// # use stoffel_rust_sdk::prelude::*;
/// # fn example() -> Result<()> {
/// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
///     .parties(5)
///     .threshold(1)
///     .build()?;
///
/// // Client automatically receives MPC configuration from runtime
/// let client = runtime.client(100)
///     .with_inputs(vec![42, 100, 25])
///     .build()?;
///
/// // Configuration is accessible
/// assert_eq!(client.config().n_parties, 5);
/// assert_eq!(client.config().protocol_type, ProtocolType::HoneyBadger);
/// # Ok(())
/// # }
/// ```
///
/// # Two Modes of Operation
///
/// ## Direct Mode (No Network)
///
/// For testing and simple examples:
///
/// ```rust,no_run
/// # use stoffel_rust_sdk::prelude::*;
/// # fn example() -> Result<()> {
/// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?.parties(5).build()?;
/// let client = runtime.client(100).with_inputs(vec![10, 20]).build()?;
///
/// // Generate shares directly
/// let shares = client.generate_input_shares_robust()?;
/// // Distribute shares to servers manually
/// # Ok(())
/// # }
/// ```
///
/// ## Network Mode (Full QUIC)
///
/// For production deployments with network connectivity:
///
/// ```rust,no_run
/// # use stoffel_rust_sdk::prelude::*;
/// # use stoffelnet::transports::quic::QuicNetworkManager;
/// # use std::sync::Arc;
/// # async fn example() -> Result<()> {
/// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?.parties(5).build()?;
/// # let network = QuicNetworkManager::with_client_id(100);
/// # let server_addrs = vec!["127.0.0.1:19200".parse().unwrap()];
/// let mut client = runtime.client(100).with_inputs(vec![10, 20]).build()?;
///
/// // Attach network for full network mode
/// client.attach_network(Arc::new(network), server_addrs);
///
/// // Network workflow:
/// // 1. client.send_inputs().await        - Initialize and connect to servers
/// // 2. client.process_message(msg, net)  - Handle incoming messages from servers
/// // 3. MPC network computes on secret-shared data
/// // 4. client.receive_outputs().await    - Reconstruct results from output shares
/// # Ok(())
/// # }
/// ```
pub struct MPCClient {
    client_id: Option<usize>,
    inputs: Vec<i64>,
    config: Option<MPCConfig>,
    share_type: crate::ShareType,
    inner: Option<HoneyBadgerMPCClient<ark_bls12_381::Fr, Avid>>,
    // Network manager (always present - clients are network-based)
    network: std::sync::Arc<stoffelnet::transports::quic::QuicNetworkManager>,
    // Server addresses for network connectivity
    server_addresses: Vec<std::net::SocketAddr>,
}

impl MPCClient {
    // =========================================================================
    // Builder Methods (Internal - used by StoffelRuntime)
    // =========================================================================

    /// Create a new MPC client with network manager (internal use only)
    ///
    /// Network manager is required - clients are network-based by design.
    pub(crate) fn new(network: std::sync::Arc<stoffelnet::transports::quic::QuicNetworkManager>) -> Self {
        Self {
            client_id: None,
            inputs: Vec::new(),
            config: None,
            share_type: crate::ShareType::Robust,  // Default
            inner: None,
            network,
            server_addresses: Vec::new(),
        }
    }

    /// Set the client ID (must be unique and different from party IDs)
    pub(crate) fn with_client_id(mut self, id: usize) -> Self {
        self.client_id = Some(id);
        self
    }

    /// Set the MPC configuration from runtime
    pub(crate) fn with_config(mut self, config: MPCConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set the private inputs this client wants to contribute
    pub(crate) fn with_inputs(mut self, inputs: Vec<i64>) -> Self {
        self.inputs = inputs;
        self
    }

    /// Set the secret sharing scheme type
    pub(crate) fn with_share_type(mut self, share_type: crate::ShareType) -> Self {
        self.share_type = share_type;
        self
    }

    /// Build the MPC client
    pub(crate) fn build(self) -> Result<MPCClient> {
        let client_id = self
            .client_id
            .ok_or_else(|| Error::InvalidInput("client_id not set".to_string()))?;
        let config = self
            .config
            .ok_or_else(|| Error::InvalidInput("MPC config not set".to_string()))?;

        Ok(MPCClient {
            client_id: Some(client_id),
            inputs: self.inputs,
            config: Some(config),
            share_type: self.share_type,
            inner: self.inner,
            network: self.network,
            server_addresses: self.server_addresses,
        })
    }

    /// Attach a network manager and server addresses for full network mode (advanced usage)
    ///
    /// This method allows you to attach a QUIC network manager to the client,
    /// enabling network-based input distribution and output reception.
    ///
    /// # Arguments
    /// * `network` - Arc-wrapped QuicNetworkManager for async operations
    /// * `server_addresses` - Socket addresses of all MPC servers
    ///
    /// # Example
    /// ```no_run
    /// use stoffel_rust_sdk::prelude::*;
    /// use stoffelnet::transports::quic::QuicNetworkManager;
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    /// #     .parties(5).threshold(1).build()?;
    /// let mut network_manager = QuicNetworkManager::with_client_id(100);
    /// // ... configure network_manager ...
    ///
    /// let server_addrs = vec![
    ///     "127.0.0.1:19200".parse().unwrap(),
    ///     "127.0.0.1:19201".parse().unwrap(),
    ///     // ... more servers
    /// ];
    ///
    /// let mut client = runtime.client(100)
    ///     .with_inputs(vec![10, 20])
    ///     .build()?;
    ///
    /// client.attach_network(Arc::new(network_manager), server_addrs);
    /// # Ok(())
    /// # }
    /// ```
    /// Set server addresses for network connectivity (optional configuration)
    pub fn set_server_addresses(&mut self, server_addresses: Vec<std::net::SocketAddr>) {
        self.server_addresses = server_addresses;
    }

    // =========================================================================
    // Accessor Methods
    // =========================================================================

    /// Get the client ID
    pub fn client_id(&self) -> usize {
        self.client_id.expect("client_id should be set after build()")
    }

    /// Get the inputs
    pub fn inputs(&self) -> &[i64] {
        &self.inputs
    }

    /// Get the instance ID from the MPC configuration
    ///
    /// This is a convenience method that reads from the shared `MPCConfig`.
    /// The instance ID comes from the `StoffelRuntime` that created this client.
    pub fn instance_id(&self) -> u64 {
        self.config.as_ref().expect("config should be set after build()").instance_id
    }

    /// Get the full MPC configuration
    ///
    /// Returns the complete MPC configuration that was passed from `StoffelRuntime`.
    /// This includes:
    /// - `n_parties`: Number of parties in the MPC network
    /// - `threshold`: Maximum number of faulty parties tolerated
    /// - `instance_id`: Unique identifier for this computation instance
    /// - `protocol_type`: The MPC protocol being used (e.g., HoneyBadger)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn example() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    /// #     .parties(5).threshold(1).build()?;
    /// let client = runtime.client(100).build()?;
    ///
    /// let config = client.config();
    /// println!("Using {} protocol", match config.protocol_type {
    ///     ProtocolType::HoneyBadger => "HoneyBadger",
    /// });
    /// println!("Network: {} parties, threshold {}", config.n_parties, config.threshold);
    /// # Ok(())
    /// # }
    /// ```
    pub fn config(&self) -> &MPCConfig {
        self.config.as_ref().expect("config should be set after build()")
    }

    // =========================================================================
    // Input Protocol Methods
    // =========================================================================
    //
    // The SDK provides two approaches for input handling:
    //
    // 1. Interactive Masking Protocol (send_inputs + process_message):
    //    - Recommended for HoneyBadger with full network integration
    //    - Servers generate mask shares, client adds masks to inputs
    //    - More secure, requires network connectivity
    //
    // 2. Direct Share Generation (generate_input_shares*):
    //    - Simpler alternative for custom protocols or testing
    //    - Client generates shares directly and sends to parties
    //    - No interactive protocol required
    //
    // =========================================================================

    /// Initialize client and prepare to send secret-shared inputs to the MPC network
    ///
    /// This method initializes the HoneyBadger MPC client which uses an interactive
    /// input protocol where:
    /// 1. Servers generate random mask shares and send them to the client
    /// 2. Client receives mask shares via `process_message()`
    /// 3. Once 2t+1 mask shares are received, client automatically:
    ///    - Reconstructs the random masks
    ///    - Adds masks to inputs: masked_input = input + mask
    ///    - Broadcasts masked inputs to all servers via RBC
    /// 4. Servers subtract their local masks to obtain secret shares of the inputs
    ///
    /// This masking protocol ensures that the client doesn't need to generate
    /// shares themselves, and provides additional security properties.
    ///
    /// # Protocol Flow
    ///
    /// ```text
    /// Client                          Servers (each generates r_i shares)
    ///   |                                  |
    ///   |<------- MaskShare[r_i] ----------| (from each server)
    ///   |                                  |
    ///   | (waits for 2t+1 shares)         |
    ///   | (reconstructs r = Σr_i)         |
    ///   | (computes m+r for each input)   |
    ///   |                                  |
    ///   |-------- MaskedInput[m+r] ------>| (broadcast via RBC)
    ///   |                                  |
    ///   |                                  | (each computes (m+r) - r_i)
    ///   |                                  | (obtains share of m)
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The MPC network is not properly configured
    /// - Network communication fails
    /// - The MPC protocol encounters an error during secret sharing
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # use std::sync::Arc;
    /// # async fn example() -> Result<()> {
    /// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .parties(5)
    ///     .threshold(1)
    ///     .build()?;
    ///
    /// let mut client = runtime.client(100)
    ///     .with_inputs(vec![42, 100, 25])
    ///     .build()?;
    ///
    /// # let network = Arc::new(todo!());
    /// // Secret share and send inputs to MPC network
    /// // This initializes the client and prepares it to receive mask shares from servers
    /// client.send_inputs(network).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Network Mode
    ///
    /// When using `attach_network()`, this method will:
    /// 1. Initialize the HoneyBadgerMPCClient
    /// 2. Connect to all MPC servers
    /// 3. Automatically handle the input masking protocol
    ///
    /// See `examples/quick_start_local_network_real.rs` for a complete example.
    pub async fn send_inputs(&mut self) -> Result<()> {
        use ark_bls12_381::Fr;

        // Initialize HoneyBadgerMPCClient if not already done
        if self.inner.is_none() {
            // Convert i64 inputs to field elements
            let field_inputs: Vec<Fr> = self.inputs.iter().map(|&x| Fr::from(x as u64)).collect();
            let input_len = field_inputs.len();

            let config = self.config.as_ref()
                .ok_or_else(|| Error::InvalidInput("MPC config not set".to_string()))?;

            // Create HoneyBadgerMPCClient
            let client = HoneyBadgerMPCClient::new(
                self.client_id.unwrap(),
                config.n_parties,
                config.threshold,
                config.instance_id,
                field_inputs,
                input_len,
            ).map_err(|e| Error::RuntimeError(format!("Failed to create HoneyBadgerMPCClient: {:?}", e)))?;

            self.inner = Some(client);
        }

        // Network-based input distribution
        // The actual input sending happens via the interactive masking protocol:
        // 1. Servers call InputServer.init() to send mask shares to this client
        // 2. Client receives mask shares via the message processing loop
        // 3. Once 2t+1 mask shares are received, client automatically broadcasts masked inputs
        //
        // The network manager handles connections internally when messages are sent/received.
        //
        // The calling code should run a message processing loop like:
        // ```
        // while let Some(msg) = network.receive().await? {
        //     client.process_message(msg, network.clone()).await?;
        // }
        // ```

        Ok(())
    }

    /// Process an incoming message from the MPC network
    ///
    /// This method handles incoming messages from MPC servers, including:
    /// - Mask shares from servers (during input phase)
    /// - Output shares (during output reconstruction)
    ///
    /// Call this method when messages arrive from the network.
    ///
    /// # Arguments
    ///
    /// * `raw_msg` - The raw message bytes received from the network
    /// * `network` - The network handle for sending responses
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # use std::sync::Arc;
    /// # async fn example() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?.build()?;
    /// # let mut client = runtime.client(100).with_inputs(vec![42]).build()?;
    /// # let network = Arc::new(todo!());
    /// // In your message handling loop:
    /// let msg = vec![]; // received from network
    /// client.process_message(msg, network).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn process_message<N>(&mut self, raw_msg: Vec<u8>, network: Arc<N>) -> Result<()>
    where
        N: stoffelnet::network_utils::Network + Send + Sync + 'static,
    {
        let inner = self.inner.as_mut()
            .ok_or_else(|| Error::RuntimeError("Inner client not initialized. Call send_inputs() first.".to_string()))?;

        inner.process(raw_msg, network).await
            .map_err(|e| Error::RuntimeError(format!("Failed to process message: {:?}", e)))
    }

    // =========================================================================
    // Direct Share Generation Methods
    // =========================================================================

    /// Generate secret shares of inputs directly and return them (simpler alternative)
    ///
    /// This is a simpler, non-interactive approach compared to `send_inputs()`:
    /// 1. Client converts inputs to field elements
    /// 2. Client generates Shamir secret shares using `RobustShare::compute_shares()`
    /// 3. Returns a vector of shares for each server
    ///
    /// The caller is responsible for sending each share to the corresponding server.
    ///
    /// # Returns
    ///
    /// Returns a vector where `result[party_id]` contains the shares for that party.
    /// Each party gets one share per input value.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn example() -> Result<()> {
    /// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .parties(5)
    ///     .threshold(1)
    ///     .build()?;
    ///
    /// let client = runtime.client(100)
    ///     .with_inputs(vec![42, 37])  // Two input values
    ///     .build()?;
    ///
    /// // Generate shares for all parties
    /// let shares_per_party = client.generate_input_shares()?;
    ///
    /// // Send shares[party_id] to each party
    /// // for (party_id, shares) in shares_per_party.iter().enumerate() {
    /// //     network.send_to_party(party_id, shares).await?;
    /// // }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Note
    ///
    /// This is a simpler protocol than the masking-based `send_inputs()` approach.
    /// Use this when you want direct control over share distribution, or when
    /// implementing custom protocols. For standard HoneyBadger protocol, use
    /// `send_inputs()` + `process_message()`.
    pub fn generate_input_shares_robust(&self) -> Result<Vec<Vec<stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare<ark_bls12_381::Fr>>>> {
        use ark_bls12_381::Fr;
        use stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare;
        use stoffelmpc_mpc::common::SecretSharingScheme;
        use ark_std::test_rng;

        let config = self.config.as_ref()
            .ok_or_else(|| Error::InvalidInput("MPC config not set".to_string()))?;

        let n_parties = config.n_parties;
        let threshold = config.threshold;

        // Convert inputs to field elements
        let field_inputs: Vec<Fr> = self.inputs.iter()
            .map(|&x| Fr::from(x as u64))
            .collect();

        // Generate shares for each input
        let mut all_shares = Vec::with_capacity(field_inputs.len());
        let mut rng = test_rng();

        for input in field_inputs {
            let shares = RobustShare::compute_shares(input, n_parties, threshold, None, &mut rng)
                .map_err(|e| Error::RuntimeError(format!("Failed to generate robust shares: {:?}", e)))?;
            all_shares.push(shares);
        }

        // Transpose: Convert from [input_idx][party_id] to [party_id][input_idx]
        let mut shares_per_party: Vec<Vec<RobustShare<Fr>>> = vec![Vec::new(); n_parties];
        for shares_for_input in all_shares {
            for (party_id, share) in shares_for_input.into_iter().enumerate() {
                shares_per_party[party_id].push(share);
            }
        }

        Ok(shares_per_party)
    }

    /// Generate non-robust secret shares of inputs directly and return them
    ///
    /// Similar to `generate_input_shares_robust()` but uses `NonRobustShare` for simpler,
    /// faster secret sharing without error correction.
    pub fn generate_input_shares_non_robust(&self) -> Result<Vec<Vec<stoffelmpc_mpc::common::share::shamir::NonRobustShare<ark_bls12_381::Fr>>>> {
        use ark_bls12_381::Fr;
        use stoffelmpc_mpc::common::share::shamir::NonRobustShare;
        use stoffelmpc_mpc::common::SecretSharingScheme;
        use ark_serialize::CanonicalSerialize;
        use ark_std::test_rng;

        let config = self.config.as_ref()
            .ok_or_else(|| Error::InvalidInput("MPC config not set".to_string()))?;

        let n_parties = config.n_parties;
        let threshold = config.threshold;

        // Convert inputs to field elements
        let field_inputs: Vec<Fr> = self.inputs.iter()
            .map(|&x| Fr::from(x as u64))
            .collect();

        // Generate shares for each input
        let mut all_shares = Vec::with_capacity(field_inputs.len());
        let mut rng = test_rng();

        for input in field_inputs {
            let shares = NonRobustShare::compute_shares(input, n_parties, threshold, None, &mut rng)
                .map_err(|e| Error::RuntimeError(format!("Failed to generate non-robust shares: {:?}", e)))?;
            all_shares.push(shares);
        }

        // Transpose: Convert from [input_idx][party_id] to [party_id][input_idx]
        let mut shares_per_party: Vec<Vec<NonRobustShare<Fr>>> = vec![Vec::new(); n_parties];
        for shares_for_input in all_shares {
            for (party_id, share) in shares_for_input.into_iter().enumerate() {
                shares_per_party[party_id].push(share);
            }
        }

        Ok(shares_per_party)
    }

    /// Generate secret shares of inputs using the configured share type
    ///
    /// This dispatches to either `generate_input_shares_robust()` or
    /// `generate_input_shares_non_robust()` based on the `share_type` configured
    /// in the runtime.
    ///
    /// Returns a type-erased representation that can be serialized and sent to parties.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn example() -> Result<()> {
    /// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .parties(5)
    ///     .share_type(ShareType::Robust)  // Configure share type
    ///     .build()?;
    ///
    /// let client = runtime.client(100)
    ///     .with_inputs(vec![42, 37])
    ///     .build()?;
    ///
    /// // Automatically uses RobustShare based on runtime configuration
    /// let shares = client.generate_input_shares()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn generate_input_shares(&self) -> Result<Vec<Vec<u8>>> {
        use ark_serialize::CanonicalSerialize;

        match self.share_type {
            crate::ShareType::Robust => {
                let shares = self.generate_input_shares_robust()?;
                // Serialize each party's shares
                let mut serialized = Vec::with_capacity(shares.len());
                for party_shares in shares {
                    let mut bytes = Vec::new();
                    party_shares.serialize_compressed(&mut bytes)
                        .map_err(|e| Error::RuntimeError(format!("Failed to serialize robust shares: {:?}", e)))?;
                    serialized.push(bytes);
                }
                Ok(serialized)
            }
            crate::ShareType::NonRobust => {
                let shares = self.generate_input_shares_non_robust()?;
                // Serialize each party's shares
                let mut serialized = Vec::with_capacity(shares.len());
                for party_shares in shares {
                    let mut bytes = Vec::new();
                    party_shares.serialize_compressed(&mut bytes)
                        .map_err(|e| Error::RuntimeError(format!("Failed to serialize non-robust shares: {:?}", e)))?;
                    serialized.push(bytes);
                }
                Ok(serialized)
            }
        }
    }

    // =========================================================================
    // Output Reconstruction Methods
    // =========================================================================

    /// Receive and reconstruct computation outputs from the MPC network
    ///
    /// This method performs the final step of MPC output reconstruction:
    /// 1. Checks that enough output shares have been received (2t+1)
    /// 2. Performs robust interpolation to reconstruct the secret output values
    /// 3. Converts the reconstructed field elements back to i64 values
    ///
    /// The shares are received via `process_message()` which handles incoming
    /// `OutputMessage`s from MPC servers. This method then performs the local
    /// reconstruction once sufficient shares are available.
    ///
    /// # Protocol Flow
    ///
    /// ```text
    /// MPC Servers                             Client
    ///    |                                       |
    ///    |-------- OutputShare[r_i] ----------->| (via process_message())
    ///    |                                       | (stored in output_shares)
    ///    |                                       |
    ///    | client.receive_outputs()         <----| (checks: have 2t+1 shares?)
    ///    |                                       |
    ///    |                                       | (performs robust interpolation)
    ///    |                                       | (RobustShare::recover_secret())
    ///    |                                       |
    ///    |                                       | (converts Fr -> i64)
    ///    |                                       | Returns reconstructed values
    /// ```
    ///
    /// # Returns
    ///
    /// Returns a vector of reconstructed output values as i64. The length depends on
    /// the number of outputs in the computation (currently typically 1).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The inner client hasn't been initialized (call `send_inputs()` first)
    /// - Not enough output shares have been received yet (need 2t+1)
    /// - The shares are invalid or inconsistent (robust interpolation fails)
    /// - Field element to i64 conversion fails
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # use std::sync::Arc;
    /// # async fn example() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    /// #     .parties(5)
    /// #     .threshold(1)
    /// #     .build()?;
    /// # let mut client = runtime.client(100)
    /// #     .with_inputs(vec![42, 100, 25])
    /// #     .build()?;
    /// # let network = Arc::new(todo!());
    /// // Step 1: Initialize and send inputs
    /// client.send_inputs().await?;
    ///
    /// // Step 2: Process output shares from MPC servers as they arrive
    /// // (this would typically be in a message handling loop)
    /// // while let Some(msg) = network.receive().await? {
    /// //     client.process_message(msg, network.clone()).await?;
    /// // }
    ///
    /// // Step 3: Once enough shares received, reconstruct the output
    /// let outputs = client.receive_outputs().await?;
    /// println!("Computation result: {:?}", outputs);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Network Mode
    ///
    /// When using `attach_network()`, this method will:
    /// 1. Wait for output shares from MPC servers via `process_message()`
    /// 2. Perform robust interpolation once 2t+1 shares are received
    /// 3. Return the reconstructed output values
    ///
    /// See `examples/quick_start_local_network_real.rs` for a complete example.
    pub async fn receive_outputs(&mut self) -> Result<Vec<i64>> {
        use ark_bls12_381::Fr;
        use ark_ff::PrimeField;
        use stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare;
        use stoffelmpc_mpc::common::SecretSharingScheme;

        // Ensure inner client is initialized
        let inner = self.inner.as_mut()
            .ok_or_else(|| Error::RuntimeError(
                "Inner client not initialized. Call send_inputs() first.".to_string()
            ))?;

        // Perform network-based output reception
        // Access the output shares that have been collected via process_message()
        let share_store = inner.output.output_shares.lock().await;
        let config = self.config.as_ref()
            .ok_or_else(|| Error::InvalidInput("MPC config not set".to_string()))?;

        // Check if we have enough shares to reconstruct
        let threshold = config.threshold;
        let n_parties = config.n_parties;
        let min_shares = 2 * threshold + 1;

        if share_store.len() < min_shares {
            return Err(Error::RuntimeError(format!(
                "Not enough output shares received. Have {}, need {} (2t+1 where t={})\n\
                 Output shares are collected via process_message() as servers send them.\n\
                 Ensure you're running a message processing loop.",
                share_store.len(),
                min_shares,
                threshold
            )));
        }

        // Get the number of output values (should match input_len from the protocol)
        let input_len = inner.output.input_len;

        // Reorganize shares: from [party_id][output_idx] to [output_idx][party_id]
        // so we can reconstruct each output independently
        let mut output_shares_by_index: Vec<Vec<RobustShare<Fr>>> = vec![Vec::new(); input_len];

        for (_party_id, party_shares) in share_store.iter() {
            if party_shares.len() != input_len {
                return Err(Error::RuntimeError(format!(
                    "Share length mismatch: expected {}, got {}",
                    input_len,
                    party_shares.len()
                )));
            }

            for (idx, share) in party_shares.iter().enumerate() {
                output_shares_by_index[idx].push(share.clone());
            }
        }

        // Reconstruct each output value using robust interpolation
        let mut reconstructed_outputs = Vec::with_capacity(input_len);

        for (idx, shares) in output_shares_by_index.iter().enumerate() {
            // Perform robust secret reconstruction (handles up to t faulty shares)
            let (_, secret) = RobustShare::recover_secret(shares, n_parties)
                .map_err(|e| Error::RuntimeError(format!(
                    "Failed to reconstruct output at index {}: {:?}",
                    idx, e
                )))?;

            // Convert field element to i64
            let output_bigint = secret.into_bigint();
            let bytes = output_bigint.to_bytes_le();

            // Convert little-endian bytes to i64
            // Note: Assumes the output value fits in i64 range
            let mut i64_bytes = [0u8; 8];
            let len = bytes.len().min(8);
            i64_bytes[..len].copy_from_slice(&bytes[..len]);
            let output_value = i64::from_le_bytes(i64_bytes);

            reconstructed_outputs.push(output_value);
        }

        Ok(reconstructed_outputs)
    }

    // =========================================================================
    // Networking Methods (Client Network Operations)
    // =========================================================================

    /// Add a server address to connect to
    ///
    /// Register the addresses of MPC servers that will perform the computation.
    /// Call this for each server before calling `connect_to_servers()`.
    ///
    /// # Arguments
    ///
    /// * `server_id` - The party ID of the server
    /// * `address` - The socket address of the server
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn example() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    /// #     .parties(3).threshold(0).build()?;
    /// let mut client = runtime.client(100).with_inputs(vec![10, 20]).build()?;
    ///
    /// // Add server addresses
    /// client.add_server(0, "127.0.0.1:19200".parse()?);
    /// client.add_server(1, "127.0.0.1:19201".parse()?);
    /// client.add_server(2, "127.0.0.1:19202".parse()?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_server(&mut self, server_id: usize, address: std::net::SocketAddr) {
        // Register server in network manager (required for connect_as_client to work)
        // This matches the pattern used in MPCServer::add_peer()
        let mut network = std::sync::Arc::get_mut(&mut self.network)
            .expect("Network should be exclusively owned during setup");

        network.add_node_with_party_id(server_id, address);

        // Store server address for connection
        self.server_addresses.push(address);
        tracing::info!("Client {} added server {} at {}", self.client_id(), server_id, address);
    }

    /// Connect to all registered MPC servers
    ///
    /// Establishes QUIC connections to all servers that were registered via `add_server()`.
    ///
    /// # Returns
    ///
    /// Returns a message receiver for incoming messages from servers.
    ///
    /// # Errors
    ///
    /// Returns an error if connection attempts fail.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # async fn example() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    /// #     .parties(3).threshold(0).build()?;
    /// let mut client = runtime.client(100).with_inputs(vec![10, 20]).build()?;
    ///
    /// client.add_server(0, "127.0.0.1:19200".parse()?);
    /// client.add_server(1, "127.0.0.1:19201".parse()?);
    /// client.add_server(2, "127.0.0.1:19202".parse()?);
    ///
    /// // Connect to all servers
    /// let rx = client.connect_to_servers().await?;
    ///
    /// // Process messages from servers
    /// tokio::spawn(async move {
    ///     while let Some(msg) = rx.recv().await {
    ///         // Handle server messages
    ///     }
    /// });
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect_to_servers(&mut self) -> Result<tokio::sync::mpsc::Receiver<Vec<u8>>> {
        use tokio::sync::mpsc;
        use tracing::{info, error, warn, debug};
        use stoffelnet::network_utils::Network;

        let client_id = self.client_id();
        let (tx, rx) = mpsc::channel(100);

        let mut dialer = (*self.network).clone();

        for server_addr in &self.server_addresses {
            info!("Client {} connecting to server at {}", client_id, server_addr);

            let mut retry_count = 0;
            let max_retries = 5;

            loop {
                match dialer.connect_as_client(*server_addr, client_id).await {
                    Ok(connection) => {
                        info!("Client {} successfully connected to server at {}", client_id, server_addr);

                        let tx_conn = tx.clone();
                        let addr = *server_addr;
                        tokio::spawn(async move {
                            loop {
                                match connection.receive().await {
                                    Ok(data) => {
                                        // Filter out QUIC handshake/control messages
                                        if data.starts_with(b"ROLE:") {
                                            debug!("Client {} ignoring handshake message from server", client_id);
                                            continue;
                                        }

                                        // Filter magic byte handshakes
                                        if data.len() >= 4 {
                                            let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                                            if magic == 1448232275 {  // "SERV"
                                                debug!("Client {} ignoring server handshake", client_id);
                                                continue;
                                            }
                                        }

                                        if let Err(e) = tx_conn.send(data).await {
                                            error!("Client {} failed to send message from server {}: {:?}", client_id, addr, e);
                                        }
                                    }
                                    Err(e) => {
                                        info!("Client {} connection to server {} closed: {}", client_id, addr, e);
                                        break;
                                    }
                                }
                            }
                        });
                        break;
                    }
                    Err(e) => {
                        retry_count += 1;
                        if retry_count >= max_retries {
                            warn!("Client {} failed to connect to server {} after {} attempts: {}",
                                client_id, server_addr, retry_count, e);
                            break;
                        }

                        info!("Client {} connection attempt {} to server {} failed: {}",
                            client_id, retry_count, server_addr, e);
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            }
        }

        Ok(rx)
    }
}

/// Builder for creating an MPC client
///
/// This builder is returned by `StoffelRuntime::client(client_id)` and automatically
/// receives the complete MPC configuration from the runtime. The configuration includes:
/// - Protocol type (e.g., HoneyBadger)
/// - Number of parties and threshold
/// - Instance ID
///
/// When `.build()` is called, the builder creates an `MPCConfig` from these parameters
/// and passes it to the `MPCClient`, ensuring the client uses the exact same configuration
/// as the runtime and all other participants.
///
/// Clients provide private inputs to the MPC network but do not participate
/// in the computation themselves.
///
/// # Example
///
/// ```rust,no_run
/// # use stoffel_rust_sdk::{Stoffel, ProtocolType};
/// # fn main() -> stoffel_rust_sdk::Result<()> {
/// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
///     .parties(5)
///     .instance_id(42)
///     .build()?;
///
/// // Builder automatically receives runtime's MPC configuration
/// let client = runtime.client(100)
///     .with_inputs(vec![10, 20, 30])
///     .build()?;
///
/// // Client has access to the full configuration
/// assert_eq!(client.config().n_parties, 5);
/// assert_eq!(client.config().protocol_type, ProtocolType::HoneyBadger);
/// # Ok(())
/// # }
/// ```
pub struct MPCClientBuilder {
    pub(crate) client_id: usize,
    pub(crate) inputs: Vec<i64>,
    pub(crate) instance_id: u64,
    pub(crate) n_parties: usize,
    pub(crate) threshold: usize,
    pub(crate) protocol_type: crate::ProtocolType,
    pub(crate) share_type: crate::ShareType,
}

impl MPCClientBuilder {
    /// Create a new MPCClientBuilder
    ///
    /// This is typically called by `StoffelRuntime::client()` rather than directly.
    pub(crate) fn new(
        client_id: usize,
        n_parties: usize,
        threshold: usize,
        instance_id: u64,
        protocol_type: crate::ProtocolType,
        share_type: crate::ShareType,
    ) -> Self {
        Self {
            client_id,
            inputs: Vec::new(),
            instance_id,
            n_parties,
            threshold,
            protocol_type,
            share_type,
        }
    }

    /// Set the private inputs this client will provide
    pub fn with_inputs(mut self, inputs: Vec<i64>) -> Self {
        self.inputs = inputs;
        self
    }

    /// Build the MPC client with network manager
    ///
    /// Creates a network-based MPC client. The network manager is created automatically
    /// and initialized with the client's ID.
    pub fn build(self) -> Result<MPCClient> {
        // Create MPC config from runtime parameters
        let config = MPCConfig {
            n_parties: self.n_parties,
            threshold: self.threshold,
            instance_id: self.instance_id,
            protocol_type: self.protocol_type,
        };

        // Create network manager for this client - clients are always network-based
        let network = std::sync::Arc::new(
            stoffelnet::transports::quic::QuicNetworkManager::with_node_id(self.client_id)
        );

        let client = MPCClient::new(network)
            .with_client_id(self.client_id)
            .with_config(config)
            .with_inputs(self.inputs)
            .with_share_type(self.share_type)
            .build()?;

        Ok(client)
    }
}

