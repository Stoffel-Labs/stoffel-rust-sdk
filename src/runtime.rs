//! StoffelRuntime - compiled program with MPC configuration
//!
//! The runtime is produced by [`Stoffel::build()`](crate::Stoffel::build) and holds:
//! - A compiled [`Program`](crate::program::Program)
//! - Optional [`MpcConfig`](crate::config::MpcConfig) for MPC operations
//!
//! # Examples
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
//! // Access the compiled program
//! let result = runtime.program().execute_local()?;
//!
//! // Query MPC configuration
//! let mpc = runtime.mpc_config().unwrap();
//! assert_eq!(mpc.parties, 5);
//! # Ok(())
//! # }
//! ```

use crate::{config, program, vm};

/// A compiled Stoffel program paired with MPC infrastructure configuration.
///
/// `StoffelRuntime` is the result of calling [`.build()`](crate::Stoffel::build) on
/// a configured [`Stoffel`](crate::Stoffel) builder. It packages together:
///
/// - The compiled bytecode as a [`Program`](crate::program::Program)
/// - Optional [`MpcConfig`](crate::config::MpcConfig) for MPC operations
///
/// From a runtime you can:
/// - Execute programs locally for testing via [`program()`](Self::program)
/// - Query MPC configuration via [`mpc_config()`](Self::mpc_config)
/// - (Future) Create MPC participants via `client()`, `server()`, `node()`
///
/// # Example
///
/// ```rust,no_run
/// use stoffel_rust_sdk::prelude::*;
///
/// # fn main() -> Result<()> {
/// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
///     .parties(5)
///     .threshold(1)
///     .instance_id(42)
///     .build()?;
///
/// // Test locally
/// let result = runtime.program().execute_local()?;
///
/// // Inspect config
/// let mpc = runtime.mpc_config().unwrap();
/// assert_eq!(mpc.parties, 5);
/// # Ok(())
/// # }
/// ```
pub struct StoffelRuntime {
    pub(crate) program: program::Program,
    pub(crate) mpc_config: Option<config::MpcConfig>,
}

impl StoffelRuntime {
    /// Get a reference to the underlying compiled program.
    ///
    /// The program can be used for local execution and testing.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .build()?;
    /// let result = runtime.program().execute_local()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn program(&self) -> &program::Program {
        &self.program
    }

    /// Get the MPC configuration, if one was set.
    ///
    /// Returns `None` if no MPC configuration was set (i.e., no `.parties()` call).
    pub fn mpc_config(&self) -> Option<&config::MpcConfig> {
        self.mpc_config.as_ref()
    }

    /// Execute the program locally on the VM (convenience method).
    ///
    /// Equivalent to `runtime.program().execute_local()`.
    pub fn execute_local(&self) -> crate::Result<vm::Value> {
        self.program.execute_local()
    }

    /// Execute a specific function locally on the VM.
    ///
    /// Equivalent to `runtime.program().execute_local_function(name)`.
    pub fn execute_local_function(&self, name: &str) -> crate::Result<vm::Value> {
        self.program.execute_local_function(name)
    }

    // TODO: client(id) -> MPCClientBuilder  (will be implemented by network agent)
    // TODO: server(id) -> MPCServerBuilder  (will be implemented by network agent)
    // TODO: node(id)   -> MPCNodeBuilder    (will be implemented by network agent)
}
