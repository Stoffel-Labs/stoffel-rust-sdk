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
//! use stoffel_rust_sdk::Stoffel;
//!
//! # fn main() -> stoffel_rust_sdk::Result<()> {
//! // Compile with MPC configuration
//! let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .parties(5)
//!     .threshold(1)
//!     .build()?;
//!
//! // Access the compiled program for local testing
//! let result = runtime.program().execute_local()?;
//! # Ok(())
//! # }
//! ```

use crate::{vm, Error, Result};

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
        std::fs::write(path, &self.bytecode).map_err(|e| Error::Io(e))
    }

    /// List all functions in this program
    pub fn list_functions(&self) -> Result<Vec<vm::FunctionInfo>> {
        let loaded = vm::LoadedProgram::from_bytecode(self.bytecode.clone());
        loaded.list_functions()
    }
}
