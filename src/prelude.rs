//! Convenient re-exports for common Stoffel SDK usage
//!
//! # Example
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! # fn main() -> Result<()> {
//! // Simple API - everything you need for basic usage
//! let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .parties(5)
//!     .threshold(1)
//!     .build()?;
//!
//! let client = runtime.client(100).with_inputs(vec![10, 20]).build()?;
//! # Ok(())
//! # }
//! ```
//!
//! For advanced usage (raw VM access, protocol internals, etc.), use:
//! ```rust,no_run
//! use stoffel_rust_sdk::advanced::*;
//! ```

// Core SDK - High-level API (Recommended)
pub use crate::{
    Stoffel,
    StoffelRuntime,
    ProtocolType,
    ShareType,
    Error,
    Result,
};

// Compilation and execution
pub use crate::compiler::{Compiler, OptimizationLevel};
pub use crate::vm::{VM, Value, LoadedProgram};
pub use crate::program::Program;

// MPC participants (for custom setups)
pub use crate::client::{MPCClient, MPCConfig, ProtocolConfig};
pub use crate::server::MPCServer;
pub use crate::session::MPCNode;

// Configuration
pub use crate::network_config::{NetworkConfig, NetworkSettings, MPCSettings, NetworkConfigBuilder};
pub use crate::secret_sharing::{SecretSharing, SecretShare};

// Network helpers module is available but types must be imported directly
// from stoffel_vm for production deployments. See network_helpers module docs.
pub use crate::network_helpers;
