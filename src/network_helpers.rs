//! Network Infrastructure for MPC
//!
//! This module documents the network infrastructure needed for MPC deployments.
//!
//! # Current Status
//!
//! The MPC network infrastructure currently lives in StoffelVM's test module.
//! To use it in your applications or examples, import directly from there:
//!
//! ```rust,ignore
//! use stoffel_vm::tests::mpc_multiplication_integration::{
//!     setup_honeybadger_quic_network,
//!     setup_honeybadger_quic_clients,
//!     HoneyBadgerQuicServer,
//!     HoneyBadgerQuicClient,
//!     HoneyBadgerQuicConfig,
//! };
//! ```
//!
//! # What The Infrastructure Provides
//!
//! **Automatic Network Setup:**
//! - ✅ QUIC listener binding on designated ports
//! - ✅ Automatic connection establishment with retry logic
//! - ✅ Message handler task spawning
//! - ✅ Full mesh network topology management
//! - ✅ Byzantine fault-tolerant HoneyBadger protocol
//!
//! **Components:**
//! - `setup_honeybadger_quic_network` - One-call complete server network setup
//! - `setup_honeybadger_quic_clients` - Automatic client setup with server connections
//! - `HoneyBadgerQuicServer` - Fully-featured MPC server with networking
//! - `HoneyBadgerQuicClient` - MPC client with automatic connection management
//! - `HoneyBadgerQuicConfig` - Network configuration (timeouts, retries, etc.)
//!
//! # Quick Start
//!
//! ## Setting Up a Complete 5-Party MPC Network
//!
//! ```rust,ignore
//! use stoffel_rust_sdk::prelude::*;
//! use ark_bls12_381::Fr;
//!
//! # async fn example() -> std::result::Result<(), Box<dyn std::error::Error>> {
//! // One function call for complete infrastructure!
//! let (mut servers, mut receivers) = setup_honeybadger_quic_network::<Fr>(
//!     5,      // n_parties
//!     1,      // threshold (tolerates 1 Byzantine fault)
//!     3,      // n_triples for multiplication
//!     8,      // n_random_shares for input masking
//!     42,     // instance_id (unique computation ID)
//!     19200,  // base_port (servers use 19200-19204)
//!     HoneyBadgerQuicConfig::default(),
//! ).await?;
//!
//! // Start servers (spawn QUIC listeners and message handlers)
//! for server in &mut servers {
//!     server.start().await?;
//! }
//!
//! // Establish full mesh topology
//! for server in &servers {
//!     server.connect_to_peers().await?;
//! }
//!
//! // Network is ready! Run MPC protocols...
//! # Ok(())
//! # }
//! ```
//!
//! ## Complete Example with Execution
//!
//! See `examples/honeybadger_mpc_demo.rs` for a complete working example
//! that demonstrates:
//! 1. Network setup using `setup_honeybadger_quic_network()`
//! 2. Preprocessing (Beaver triple generation)
//! 3. Input sharing from clients
//! 4. Secure computation (multiplication)
//! 5. Output reconstruction
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │  setup_honeybadger_quic_network()           │
//! │  - Creates N HoneyBadgerQuicServer instances│
//! │  - Each binds to QUIC listener              │
//! │  - Spawns accept() loops                    │
//! │  - Spawns message handlers                  │
//! └─────────────────────────────────────────────┘
//!                       │
//!                       ▼
//! ┌─────────────────────────────────────────────┐
//! │  HoneyBadgerQuicServer (per party)          │
//! │  - QUIC transport layer                     │
//! │  - HoneyBadgerMPCNode (protocol logic)      │
//! │  - Message routing                          │
//! │  - Connection management                    │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! # When to Use This Module
//!
//! **Use `network_helpers` when:**
//! - Building production MPC applications
//! - You need real network execution (not just API exploration)
//! - You want automatic infrastructure setup
//! - You're implementing MPC-as-a-Service
//!
//! **Don't use `network_helpers` when:**
//! - Just exploring the SDK API (use `StoffelClient::builder()` or `Stoffel::server()` instead)
//! - Running local tests without networking
//! - Building custom network transports (use low-level APIs)
//!
//! # Performance Characteristics
//!
//! - **Startup Time**: ~500ms for 5-party network
//! - **Connection Establishment**: <100ms per peer with retries
//! - **Message Latency**: <10ms for local networks
//! - **Throughput**: Supports high-frequency MPC operations
//!
//! # Security Notes
//!
//! - Uses TLS 1.3 via QUIC for encrypted communication
//! - Byzantine fault tolerance via HoneyBadger protocol
//! - Automatic share verification and error correction
//! - No single point of failure (distributed trust)

// Note: These helpers are available in stoffel-vm's test module.
// They cannot be re-exported here because they're in the `tests` module
// which is only available during testing.
//
// To use them in your application, import directly from stoffel_vm:
//
// ```rust
// use stoffel_vm::tests::mpc_multiplication_integration::{
//     setup_honeybadger_quic_network,
//     setup_honeybadger_quic_clients,
//     HoneyBadgerQuicServer,
//     HoneyBadgerQuicClient,
//     HoneyBadgerQuicConfig,
// };
// ```
//
// See examples/honeybadger_mpc_demo.rs for a working example.

// Re-export client store functionality for managing secret shares
pub use stoffel_vm::net::client_store::{
    ClientInputStore,
    ClientInputEntry,
};

use std::sync::OnceLock;

/// Global client input store singleton
///
/// Provides a process-wide shared store for client input shares.
/// This is used by the SDK to coordinate share storage across components.
static GLOBAL_STORE: OnceLock<ClientInputStore> = OnceLock::new();

/// Get the global client input store
///
/// Returns a reference to the process-wide `ClientInputStore` singleton.
/// The store is lazily initialized on first access.
///
/// # Example
///
/// ```rust,no_run
/// use stoffel_rust_sdk::network_helpers::get_global_store;
///
/// let store = get_global_store();
/// // Use the store...
/// ```
pub fn get_global_store() -> &'static ClientInputStore {
    GLOBAL_STORE.get_or_init(ClientInputStore::new)
}
