//! MPC Server implementation for server-side computation
//!
//! This module provides the `MPCServer` abstraction for MPC network servers that perform
//! secure computation on secret-shared data.

use crate::{Error, Result};
use crate::client::MPCConfig;

// Re-export key types from mpc-protocols for convenience
#[cfg(feature = "mpc-local")]
pub use stoffelmpc_mpc::honeybadger::{
    HoneyBadgerMPCNode,
};

#[cfg(feature = "mpc-local")]
pub use stoffelmpc_mpc::common::rbc::rbc::Avid;

/// MPC network server
///
/// `MPCServer` represents a single server in the MPC network that performs secure computation
/// on secret-shared data. It is an abstraction over the underlying MPC protocol that handles:
/// - **Preprocessing**: Generates cryptographic material (beaver triples, random shares) offline
/// - **Input reception**: Receives secret shares from clients
/// - **Secure computation**: Executes the Stoffel program on secret-shared data collaboratively with other servers
/// - **Output distribution**: Sends result shares back to clients
///
/// Multiple servers work together using the MPC protocol to compute on client inputs without
/// any individual server learning the private inputs. The protocol ensures correctness and
/// privacy even if up to `threshold` servers are faulty or malicious.
///
/// # Architecture: Configuration from StoffelRuntime
///
/// `MPCServer` receives its configuration (`MPCConfig`) from the `StoffelRuntime` that
/// creates it. This ensures all servers in the network use consistent parameters:
///
/// ```text
/// StoffelRuntime --creates--> MPCServerBuilder --builds--> MPCServer
///       |                          |                        |
///       |                          |                        |
///    MPCConfig ────────────────────┴────────────────────────┘
///         (protocol, instance_id, n_parties, threshold)
/// ```
///
/// The configuration includes:
/// - Protocol type (e.g., HoneyBadger for Byzantine fault tolerance)
/// - Instance ID (ensures all servers are computing on the same instance)
/// - Party count and threshold (network topology and fault tolerance parameters)
///
/// # Usage
///
/// Always create servers from a `StoffelRuntime`:
///
/// ```rust,no_run
/// # use stoffel_rust_sdk::prelude::*;
/// # fn example() -> Result<()> {
/// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
///     .parties(5)
///     .threshold(1)
///     .build()?;
///
/// // Server automatically receives MPC configuration from runtime
/// let server = runtime.server(0)
///     .with_preprocessing(10, 20)  // 10 beaver triples, 20 random shares
///     .build()?;
///
/// // Configuration is accessible
/// assert_eq!(server.config().n_parties, 5);
/// assert_eq!(server.config().protocol_type, ProtocolType::HoneyBadger);
///
/// // Server workflow (when network integration is complete):
/// // 1. server.run_preprocessing()      - Generate cryptographic material
/// // 2. server.receive_client_inputs()  - Accept secret shares from clients
/// // 3. server.compute()                - Execute program on secret-shared data
/// // 4. server.send_outputs()           - Send result shares to clients
/// # Ok(())
/// # }
/// ```
pub struct MPCServer {
    party_id: Option<usize>,
    n_triples: usize,
    n_random_shares: usize,
    config: Option<MPCConfig>,
    #[cfg(feature = "mpc-local")]
    inner: Option<HoneyBadgerMPCNode<ark_bls12_381::Fr, Avid>>,
    // Store received input shares from clients
    input_shares: Vec<stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare<ark_bls12_381::Fr>>,
    // Network manager for MPC operations (required for all network operations)
    #[cfg(feature = "mpc-local")]
    network: std::sync::Arc<stoffelnet::transports::quic::QuicNetworkManager>,
}

impl MPCServer {
    // =========================================================================
    // Builder Methods (Internal - used by StoffelRuntime)
    // =========================================================================

    /// Create a new MPC node builder with network manager (internal use only)
    ///
    /// Network manager is created and provided by the StoffelRuntime
    #[cfg(feature = "mpc-local")]
    pub(crate) fn new(network: std::sync::Arc<stoffelnet::transports::quic::QuicNetworkManager>) -> Self {
        Self {
            party_id: None,
            n_triples: 0,
            n_random_shares: 0,
            config: None,
            inner: None,
            input_shares: Vec::new(),
            network,
        }
    }

    /// Create a new MPC node builder without mpc-local feature
    #[cfg(not(feature = "mpc-local"))]
    pub(crate) fn new() -> Self {
        Self {
            party_id: None,
            n_triples: 0,
            n_random_shares: 0,
            config: None,
            input_shares: Vec::new(),
        }
    }

    /// Set this party's ID (must be unique, typically 0 to n-1)
    pub(crate) fn with_party_id(mut self, id: usize) -> Self {
        self.party_id = Some(id);
        self
    }

    /// Set the MPC configuration from runtime
    pub(crate) fn with_config(mut self, config: MPCConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set preprocessing material requirements (beaver triples, random shares)
    ///
    /// # Arguments
    /// * `n_triples` - Number of beaver triples needed for multiplication
    /// * `n_random_shares` - Number of random shares needed (inputs + 2*triples)
    pub(crate) fn with_preprocessing(mut self, n_triples: usize, n_random_shares: usize) -> Self {
        self.n_triples = n_triples;
        self.n_random_shares = n_random_shares;
        self
    }

    /// Build the MPC node
    pub(crate) fn build(self) -> Result<MPCServer> {
        let party_id = self
            .party_id
            .ok_or_else(|| Error::InvalidInput("party_id not set".to_string()))?;
        let config = self
            .config
            .ok_or_else(|| Error::InvalidInput("MPC config not set".to_string()))?;

        // Validate party_id
        if party_id >= config.n_parties {
            return Err(Error::InvalidInput(format!(
                "party_id={} must be < n_parties={}",
                party_id, config.n_parties
            )));
        }

        Ok(MPCServer {
            party_id: Some(party_id),
            n_triples: self.n_triples,
            n_random_shares: self.n_random_shares,
            config: Some(config),
            #[cfg(feature = "mpc-local")]
            inner: self.inner,
            input_shares: Vec::new(),
            #[cfg(feature = "mpc-local")]
            network: self.network,
        })
    }

    // =========================================================================
    // Accessor Methods
    // =========================================================================

    /// Get this party's ID
    pub fn party_id(&self) -> usize {
        self.party_id.expect("party_id should be set after build()")
    }

    /// Get the instance ID from the MPC configuration
    ///
    /// This is a convenience method that reads from the shared `MPCConfig`.
    /// The instance ID comes from the `StoffelRuntime` that created this node.
    pub fn instance_id(&self) -> u64 {
        self.config.as_ref().expect("config should be set after build()").instance_id
    }

    /// Get the full MPC configuration
    ///
    /// Returns the complete MPC configuration that was passed from `StoffelRuntime`.
    /// This includes:
    /// - `n_parties`: Total number of parties in the MPC network
    /// - `threshold`: Maximum number of faulty parties the protocol can tolerate
    /// - `instance_id`: Unique identifier ensuring all nodes compute on the same instance
    /// - `protocol_type`: The MPC protocol being used (e.g., HoneyBadger)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn example() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    /// #     .parties(5).threshold(1).build()?;
    /// let node = runtime.node(0).build()?;
    ///
    /// let config = node.config();
    /// println!("Node {}/{} in network", node.party_id(), config.n_parties);
    /// println!("Tolerates up to {} faulty parties", config.threshold);
    /// # Ok(())
    /// # }
    /// ```
    pub fn config(&self) -> &MPCConfig {
        self.config.as_ref().expect("config should be set after build()")
    }

    // =========================================================================
    // MPC Protocol Methods
    // =========================================================================

    /// Generate preprocessing material (beaver triples and random shares)
    ///
    /// This method runs the offline preprocessing phase to generate cryptographic
    /// material needed for secure computation. This can be done before clients
    /// submit inputs and doesn't require knowledge of the actual data.
    ///
    /// Generates:
    /// - Beaver triples: Used for secure multiplication
    /// - Random shares: Used for secret sharing client inputs
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The MPC network is not properly connected
    /// - Communication with other nodes fails
    /// - The preprocessing protocol encounters an error
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
    /// let mut node = runtime.node(0)
    ///     .with_preprocessing(10, 20)
    ///     .build()?;
    ///
    /// // Generate preprocessing material
    /// node.run_preprocessing().await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "mpc-local")]
    pub async fn run_preprocessing(&mut self) -> Result<()> {
        use stoffelmpc_mpc::common::{MPCProtocol, PreprocessingMPCProtocol};
        use stoffelmpc_mpc::honeybadger::HoneyBadgerMPCNodeOpts;
        use ark_std::rand::SeedableRng;

        // Initialize HoneyBadgerMPCNode if not already done
        if self.inner.is_none() {
            let config = self.config.as_ref()
                .ok_or_else(|| Error::InvalidInput("MPC config not set".to_string()))?;
            let party_id = self.party_id();

            let opts = HoneyBadgerMPCNodeOpts {
                n_parties: config.n_parties,
                threshold: config.threshold,
                n_triples: self.n_triples,
                n_random_shares: self.n_random_shares,
                instance_id: config.instance_id,
            };

            // Create the HoneyBadger MPC node
            let node = <HoneyBadgerMPCNode<ark_bls12_381::Fr, Avid> as MPCProtocol<
                ark_bls12_381::Fr,
                stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare<ark_bls12_381::Fr>,
                stoffelnet::transports::quic::QuicNetworkManager,
            >>::setup(party_id, opts)
                .map_err(|e| Error::RuntimeError(format!("Failed to setup HoneyBadgerMPCNode: {:?}", e)))?;

            self.inner = Some(node);
        }

        // Run preprocessing using the attached network
        let mut rng = ark_std::rand::rngs::StdRng::from_entropy();

        self.inner.as_mut()
            .ok_or_else(|| Error::RuntimeError("MPC node not initialized".to_string()))?
            .run_preprocessing(self.network.clone(), &mut rng)
            .await
            .map_err(|e| Error::RuntimeError(format!("Preprocessing failed: {:?}", e)))?;

        Ok(())
    }

    /// Receive and store secret shares from clients
    ///
    /// This method accepts secret-shared inputs from clients in the MPC network.
    /// Each node receives shares such that no individual node learns the private inputs.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network communication fails
    /// - Received shares are invalid or malformed
    /// - The protocol detects inconsistent shares
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # async fn example() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    /// #     .parties(5)
    /// #     .threshold(1)
    /// #     .build()?;
    /// # let mut node = runtime.node(0).build()?;
    /// # node.run_preprocessing().await?;
    /// // Accept secret shares from clients
    /// node.receive_client_inputs().await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "mpc-local")]
    pub async fn receive_client_inputs(&mut self, client_id: stoffelnet::network_utils::ClientId, num_inputs: usize) -> Result<()> {
        // Ensure the MPC node is initialized
        let node = self.inner.as_mut()
            .ok_or_else(|| Error::RuntimeError(
                "MPC node not initialized. Call run_preprocessing() first.".to_string()
            ))?;

        // Take random shares from preprocessing material for input masking
        let local_shares = node
            .preprocessing_material
            .lock()
            .await
            .take_random_shares(num_inputs)
            .map_err(|e| Error::RuntimeError(format!("Failed to take random shares: {:?}", e)))?;

        // Initialize input protocol with client ID
        node.preprocess.input
            .init(client_id, local_shares, num_inputs, self.network.clone())
            .await
            .map_err(|e| Error::RuntimeError(format!("Failed to initialize input protocol: {:?}", e)))?;

        Ok(())
    }

    /// Execute the Stoffel program on secret-shared data
    ///
    /// This method runs the secure computation phase where nodes collaboratively
    /// execute the compiled Stoffel program on the secret-shared inputs without
    /// revealing them. Uses the preprocessing material (beaver triples) for
    /// secure multiplication operations.
    ///
    /// # Arguments
    ///
    /// * `bytecode` - The compiled Stoffel program bytecode to execute
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The bytecode is invalid or incompatible
    /// - Communication with other nodes fails during computation
    /// - The MPC protocol encounters an error
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # async fn example() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    /// #     .parties(5)
    /// #     .threshold(1)
    /// #     .build()?;
    /// # let mut node = runtime.node(0).build()?;
    /// # node.run_preprocessing().await?;
    /// # node.receive_client_inputs().await?;
    /// // Execute program on secret-shared data
    /// let bytecode = runtime.program().bytecode();
    /// node.compute(bytecode).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "mpc-local")]
    pub async fn compute(&mut self, client_id: stoffelnet::network_utils::ClientId) -> Result<()> {
        use stoffelmpc_mpc::common::MPCProtocol;
        use stoffelmpc_mpc::honeybadger::{ProtocolType, SessionId};

        // Ensure the MPC node is initialized
        let node = self.inner.as_mut()
            .ok_or_else(|| Error::RuntimeError(
                "MPC node not initialized. Call run_preprocessing() first.".to_string()
            ))?;

        // Run multiplication using the network
        // Note: This is a simplified implementation that runs a single multiplication
        // Full VM integration would parse bytecode and execute all operations
        let config = self.config.as_ref()
            .ok_or_else(|| Error::RuntimeError("Config not set".to_string()))?;

        // Get input shares from storage (assumes 2 inputs for multiplication)
        let (x_shares, y_shares) = {
            let input_store = node.preprocess.input.input_shares.lock().await;
            let inputs = input_store.get(&client_id)
                .ok_or_else(|| Error::RuntimeError(format!("No input shares found for client {}", client_id)))?;

            if inputs.len() < 2 {
                return Err(Error::InvalidInput("Need at least 2 input shares for multiplication".to_string()));
            }

            (vec![inputs[0].clone()], vec![inputs[1].clone()])
        };

        // Run MPC multiplication
        node.mul(x_shares, y_shares, self.network.clone())
            .await
            .map_err(|e| Error::RuntimeError(format!("Multiplication failed: {:?}", e)))?;

        Ok(())
    }

    /// Send output shares back to clients
    ///
    /// This method distributes shares of the computation result to the clients
    /// so they can reconstruct the final output.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network communication fails
    /// - Output shares haven't been generated yet
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # async fn example() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    /// #     .parties(5)
    /// #     .threshold(1)
    /// #     .build()?;
    /// # let mut node = runtime.node(0).build()?;
    /// # node.run_preprocessing().await?;
    /// # node.receive_client_inputs().await?;
    /// # node.compute(runtime.program().bytecode()).await?;
    /// // Send result shares to clients
    /// node.send_outputs().await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "mpc-local")]
    pub async fn send_outputs(&mut self, output_client_id: stoffelnet::network_utils::ClientId, session_id: stoffelmpc_mpc::honeybadger::SessionId) -> Result<()> {
        // Ensure the MPC node is initialized
        let node = self.inner.as_ref()
            .ok_or_else(|| Error::RuntimeError(
                "MPC node not initialized. Call run_preprocessing() first.".to_string()
            ))?;

        // Collect output shares from computation storage
        let storage_map = node.operations.mul.mult_storage.lock().await;

        if let Some(storage_mutex) = storage_map.get(&session_id) {
            let storage = storage_mutex.lock().await;

            if storage.protocol_output.is_empty() {
                return Err(Error::RuntimeError(format!("No output shares found for session {:?}", session_id)));
            }

            let shares_for_output = storage.protocol_output.clone();
            let num_outputs = shares_for_output.len();

            // Initialize output protocol to send to client
            node.output
                .init(output_client_id, shares_for_output, num_outputs, self.network.clone())
                .await
                .map_err(|e| Error::RuntimeError(format!("Failed to initialize output protocol: {:?}", e)))?;

            Ok(())
        } else {
            Err(Error::RuntimeError(format!("No computation storage found for session {:?}", session_id)))
        }
    }

    // =========================================================================
    // Stubs for builds without mpc-local feature
    // =========================================================================

    /// Run preprocessing (non-async stub for non-mpc-local builds)
    #[cfg(not(feature = "mpc-local"))]
    pub fn run_preprocessing(&mut self) -> Result<()> {
        Err(Error::RuntimeError(
            "run_preprocessing() requires the 'mpc-local' feature to be enabled".to_string()
        ))
    }

    /// Receive client inputs (non-async stub for non-mpc-local builds)
    #[cfg(not(feature = "mpc-local"))]
    pub fn receive_client_inputs(&mut self) -> Result<()> {
        Err(Error::RuntimeError(
            "receive_client_inputs() requires the 'mpc-local' feature to be enabled".to_string()
        ))
    }

    /// Execute computation (non-async stub for non-mpc-local builds)
    #[cfg(not(feature = "mpc-local"))]
    pub fn compute(&mut self, _bytecode: &[u8]) -> Result<()> {
        Err(Error::RuntimeError(
            "compute() requires the 'mpc-local' feature to be enabled".to_string()
        ))
    }

    /// Send outputs (non-async stub for non-mpc-local builds)
    #[cfg(not(feature = "mpc-local"))]
    pub fn send_outputs(&mut self) -> Result<()> {
        Err(Error::RuntimeError(
            "send_outputs() requires the 'mpc-local' feature to be enabled".to_string()
        ))
    }

    /// Receive input shares from a client
    ///
    /// This method stores the shares that were generated by a client for this specific party.
    /// Each server receives its own designated shares (where share.id == party_id).
    ///
    /// # Arguments
    /// * `shares` - The input shares for this party (one share per client input)
    ///
    /// # Example
    /// ```no_run
    /// use stoffel_rust_sdk::prelude::*;
    ///
    /// # fn main() -> Result<()> {
    /// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .parties(5)
    ///     .threshold(1)
    ///     .build()?;
    ///
    /// let client = runtime.client(100).with_inputs(vec![10, 20]).build()?;
    /// let mut server = runtime.server(0).build()?;
    ///
    /// // Client generates shares
    /// let shares_per_party = client.generate_input_shares_robust()?;
    ///
    /// // Server 0 receives its shares (shares_per_party[0])
    /// server.receive_input_shares(&shares_per_party[0])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn receive_input_shares(
        &mut self,
        shares: &[stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare<ark_bls12_381::Fr>]
    ) -> Result<()> {
        let party_id = self.party_id();

        // Validate that all shares are for this party
        for (idx, share) in shares.iter().enumerate() {
            if share.id != party_id {
                return Err(Error::InvalidInput(format!(
                    "Share {} has id={} but should be {} for this party",
                    idx, share.id, party_id
                )));
            }
        }

        // Store the shares
        self.input_shares = shares.to_vec();

        Ok(())
    }

    /// Get the number of input shares currently stored
    pub fn num_input_shares(&self) -> usize {
        self.input_shares.len()
    }

    /// Get a reference to the stored input shares
    pub fn input_shares(&self) -> &[stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare<ark_bls12_381::Fr>] {
        &self.input_shares
    }
}

/// Builder for creating an MPC server
///
/// This builder is returned by `StoffelRuntime::server(party_id)` and automatically
/// receives the MPC configuration from the runtime. The configuration includes:
/// - Protocol type (e.g., HoneyBadger)
/// - Number of parties and threshold
/// - Instance ID
///
/// When `.build()` is called, the builder creates an `MPCConfig` from these parameters
/// and passes it to the `MPCServer`, ensuring tight coupling with the runtime's configuration.
///
/// # Example
///
/// ```rust,no_run
/// # use stoffel_rust_sdk::Stoffel;
/// # fn main() -> stoffel_rust_sdk::Result<()> {
/// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
///     .parties(5)
///     .threshold(1)
///     .build()?;
///
/// // Builder automatically receives runtime's MPC configuration
/// let server = runtime.server(0)
///     .with_preprocessing(10, 25)
///     .build()?;
///
/// // Server has access to the full configuration
/// assert_eq!(server.config().n_parties, 5);
/// assert_eq!(server.config().threshold, 1);
/// # Ok(())
/// # }
/// ```
pub struct MPCServerBuilder {
    pub(crate) party_id: usize,
    pub(crate) n_triples: Option<usize>,
    pub(crate) n_random_shares: Option<usize>,
    pub(crate) n_parties: usize,
    pub(crate) threshold: usize,
    pub(crate) instance_id: u64,
    pub(crate) protocol_type: crate::ProtocolType,
}

impl MPCServerBuilder {
    /// Create a new MPCServerBuilder
    ///
    /// This is typically called by `StoffelRuntime::server()` rather than directly.
    pub(crate) fn new(
        party_id: usize,
        n_parties: usize,
        threshold: usize,
        instance_id: u64,
        protocol_type: crate::ProtocolType,
    ) -> Self {
        Self {
            party_id,
            n_triples: None,
            n_random_shares: None,
            n_parties,
            threshold,
            instance_id,
            protocol_type,
        }
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

    /// Build the MPC server
    pub fn build(self) -> crate::Result<MPCServer> {
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

        // Create network manager for this party (runtime-managed abstraction)
        #[cfg(feature = "mpc-local")]
        let network = std::sync::Arc::new(
            stoffelnet::transports::quic::QuicNetworkManager::with_node_id(self.party_id)
        );

        #[cfg(feature = "mpc-local")]
        let server = MPCServer::new(network);

        #[cfg(not(feature = "mpc-local"))]
        let server = MPCServer::new();

        server
            .with_party_id(self.party_id)
            .with_config(config)
            .with_preprocessing(n_triples, n_random_shares)
            .build()
    }
}
