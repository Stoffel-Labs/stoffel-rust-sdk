//! Convenient re-exports for common Stoffel SDK usage
//!
//! # Example
//!
//! ```rust
//! use stoffel_rust_sdk::prelude::*;
//! ```

pub use crate::{
    Stoffel,
    StoffelRuntime,
    ProtocolType,
    ShareType,
    Error,
    Result,
};

pub use crate::compiler::{Compiler, OptimizationLevel};
pub use crate::vm::{VM, Value, LoadedProgram};
pub use crate::client::{MPCClient, MPCConfig, ProtocolConfig};
pub use crate::server::MPCServer;
pub use crate::session::MPCNode;
pub use crate::program::Program;
pub use crate::network_config::{NetworkConfig, NetworkSettings, MPCSettings, NetworkConfigBuilder};
pub use crate::secret_sharing::{SecretSharing, SecretShare};

/// Network infrastructure helpers (QUIC setup, connections, message handlers)
#[cfg(feature = "mpc-local")]
pub use crate::network_helpers::*;
