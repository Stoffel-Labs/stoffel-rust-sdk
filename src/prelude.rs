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
//! let bytecode = runtime.program().bytecode();
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

// Config types
pub use crate::config::{MpcConfig, MpcBackendConfig, Curve, StoffelConfig};

// Backend types (protocol selection + real engines from StoffelVM)
pub use crate::backend::MpcBackend;
pub use crate::backend::{MpcEngine, MpcRunner};

// Coordinator (Coordinator trait from stoffel-mpc-coordinator)
pub use crate::coordinator::offchain::Coordinator;
