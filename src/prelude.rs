//! Convenient re-exports for common Stoffel SDK usage.
//!
//! # Example
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
//! let result = runtime.program().execute_local()?;
//! # Ok(())
//! # }
//! ```

// Core entry point and runtime
pub use crate::Stoffel;
pub use crate::runtime::StoffelRuntime;

// Error handling
pub use crate::error::{Error, Result};

// Compilation and execution
pub use crate::program::Program;
pub use crate::compiler::Compiler;
pub use crate::vm::{VM, Value};

// Shared types
pub use crate::types::{PartyId, ClientId, ComputationId};

// Config types will be re-exported once Agent B's work is merged:
// pub use crate::config::{MpcConfig, Curve, ...};
