//! # Stoffel Rust SDK
//!
//! A friendly, high-level Rust SDK for the Stoffel ecosystem, providing easy access to:
//! - **Stoffel-Lang**: Compile Stoffel programs to bytecode
//! - **StoffelVM**: Execute bytecode in the Stoffel virtual machine
//! - **MPC Infrastructure**: Multi-party computation runtime and participants
//!
//! ## Architecture
//!
//! The SDK is organized around three main concepts:
//!
//! - **[`Stoffel`]**: The main gateway for all SDK functionality
//! - **[`StoffelRuntime`]**: A compiled program with MPC infrastructure configuration
//! - **[`Program`]**: Pure bytecode that can be executed locally or in an MPC network
//!
//! ## MPC Protocol Configuration
//!
//! The SDK uses the **HoneyBadger MPC protocol** by default. This is a robust,
//! Byzantine fault-tolerant protocol suitable for most use cases. Developers don't
//! need to worry about protocol details - the SDK handles it automatically.
//!
//! For advanced users, the protocol can be explicitly selected using `.protocol()`:
//!
//! ```rust,no_run
//! # use stoffel_rust_sdk::prelude::*;
//! # fn main() -> Result<()> {
//! let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .parties(5)
//!     .protocol(ProtocolType::HoneyBadger)  // Explicit protocol selection
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
//! ### MPC Network Setup
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! # fn main() -> Result<()> {
//! // Compile with MPC configuration (uses HoneyBadger protocol by default)
//! let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .parties(5)        // 5-party MPC network
//!     .threshold(1)      // Tolerates 1 faulty party (Byzantine fault tolerant)
//!     .instance_id(42)   // Computation ID
//!     .build()?;
//!
//! // Protocol is automatically configured (HoneyBadger)
//! println!("Protocol: {:?}", runtime.protocol_type());
//!
//! // Test locally
//! let result = runtime.program().execute_local()?;
//!
//! // Create MPC participants (inherit protocol from runtime)
//! let client = runtime.client(100).with_inputs(vec![10, 20]).build()?;
//! let node = runtime.node(0).with_preprocessing(10, 25).build()?;
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
//! let runtime = Stoffel::builder()
//!     .source("main main() -> int64:\n  return 42")
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
//! // Create nodes (perform secure computation using HoneyBadger)
//! let node1 = runtime.node(0).with_preprocessing(10, 25).build()?;
//! let node2 = runtime.node(1).with_preprocessing(10, 25).build()?;
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
pub mod vm;
pub mod program;
pub mod network_config;
pub mod prelude;

#[cfg(feature = "mpc-local")]
pub mod mpc_local;

#[cfg(feature = "mpc-local")]
pub mod stoffel_mpc;

pub use error::{Error, Result};

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
/// MPCClient / MPCNode (participants using configured protocol)
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
/// let node = runtime.node(0).build()?;
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
pub struct Stoffel<F = ark_bls12_381::Fr> {
    source: Option<String>,
    file_path: Option<String>,
    bytecode: Option<Vec<u8>>,
    optimize: bool,
    n_parties: Option<usize>,
    threshold: Option<usize>,
    instance_id: u64,
    network_config: Option<network_config::NetworkConfig>,
    protocol_type: ProtocolType,  // Default MPC protocol
    _field: std::marker::PhantomData<F>,
}

// Default implementation using BLS12-381 field
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
}

impl<F> Stoffel<F> {
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
            _field: std::marker::PhantomData,
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
            _field: std::marker::PhantomData,
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
    /// let node = runtime.node(0).build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn build(self) -> Result<StoffelRuntime<F>> {
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
                _field: std::marker::PhantomData,
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
                _field: std::marker::PhantomData,
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

impl<F> Default for Stoffel<F> {
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
/// let node = runtime.node(0).build()?;
/// # Ok(())
/// # }
/// ```
pub struct StoffelRuntime<F = ark_bls12_381::Fr> {
    program: program::Program,
    n_parties: Option<usize>,
    threshold: Option<usize>,
    instance_id: u64,
    network_config: Option<network_config::NetworkConfig>,
    /// The configured MPC protocol type
    /// Currently defaults to HoneyBadger, but in the future can be configured
    protocol_type: ProtocolType,
    _field: std::marker::PhantomData<F>,
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

impl<F> StoffelRuntime<F> {
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
    pub fn client(&self, client_id: usize) -> program::MPCClientBuilder {
        let (_, _, instance_id) = self.mpc_config()
            .expect("Cannot create MPC client without MPC configuration. Use .parties(n).threshold(t) when building.");

        program::MPCClientBuilder {
            program: self.program.clone(),
            client_id,
            inputs: Vec::new(),
            instance_id,
        }
    }

    /// Create an MPC node builder
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
    /// let node = runtime.node(0)
    ///     .with_preprocessing(10, 25)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn node(&self, party_id: usize) -> program::MPCNodeBuilder {
        let (n_parties, threshold, instance_id) = self.mpc_config()
            .expect("Cannot create MPC node without MPC configuration. Use .parties(n).threshold(t) when building.");

        program::MPCNodeBuilder {
            program: self.program.clone(),
            party_id,
            n_triples: None,
            n_random_shares: None,
            n_parties,
            threshold,
            instance_id,
        }
    }
}
