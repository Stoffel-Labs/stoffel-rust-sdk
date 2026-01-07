//! MPCaaS (MPC as a Service) API
//!
//! This module provides the primary API for building MPC applications with Stoffel.
//! It implements a client-server architecture where:
//!
//! - **[`StoffelClient`]**: For app developers connecting to an MPC network
//! - **[`StoffelServer`]**: For infrastructure operators running MPC compute nodes
//!
//! # Architecture
//!
//! The MPCaaS architecture separates concerns between:
//!
//! - **Clients**: Provide secret inputs, receive computation outputs
//! - **Servers**: Run the MPC protocol, never see individual inputs
//!
//! This enables a deployment model where a fixed set of MPC servers can serve
//! many clients, each providing their own private inputs.
//!
//! # Quick Start
//!
//! ## Client (App Developer)
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
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
//! ## Server (Infrastructure Operator)
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let program = Stoffel::compile("main main() -> secret int64:\n  ...")?
//!         .build()?;
//!
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

pub mod client;
pub mod server;
pub mod protocol;
pub mod client_handler;
pub mod peer_manager;
pub mod handle;

// Client API
pub use client::{StoffelClient, StoffelClientBuilder, ClientState};

// Server API
pub use server::{StoffelServer, StoffelServerBuilder, ServerState};

// Supporting types
pub use handle::ComputationHandle;
pub use peer_manager::{PeerManager, PeerState, PeerInfo, DiscoveryMode, PartyId};
pub use client_handler::{ClientHandler, ClientId};
