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
//! // Query MPC configuration
//! let mpc = runtime.mpc_config().unwrap();
//! assert_eq!(mpc.parties, 5);
//!
//! // Access the compiled program bytecode
//! let bytecode = runtime.program().bytecode();
//! # Ok(())
//! # }
//! ```

use crate::{client, config, program, server, types};

/// A compiled Stoffel program paired with MPC infrastructure configuration.
///
/// `StoffelRuntime` is the result of calling [`.build()`](crate::Stoffel::build) on
/// a configured [`Stoffel`](crate::Stoffel) builder. It packages together:
///
/// - The compiled bytecode as a [`Program`](crate::program::Program)
/// - Optional [`MpcConfig`](crate::config::MpcConfig) for MPC operations
///
/// From a runtime you can:
/// - Query MPC configuration via [`mpc_config()`](Self::mpc_config)
/// - Create MPC participants via [`client()`](Self::client), [`server()`](Self::server)
/// - Access compiled bytecode via [`program()`](Self::program)
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
/// // Inspect config
/// let mpc = runtime.mpc_config().unwrap();
/// assert_eq!(mpc.parties, 5);
/// # Ok(())
/// # }
/// ```
pub struct StoffelRuntime {
    pub(crate) program: program::Program,
    pub(crate) mpc_config: Option<config::MpcConfig>,
    pub(crate) inputs: Vec<(String, types::Value)>,
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
    /// let bytecode = runtime.program().bytecode();
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

    /// Get the stored inputs.
    pub fn inputs(&self) -> &[(String, types::Value)] {
        &self.inputs
    }

    /// Create a client builder pre-configured with this runtime's MPC config.
    ///
    /// The client will interact with the coordinator to submit inputs and
    /// receive computation outputs.
    pub fn client(&self) -> client::ClientBuilder {
        let mut builder = client::ClientBuilder::new();
        if let Some(ref mpc) = self.mpc_config {
            builder = builder.client_id(types::ClientId(mpc.instance_id));
        }
        builder
    }

    /// Create a server builder pre-configured with this runtime's MPC config.
    ///
    /// The server registers with the coordinator, receives the program,
    /// and executes MPC computation using the selected backend engine.
    pub fn server(&self, party_id: usize) -> server::ServerBuilder {
        let mut builder = server::ServerBuilder::new(party_id);
        if let Some(ref mpc) = self.mpc_config {
            builder = builder
                .with_preprocessing(
                    1000, // default triples
                    500,  // default random shares
                );
        }
        builder
    }

}
