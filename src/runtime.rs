//! StoffelRuntime - compiled program with MPC configuration
//!
//! The runtime is produced by [`Stoffel::build()`](crate::Stoffel::build) and holds:
//! - A compiled [`Program`](crate::program::Program)
//! - MPC configuration (parties, threshold, instance ID)
//! - Optional network configuration
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
//! let (n, t, id) = runtime.mpc_config().unwrap();
//! # Ok(())
//! # }
//! ```

use crate::{program, network_config, vm};

/// A compiled Stoffel program paired with MPC infrastructure configuration.
///
/// `StoffelRuntime` is the result of calling [`.build()`](crate::Stoffel::build) on
/// a configured [`Stoffel`](crate::Stoffel) builder. It packages together:
///
/// - The compiled bytecode as a [`Program`](crate::program::Program)
/// - MPC parameters (number of parties, threshold, instance ID)
/// - Optional [`NetworkConfig`](crate::network_config::NetworkConfig) for production deployments
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
/// assert_eq!(runtime.mpc_config(), Some((5, 1, 42)));
/// # Ok(())
/// # }
/// ```
pub struct StoffelRuntime {
    pub(crate) program: program::Program,
    pub(crate) n_parties: Option<usize>,
    pub(crate) threshold: Option<usize>,
    pub(crate) instance_id: u64,
    pub(crate) network_config: Option<network_config::NetworkConfig>,
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

    /// Get the MPC configuration as a tuple of (n_parties, threshold, instance_id).
    ///
    /// Returns `None` if no MPC configuration was set (i.e., no `.parties()` call).
    pub fn mpc_config(&self) -> Option<(usize, usize, u64)> {
        if let (Some(n), Some(t)) = (self.n_parties, self.threshold) {
            Some((n, t, self.instance_id))
        } else {
            None
        }
    }

    /// Get the network configuration, if one was provided.
    pub fn network_config(&self) -> Option<&network_config::NetworkConfig> {
        self.network_config.as_ref()
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
