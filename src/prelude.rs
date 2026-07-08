//! Convenient re-exports for common Stoffel SDK usage
//!
//! # Example
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // === Client Side (App Developers) ===
//!     let client = StoffelClient::builder()
//!         .with_servers(&["server1:19200", "server2:19200", "server3:19200"])
//!         .connect()
//!         .await?;
//!
//!     let result = client.run(&[42, 100]).await?;
//!     println!("Result: {:?}", result);
//!     Ok(())
//! }
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

// Configuration
pub use crate::network_config::{NetworkConfig, NetworkSettings, MPCSettings, NetworkConfigBuilder};
pub use crate::secret_sharing::{SecretSharing, SecretShare};

// Network helpers module is available but types must be imported directly
// from stoffel_vm for production deployments. See network_helpers module docs.
pub use crate::network_helpers;

// MPCaaS API (Primary) - for production MPC deployments
pub use crate::mpcaas::{
    // Client API (for app developers)
    StoffelClient, StoffelClientBuilder, ClientState,
    // Server API (for infrastructure operators)
    StoffelServer, StoffelServerBuilder, ServerState,
    // Async computation handle
    ComputationHandle,
    // Peer and client management (for server implementations)
    PeerManager, PeerState, PeerInfo, DiscoveryMode, PartyId,
    ClientHandler, ClientId,
};
