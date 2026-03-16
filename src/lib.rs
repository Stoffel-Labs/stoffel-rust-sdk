//! # Stoffel Rust SDK
//!
//! The Rust SDK for building Multi-Party Computation (MPC) applications with Stoffel.
//!
//! - **Compile** programs written in StoffelLang to bytecode
//! - **Execute** them across an MPC network where no party sees plaintext data
//! - **Deploy** production MPC networks with generated Docker artifacts
//!
//! ## Quick Start
//!
//! ### Compile and Run Locally
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! # async fn run() -> Result<()> {
//! let results = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .parties(5)
//!     .threshold(1)
//!     .execute_local()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! [`execute_local()`](Stoffel::execute_local) delegates to
//! [`StoffelNetwork`](network::StoffelNetwork), which composes a coordinator
//! and N server tasks on localhost for development and testing.
//!
//! ### Deploy to Production
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! # fn main() -> Result<()> {
//! StoffelNetwork::builder()
//!     .program_file("program.stfl")
//!     .parties(5)
//!     .threshold(1)
//!     .build()?
//!     .scaffold("./deployment")?;
//! # Ok(())
//! # }
//! ```
//!
//! [`scaffold()`](network::StoffelNetwork::scaffold) generates a complete
//! deployment directory — `main.rs` binaries, TOML configs, `docker-compose.yml`,
//! and Dockerfiles — ready for `docker-compose up`.
//!
//! ### Build an MPC Runtime
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! # fn main() -> Result<()> {
//! let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .parties(5)
//!     .threshold(1)
//!     .build()?;
//!
//! let bytecode = runtime.program().bytecode();
//! # Ok(())
//! # }
//! ```
//!
//! ## Architecture
//!
//! ```text
//! StoffelNetwork (orchestrator)
//!   ├── StoffelCoordinator    JSON-RPC/TLS round management
//!   ├── StoffelServer × N     QUIC P2P, HoneyBadger MPC engine
//!   └── StoffelClient         input submission, output retrieval
//! ```
//!
//! The [`Stoffel`] builder is the quick-start entry point. For full control
//! over individual actors, use [`StoffelCoordinator`](coordinator::offchain::StoffelCoordinator),
//! [`StoffelServer`](server::StoffelServer), and [`StoffelClient`](client::StoffelClient)
//! directly. All actors support [`from_config()`](server::StoffelServer::from_config)
//! for TOML-driven deployment with env var overrides.

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
pub mod network;
pub mod observability;
pub mod server;
pub mod prelude;

pub use error::{Error, Result};

// ── Stoffel entry point / builder ────────────────────────────────────

/// The main entry point for the Stoffel SDK.
///
/// `Stoffel` is a compile-configure-run builder. Create an instance via a
/// factory method, configure MPC parameters, then either [`build()`](Self::build)
/// a [`StoffelRuntime`](runtime::StoffelRuntime) or call
/// [`execute_local()`](Self::execute_local) to run a full MPC network on localhost.
///
/// For production deployment, use [`StoffelNetwork`](network::StoffelNetwork)
/// directly — it provides [`scaffold()`](network::StoffelNetwork::scaffold) to
/// generate Docker-ready deployment artifacts.
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
/// | `.with_inputs(inputs)` | none | Named inputs for computation |
/// | `.backend(backend)` | HoneyBadger | MPC backend protocol |
/// | `.config_file(path)` | none | Load MPC config from TOML file |
///
/// # Build / Execute
///
/// | Method | Description |
/// |--------|-------------|
/// | `.build()` | Validate and produce a `StoffelRuntime` |
/// | `.execute_local().await` | Full MPC on localhost via [`StoffelNetwork`](network::StoffelNetwork) |
/// | `.execute_local_function(name).await` | Named function, full MPC (async) |
///
/// # Examples
///
/// ```rust,no_run
/// use stoffel_rust_sdk::Stoffel;
///
/// # fn main() -> stoffel_rust_sdk::Result<()> {
/// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
///     .parties(5)
///     .threshold(1)
///     .build()?;
///
/// let program = runtime.program();
/// # Ok(())
/// # }
/// ```
pub struct Stoffel {
    bytecode: Vec<u8>,
    n_parties: Option<usize>,
    threshold: Option<usize>,
    mpc_backend: Option<backend::MpcBackend>,
    inputs: Vec<(String, types::Value)>,
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

    /// Internal helper: create a builder that already holds bytecode.
    fn from_bytecode(bytecode: Vec<u8>) -> Self {
        Self {
            bytecode,
            n_parties: None,
            threshold: None,
            mpc_backend: None,
            inputs: Vec::new(),
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

    /// Provide named inputs for the computation.
    ///
    /// Inputs are passed to the MPC network via the coordinator during
    /// `execute_local()`, or stored in the runtime for use with `client()`.
    pub fn with_inputs(mut self, inputs: &[(&str, impl Into<types::Value> + Clone)]) -> Self {
        self.inputs = inputs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone().into()))
            .collect();
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
    /// MPC parameters (parties, threshold, backend) are extracted
    /// from the config if not already set explicitly.
    pub fn config_file(mut self, path: impl AsRef<std::path::Path>) -> Result<Self> {
        let stoffel_config = config::StoffelConfig::load(
            path.as_ref().to_str().unwrap_or(""),
        )?;

        if self.n_parties.is_none() {
            self.n_parties = Some(stoffel_config.mpc.parties);
        }
        if self.threshold.is_none() {
            self.threshold = Some(stoffel_config.mpc.threshold);
        }
        if self.mpc_backend.is_none() {
            self.mpc_backend = Some(match stoffel_config.mpc.backend {
                config::MpcBackendConfig::HoneyBadger => backend::MpcBackend::HoneyBadger,
                config::MpcBackendConfig::Avss { curve } => backend::MpcBackend::Avss { curve },
            });
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
        // Build MpcConfig if parties were configured
        let mpc_config = if let Some(n) = self.n_parties {
            let t = self.threshold.unwrap_or(1);

            let mpc = config::MpcConfig {
                parties: n,
                threshold: t,
                backend: match self.mpc_backend {
                    Some(backend::MpcBackend::HoneyBadger) | None => {
                        config::MpcBackendConfig::HoneyBadger
                    }
                    Some(backend::MpcBackend::Avss { curve }) => {
                        config::MpcBackendConfig::Avss { curve }
                    }
                },
            };
            mpc.validate()?;
            Some(mpc)
        } else {
            None
        };

        Ok(runtime::StoffelRuntime {
            program: program::Program::new(self.bytecode),
            mpc_config,
            inputs: self.inputs,
        })
    }

    /// Execute the computation locally with a full MPC network on localhost.
    ///
    /// Delegates to [`StoffelNetwork::execute_local()`](network::StoffelNetwork::execute_local),
    /// which composes an off-chain coordinator and N server tasks as in-process
    /// Tokio tasks.
    ///
    /// **Development only.** All parties share a process address space. For
    /// production, use [`StoffelNetwork::scaffold()`](network::StoffelNetwork::scaffold)
    /// to generate deployment artifacts with proper process isolation.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::Stoffel;
    /// # async fn run() -> stoffel_rust_sdk::Result<()> {
    /// let results = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .parties(5)
    ///     .threshold(1)
    ///     .execute_local()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute_local(self) -> Result<Vec<vm::Value>> {
        network::StoffelNetwork::builder()
            .program(self.bytecode)
            .parties(self.n_parties.unwrap_or(5))
            .threshold(self.threshold.unwrap_or(1))
            .backend(self.mpc_backend.unwrap_or(backend::MpcBackend::HoneyBadger))
            .build()?
            .with_inputs_from(self.inputs)
            .execute_local()
            .await
    }

    /// Execute a specific function locally with a full MPC network.
    ///
    /// Same as [`execute_local()`](Self::execute_local) but for a named function.
    pub async fn execute_local_function(self, _name: &str) -> Result<Vec<vm::Value>> {
        // TODO: pass function name through to StoffelNetwork
        self.execute_local().await
    }

}
