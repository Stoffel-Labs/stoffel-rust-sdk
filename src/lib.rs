//! # Stoffel Rust SDK
//!
//! A production-ready Rust SDK for the Stoffel ecosystem, providing:
//! - **Stoffel-Lang**: Compile Stoffel programs to bytecode
//! - **StoffelVM**: Execute bytecode in the Stoffel virtual machine
//! - **MPC Infrastructure**: Complete multi-party computation with automatic networking
//! - **Network Helpers**: Production-ready QUIC networking infrastructure
//!
//! ## Architecture
//!
//! The SDK is organized around three main concepts:
//!
//! - **[`Stoffel`]**: The main gateway for all SDK functionality
//! - **[`StoffelRuntime`]**: A compiled program with MPC infrastructure configuration
//! - **[`Program`]**: Pure bytecode that can be executed locally or in an MPC network
//! - **`network_helpers`**: Production networking infrastructure (QUIC listeners, connections, handlers)
//!
//! ## MPC Protocol and Secret Sharing Configuration
//!
//! The SDK uses sensible defaults that work together:
//! - **Protocol**: HoneyBadger (Byzantine fault-tolerant, asynchronous)
//! - **Secret Sharing**: RobustShare (error correction, required for HoneyBadger)
//!
//! These defaults provide robust security suitable for most use cases. Developers don't
//! need to worry about configuration details - the SDK handles it automatically.
//!
//! For advanced users, both the protocol and share type can be explicitly configured:
//!
//! ```rust,no_run
//! # use stoffel_rust_sdk::prelude::*;
//! # fn main() -> Result<()> {
//! // Default: HoneyBadger with RobustShare (automatic)
//! let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .parties(5)
//!     .build()?;
//!
//! // Advanced: Explicit configuration for semi-honest settings
//! let runtime2 = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .parties(5)
//!     .protocol(ProtocolType::HoneyBadger)  // Explicit protocol
//!     .share_type(ShareType::NonRobust)     // Faster, but requires honest parties
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! Future versions will support additional protocols for specialized use cases.
//!
//! ## Quick Start
//!
//! ### Local Execution
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! # fn main() -> Result<()> {
//! // Compile and execute locally (no MPC)
//! let result = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .execute_local()?;
//!
//! println!("Result: {:?}", result);
//! # Ok(())
//! # }
//! ```
//!
//! ### Production MPC with Automatic Networking
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//! use ark_bls12_381::Fr;
//!
//! # async fn example() -> std::result::Result<(), Box<dyn std::error::Error>> {
//! // Step 1: Compile Stoffel program
//! let runtime = Stoffel::compile("main main(a: secret int64, b: secret int64) -> secret int64:\n  return a * b")?
//!     .parties(5)        // 5-party MPC network
//!     .threshold(1)      // Tolerates 1 Byzantine fault
//!     .instance_id(42)   // Computation ID
//!     .build()?;
//!
//! // Step 2: Setup complete network infrastructure (ONE FUNCTION CALL!)
//! let (servers, receivers) = setup_honeybadger_quic_network::<Fr>(
//!     5,      // n_parties
//!     1,      // threshold
//!     3,      // n_triples for preprocessing
//!     8,      // n_random_shares
//!     42,     // instance_id
//!     19200,  // base_port
//!     HoneyBadgerQuicConfig::default(),
//! ).await?;
//!
//! // Step 3: Start and connect
//! for server in &mut servers {
//!     server.start().await?;
//! }
//! for server in &servers {
//!     server.connect_to_peers().await?;
//! }
//!
//! // Step 4: Run MPC protocol!
//! // (preprocessing, input sharing, computation, output reconstruction)
//! // See examples/quick_start_local_network_real.rs for complete implementation
//! # Ok(())
//! # }
//! ```
//!
//! ### Simple API Usage (without network execution)
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! # fn main() -> Result<()> {
//! // Compile with MPC configuration
//! let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .parties(5)
//!     .threshold(1)
//!     .build()?;
//!
//! // Create MPC participants (automatic network manager creation)
//! let client = runtime.client(100).with_inputs(vec![10, 20]).build()?;
//! let server = runtime.server(0).with_preprocessing(10, 25).build()?;
//! let node = runtime.node(0).with_inputs(vec![10, 20]).build()?;
//!
//! // For actual execution, use network_helpers (see above)
//! # Ok(())
//! # }
//! ```
//!
//! ## Examples
//!
//! ### Compiling Source Code
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! # fn main() -> Result<()> {
//! // Simple compilation
//! let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .parties(5)
//!     .threshold(1)
//!     .build()?;
//!
//! // With optimization
//! let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .optimize(true)
//!     .parties(5)
//!     .threshold(1)
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Running Programs Locally
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! # fn main() -> Result<()> {
//! let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .parties(5)
//!     .build()?;
//!
//! // Execute on local VM for testing
//! let result = runtime.program().execute_local()?;
//! println!("Result: {:?}", result);
//! # Ok(())
//! # }
//! ```
//!
//! ### Setting Up MPC Infrastructure
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! # fn main() -> Result<()> {
//! // Configure MPC runtime (HoneyBadger protocol is used automatically)
//! let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .parties(5)         // Number of MPC nodes
//!     .threshold(1)       // Byzantine fault tolerance: can handle 1 malicious party
//!     .instance_id(1234)
//!     .build()?;
//!
//! // All participants use the same protocol (HoneyBadger)
//! println!("Using protocol: {:?}", runtime.protocol_type());
//!
//! // Create clients (provide inputs)
//! let client1 = runtime.client(100).with_inputs(vec![10, 20]).build()?;
//! let client2 = runtime.client(101).with_inputs(vec![30, 40]).build()?;
//!
//! // Create servers (perform secure computation using HoneyBadger)
//! let server1 = runtime.server(0).with_preprocessing(10, 25).build()?;
//! let server2 = runtime.server(1).with_preprocessing(10, 25).build()?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Advanced: Explicit Protocol Selection
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! # fn main() -> Result<()> {
//! // Advanced users can explicitly select the protocol
//! let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .parties(5)
//!     .threshold(1)
//!     .protocol(ProtocolType::HoneyBadger)  // Explicit selection
//!     .build()?;
//!
//! // This is useful when multiple protocols become available in the future
//! # Ok(())
//! # }
//! ```

pub mod compiler;
pub mod error;
pub mod client;
pub mod server;
pub mod session;
pub mod vm;
pub mod program;
pub mod network_config;
pub mod secret_sharing;
pub mod backend;

/// Advanced APIs for power users (low-level access)
///
/// This module exposes raw VM types, MPC protocol internals, and
/// network management for custom implementations.
///
/// Most users should use the high-level `Stoffel` builder in the root module instead.
pub mod advanced;

/// Network infrastructure helpers for production MPC deployments
///
/// Re-exports HoneyBadger QUIC network setup utilities from stoffel-vm.
/// These provide automatic setup of QUIC listeners, connections, and message handlers.
pub mod network_helpers;

/// Convenient re-exports for common usage
pub mod prelude;

pub mod mpc_local;

pub mod stoffel_mpc;

pub use error::{Error, Result};

// Re-export key types from mpc-protocols for advanced users
// These types are used internally by the SDK and exposed for advanced usage
pub mod mpc_types {
    //! Re-exported types from the mpc-protocols crate
    //!
    //! These are the underlying MPC types used by the Stoffel SDK.
    //! Most users don't need to interact with these directly.

    // HoneyBadger protocol types
    pub use stoffelmpc_mpc::honeybadger::{
        HoneyBadgerMPCClient,
        HoneyBadgerMPCNode,
        HoneyBadgerError,
        ProtocolType as MPCSubProtocol,  // Renamed to avoid confusion with SDK's ProtocolType
        SessionId,
    };

    // Secret sharing types
    pub use stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::{
        Robust,
        RobustShare,
    };
    pub use stoffelmpc_mpc::common::share::shamir::{
        NonRobust,
        NonRobustShare,
    };

    // Common MPC traits and types
    pub use stoffelmpc_mpc::common::{
        MPCProtocol,
        PreprocessingMPCProtocol,
    };

    // Network types
    pub use stoffelnet::network_utils::{
        Network,
        NetworkError,
        PartyId,
        ClientId,
    };
}

/// High-level Stoffel SDK - the main entry point for the Stoffel ecosystem
///
/// `Stoffel` is the gateway for all SDK functionality. It provides methods for:
/// - Compiling Stoffel source code to bytecode
/// - Configuring MPC infrastructure (parties, threshold, protocol, network)
/// - Building [`StoffelRuntime`] instances for MPC execution
///
/// The SDK uses the **HoneyBadger MPC protocol** by default, which provides Byzantine
/// fault tolerance without requiring any configuration. Advanced users can explicitly
/// select protocols when needed.
///
/// The generic parameter `F` specifies the cryptographic field used for MPC computations.
/// By default, it uses BLS12-381 (`ark_bls12_381::Fr`).
///
/// # Architecture
///
/// ```text
/// Stoffel::compile()
///     ↓
/// StoffelBuilder (configure MPC params + protocol)
///     ↓
/// StoffelRuntime (holds Program + MPC config + Protocol)
///     ↓
/// MPCClient / MPCServer / MPCNode (participants using configured protocol)
/// ```
///
/// # Examples
///
/// ## Quick Local Execution
///
/// ```rust,no_run
/// use stoffel_rust_sdk::Stoffel;
///
/// # fn main() -> stoffel_rust_sdk::Result<()> {
/// // Compile and execute locally without MPC setup
/// let result = Stoffel::compile("main main() -> int64:\n  return 42")?
///     .execute_local()?;
/// println!("Result: {:?}", result);
/// # Ok(())
/// # }
/// ```
///
/// ## MPC Infrastructure Setup
///
/// ```rust,no_run
/// use stoffel_rust_sdk::Stoffel;
///
/// # fn main() -> stoffel_rust_sdk::Result<()> {
/// // Compile with MPC configuration (HoneyBadger protocol is automatic)
/// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
///     .parties(5)       // 5-party network
///     .threshold(1)     // Byzantine fault tolerance: tolerates 1 malicious party
///     .instance_id(42)  // Computation ID
///     .build()?;
///
/// // Check which protocol is being used
/// println!("Protocol: {:?}", runtime.protocol_type());  // HoneyBadger
///
/// // Test locally before deploying
/// let result = runtime.program().execute_local()?;
///
/// // Create MPC participants (inherit HoneyBadger protocol from runtime)
/// let client = runtime.client(100).with_inputs(vec![42]).build()?;
/// let server = runtime.server(0).build()?;
/// # Ok(())
/// # }
/// ```
///
/// # Builder Pattern
///
/// Stoffel uses the builder pattern. Create instances with:
/// - `Stoffel::compile(source)` - Compile from source code
/// - `Stoffel::compile_file(path)` - Compile from a file
/// - `Stoffel::load(bytecode)` - Load from bytecode
/// - `Stoffel::new()` - Create empty builder
pub struct Stoffel {
    source: Option<String>,
    file_path: Option<String>,
    bytecode: Option<Vec<u8>>,
    optimize: bool,
    n_parties: Option<usize>,
    threshold: Option<usize>,
    instance_id: u64,
    network_config: Option<network_config::NetworkConfig>,
    protocol_type: ProtocolType,  // Default MPC protocol (HoneyBadger with BLS12-381)
    share_type: ShareType,  // Secret sharing scheme configuration
}

impl Stoffel {
    /// Compile Stoffel source code
    ///
    /// Returns a Stoffel builder to configure MPC parameters.
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
    /// # Ok(())
    /// # }
    /// ```
    pub fn compile(source: &str) -> Result<Self> {
        let compiler = compiler::Compiler::new();
        let bytecode = compiler.compile_source(source)?;
        Ok(Self::from_bytecode(bytecode))
    }

    /// Compile a Stoffel program from a file
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::Stoffel;
    /// # fn main() -> stoffel_rust_sdk::Result<()> {
    /// let runtime = Stoffel::compile_file("program.stfl")?
    ///     .parties(5)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn compile_file(path: &str) -> Result<Self> {
        let compiler = compiler::Compiler::new();
        let bytecode = compiler.compile_file(path)?;
        Ok(Self::from_bytecode(bytecode))
    }

    /// Load from bytecode
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::Stoffel;
    /// # fn main() -> stoffel_rust_sdk::Result<()> {
    /// let bytecode = std::fs::read("program.stfb")?;
    /// let runtime = Stoffel::load(bytecode)
    ///     .parties(5)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load(bytecode: Vec<u8>) -> Self {
        Self::from_bytecode(bytecode)
    }

    /// Create a new Stoffel builder
    pub fn new() -> Self {
        Self {
            source: None,
            file_path: None,
            bytecode: None,
            optimize: false,
            n_parties: None,
            threshold: None,
            instance_id: 0,
            network_config: None,
            protocol_type: ProtocolType::HoneyBadger,  // Default protocol
            share_type: ShareType::Robust,  // Default share type
        }
    }

    /// Create a builder from bytecode
    pub(crate) fn from_bytecode(bytecode: Vec<u8>) -> Self {
        Self {
            source: None,
            file_path: None,
            bytecode: Some(bytecode),
            optimize: false,
            n_parties: None,
            threshold: None,
            instance_id: 0,
            network_config: None,
            protocol_type: ProtocolType::HoneyBadger,  // Default protocol
            share_type: ShareType::Robust,  // Default share type
        }
    }

    /// Set the source code to compile
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set the file path to compile
    pub fn file(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    /// Enable optimization
    pub fn optimize(mut self, optimize: bool) -> Self {
        self.optimize = optimize;
        self
    }

    /// Set the number of MPC parties
    ///
    /// This is the total number of servers in the MPC network.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::Stoffel;
    /// # fn main() -> stoffel_rust_sdk::Result<()> {
    /// let program = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .parties(5)
    ///     .threshold(1)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn parties(mut self, n: usize) -> Self {
        self.n_parties = Some(n);
        self
    }

    /// Set the fault tolerance threshold
    ///
    /// The protocol can tolerate up to `t` faulty parties where `n >= 3t + 1`.
    /// If not set, defaults to 1.
    pub fn threshold(mut self, t: usize) -> Self {
        self.threshold = Some(t);
        self
    }

    /// Set the instance ID for this computation
    ///
    /// Defaults to 0 if not set.
    pub fn instance_id(mut self, id: u64) -> Self {
        self.instance_id = id;
        self
    }

    /// Set the MPC protocol to use
    ///
    /// Currently only HoneyBadger is supported. This method is provided for
    /// future extensibility when additional protocols are added.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::{Stoffel, ProtocolType};
    /// # fn main() -> stoffel_rust_sdk::Result<()> {
    /// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .parties(5)
    ///     .protocol(ProtocolType::HoneyBadger)  // Explicit protocol selection
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn protocol(mut self, protocol: ProtocolType) -> Self {
        self.protocol_type = protocol;
        self
    }

    /// Set the secret sharing scheme type
    ///
    /// This configures which secret sharing implementation to use for MPC operations.
    /// By default, `ShareType::Robust` is used, which provides error correction.
    ///
    /// # Arguments
    ///
    /// * `share_type` - The type of secret sharing to use
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::{Stoffel, ShareType};
    /// # fn main() -> stoffel_rust_sdk::Result<()> {
    /// // Use RobustShare (default - provides error correction)
    /// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .parties(5)
    ///     .share_type(ShareType::Robust)
    ///     .build()?;
    ///
    /// // Use NonRobustShare (simpler, faster)
    /// let runtime2 = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .parties(5)
    ///     .share_type(ShareType::NonRobust)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn share_type(mut self, share_type: ShareType) -> Self {
        self.share_type = share_type;
        self
    }

    /// Load network configuration from a TOML file
    ///
    /// This will set MPC parameters (parties, threshold) from the config file.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::Stoffel;
    /// # fn main() -> stoffel_rust_sdk::Result<()> {
    /// let program = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .network_config_file("stoffel.toml")?
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn network_config_file(mut self, path: &str) -> Result<Self> {
        let config = network_config::NetworkConfig::from_file(path)?;

        // Extract MPC parameters from config if not already set
        if self.n_parties.is_none() {
            self.n_parties = Some(config.mpc.n_parties);
        }
        if self.threshold.is_none() {
            self.threshold = Some(config.mpc.threshold);
        }
        if let Some(id) = config.mpc.instance_id {
            self.instance_id = id;
        }

        self.network_config = Some(config);
        Ok(self)
    }

    /// Set network configuration manually
    ///
    /// This will override any previously set MPC parameters.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::Stoffel;
    /// # use stoffel_rust_sdk::network_config::{NetworkConfig, NetworkSettings, MPCSettings};
    /// # fn main() -> stoffel_rust_sdk::Result<()> {
    /// let config = NetworkConfig::new(
    ///     NetworkSettings {
    ///         party_id: 0,
    ///         bind_address: "127.0.0.1:9001".to_string(),
    ///         bootstrap_address: "127.0.0.1:9000".to_string(),
    ///         min_parties: 3,
    ///     },
    ///     MPCSettings {
    ///         n_parties: 5,
    ///         threshold: 1,
    ///         instance_id: None,
    ///     },
    /// );
    ///
    /// let program = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .network_config(config)?
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn network_config(mut self, config: network_config::NetworkConfig) -> Result<Self> {
        config.validate()?;

        // Extract MPC parameters from config
        self.n_parties = Some(config.mpc.n_parties);
        self.threshold = Some(config.mpc.threshold);
        if let Some(id) = config.mpc.instance_id {
            self.instance_id = id;
        }

        self.network_config = Some(config);
        Ok(self)
    }

    /// Compile the source/file/bytecode and build a StoffelRuntime with MPC configuration
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
    /// // Create MPC participants from the runtime
    /// let client = runtime.client(100).with_inputs(vec![42]).build()?;
    /// let server = runtime.server(0).build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn build(self) -> Result<StoffelRuntime> {
        // Get or compile bytecode
        let bytecode = if let Some(bc) = self.bytecode {
            bc
        } else {
            let mut compiler = compiler::Compiler::new();
            if self.optimize {
                compiler = compiler.optimize(true);
            }

            if let Some(source) = self.source {
                compiler.compile_source(&source)?
            } else if let Some(path) = self.file_path {
                compiler.compile_file(&path)?
            } else {
                return Err(Error::InvalidInput("No source, file, or bytecode provided".to_string()));
            }
        };

        // Validate network config if set
        if let Some(ref config) = self.network_config {
            config.validate()?;
        }

        // If parties is set, validate the MPC configuration
        if let Some(n_parties) = self.n_parties {
            let threshold = self.threshold.unwrap_or(1);

            // Validate parameters
            if n_parties < 3 * threshold + 1 {
                return Err(Error::InvalidInput(format!(
                    "Invalid parameters: n={} must be >= 3t+1={} for t={}",
                    n_parties,
                    3 * threshold + 1,
                    threshold
                )));
            }

            Ok(StoffelRuntime {
                program: program::Program::new(bytecode),
                n_parties: Some(n_parties),
                threshold: Some(threshold),
                instance_id: self.instance_id,
                network_config: self.network_config,
                protocol_type: self.protocol_type,  // Use configured protocol
                share_type: self.share_type,  // Use configured share type
            })
        } else {
            // No MPC configuration
            Ok(StoffelRuntime {
                program: program::Program::new(bytecode),
                n_parties: None,
                threshold: None,
                instance_id: self.instance_id,
                network_config: self.network_config,
                protocol_type: self.protocol_type,  // Use configured protocol
                share_type: self.share_type,  // Use configured share type
            })
        }
    }

    /// Compile and execute locally without building a Program (convenience method for testing)
    ///
    /// This skips MPC configuration and runs the program directly on the local VM.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::Stoffel;
    /// # fn main() -> stoffel_rust_sdk::Result<()> {
    /// // Quick local test - no need for MPC config
    /// let result = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .execute_local()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn execute_local(self) -> Result<vm::Value> {
        // Get or compile bytecode
        let bytecode = if let Some(bc) = self.bytecode {
            bc
        } else {
            let mut compiler = compiler::Compiler::new();
            if self.optimize {
                compiler = compiler.optimize(true);
            }

            if let Some(source) = self.source {
                compiler.compile_source(&source)?
            } else if let Some(path) = self.file_path {
                compiler.compile_file(&path)?
            } else {
                return Err(Error::InvalidInput("No source, file, or bytecode provided".to_string()));
            }
        };

        let vm = vm::VM::new();
        vm.run_bytecode(&bytecode, "main")
    }
}

impl Default for Stoffel {
    fn default() -> Self {
        Self::new()
    }
}

/// Stoffel Runtime - manages a compiled program with MPC configuration
///
/// This is what you get after calling `.build()` on a Stoffel builder.
/// It contains the compiled program along with MPC infrastructure configuration,
/// and provides methods to create MPC participants (clients and nodes).
///
/// By default, the runtime uses the HoneyBadger MPC protocol. Advanced users can
/// specify different protocols in the future.
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
/// // Access the program for local execution
/// let result = runtime.program().execute_local()?;
///
/// // Create MPC participants (automatically configured with HoneyBadger protocol)
/// let client = runtime.client(100).with_inputs(vec![42]).build()?;
/// let server = runtime.server(0).build()?;
/// # Ok(())
/// # }
/// ```
pub struct StoffelRuntime {
    program: program::Program,
    n_parties: Option<usize>,
    threshold: Option<usize>,
    instance_id: u64,
    network_config: Option<network_config::NetworkConfig>,
    /// The configured MPC protocol type
    /// Currently defaults to HoneyBadger (using BLS12-381 field and AVID RBC)
    protocol_type: ProtocolType,
    /// The configured secret sharing scheme type
    share_type: ShareType,
}

/// MPC Protocol Type - specifies which MPC protocol to use
///
/// The SDK automatically uses HoneyBadger by default. Developers don't need to
/// worry about protocol selection for typical use cases.
///
/// # HoneyBadger Protocol
///
/// The default protocol provides:
/// - **Byzantine Fault Tolerance**: Handles up to `t` malicious parties (where `n >= 3t+1`)
/// - **Asynchronous Communication**: No timing assumptions or synchronization required
/// - **Robust Secret Sharing**: Built-in error detection and correction
/// - **Optimal Resilience**: Maximum fault tolerance for the given number of parties
///
/// # Future Protocols
///
/// Future versions will support additional protocols for specialized scenarios:
/// - Semi-honest protocols (better performance when all parties are trusted)
/// - Threshold signatures (for specific cryptographic operations)
/// - Dishonest majority protocols (when most parties may be malicious)
///
/// # Example
///
/// ```rust,no_run
/// use stoffel_rust_sdk::prelude::*;
///
/// # fn main() -> Result<()> {
/// // Default: HoneyBadger is used automatically
/// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
///     .parties(5)
///     .build()?;
///
/// assert_eq!(runtime.protocol_type(), ProtocolType::HoneyBadger);
///
/// // Advanced: Explicit protocol selection (for future protocols)
/// let runtime2 = Stoffel::compile("main main() -> int64:\n  return 42")?
///     .parties(5)
///     .protocol(ProtocolType::HoneyBadger)
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolType {
    /// HoneyBadger MPC protocol with robust secret sharing
    ///
    /// This is the default protocol. It provides Byzantine fault tolerance,
    /// asynchronous communication, and optimal resilience. Suitable for most
    /// production use cases where parties may be malicious or unreliable.
    HoneyBadger,
}

/// Secret sharing scheme type
///
/// This enum specifies which secret sharing implementation to use for MPC operations.
/// Different share types provide different security guarantees and performance characteristics.
///
/// The ShareType is a thin wrapper around the actual share implementations from the
/// `mpc-protocols` crate, providing a user-friendly SDK interface.
///
/// # Default: RobustShare
///
/// By default, `ShareType::Robust` is used, which matches the default `ProtocolType::HoneyBadger`.
/// HoneyBadger requires robust shares for its Byzantine fault tolerance properties.
/// Developers don't need to specify the share type unless they want to change it.
///
/// # Changing the Share Type
///
/// Advanced developers can explicitly set the share type based on their security model:
/// - Use `ShareType::NonRobust` for semi-honest settings where all parties are trusted
/// - Keep `ShareType::Robust` (default) for Byzantine fault-tolerant scenarios
///
/// # Implementation Details
///
/// - `ShareType::Robust` maps to `stoffelmpc_mpc::honeybadger::robust_interpolate::RobustShare`
/// - `ShareType::NonRobust` maps to `stoffelmpc_mpc::common::share::NonRobustShare`
///
/// # Examples
///
/// ```rust,no_run
/// # use stoffel_rust_sdk::prelude::*;
/// # fn main() -> Result<()> {
/// // Default: RobustShare is used automatically (matches HoneyBadger)
/// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
///     .parties(5)
///     .build()?;
///
/// assert_eq!(runtime.protocol_type(), ProtocolType::HoneyBadger);
/// assert_eq!(runtime.share_type(), ShareType::Robust);
///
/// // Advanced: Explicit share type for semi-honest settings
/// let runtime2 = Stoffel::compile("main main() -> int64:\n  return 42")?
///     .parties(5)
///     .share_type(ShareType::NonRobust)  // Faster, but requires honest parties
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareType {
    /// RobustShare - Shamir secret sharing with error correction
    ///
    /// This is the default share type. It uses Reed-Solomon erasure coding
    /// to provide robust reconstruction even when some shares are corrupted.
    /// Required for HoneyBadger protocol's Byzantine fault tolerance.
    ///
    /// Maps to: `stoffelmpc_mpc::honeybadger::robust_interpolate::RobustShare<F>`
    ///
    /// Properties:
    /// - Error correction capability
    /// - Byzantine fault tolerant
    /// - Slightly higher computational cost
    Robust,

    /// NonRobustShare - Standard Shamir secret sharing
    ///
    /// Simple Shamir secret sharing without error correction. Faster but
    /// requires all shares to be correct. Suitable for semi-honest settings
    /// or when error correction is not needed.
    ///
    /// Maps to: `stoffelmpc_mpc::common::share::NonRobustShare<F>`
    ///
    /// Properties:
    /// - No error correction
    /// - Faster computation
    /// - Requires honest parties
    NonRobust,
}

impl Default for ShareType {
    fn default() -> Self {
        ShareType::Robust
    }
}

impl StoffelRuntime {
    /// Get a reference to the underlying program
    pub fn program(&self) -> &program::Program {
        &self.program
    }

    /// Get the MPC configuration (n_parties, threshold, instance_id)
    pub fn mpc_config(&self) -> Option<(usize, usize, u64)> {
        if let (Some(n), Some(t)) = (self.n_parties, self.threshold) {
            Some((n, t, self.instance_id))
        } else {
            None
        }
    }

    /// Get the network configuration if set
    pub fn network_config(&self) -> Option<&network_config::NetworkConfig> {
        self.network_config.as_ref()
    }

    /// Get the configured MPC protocol type
    ///
    /// Returns the protocol type that will be used for MPC operations.
    /// Currently always returns `ProtocolType::HoneyBadger`.
    pub fn protocol_type(&self) -> ProtocolType {
        self.protocol_type
    }

    /// Get the configured secret sharing scheme type
    ///
    /// Returns the share type that will be used for secret sharing operations.
    /// By default, returns `ShareType::Robust`.
    pub fn share_type(&self) -> ShareType {
        self.share_type
    }

    /// Create an MPC client builder
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
    /// let client = runtime.client(100)
    ///     .with_inputs(vec![42, 10])
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn client(&self, client_id: usize) -> client::MPCClientBuilder {
        let (n_parties, threshold, instance_id) = self.mpc_config()
            .expect("Cannot create MPC client without MPC configuration. Use .parties(n).threshold(t) when building.");

        client::MPCClientBuilder::new(
            client_id,
            n_parties,
            threshold,
            instance_id,
            self.protocol_type,
            self.share_type,
        )
    }

    /// Create an MPC server builder
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
    /// let server = runtime.server(0)
    ///     .with_preprocessing(10, 25)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn server(&self, party_id: usize) -> server::MPCServerBuilder {
        let (n_parties, threshold, instance_id) = self.mpc_config()
            .expect("Cannot create MPC server without MPC configuration. Use .parties(n).threshold(t) when building.");

        server::MPCServerBuilder::new(
            party_id,
            n_parties,
            threshold,
            instance_id,
            self.protocol_type,
        )
    }

    /// Create an MPC node builder
    ///
    /// Nodes are for peer-to-peer scenarios where all parties both provide inputs
    /// and participate in computation.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::Stoffel;
    /// # fn main() -> stoffel_rust_sdk::Result<()> {
    /// let runtime = Stoffel::compile("main main(a: secret int64, b: secret int64) -> secret int64:\n  return a * b")?
    ///     .parties(5)
    ///     .threshold(1)
    ///     .build()?;
    ///
    /// let node = runtime.node(0)
    ///     .with_inputs(vec![10, 20])
    ///     .with_preprocessing(3, 8)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn node(&self, party_id: usize) -> session::MPCNodeBuilder {
        let (n_parties, threshold, instance_id) = self.mpc_config()
            .expect("Cannot create MPC node without MPC configuration. Use .parties(n).threshold(t) when building.");

        session::MPCNodeBuilder::new(
            party_id,
            n_parties,
            threshold,
            instance_id,
            self.protocol_type,
        )
    }
}
