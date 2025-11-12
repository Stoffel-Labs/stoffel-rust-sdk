//! Network-based MPC Server implementation for server-side computation
//!
//! This module provides the `MPCServer` abstraction for MPC network servers that perform
//! secure computation on secret-shared data. Servers are network-based by design and use
//! QUIC networking to communicate with other servers and clients.

use crate::{Error, Result};
use crate::client::MPCConfig;

// Re-export key types from mpc-protocols for convenience
pub use stoffelmpc_mpc::honeybadger::{
    HoneyBadgerMPCNode,
};

pub use stoffelmpc_mpc::common::rbc::rbc::Avid;

/// Network-based MPC server for secure computation
///
/// `MPCServer` represents a network-based server in the MPC network that performs secure computation
/// on secret-shared data. Servers are network-based by design and use QUIC networking to:
/// - **Connect to peers**: Establish connections with other MPC servers
/// - **Receive inputs**: Accept secret shares from clients over the network
/// - **Collaborate**: Perform secure computation with other servers via network messages
/// - **Distribute outputs**: Send result shares back to clients
///
/// The server handles:
/// - **Preprocessing**: Generates cryptographic material (beaver triples, random shares) offline
/// - **Input reception**: Receives secret shares from clients via QUIC
/// - **Secure computation**: Executes the Stoffel program on secret-shared data collaboratively
/// - **Output distribution**: Sends result shares back to clients via QUIC
///
/// Multiple servers work together using the MPC protocol to compute on client inputs without
/// any individual server learning the private inputs. The protocol ensures correctness and
/// privacy even if up to `threshold` servers are faulty or malicious.
///
/// **Note**: Servers are network-based by design. The network manager is created automatically
/// during server construction and connectivity is established via `add_peer()`, `bind_and_listen()`,
/// and `connect_to_peers()`.
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
    inner: Option<HoneyBadgerMPCNode<ark_bls12_381::Fr, Avid>>,
    // Store received input shares from clients
    input_shares: Vec<stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare<ark_bls12_381::Fr>>,
    // Network manager (always present - servers are network-based)
    network: std::sync::Arc<stoffelnet::transports::quic::QuicNetworkManager>,
    // VM instance for executing bytecode on MPC shares
    //
    // NOTE: Unlike vm::VM::run_bytecode() which creates a new VM per execution,
    // MPCServer maintains a persistent VM instance because:
    // 1. MPC execution requires all servers to execute collaboratively with the same loaded bytecode
    // 2. The VM must persist between load_bytecode() and execute_function() calls
    // 3. Bytecode loading and function execution are separate operations in MPC context
    vm: Option<stoffel_vm::core_vm::VirtualMachine>,
}

impl MPCServer {
    // =========================================================================
    // Builder Methods (Internal - used by StoffelRuntime)
    // =========================================================================

    /// Create a new MPC server with network manager (internal use only)
    ///
    /// Network manager is required - servers are network-based by design.
    /// The network manager is created and provided by the StoffelRuntime.
    pub(crate) fn new(network: std::sync::Arc<stoffelnet::transports::quic::QuicNetworkManager>) -> Self {
        // Initialize VM for bytecode execution
        let mut vm = stoffel_vm::core_vm::VirtualMachine::new();
        vm.register_standard_library();

        Self {
            party_id: None,
            n_triples: 0,
            n_random_shares: 0,
            config: None,
            inner: None,
            input_shares: Vec::new(),
            network,
            vm: Some(vm),
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
            inner: self.inner,
            input_shares: Vec::new(),
            network: self.network,
            vm: self.vm,
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
    /// Initialize the HoneyBadger MPC node without running preprocessing
    ///
    /// This method must be called before spawning message processors.
    /// It sets up the MPC node with the configured parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - MPC configuration is not set
    /// - Node setup fails
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # async fn example() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    /// #     .parties(3).threshold(0).build()?;
    /// let mut server = runtime.server(0)
    ///     .with_preprocessing(5, 10)
    ///     .build()?;
    ///
    /// // Initialize node before spawning message processors
    /// server.initialize_node()?;
    ///
    /// // Now safe to spawn message processor
    /// let rx = server.bind_and_listen("127.0.0.1:19200".parse()?).await?;
    /// let _handle = server.spawn_message_processor(rx, 0);
    /// # Ok(())
    /// # }
    /// ```
    pub fn initialize_node(&mut self) -> Result<()> {
        use stoffelmpc_mpc::common::MPCProtocol;
        use stoffelmpc_mpc::honeybadger::HoneyBadgerMPCNodeOpts;

        // Only initialize if not already done
        if self.inner.is_some() {
            return Ok(());
        }

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
        Ok(())
    }

    pub async fn run_preprocessing(&mut self) -> Result<()> {
        use stoffelmpc_mpc::common::PreprocessingMPCProtocol;
        use ark_std::rand::SeedableRng;

        // Initialize node if not already done
        self.initialize_node()?;

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

    /// Execute the Stoffel program on secret-shared data (collaborative MPC)
    ///
    /// **IMPORTANT**: This method must be called simultaneously on ALL servers in the MPC network!
    ///
    /// MPC execution requires coordination between all parties:
    /// 1. All servers load the same bytecode
    /// 2. All servers execute the same function at the same time
    /// 3. When the VM encounters MPC operations (e.g., MUL on shares), all servers
    ///    participate in the protocol collaboratively via network communication
    /// 4. Each server gets its own share of the result
    ///
    /// # Collaborative Execution Pattern
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # use tokio;
    /// # async fn example() -> Result<()> {
    /// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .parties(3)
    ///     .threshold(0)
    ///     .build()?;
    ///
    /// let bytecode = runtime.program().bytecode();
    /// let mut servers = vec![
    ///     runtime.server(0).build()?,
    ///     runtime.server(1).build()?,
    ///     runtime.server(2).build()?,
    /// ];
    ///
    /// // All servers must execute simultaneously
    /// let handles: Vec<_> = servers.iter_mut().map(|server| {
    ///     let bc = bytecode.clone();
    ///     tokio::spawn(async move {
    ///         server.compute(&bc, "main").await
    ///     })
    /// }).collect();
    ///
    /// let results = futures::future::join_all(handles).await;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `bytecode` - The compiled Stoffel program bytecode to execute (.stfl format)
    /// * `function_name` - The name of the function to execute (typically "main")
    ///
    /// # Returns
    ///
    /// Returns `Ok(Value)` containing this server's share of the result:
    /// - For non-MPC programs: A clear value (e.g., `Value::I64(42)`)
    /// - For MPC programs: A secret share (e.g., `Value::Share(...)`)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The bytecode is invalid or incompatible
    /// - The specified function doesn't exist in the bytecode
    /// - The MPC node is not initialized
    /// - VM execution fails
    /// - Network communication with other servers fails during MPC operations
    ///
    /// # Note on MPC Operations
    ///
    /// The VM will automatically use MPC protocols when operating on secret-shared values.
    /// For example, when the VM executes `MUL` on two shares, it triggers the secure
    /// multiplication protocol, requiring all servers to participate via network messages.
    ///
    /// See `external/stoffel-vm/crates/stoffel-vm/src/tests/vm_mesh_integration.rs` for
    /// a complete example of collaborative VM execution.
    pub async fn compute(&mut self, bytecode: &[u8], function_name: &str) -> Result<stoffel_vm_types::core_types::Value> {
        // Ensure the MPC node is initialized
        let _node = self.inner.as_ref()
            .ok_or_else(|| Error::RuntimeError(
                "MPC node not initialized. Call run_preprocessing() first.".to_string()
            ))?;

        // Load bytecode into the VM
        self.load_bytecode(bytecode)?;

        tracing::info!("Server {} executing function '{}' from bytecode", self.party_id(), function_name);
        tracing::warn!("NOTE: For MPC operations, ALL servers must call compute() simultaneously!");

        // Execute the specified function
        // When the VM encounters MPC operations on shares, it will automatically
        // coordinate with other servers via the MPC engine and network
        let result = self.execute_function(function_name)?;

        tracing::info!("Server {} completed execution of '{}'", self.party_id(), function_name);

        Ok(result)
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

    /// Process a received message from the network
    ///
    /// This method handles incoming MPC protocol messages from other servers.
    /// It should be called in a message processing loop for each received message.
    ///
    /// # Arguments
    /// * `message` - Raw message bytes received from the network
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The MPC node is not initialized
    /// - Message processing fails
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # async fn example() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?.parties(3).build()?;
    /// # let mut server = runtime.server(0).build()?;
    /// # let rx = server.bind_and_listen("127.0.0.1:19300".parse().unwrap()).await?;
    /// // Spawn a task to process incoming messages
    /// tokio::spawn(async move {
    ///     let mut rx = rx;
    ///     while let Some(msg) = rx.recv().await {
    ///         if let Err(e) = server.process_message(msg).await {
    ///             eprintln!("Failed to process message: {:?}", e);
    ///         }
    ///     }
    /// });
    /// # Ok(())
    /// # }
    /// ```
    pub async fn process_message(&mut self, message: Vec<u8>) -> Result<()> {
        use stoffelmpc_mpc::common::MPCProtocol;

        // Ensure the MPC node is initialized
        let node = self.inner.as_mut()
            .ok_or_else(|| Error::RuntimeError(
                "MPC node not initialized. Call run_preprocessing() first.".to_string()
            ))?;

        // Process the message using the MPC protocol
        node.process(message, self.network.clone())
            .await
            .map_err(|e| Error::RuntimeError(format!("Failed to process message: {:?}", e)))?;

        Ok(())
    }

    /// Get a mutable reference to the underlying HoneyBadger MPC node (advanced use)
    ///
    /// This method exposes the underlying MPC node for advanced operations that are not
    /// yet wrapped in the SDK API. Use with caution as direct manipulation can bypass
    /// SDK invariants.
    ///
    /// # Returns
    /// * `Some(&mut HoneyBadgerMPCNode)` - Mutable reference to the node if initialized
    /// * `None` - Node has not been initialized yet (call `run_preprocessing()` first)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # async fn example() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?.parties(3).build()?;
    /// # let mut server = runtime.server(0).with_preprocessing(3, 8).build()?;
    /// # server.run_preprocessing().await?;
    /// // Access underlying node for advanced operations
    /// if let Some(node) = server.node_mut() {
    ///     // Perform advanced operations on the node
    ///     let preprocessing_len = node.preprocessing_material.lock().await.len();
    ///     println!("Preprocessing material: {:?}", preprocessing_len);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn node_mut(&mut self) -> Option<&mut HoneyBadgerMPCNode<ark_bls12_381::Fr, Avid>> {
        self.inner.as_mut()
    }

    /// Get the network manager for this server
    ///
    /// This provides access to the underlying QUIC network manager for advanced
    /// networking operations.
    ///
    /// # Returns
    /// Arc-wrapped reference to the QuicNetworkManager
    pub fn network(&self) -> std::sync::Arc<stoffelnet::transports::quic::QuicNetworkManager> {
        self.network.clone()
    }

    // =========================================================================
    // Bytecode Execution Methods
    // =========================================================================

    /// Load bytecode from bytes and register it with the VM
    ///
    /// This method parses compiled Stoffel bytecode and registers all functions
    /// with the server's VM instance for execution.
    ///
    /// # Arguments
    /// * `bytecode` - Compiled Stoffel bytecode bytes (.stfl format)
    ///
    /// # Returns
    /// * `Ok(())` - Bytecode loaded successfully
    /// * `Err(_)` - Failed to parse or register bytecode
    ///
    /// # Example
    /// ```no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .parties(3)
    ///     .threshold(0)
    ///     .build()?;
    ///
    /// let mut server = runtime.server(0).build()?;
    /// let bytecode = runtime.program().bytecode();
    /// server.load_bytecode(bytecode)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_bytecode(&mut self, bytecode: &[u8]) -> Result<()> {
        // Get mutable reference to VM
        let vm = self.vm.as_mut()
            .ok_or_else(|| Error::RuntimeError("VM not initialized".to_string()))?;

        // Use the shared bytecode loading utility from vm module
        crate::vm::load_bytecode_into_vm(vm, bytecode)?;

        tracing::info!("Server {} loaded bytecode functions", self.party_id());
        Ok(())
    }

    /// Execute a function from the loaded bytecode on MPC shares
    ///
    /// This method executes a specific function from the loaded bytecode. The function
    /// should operate on secret-shared values loaded from input shares.
    ///
    /// # Arguments
    /// * `function_name` - Name of the function to execute (typically "main")
    ///
    /// # Returns
    /// * `Ok(Value)` - Result of the computation (typically a secret-shared value)
    /// * `Err(_)` - Execution failed
    ///
    /// # Example
    /// ```no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?.build()?;
    /// # let mut server = runtime.server(0).build()?;
    /// # let bytecode = runtime.program().bytecode();
    /// # server.load_bytecode(bytecode)?;
    /// // Execute the main function
    /// let result = server.execute_function("main")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn execute_function(&mut self, function_name: &str) -> Result<stoffel_vm_types::core_types::Value> {
        let vm = self.vm.as_mut()
            .ok_or_else(|| Error::RuntimeError("VM not initialized".to_string()))?;

        vm.execute(function_name)
            .map_err(|e| Error::RuntimeError(format!("VM execution failed: {}", e)))
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

    // =========================================================================
    // Networking Methods (Server Lifecycle Management)
    // =========================================================================

    /// Add a peer server to the network topology
    ///
    /// This method registers another server in the MPC network. Call this for each
    /// peer server before calling `connect_to_peers()`.
    ///
    /// # Arguments
    /// * `peer_id` - The party ID of the peer server (0 to n-1)
    /// * `address` - The network address where the peer will listen
    ///
    /// # Example
    /// ```no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?.parties(3).build()?;
    /// let mut server = runtime.server(0).build()?;
    ///
    /// // Add peer servers
    /// server.add_peer(1, "127.0.0.1:8001".parse().unwrap());
    /// server.add_peer(2, "127.0.0.1:8002".parse().unwrap());
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_peer(&mut self, peer_id: usize, address: std::net::SocketAddr) {
        let mut network = std::sync::Arc::get_mut(&mut self.network)
            .expect("Network should be exclusively owned during setup");

        network.add_node_with_party_id(peer_id, address);
    }

    /// Start the QUIC listener and begin accepting connections
    ///
    /// This method binds to the specified address and starts accepting incoming
    /// connections from peers and clients. It spawns background tasks to handle
    /// the accept loop and message routing.
    ///
    /// Returns a channel receiver for incoming MPC protocol messages.
    ///
    /// # Arguments
    /// * `bind_address` - The local address to bind to
    ///
    /// # Returns
    /// A `Receiver<Vec<u8>>` for receiving MPC protocol messages from the network
    ///
    /// # Example
    /// ```no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # async fn example() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?.parties(3).build()?;
    /// let mut server = runtime.server(0).build()?;
    ///
    /// // Start listening for connections
    /// let mut rx = server.bind_and_listen("127.0.0.1:8000".parse().unwrap()).await?;
    ///
    /// // Process incoming messages
    /// while let Some(msg) = rx.recv().await {
    ///     server.process_message(msg).await?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn bind_and_listen(
        &mut self,
        bind_address: std::net::SocketAddr,
    ) -> Result<tokio::sync::mpsc::Receiver<Vec<u8>>> {
        use stoffelnet::transports::quic::NetworkManager;

        // Create channel for routing messages
        let (tx, rx) = tokio::sync::mpsc::channel::<Vec<u8>>(1000);

        // Clone network manager for the accept task
        let network_clone = self.network.clone();

        // Clone for calling listen (needs &mut)
        let mut network_for_listen = self.network.as_ref().clone();

        // Start listening
        network_for_listen
            .listen(bind_address)
            .await
            .map_err(|e| Error::RuntimeError(format!("Failed to bind to {}: {}", bind_address, e)))?;

        // Spawn accept loop
        tokio::spawn(async move {
            let mut acceptor = (*network_clone).clone();
            loop {
                match acceptor.accept().await {
                    Ok(connection) => {
                        let tx_clone = tx.clone();

                        // Spawn task to handle this connection
                        tokio::spawn(async move {
                            loop {
                                match connection.receive().await {
                                    Ok(data) => {
                                        // Filter QUIC handshake messages
                                        if data.starts_with(b"ROLE:") {
                                            continue;
                                        }

                                        // Check for magic byte (0x56535453 "SERV" in little-endian)
                                        if data.len() >= 4 {
                                            let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                                            if magic == 0x56535453 {
                                                continue;
                                            }
                                        }

                                        // Send to message channel
                                        if tx_clone.send(data).await.is_err() {
                                            break; // Channel closed
                                        }
                                    }
                                    Err(_) => break, // Connection closed
                                }
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Accept error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Spawn a background task to process incoming MPC protocol messages
    ///
    /// This method spawns an async task that continuously processes messages from the
    /// given receiver channel. Each message is passed to the underlying HoneyBadgerMPCNode
    /// for processing. This is required for collaborative MPC protocols like preprocessing
    /// and secure computation to function correctly.
    ///
    /// **Important**: You must call `initialize_node()` BEFORE calling this method.
    /// The node must be initialized so the message processor has access to it.
    ///
    /// # Arguments
    /// * `receiver` - Message receiver channel from `bind_and_listen()`
    /// * `party_id` - This server's party ID (for logging)
    ///
    /// # Returns
    /// A `JoinHandle` for the spawned task. You can use this to monitor or cancel the task.
    ///
    /// # Panics
    /// Panics if the MPC node has not been initialized via `initialize_node()`.
    ///
    /// # Example
    /// ```no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # async fn example() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?.parties(3).build()?;
    /// let mut server = runtime.server(0).build()?;
    ///
    /// // IMPORTANT: Initialize node first
    /// server.initialize_node()?;
    ///
    /// // Bind and listen
    /// let rx = server.bind_and_listen("127.0.0.1:8000".parse().unwrap()).await?;
    ///
    /// // Spawn message processor (required for MPC protocols to work)
    /// let processor_handle = server.spawn_message_processor(rx, 0);
    ///
    /// // Now you can run MPC protocols
    /// server.run_preprocessing().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn spawn_message_processor(
        &mut self,
        mut receiver: tokio::sync::mpsc::Receiver<Vec<u8>>,
        party_id: usize,
    ) -> tokio::task::JoinHandle<()> {
        use stoffelmpc_mpc::common::MPCProtocol;

        // The node MUST be initialized before spawning the processor
        // This ensures the cloned node shares state via Arc<Mutex<>>
        let mut node = self.inner.clone()
            .expect("MPC node must be initialized before spawning message processor. Call initialize_node() first.");
        let network = self.network.clone();

        tokio::spawn(async move {
            while let Some(raw_msg) = receiver.recv().await {
                // Process message directly on the cloned node
                // HoneyBadgerMPCNode uses Arc<Mutex<>> internally, so cloning shares state
                if let Err(e) = node.process(raw_msg, network.clone()).await {
                    eprintln!("Server {} failed to process message: {:?}", party_id, e);
                }
            }
        })
    }

    /// Connect to all registered peer servers
    ///
    /// This method establishes QUIC connections to all peers that were registered
    /// via `add_peer()`. It uses exponential backoff for retries.
    ///
    /// Returns a vector of message receivers, one for each peer connection.
    ///
    /// # Example
    /// ```no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # async fn example() -> Result<()> {
    /// # let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?.parties(3).build()?;
    /// let mut server = runtime.server(0).build()?;
    ///
    /// server.add_peer(1, "127.0.0.1:8001".parse().unwrap());
    /// server.add_peer(2, "127.0.0.1:8002".parse().unwrap());
    ///
    /// // Connect to all peers
    /// let peer_channels = server.connect_to_peers().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect_to_peers(&self) -> Result<Vec<tokio::sync::mpsc::Receiver<Vec<u8>>>> {
        use stoffelnet::transports::quic::NetworkManager;
        use stoffelnet::network_utils::Network;

        let mut receivers = Vec::new();
        let party_id = self.party_id();

        // Get list of peers to connect to
        let peers: Vec<_> = self.network.parties()
            .iter()
            .filter_map(|p| {
                // Convert UUID back to party_id
                let peer_party_id = p.uuid().as_u128() as usize;
                if peer_party_id != party_id {
                    Some((peer_party_id, p.address()))
                } else {
                    None
                }
            })
            .collect();

        // Clone the network manager once for connecting
        let mut dialer = self.network.as_ref().clone();

        for (peer_id, address) in peers {
            let (tx, rx) = tokio::sync::mpsc::channel::<Vec<u8>>(1000);
            let mut dialer_for_peer = dialer.clone();

            // Spawn connection task with retry logic
            tokio::spawn(async move {
                const MAX_RETRIES: usize = 5;
                const INITIAL_BACKOFF_MS: u64 = 100;

                for attempt in 0..MAX_RETRIES {
                    match dialer_for_peer.connect_as_server(address, peer_id).await {
                        Ok(connection) => {
                            // Connection successful, start receiving messages
                            loop {
                                match connection.receive().await {
                                    Ok(data) => {
                                        // Filter handshake messages
                                        if data.starts_with(b"ROLE:") {
                                            continue;
                                        }

                                        if data.len() >= 4 {
                                            let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                                            if magic == 0x56535453 {
                                                continue;
                                            }
                                        }

                                        if tx.send(data).await.is_err() {
                                            break;
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                            break;
                        }
                        Err(e) => {
                            if attempt < MAX_RETRIES - 1 {
                                let backoff = INITIAL_BACKOFF_MS * 2u64.pow(attempt as u32);
                                tokio::time::sleep(tokio::time::Duration::from_millis(backoff)).await;
                            } else {
                                eprintln!("Failed to connect to peer {} after {} attempts: {}", peer_id, MAX_RETRIES, e);
                            }
                        }
                    }
                }
            });

            receivers.push(rx);
        }

        Ok(receivers)
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

    /// Build the MPC server with network manager
    ///
    /// Creates a network-based MPC server. The network manager is created automatically
    /// and initialized with the server's party ID.
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

        // Create network manager for this party - servers are always network-based
        let network = std::sync::Arc::new(
            stoffelnet::transports::quic::QuicNetworkManager::with_node_id(self.party_id)
        );

        let server = MPCServer::new(network)
            .with_party_id(self.party_id)
            .with_config(config)
            .with_preprocessing(n_triples, n_random_shares)
            .build()?;

        Ok(server)
    }
}
