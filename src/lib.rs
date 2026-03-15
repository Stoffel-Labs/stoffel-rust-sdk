//! # Stoffel Rust SDK
//!
//! A production-ready Rust SDK for the Stoffel ecosystem, providing:
//! - **Stoffel-Lang**: Compile Stoffel programs to bytecode
//! - **StoffelVM**: Execute bytecode in the Stoffel virtual machine
//! - **MPC Infrastructure**: Multi-party computation configuration and execution
//!
//! ## Quick Start
//!
//! ### Local Execution
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! # fn main() -> Result<()> {
//! let result = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .execute_local()?;
//! println!("Result: {:?}", result);
//! # Ok(())
//! # }
//! ```
//!
//! ### Building an MPC Runtime
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! # fn main() -> Result<()> {
//! let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .parties(5)
//!     .threshold(1)
//!     .instance_id(42)
//!     .build()?;
//!
//! // Test locally before deploying to MPC network
//! let result = runtime.program().execute_local()?;
//! # Ok(())
//! # }
//! ```

// ── Module declarations ──────────────────────────────────────────────

pub mod error;
pub mod types;
pub mod config;
pub mod compiler;
pub mod vm;
pub mod program;
pub mod runtime;
pub mod backend;
pub mod client;
pub mod consensus;
pub mod coordinator;
pub mod observability;
pub mod server;
pub mod prelude;

pub use error::{Error, Result};

// ── Stoffel entry point / builder ────────────────────────────────────

/// The main entry point for the Stoffel SDK.
///
/// `Stoffel` acts as both entry point and builder: you create an instance via one of the
/// factory methods ([`compile`](Self::compile), [`compile_file`](Self::compile_file),
/// [`load`](Self::load)), configure MPC parameters with chained methods, and then either
/// [`build()`](Self::build) a [`StoffelRuntime`](runtime::StoffelRuntime) or call
/// [`execute_local()`](Self::execute_local) for quick testing.
///
/// # Entry Points
///
/// | Method | Description |
/// |--------|-------------|
/// | [`Stoffel::compile(source)`](Self::compile) | Compile source code |
/// | [`Stoffel::compile_file(path)`](Self::compile_file) | Compile from file |
/// | [`Stoffel::load(bytecode)`](Self::load) | Load pre-compiled bytecode |
///
/// # Configuration (all optional)
///
/// | Method | Default | Description |
/// |--------|---------|-------------|
/// | `.parties(n)` | 5 | Number of MPC parties |
/// | `.threshold(t)` | 1 | Byzantine fault tolerance |
/// | `.instance_id(id)` | random | Computation identifier |
/// | `.backend(backend)` | HoneyBadger | MPC backend protocol |
/// | `.network_config_file(path)` | none | Load MPC config from TOML file |
///
/// # Build / Execute
///
/// | Method | Description |
/// |--------|-------------|
/// | `.build()` | Validate and produce a `StoffelRuntime` |
/// | `.execute_local()` | Convenience: compile and run on local VM |
/// | `.execute_local_function(name)` | Run a specific function locally |
///
/// # Examples
///
/// ```rust,no_run
/// use stoffel_rust_sdk::Stoffel;
///
/// # fn main() -> stoffel_rust_sdk::Result<()> {
/// // Quick local execution
/// let result = Stoffel::compile("main main() -> int64:\n  return 42")?
///     .execute_local()?;
///
/// // Full MPC runtime
/// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
///     .parties(5)
///     .threshold(1)
///     .instance_id(42)
///     .build()?;
/// # Ok(())
/// # }
/// ```
pub struct Stoffel {
    source: Option<String>,
    file_path: Option<String>,
    bytecode: Option<Vec<u8>>,
    optimize: bool,
    n_parties: Option<usize>,
    threshold: Option<usize>,
    instance_id: u64,
    mpc_backend: Option<backend::MpcBackend>,
}

impl Stoffel {
    // ── Factory methods ──────────────────────────────────────────────

    /// Compile Stoffel source code.
    ///
    /// The source is compiled immediately; the resulting builder carries the
    /// bytecode forward for configuration and building.
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

    /// Compile a Stoffel program from a file path.
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
    pub fn compile_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let compiler = compiler::Compiler::new();
        let bytecode = compiler.compile_file(path.as_ref().to_str().unwrap_or(""))?;
        Ok(Self::from_bytecode(bytecode))
    }

    /// Load pre-compiled bytecode.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::Stoffel;
    /// # fn main() -> stoffel_rust_sdk::Result<()> {
    /// let bytecode = std::fs::read("program.stfb")?;
    /// let runtime = Stoffel::load(&bytecode)
    ///     .parties(5)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load(bytecode: &[u8]) -> Self {
        Self::from_bytecode(bytecode.to_vec())
    }

    /// Create a new empty builder (advanced use).
    pub fn new() -> Self {
        Self {
            source: None,
            file_path: None,
            bytecode: None,
            optimize: false,
            n_parties: None,
            threshold: None,
            instance_id: 0,
            mpc_backend: None,
        }
    }

    /// Internal helper: create a builder that already holds bytecode.
    fn from_bytecode(bytecode: Vec<u8>) -> Self {
        Self {
            source: None,
            file_path: None,
            bytecode: Some(bytecode),
            optimize: false,
            n_parties: None,
            threshold: None,
            instance_id: 0,
            mpc_backend: None,
        }
    }

    // ── Configuration methods ────────────────────────────────────────

    /// Set the number of MPC parties.
    ///
    /// Must be >= 4 for the HoneyBadger protocol (validated in [`build()`](Self::build)).
    /// Default: 5.
    pub fn parties(mut self, n: usize) -> Self {
        self.n_parties = Some(n);
        self
    }

    /// Set the Byzantine fault-tolerance threshold.
    ///
    /// The protocol tolerates up to `t` faulty parties where `n >= 3t + 1`.
    /// Default: 1.
    pub fn threshold(mut self, t: usize) -> Self {
        self.threshold = Some(t);
        self
    }

    /// Set the computation instance ID.
    ///
    /// Each independent computation should use a unique instance ID.
    /// Default: 0.
    pub fn instance_id(mut self, id: u64) -> Self {
        self.instance_id = id;
        self
    }

    /// Enable compiler optimization.
    pub fn optimize(mut self, enable: bool) -> Self {
        self.optimize = enable;
        self
    }

    /// Set the source code to compile (alternative to factory methods).
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set the file path to compile (alternative to factory methods).
    pub fn file(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    /// Set the MPC backend protocol.
    ///
    /// Default: [`MpcBackend::HoneyBadger`](backend::MpcBackend::HoneyBadger).
    pub fn backend(mut self, backend: backend::MpcBackend) -> Self {
        self.mpc_backend = Some(backend);
        self
    }

    /// Load MPC configuration from a TOML file.
    ///
    /// MPC parameters (parties, threshold, instance_id, backend) are extracted
    /// from the config if not already set explicitly.
    pub fn network_config_file(mut self, path: impl AsRef<std::path::Path>) -> Result<Self> {
        let stoffel_config = config::StoffelConfig::load(
            path.as_ref().to_str().unwrap_or(""),
        )?;

        if self.n_parties.is_none() {
            self.n_parties = Some(stoffel_config.mpc.parties);
        }
        if self.threshold.is_none() {
            self.threshold = Some(stoffel_config.mpc.threshold);
        }
        if self.instance_id == 0 {
            self.instance_id = stoffel_config.mpc.instance_id;
        }

        Ok(self)
    }

    // ── Build / execute ──────────────────────────────────────────────

    /// Validate configuration and build a [`StoffelRuntime`](runtime::StoffelRuntime).
    ///
    /// # Validation
    ///
    /// When MPC parties are configured, the following checks are enforced:
    /// - `parties >= 4`
    /// - `parties >= 3 * threshold + 1`
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
    pub fn build(self) -> Result<runtime::StoffelRuntime> {
        // Resolve bytecode
        let bytecode = self.resolve_bytecode()?;

        // Build MpcConfig if parties were configured
        let mpc_config = if let Some(n) = self.n_parties {
            let t = self.threshold.unwrap_or(1);

            let mpc = config::MpcConfig {
                parties: n,
                threshold: t,
                instance_id: self.instance_id,
                backend: match self.mpc_backend {
                    Some(backend::MpcBackend::HoneyBadger) | None => {
                        config::MpcBackendConfig::HoneyBadger
                    }
                    Some(backend::MpcBackend::Avss { curve }) => {
                        config::MpcBackendConfig::Avss {
                            curve: config::Curve::from_backend(curve),
                        }
                    }
                },
            };
            mpc.validate()?;
            Some(mpc)
        } else {
            None
        };

        Ok(runtime::StoffelRuntime {
            program: program::Program::new(bytecode),
            mpc_config,
        })
    }

    /// Compile and execute the "main" function locally on the VM.
    ///
    /// This is a convenience shortcut that skips MPC configuration entirely.
    /// Useful for quick testing.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::Stoffel;
    /// # fn main() -> stoffel_rust_sdk::Result<()> {
    /// let result = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .execute_local()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn execute_local(self) -> Result<vm::Value> {
        let bytecode = self.resolve_bytecode()?;
        let v = vm::VM::new();
        v.run_bytecode(&bytecode, "main")
    }

    /// Compile and execute a specific function locally on the VM.
    pub fn execute_local_function(self, name: &str) -> Result<vm::Value> {
        let bytecode = self.resolve_bytecode()?;
        let v = vm::VM::new();
        v.run_bytecode(&bytecode, name)
    }

    // ── Internal helpers ─────────────────────────────────────────────

    /// Resolve bytecode from whichever source was provided.
    fn resolve_bytecode(&self) -> Result<Vec<u8>> {
        if let Some(ref bc) = self.bytecode {
            return Ok(bc.clone());
        }

        let mut comp = compiler::Compiler::new();
        if self.optimize {
            comp = comp.optimize(true);
        }

        if let Some(ref source) = self.source {
            comp.compile_source(source)
        } else if let Some(ref path) = self.file_path {
            comp.compile_file(path)
        } else {
            Err(Error::InvalidInput(
                "No source, file, or bytecode provided".to_string(),
            ))
        }
    }
}

impl Default for Stoffel {
    fn default() -> Self {
        Self::new()
    }
}
