//! Convenient re-exports for common Stoffel SDK usage
//!
//! # Example
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! # async fn example() -> Result<()> {
//! // Simple API - uses localhost defaults (127.0.0.1:19200+i)
//! let result = Stoffel::compile("main main(a: secret int64, b: secret int64) -> secret int64:\n  return a * b")?
//!     .parties(5)
//!     .threshold(1)
//!     .with_inputs(vec![vec![7], vec![6]])  // 7 * 6 = 42
//!     .execute()  // Full MPC with QUIC networking
//!     .await?;
//!
//! println!("Result: {:?}", result);  // 42
//! # Ok(())
//! # }
//!
//! # fn main() -> Result<()> {
//! // Local execution (no MPC, for testing)
//! let result = Stoffel::compile("main main() -> int64:\n  return 42")?
//!     .execute_local()?;
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

// MPC execution configuration
pub use crate::mpc_network::MPCExecutionConfig;

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

// MPCaaS Client API (for app developers)
pub use crate::stoffel_client::{StoffelClient, StoffelClientBuilder, ClientState};
pub use crate::computation_handle::ComputationHandle;

// MPCaaS Server API (for infrastructure operators)
pub use crate::stoffel_server::{StoffelServer, StoffelServerBuilder, ServerState};

// Peer and client management (for server implementations)
pub use crate::peer_manager::{PeerManager, PeerState, PeerInfo, DiscoveryMode, PartyId};
pub use crate::client_handler::{ClientHandler, ClientId};
// Note: ClientHandler's ClientState is available via client_handler::ClientState if needed
