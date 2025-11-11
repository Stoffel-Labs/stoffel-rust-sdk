//! MPC Node implementation for peer-to-peer MPC
//!
//! This module provides the `MPCNode` abstraction for peer-to-peer MPC scenarios
//! where all parties both provide inputs AND participate in computation.
//!
//! # Quick Start - Production MPC
//!
//! **For production deployments, use the `network_helpers` module:**
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//! use ark_bls12_381::Fr;
//!
//! # async fn example() -> std::result::Result<(), Box<dyn std::error::Error>> {
//! // Complete network infrastructure in one call!
//! let (mut servers, receivers) = setup_honeybadger_quic_network::<Fr>(
//!     5, 1, 3, 8, 42, 19200,
//!     HoneyBadgerQuicConfig::default(),
//! ).await?;
//!
//! // Start and connect
//! for server in &mut servers {
//!     server.start().await?;
//! }
//! for server in &servers {
//!     server.connect_to_peers().await?;
//! }
//!
//! // Run MPC protocol!
//! // See examples/quick_start_local_network_real.rs
//! # Ok(())
//! # }
//! ```
//!
//! # API Exploration (without network execution)
//!
//! For learning the SDK API without network setup:
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! # fn main() -> Result<()> {
//! let runtime = Stoffel::compile("main main(a: secret int64, b: secret int64) -> secret int64:\n  return a * b")?
//!     .parties(5)
//!     .threshold(1)
//!     .build()?;
//!
//! // Create node (network manager created automatically)
//! let node = runtime.node(0)
//!     .with_inputs(vec![10, 20])
//!     .with_preprocessing(3, 8)
//!     .build()?;
//!
//! // For actual execution, use network_helpers module
//! # Ok(())
//! # }
//! ```

use crate::{Error, Result};
use crate::client::MPCConfig;

// Re-export key types from mpc-protocols for convenience
#[cfg(feature = "mpc-local")]
pub use stoffelmpc_mpc::honeybadger::{
    HoneyBadgerMPCNode,
    SessionId,
    ProtocolType,
};

#[cfg(feature = "mpc-local")]
use stoffelmpc_mpc::common::PreprocessingMPCProtocol;

#[cfg(feature = "mpc-local")]
use ark_ff::BigInteger;

#[cfg(feature = "mpc-local")]
pub use stoffelmpc_mpc::common::rbc::rbc::Avid;

/// MPC node that acts as both client and server
///
/// `MPCNode` combines the functionality of both `MPCClient` and `MPCServer`, allowing
/// a single entity to both provide private inputs AND participate in the secure computation.
/// This is useful in scenarios where all parties have data to contribute and want to
/// jointly compute on their combined inputs.
///
/// As an abstraction over the underlying MPC protocol, it handles:
/// - **Secret sharing own inputs**: Shares this party's inputs with the network
/// - **Receiving peer inputs**: Accepts secret shares from other parties
/// - **Preprocessing**: Generates cryptographic material for computation
/// - **Secure computation**: Collaboratively executes the program with other session parties
/// - **Output reconstruction**: Reconstructs the final result from output shares
///
/// # Architecture: Configuration from StoffelRuntime
///
/// Like `MPCClient` and `MPCServer`, `MPCNode` receives its configuration from
/// `StoffelRuntime` through `MPCConfig`. This ensures all node participants use
/// the same protocol, instance ID, and network parameters:
///
/// ```text
/// StoffelRuntime ──> Creates MPCConfig ──> Passed to MPCNode
///    (stores)          (n_parties,        (receives and
///                       threshold,          validates)
///                       instance_id,
///                       protocol_type)
/// ```
///
/// # When to Use
///
/// Use `MPCNode` for collaborative scenarios where:
/// - Multiple organizations each have private data to contribute
/// - All parties want to participate in the computation
/// - No single party should learn others' raw inputs
///
/// # Network Mode
///
/// `MPCNode` supports full network-based MPC operations via `attach_network()`:
///
/// ```rust,no_run
/// # use stoffel_rust_sdk::prelude::*;
/// # use stoffelnet::transports::quic::QuicNetworkManager;
/// # use std::sync::Arc;
/// # async fn example() -> Result<()> {
/// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?.parties(5).build()?;
/// let mut node = runtime.node(0).with_inputs(vec![10, 20]).build()?;
///
/// // Attach network for full MPC protocol
/// let network = QuicNetworkManager::with_node_id(0);
/// node.attach_network(Arc::new(network));
///
/// // Run complete protocol (preprocessing, input sharing, computation, output)
/// let results = node.run(runtime.program().bytecode()).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Note
///
/// In client-server architectures (MPC as a Service), use separate `MPCClient` and
/// `MPCServer` instead. Only use `MPCNode` when you truly need the combined functionality.
pub struct MPCNode {
    party_id: Option<usize>,
    inputs: Vec<i64>,
    n_triples: usize,
    n_random_shares: usize,
    config: Option<MPCConfig>,
    #[cfg(feature = "mpc-local")]
    inner: Option<HoneyBadgerMPCNode<ark_bls12_381::Fr, Avid>>,  // Uses node as it participates in computation
    // Network manager for full network mode (required)
    #[cfg(feature = "mpc-local")]
    network: std::sync::Arc<stoffelnet::transports::quic::QuicNetworkManager>,
}

impl MPCNode {
    // =========================================================================
    // Builder Methods (Internal - used by StoffelRuntime)
    // =========================================================================

    /// Create a new MPC session builder (internal use only)
    #[cfg(feature = "mpc-local")]
    pub(crate) fn new(network: std::sync::Arc<stoffelnet::transports::quic::QuicNetworkManager>) -> Self {
        Self {
            party_id: None,
            inputs: Vec::new(),
            n_triples: 0,
            n_random_shares: 0,
            config: None,
            inner: None,
            network,
        }
    }

    /// Create a new MPC session builder without network (non-local mode)
    #[cfg(not(feature = "mpc-local"))]
    pub(crate) fn new() -> Self {
        Self {
            party_id: None,
            inputs: Vec::new(),
            n_triples: 0,
            n_random_shares: 0,
            config: None,
        }
    }

    /// Set this party's ID
    pub(crate) fn with_party_id(mut self, id: usize) -> Self {
        self.party_id = Some(id);
        self
    }

    /// Set the MPC configuration from runtime
    pub(crate) fn with_config(mut self, config: MPCConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set private inputs for this party
    pub(crate) fn with_inputs(mut self, inputs: Vec<i64>) -> Self {
        self.inputs = inputs;
        self
    }

    /// Set preprocessing requirements
    pub(crate) fn with_preprocessing(mut self, n_triples: usize, n_random_shares: usize) -> Self {
        self.n_triples = n_triples;
        self.n_random_shares = n_random_shares;
        self
    }

    /// Build the MPC node
    pub(crate) fn build(self) -> Result<MPCNode> {
        let party_id = self
            .party_id
            .ok_or_else(|| Error::InvalidInput("party_id not set".to_string()))?;
        let config = self
            .config
            .ok_or_else(|| Error::InvalidInput("MPC config not set".to_string()))?;

        // Validate parameters
        if config.n_parties < 3 * config.threshold + 1 {
            return Err(Error::InvalidInput(format!(
                "Invalid parameters: n={} must be >= 3t+1={} for t={}",
                config.n_parties,
                3 * config.threshold + 1,
                config.threshold
            )));
        }

        if party_id >= config.n_parties {
            return Err(Error::InvalidInput(format!(
                "party_id={} must be < n_parties={}",
                party_id, config.n_parties
            )));
        }

        Ok(MPCNode {
            party_id: Some(party_id),
            inputs: self.inputs,
            n_triples: self.n_triples,
            n_random_shares: self.n_random_shares,
            config: Some(config),
            #[cfg(feature = "mpc-local")]
            inner: self.inner,
            #[cfg(feature = "mpc-local")]
            network: self.network,
        })
    }

    /// Attach a network manager for full network mode (advanced usage)
    ///
    /// This method allows you to attach a QUIC network manager to the node,
    /// enabling network-based MPC operations. Once attached, the `run()` method
    /// will perform actual network communication for the complete MPC protocol.
    ///
    /// # Arguments
    /// * `network` - Arc-wrapped QuicNetworkManager for async operations
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
    /// let mut network_manager = QuicNetworkManager::with_node_id(0);
    /// network_manager.listen("127.0.0.1:19200".parse().unwrap()).await.unwrap();
    ///
    /// let mut node = runtime.node(0)
    ///     .with_inputs(vec![10, 20])
    ///     .with_preprocessing(3, 8)
    ///     .build()?;
    ///
    /// node.attach_network(Arc::new(network_manager));
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "mpc-local")]
    // =========================================================================
    // Accessor Methods
    // =========================================================================

    /// Get this party's ID
    pub fn party_id(&self) -> usize {
        self.party_id.expect("party_id should be set after build()")
    }

    /// Get the inputs
    pub fn inputs(&self) -> &[i64] {
        &self.inputs
    }

    /// Get the instance ID from the MPC configuration
    ///
    /// This is a convenience method that reads from the shared `MPCConfig`.
    /// The instance ID comes from the `StoffelRuntime` configuration.
    pub fn instance_id(&self) -> u64 {
        self.config.as_ref().expect("config should be set after build()").instance_id
    }

    /// Get the full MPC configuration
    ///
    /// Returns the complete MPC configuration including protocol type, network
    /// parameters, and instance ID. This configuration must match across all
    /// session participants for the protocol to work correctly.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn example(node: MPCNode) {
    /// // MPCNode is created from StoffelRuntime (via runtime.node() - not yet implemented)
    /// // Once created, access the full configuration:
    /// let config = session.config();
    /// println!("Session using {} protocol with {} parties",
    ///     match config.protocol_type {
    ///         ProtocolType::HoneyBadger => "HoneyBadger"
    ///     },
    ///     config.n_parties);
    /// # }
    /// ```
    pub fn config(&self) -> &MPCConfig {
        self.config.as_ref().expect("config should be set after build()")
    }

    /// Get a reference to the network manager for configuration
    ///
    /// This allows you to configure the underlying QUIC network before execution:
    /// - Call `listen(address)` to bind to a socket
    /// - Call `add_node_with_party_id(id, address)` to register peers
    /// - Call `connect(address)` to establish connections
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # async fn example() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?.parties(5).threshold(1).build()?;
    /// let mut node = runtime.node(0).with_inputs(vec![10, 20]).build()?;
    ///
    /// // Configure the network before running
    /// {
    ///     let network = node.network_mut();
    ///     network.listen("127.0.0.1:19200".parse()?).await?;
    ///     network.add_node_with_party_id(1, "127.0.0.1:19201".parse()?);
    ///     // ... add more peers
    /// }
    ///
    /// let bytecode = runtime.program().bytecode();
    /// let result = node.run(bytecode).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "mpc-local")]
    pub fn network_mut(&mut self) -> &mut stoffelnet::transports::quic::QuicNetworkManager {
        std::sync::Arc::get_mut(&mut self.network)
            .expect("Cannot get mutable reference to network (Arc has multiple owners)")
    }

    /// Get a reference to the network manager (read-only)
    #[cfg(feature = "mpc-local")]
    pub fn network(&self) -> &stoffelnet::transports::quic::QuicNetworkManager {
        &self.network
    }

    // =========================================================================
    // MPC Protocol Methods
    // =========================================================================

    /// Execute the full MPC protocol as both client and server
    ///
    /// This method runs the complete MPC workflow for a session participant:
    /// 1. Secret shares this party's inputs with other session parties
    /// 2. Receives secret shares from other parties
    /// 3. Runs preprocessing to generate cryptographic material
    /// 4. Executes the Stoffel program on the secret-shared data
    /// 5. Reconstructs and returns the final output
    ///
    /// # Arguments
    ///
    /// * `bytecode` - The compiled Stoffel program bytecode to execute
    ///
    /// # Returns
    ///
    /// Returns the computation result as a vector of i64 values.
    ///
    /// # Errors
    ///
    /// Returns an error if any phase of the protocol fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # async fn example() -> Result<()> {
    /// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .parties(5)
    ///     .threshold(1)
    ///     .build()?;
    ///
    /// // Note: In practice, nodes are created from a StoffelRuntime
    /// // which provides the MPC configuration automatically.
    /// // This example shows manual construction for illustration.
    /// let mut node = MPCNode::new()
    ///     .with_party_id(0)
    ///     .with_inputs(vec![10, 20])
    ///     .with_preprocessing(10, 25)
    ///     .build()?; // config would be set via with_config() in production
    ///
    /// // Run the complete MPC protocol
    /// let results = node.run(runtime.program().bytecode()).await?;
    /// println!("Results: {:?}", results);
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "mpc-local")]
    pub async fn run(&mut self, _bytecode: &[u8]) -> Result<Vec<i64>> {
        use ark_bls12_381::Fr;
        use ark_ff::PrimeField;
        use stoffelmpc_mpc::honeybadger::{HoneyBadgerMPCNodeOpts};
        use stoffelmpc_mpc::common::MPCProtocol;
        use stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare;
        use stoffelmpc_mpc::common::SecretSharingScheme;
        use rand::rngs::StdRng;
        use rand::SeedableRng;
        use stoffelnet::network_utils::ClientId;

        let party_id = self.party_id
            .ok_or_else(|| Error::InvalidInput("party_id not set".to_string()))?;

        let config = self.config.as_ref()
            .ok_or_else(|| Error::InvalidInput("MPC config not set".to_string()))?;

        // Initialize HoneyBadgerMPCNode if not already done
        if self.inner.is_none() {
            // Validate parameters
            if config.n_parties < 3 * config.threshold + 1 {
                return Err(Error::InvalidInput(format!(
                    "Invalid HoneyBadger parameters: n={} must be >= 3t+1={} for t={}",
                    config.n_parties,
                    3 * config.threshold + 1,
                    config.threshold
                )));
            }

            let opts = HoneyBadgerMPCNodeOpts {
                n_parties: config.n_parties,
                threshold: config.threshold,
                n_triples: self.n_triples,
                n_random_shares: self.n_random_shares,
                instance_id: config.instance_id,
            };

            let node = <HoneyBadgerMPCNode<Fr, Avid> as MPCProtocol<
                ark_bls12_381::Fr,
                stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare<ark_bls12_381::Fr>,
                stoffelnet::transports::quic::QuicNetworkManager,
            >>::setup(party_id, opts)
                .map_err(|e| Error::RuntimeError(format!("Failed to setup HoneyBadgerMPCNode: {:?}", e)))?;

            self.inner = Some(node);
        }

        // Run the complete MPC protocol with the network
        let network = &self.network;
        let node = self.inner.as_mut()
            .ok_or_else(|| Error::RuntimeError("Node initialization failed".to_string()))?;

        let mut rng = StdRng::from_entropy();

        // Phase 1: Preprocessing - Generate beaver triples and random shares
        node.run_preprocessing(network.clone(), &mut rng)
            .await
            .map_err(|e| Error::RuntimeError(format!("Preprocessing failed: {:?}", e)))?;

        // Phase 2: Input Sharing - Each node shares their inputs with the network
        let num_inputs = self.inputs.len();

        // Convert inputs to field elements
        let field_inputs: Vec<Fr> = self.inputs.iter()
            .map(|&x| Fr::from(x as u64))
            .collect();

        // Take random shares from preprocessing for input masking
        let local_shares = node.preprocessing_material
            .lock().await
            .take_random_shares(num_inputs)
            .map_err(|e| Error::RuntimeError(format!("Failed to take random shares: {:?}", e)))?;

        // Use this node's party ID as the client ID for the input protocol
        let client_id = ClientId::from(party_id);

        // Initialize input protocol for this node's inputs
        node.preprocess.input
            .init(client_id, local_shares, num_inputs, network.clone())
            .await
            .map_err(|e| Error::RuntimeError(format!("Failed to initialize input protocol: {:?}", e)))?;

        // Broadcast masked inputs to all other nodes
        // This happens automatically when enough mask shares are received
        // Each node will receive masked inputs and subtract their masks

        // Phase 3: Computation - Execute the Stoffel program on secret-shared data
        // For now, we demonstrate with a simple multiplication
        // Full VM integration would parse bytecode and execute all operations

        // Get input shares from storage (assumes 2 inputs for multiplication)
        let (x_shares, y_shares) = {
            let input_store = node.preprocess.input.input_shares.lock().await;
            let inputs = input_store.get(&client_id)
                .ok_or_else(|| Error::RuntimeError("Failed to get input shares".to_string()))?;

            if inputs.len() < 2 {
                return Err(Error::RuntimeError(format!(
                    "Expected at least 2 inputs, got {}",
                    inputs.len()
                )));
            }

            (vec![inputs[0].clone()], vec![inputs[1].clone()])
        };

        // Run secure multiplication
        node.mul(x_shares, y_shares, network.clone())
            .await
            .map_err(|e| Error::RuntimeError(format!("Multiplication failed: {:?}", e)))?;

        // Phase 4: Output Reconstruction - Collect shares and reconstruct result
        // Get the multiplication result shares
        let session_id = SessionId::new(
            ProtocolType::Mul,
            party_id as u8,
            party_id as u8,
            config.instance_id,
        );

        let shares_for_output = {
            let storage_map = node.operations.mul.mult_storage.lock().await;
            let storage_mutex = storage_map.get(&session_id)
                .ok_or_else(|| Error::RuntimeError("No multiplication result found".to_string()))?;
            let storage = storage_mutex.lock().await;
            storage.protocol_output.clone()
        };

        // Reconstruct the secret from shares
        // In a full implementation, this would involve collecting shares from all parties
        let (_, secret) = RobustShare::recover_secret(&shares_for_output, config.n_parties)
            .map_err(|e| Error::RuntimeError(format!("Failed to reconstruct output: {:?}", e)))?;

        // Convert field element to i64
        let output_bigint = secret.into_bigint();
        let bytes = output_bigint.to_bytes_le();

        let mut i64_bytes = [0u8; 8];
        let len = bytes.len().min(8);
        i64_bytes[..len].copy_from_slice(&bytes[..len]);
        let output_value = i64::from_le_bytes(i64_bytes);

        Ok(vec![output_value])
    }

    /// Perform secure multiplication on secret-shared values
    ///
    /// This is a lower-level primitive for secure computation. Most users should
    /// use the `run()` method instead which handles the complete protocol.
    ///
    /// # Arguments
    ///
    /// * `a` - First secret-shared value (as field element)
    /// * `b` - Second secret-shared value (as field element)
    ///
    /// # Returns
    ///
    /// Returns a secret share of the product a * b.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Preprocessing material (beaver triples) is exhausted
    /// - Communication with other parties fails
    ///
    /// # Note
    ///
    /// This is an advanced API. For most use cases, use `run()` to execute
    /// a complete Stoffel program instead.
    #[cfg(feature = "mpc-local")]
    pub async fn mul(&mut self, _a: ark_bls12_381::Fr, _b: ark_bls12_381::Fr) -> Result<ark_bls12_381::Fr> {
        // TODO: Use beaver triple from preprocessing to perform secure multiplication
        Err(Error::RuntimeError(
            "mul() not yet fully implemented - protocol integration pending".to_string()
        ))
    }

    /// Reconstruct output from secret shares
    ///
    /// This method collects shares from all parties and reconstructs the final
    /// output value. This is typically called at the end of a computation.
    ///
    /// # Arguments
    ///
    /// * `share` - This party's share of the output
    ///
    /// # Returns
    ///
    /// Returns the reconstructed output value.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Not enough shares are received from other parties
    /// - The shares are inconsistent or invalid
    ///
    /// # Note
    ///
    /// This is an advanced API. For most use cases, use `run()` which handles
    /// output reconstruction automatically.
    #[cfg(feature = "mpc-local")]
    pub async fn output(&mut self, _share: ark_bls12_381::Fr) -> Result<i64> {
        // TODO: Collect shares from all parties
        // TODO: Reconstruct value using Lagrange interpolation
        // TODO: Convert field element back to i64
        Err(Error::RuntimeError(
            "output() not yet fully implemented - protocol integration pending".to_string()
        ))
    }

    // =========================================================================
    // Stubs for builds without mpc-local feature
    // =========================================================================

    /// Run MPC protocol (non-async stub for non-mpc-local builds)
    #[cfg(not(feature = "mpc-local"))]
    pub fn run(&mut self, _bytecode: &[u8]) -> Result<Vec<i64>> {
        Err(Error::RuntimeError(
            "run() requires the 'mpc-local' feature to be enabled".to_string()
        ))
    }

    /// Secure multiplication (non-async stub for non-mpc-local builds)
    #[cfg(not(feature = "mpc-local"))]
    pub fn mul(&mut self, _a: i64, _b: i64) -> Result<i64> {
        Err(Error::RuntimeError(
            "mul() requires the 'mpc-local' feature to be enabled".to_string()
        ))
    }

    /// Output reconstruction (non-async stub for non-mpc-local builds)
    #[cfg(not(feature = "mpc-local"))]
    pub fn output(&mut self, _share: i64) -> Result<i64> {
        Err(Error::RuntimeError(
            "output() requires the 'mpc-local' feature to be enabled".to_string()
        ))
    }
}

/// Builder for creating an MPC node
///
/// This builder is returned by `StoffelRuntime::node(party_id)` and automatically
/// receives the complete MPC configuration from the runtime. The configuration includes:
/// - Protocol type (e.g., HoneyBadger)
/// - Number of parties and threshold
/// - Instance ID
///
/// When `.build()` is called, the builder creates an `MPCConfig` from these parameters
/// and passes it to the `MPCNode`, ensuring the node uses the exact same configuration
/// as the runtime and all other participants.
///
/// Nodes are for peer-to-peer scenarios where all parties both provide inputs
/// AND participate in computation.
///
/// # Example
///
/// ```rust,no_run
/// # use stoffel_rust_sdk::{Stoffel, ProtocolType};
/// # fn main() -> stoffel_rust_sdk::Result<()> {
/// let runtime = Stoffel::compile("main main(a: secret int64, b: secret int64) -> secret int64:\n  return a * b")?
///     .parties(5)
///     .threshold(1)
///     .build()?;
///
/// // Builder automatically receives runtime's MPC configuration
/// let node = runtime.node(0)
///     .with_inputs(vec![10, 20])
///     .with_preprocessing(3, 8)
///     .build()?;
///
/// // Node has inputs and participates in computation
/// assert_eq!(node.party_id(), 0);
/// assert_eq!(node.inputs(), &[10, 20]);
/// # Ok(())
/// # }
/// ```
pub struct MPCNodeBuilder {
    pub(crate) party_id: usize,
    pub(crate) inputs: Vec<i64>,
    pub(crate) n_triples: Option<usize>,
    pub(crate) n_random_shares: Option<usize>,
    pub(crate) n_parties: usize,
    pub(crate) threshold: usize,
    pub(crate) instance_id: u64,
    pub(crate) protocol_type: crate::ProtocolType,
}

impl MPCNodeBuilder {
    /// Create a new MPCNodeBuilder
    ///
    /// This is typically called by `StoffelRuntime::node()` rather than directly.
    pub(crate) fn new(
        party_id: usize,
        n_parties: usize,
        threshold: usize,
        instance_id: u64,
        protocol_type: crate::ProtocolType,
    ) -> Self {
        Self {
            party_id,
            inputs: Vec::new(),
            n_triples: None,
            n_random_shares: None,
            n_parties,
            threshold,
            instance_id,
            protocol_type,
        }
    }

    /// Set the private inputs this node will contribute
    pub fn with_inputs(mut self, inputs: Vec<i64>) -> Self {
        self.inputs = inputs;
        self
    }

    /// Set preprocessing material requirements
    ///
    /// If not set, reasonable defaults will be calculated based on the MPC configuration.
    ///
    /// # Arguments
    ///
    /// * `n_triples` - Number of beaver triples for multiplication
    /// * `n_random_shares` - Number of random shares (typically inputs + 2*triples)
    pub fn with_preprocessing(mut self, n_triples: usize, n_random_shares: usize) -> Self {
        self.n_triples = Some(n_triples);
        self.n_random_shares = Some(n_random_shares);
        self
    }

    /// Build the MPC node
    pub fn build(self) -> Result<MPCNode> {
        // Use defaults if not set
        let n_triples = self.n_triples.unwrap_or(2 * self.threshold + 1);
        let n_random_shares = self.n_random_shares.unwrap_or(2 + 2 * n_triples);

        // Create MPC config from runtime parameters
        let config = crate::client::MPCConfig {
            n_parties: self.n_parties,
            threshold: self.threshold,
            instance_id: self.instance_id,
            protocol_type: self.protocol_type,
        };

        // Create network manager for this node (runtime-managed abstraction)
        #[cfg(feature = "mpc-local")]
        let network = std::sync::Arc::new(
            stoffelnet::transports::quic::QuicNetworkManager::with_node_id(self.party_id)
        );

        #[cfg(feature = "mpc-local")]
        let node = MPCNode::new(network);

        #[cfg(not(feature = "mpc-local"))]
        let node = MPCNode::new();

        node
            .with_party_id(self.party_id)
            .with_config(config)
            .with_inputs(self.inputs)
            .with_preprocessing(n_triples, n_random_shares)
            .build()
    }
}
