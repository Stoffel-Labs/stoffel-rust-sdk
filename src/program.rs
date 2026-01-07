//! Stoffel program abstraction - Pure bytecode container
//!
//! This module provides the core Program abstraction for Stoffel.
//! A Program is simply compiled bytecode - it contains no MPC configuration.
//!
//! # Philosophy
//!
//! In Stoffel's architecture:
//! - **Program** = Pure compiled bytecode (this module)
//! - **StoffelRuntime** = Program + MPC infrastructure configuration
//! - **MPC configuration** is infrastructure-level, not program-level
//!
//! Programs can be:
//! - Executed locally for testing via `.execute_local()`
//! - Saved to disk via `.save(path)`
//! - Used by StoffelRuntime to create MPC infrastructure
//!
//! # Examples
//!
//! ## Local execution (testing)
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::Stoffel;
//!
//! # fn main() -> stoffel_rust_sdk::Result<()> {
//! // Compile and build a runtime
//! let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .build()?;
//!
//! // Get the underlying program
//! let program = runtime.program();
//!
//! // Test locally
//! let result = program.execute_local()?;
//! # Ok(())
//! # }
//! ```
//!
//! ## MPC infrastructure setup
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Compile the program
//!     let program = Stoffel::compile("main main() -> secret int64:\n  return 42")?
//!         .build()?;
//!
//!     // Create an MPC server with the program
//!     let server = Stoffel::server(0)
//!         .bind("0.0.0.0:19200")
//!         .with_peers(&[(1, "server2:19200"), (2, "server3:19200")])
//!         .with_program(program.program().clone())
//!         .with_preprocessing(10, 20)
//!         .build()?;
//!
//!     server.start().await?;
//!     server.run_forever().await
//! }
//! ```

use crate::{vm, network_config::NetworkConfig, Error, Result};

/// A compiled Stoffel program
///
/// A Program is simply compiled bytecode that can be executed locally or in an MPC network.
/// The Program itself doesn't contain MPC configuration - that's managed by the Stoffel
/// infrastructure layer when creating clients and nodes.
///
/// Programs can be:
/// - Executed locally for testing via `.execute_local()`
/// - Used by Stoffel to create MPC infrastructure (nodes and clients)
#[derive(Clone)]
pub struct Program {
    /// The compiled bytecode
    bytecode: Vec<u8>,
}

impl Program {
    /// Create a new Program directly (internal use - users should use Stoffel::compile())
    ///
    /// This is called by StoffelBuilder after compilation.
    pub(crate) fn new(bytecode: Vec<u8>) -> Self {
        Self {
            bytecode,
        }
    }

    /// Get a reference to the bytecode
    pub fn bytecode(&self) -> &[u8] {
        &self.bytecode
    }

    /// Save the bytecode to a file
    pub fn save(&self, path: &str) -> Result<()> {
        std::fs::write(path, &self.bytecode).map_err(|e| Error::IoError(e))
    }

    /// Execute the program locally on the VM for testing
    ///
    /// This runs the "main" function locally without MPC. Useful for testing
    /// program logic before setting up MPC infrastructure.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::Stoffel;
    /// # fn main() -> stoffel_rust_sdk::Result<()> {
    /// let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .build()?;
    /// let program = runtime.program();
    /// let result = program.execute_local()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn execute_local(&self) -> Result<vm::Value> {
        self.execute_local_function("main")
    }

    /// Execute a specific function locally on the VM
    pub fn execute_local_function(&self, function_name: &str) -> Result<vm::Value> {
        let vm = vm::VM::new();
        vm.run_bytecode(&self.bytecode, function_name)
    }

    /// Execute a function locally with arguments
    pub fn execute_local_with_args(&self, function_name: &str, args: Vec<vm::Value>) -> Result<vm::Value> {
        let loaded = vm::LoadedProgram::from_bytecode(self.bytecode.clone());
        loaded.execute_with_args(function_name, args)
    }

    /// List all functions in this program
    pub fn list_functions(&self) -> Result<Vec<vm::FunctionInfo>> {
        let loaded = vm::LoadedProgram::from_bytecode(self.bytecode.clone());
        loaded.list_functions()
    }
}

// MPC Participant builders are in the mpcaas module:
// - StoffelServerBuilder  → src/mpcaas/server.rs
// - StoffelClientBuilder  → src/mpcaas/client.rs
//
// This keeps the Program abstraction focused on compiled bytecode,
// while builders live with the MPC participant types they create.
