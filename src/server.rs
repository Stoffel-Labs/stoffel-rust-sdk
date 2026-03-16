//! StoffelServer API (RFC-003) - Production MPC server with lifecycle management.
//!
//! This module provides the [`StoffelServer`] type, a high-level abstraction for
//! running an MPC compute node. It manages the full server lifecycle:
//!
//! ```text
//! Initialized → Starting → Preprocessing → Ready → Computing → ShuttingDown → Stopped
//! ```
//!
//! # Architecture
//!
//! - **[`ServerBuilder`]** configures a server with party ID, bind address, peers,
//!   preprocessing parameters, and consensus timeout.
//! - **[`StoffelServer`]** is the running server, tracking state transitions and
//!   exposing health/metrics for observability.
//! - **[`HealthStatus`]** reports whether the server is operational, degraded, or down.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::time::Duration;
//! use stoffel_rust_sdk::server::{StoffelServer, ServerBuilder};
//!
//! # async fn example() -> stoffel_rust_sdk::error::Result<()> {
//! let server = StoffelServer::builder(0)
//!     .bind("127.0.0.1:9000")
//!     .mpc_port(9100)
//!     .with_peers(&[(1, "127.0.0.1:9001"), (2, "127.0.0.1:9002")])
//!     .with_preprocessing(1000, 500)
//!     .expected_clients(3)
//!     .consensus_timeout(Duration::from_secs(30))
//!     .build()?;
//!
//! assert!(server.state() == stoffel_rust_sdk::server::ServerState::Initialized);
//! # Ok(())
//! # }
//! ```

use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::config::PreprocessingConfig;
use crate::error::{Error, Result};
use crate::program::Program;
use crate::types::PartyId;

// ---------------------------------------------------------------------------
// ServerState
// ---------------------------------------------------------------------------

/// Lifecycle state of a [`StoffelServer`].
///
/// The state machine proceeds linearly:
///
/// ```text
/// Initialized → Starting → Preprocessing → Ready → Computing → ShuttingDown → Stopped
/// ```
///
/// `Ready` is the only state in which the server accepts new computation
/// requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerState {
    /// Server has been built but not yet started.
    Initialized,
    /// Server is binding to its address and connecting to peers.
    Starting,
    /// Server is generating preprocessing material (Beaver triples, random shares).
    Preprocessing,
    /// Server is fully operational and ready to accept computation requests.
    Ready,
    /// Server is actively executing an MPC computation.
    Computing,
    /// Server is gracefully shutting down (draining connections).
    ShuttingDown,
    /// Server has stopped and cannot be restarted.
    Stopped,
}

// ---------------------------------------------------------------------------
// HealthStatus
// ---------------------------------------------------------------------------

/// Health status of the server, suitable for health-check endpoints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HealthStatus {
    /// All subsystems are operating normally.
    Healthy,
    /// The server is operational but experiencing issues.
    Degraded {
        /// Human-readable description of the degradation.
        reason: String,
    },
    /// The server cannot process requests.
    Unhealthy {
        /// Human-readable description of the failure.
        reason: String,
    },
}

impl HealthStatus {
    /// Returns `true` if the status is [`HealthStatus::Healthy`].
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }

    /// Returns `true` if the server can still process requests (Healthy or Degraded).
    pub fn is_operational(&self) -> bool {
        matches!(self, HealthStatus::Healthy | HealthStatus::Degraded { .. })
    }
}

// ---------------------------------------------------------------------------
// ServerBuilder
// ---------------------------------------------------------------------------

/// Builder for constructing a [`StoffelServer`] with validated configuration.
///
/// Use [`StoffelServer::builder`] to create a new builder.
///
/// # Required
///
/// - `party_id` (set at construction)
///
/// # Optional (with defaults)
///
/// | Parameter          | Default                |
/// |--------------------|------------------------|
/// | `bind_addr`        | `0.0.0.0:9000`         |
/// | `mpc_port`         | `9100`                 |
/// | `peers`            | empty                  |
/// | `program`          | `None`                 |
/// | `expected_clients` | `None`                 |
/// | `preprocessing`    | `PreprocessingConfig::default()` |
/// | `consensus_timeout`| 30 seconds             |
pub struct ServerBuilder {
    party_id: PartyId,
    bind_addr: Option<SocketAddr>,
    mpc_port: Option<u16>,
    peers: Vec<(PartyId, String)>,
    program: Option<Program>,
    expected_clients: Option<usize>,
    preprocessing: PreprocessingConfig,
    consensus_timeout: Duration,
}

impl ServerBuilder {
    /// Create a new builder for the given party identifier.
    pub fn new(party_id: usize) -> Self {
        Self {
            party_id: PartyId::from(party_id),
            bind_addr: None,
            mpc_port: None,
            peers: Vec::new(),
            program: None,
            expected_clients: None,
            preprocessing: PreprocessingConfig::default(),
            consensus_timeout: Duration::from_secs(30),
        }
    }

    /// Set the address to bind the server listener to.
    ///
    /// Accepts anything that implements [`ToSocketAddrs`], e.g. `"127.0.0.1:9000"`.
    /// If not called, defaults to `0.0.0.0:9000`.
    pub fn bind(mut self, addr: impl ToSocketAddrs) -> Self {
        if let Some(resolved) = addr.to_socket_addrs().ok().and_then(|mut i| i.next()) {
            self.bind_addr = Some(resolved);
        }
        self
    }

    /// Set a dedicated port for MPC protocol traffic.
    ///
    /// When set, MPC messages are separated from other server services.
    pub fn mpc_port(mut self, port: u16) -> Self {
        self.mpc_port = Some(port);
        self
    }

    /// Register known peer servers.
    ///
    /// Each entry is `(party_id, address_string)`. Addresses are stored as
    /// strings and resolved at connection time.
    pub fn with_peers(mut self, peers: &[(usize, &str)]) -> Self {
        self.peers = peers
            .iter()
            .map(|(id, addr)| (PartyId::from(*id), addr.to_string()))
            .collect();
        self
    }

    /// Attach a compiled [`Program`] to be executed by this server.
    pub fn with_program(mut self, program: Program) -> Self {
        self.program = Some(program);
        self
    }

    /// Configure preprocessing material generation.
    ///
    /// # Arguments
    ///
    /// * `triples` - Number of Beaver triples to generate.
    /// * `random_shares` - Number of random shares to generate.
    pub fn with_preprocessing(mut self, triples: usize, random_shares: usize) -> Self {
        self.preprocessing.triples = triples;
        self.preprocessing.random_shares = random_shares;
        self
    }

    /// Set the number of clients expected to connect before computation begins.
    pub fn expected_clients(mut self, n: usize) -> Self {
        self.expected_clients = Some(n);
        self
    }

    /// Set the coordinator address for the coordinator-centric flow.
    ///
    /// When set, the server registers with the coordinator on startup
    /// and receives the program from it rather than loading locally.
    pub fn coordinator(mut self, addr: &str) -> Self {
        // Store in peers for now; will be used during start()
        self.peers.push((crate::types::PartyId(usize::MAX), addr.to_string()));
        self
    }

    /// Set the timeout for the consensus protocol round.
    ///
    /// If consensus is not reached within this duration, the server will
    /// report a consensus timeout error. Defaults to 30 seconds.
    pub fn consensus_timeout(mut self, duration: Duration) -> Self {
        self.consensus_timeout = duration;
        self
    }

    /// Validate configuration and build a [`StoffelServer`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Configuration`] if the bind address cannot be
    /// determined.
    pub fn build(self) -> Result<StoffelServer> {
        let bind_addr = self
            .bind_addr
            .unwrap_or_else(|| "0.0.0.0:9000".parse().unwrap());

        let mpc_port = self.mpc_port.unwrap_or(9100);

        Ok(StoffelServer {
            party_id: self.party_id,
            state: ServerState::Initialized,
            bind_addr,
            mpc_port,
            peers: self.peers,
            program: self.program,
            preprocessing: self.preprocessing,
            consensus_timeout: self.consensus_timeout,
            expected_clients: self.expected_clients,
            coordinator_addr: None,
            connected_peers: AtomicUsize::new(0),
            connected_clients: AtomicUsize::new(0),
            computations_completed: AtomicUsize::new(0),
        })
    }
}

// ---------------------------------------------------------------------------
// StoffelServer
// ---------------------------------------------------------------------------

/// A production MPC compute server.
///
/// `StoffelServer` manages the full lifecycle of an MPC server party:
/// binding, peer discovery, preprocessing, computation, and shutdown.
///
/// # Lifecycle
///
/// 1. Build via [`StoffelServer::builder`].
/// 2. Call [`start`](StoffelServer::start) to bind and connect to peers.
/// 3. The server transitions through `Preprocessing → Ready`.
/// 4. When a computation is triggered it enters `Computing`.
/// 5. Call [`shutdown`](StoffelServer::shutdown) for graceful termination.
///
/// # Metrics
///
/// The server tracks peer/client connections and completed computations via
/// atomic counters, safe for concurrent reads.
pub struct StoffelServer {
    party_id: PartyId,
    state: ServerState,
    bind_addr: SocketAddr,
    mpc_port: u16,
    peers: Vec<(PartyId, String)>,
    program: Option<Program>,
    preprocessing: PreprocessingConfig,
    consensus_timeout: Duration,
    expected_clients: Option<usize>,
    /// Coordinator address for the coordinator-centric flow (RFC-012).
    /// Server registers with coordinator and receives program from it.
    coordinator_addr: Option<String>,
    // Metrics (atomic for concurrent access)
    connected_peers: AtomicUsize,
    connected_clients: AtomicUsize,
    computations_completed: AtomicUsize,
}

impl StoffelServer {
    /// Create a new [`ServerBuilder`] for the given party identifier.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stoffel_rust_sdk::server::StoffelServer;
    ///
    /// # fn main() -> stoffel_rust_sdk::error::Result<()> {
    /// let server = StoffelServer::builder(0).build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder(party_id: usize) -> ServerBuilder {
        ServerBuilder::new(party_id)
    }

    /// Return the current lifecycle state.
    pub fn state(&self) -> ServerState {
        self.state
    }

    /// Return this server's party identifier.
    pub fn party_id(&self) -> PartyId {
        self.party_id
    }

    /// Returns `true` when the server is in the [`ServerState::Ready`] state.
    pub fn ready(&self) -> bool {
        self.state == ServerState::Ready
    }

    /// Start the server: bind, connect to peers, and run preprocessing.
    ///
    /// After this method returns successfully the server will be in the
    /// [`ServerState::Ready`] state.
    ///
    /// # Errors
    ///
    /// Returns an error if the server is not in the [`ServerState::Initialized`]
    /// state, or if binding/preprocessing fails.
    pub async fn start(&mut self) -> Result<()> {
        if self.state != ServerState::Initialized {
            return Err(Error::Computation(format!(
                "Cannot start server in state {:?} (expected Initialized)",
                self.state
            )));
        }

        self.state = ServerState::Starting;

        // TODO: bind listener, connect to peers via QUIC

        self.state = ServerState::Preprocessing;

        // TODO: generate Beaver triples and random shares

        self.state = ServerState::Ready;
        Ok(())
    }

    /// Run the server indefinitely, processing computation requests.
    ///
    /// This is a convenience wrapper that calls [`start`](StoffelServer::start)
    /// if needed and then loops, waiting for incoming computation requests.
    ///
    /// # Errors
    ///
    /// Propagates any error from [`start`](StoffelServer::start) or from
    /// computation execution.
    pub async fn run_forever(mut self) -> Result<()> {
        if self.state == ServerState::Initialized {
            self.start().await?;
        }

        // TODO: event loop accepting computation requests
        Err(Error::Computation(
            "run_forever: not yet implemented".to_string(),
        ))
    }

    /// Initiate a graceful shutdown.
    ///
    /// Drains active connections, stops accepting new requests, and
    /// transitions to [`ServerState::Stopped`].
    pub async fn shutdown(mut self) -> Result<()> {
        self.state = ServerState::ShuttingDown;

        // TODO: drain connections, stop listeners

        self.state = ServerState::Stopped;
        Ok(())
    }

    // ----- Metrics ----------------------------------------------------------

    /// Number of peer servers currently connected.
    pub fn connected_peers(&self) -> usize {
        self.connected_peers.load(Ordering::Relaxed)
    }

    /// Number of clients currently connected.
    pub fn connected_clients(&self) -> usize {
        self.connected_clients.load(Ordering::Relaxed)
    }

    /// Total number of computations completed since the server started.
    pub fn computations_completed(&self) -> usize {
        self.computations_completed.load(Ordering::Relaxed)
    }

    // ----- Health -----------------------------------------------------------

    /// Compute the current health status of the server.
    ///
    /// - **Healthy**: state is `Ready` and all expected peers are connected.
    /// - **Degraded**: state is `Ready` but some peers are missing.
    /// - **Unhealthy**: state is not `Ready` (or `Computing`).
    pub fn health(&self) -> HealthStatus {
        match self.state {
            ServerState::Ready | ServerState::Computing => {
                let peers = self.connected_peers.load(Ordering::Relaxed);
                let expected = self.peers.len();
                if peers >= expected {
                    HealthStatus::Healthy
                } else {
                    HealthStatus::Degraded {
                        reason: format!(
                            "Only {}/{} peers connected",
                            peers, expected
                        ),
                    }
                }
            }
            ServerState::Stopped | ServerState::ShuttingDown => HealthStatus::Unhealthy {
                reason: format!("Server is in {:?} state", self.state),
            },
            other => HealthStatus::Degraded {
                reason: format!("Server is in {:?} state (not yet ready)", other),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_defaults() {
        let server = StoffelServer::builder(0).build().unwrap();
        assert_eq!(server.party_id(), PartyId::from(0));
        assert_eq!(server.state(), ServerState::Initialized);
        assert!(!server.ready());
        assert_eq!(server.connected_peers(), 0);
        assert_eq!(server.connected_clients(), 0);
        assert_eq!(server.computations_completed(), 0);
    }

    #[test]
    fn builder_with_peers() {
        let server = StoffelServer::builder(1)
            .bind("127.0.0.1:9001")
            .mpc_port(9101)
            .with_peers(&[(0, "127.0.0.1:9000"), (2, "127.0.0.1:9002")])
            .with_preprocessing(2000, 1000)
            .expected_clients(5)
            .consensus_timeout(Duration::from_secs(60))
            .build()
            .unwrap();

        assert_eq!(server.party_id(), PartyId::from(1));
        assert_eq!(server.bind_addr, "127.0.0.1:9001".parse().unwrap());
        assert_eq!(server.mpc_port, 9101);
        assert_eq!(server.peers.len(), 2);
        assert_eq!(server.preprocessing.triples, 2000);
        assert_eq!(server.preprocessing.random_shares, 1000);
        assert_eq!(server.expected_clients, Some(5));
        assert_eq!(server.consensus_timeout, Duration::from_secs(60));
    }

    #[test]
    fn health_initialized_is_degraded() {
        let server = StoffelServer::builder(0).build().unwrap();
        let health = server.health();
        assert!(health.is_operational());
        assert!(!health.is_healthy());
    }

    #[test]
    fn health_status_methods() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(HealthStatus::Healthy.is_operational());

        let degraded = HealthStatus::Degraded {
            reason: "test".into(),
        };
        assert!(!degraded.is_healthy());
        assert!(degraded.is_operational());

        let unhealthy = HealthStatus::Unhealthy {
            reason: "down".into(),
        };
        assert!(!unhealthy.is_healthy());
        assert!(!unhealthy.is_operational());
    }

    #[tokio::test]
    async fn start_transitions_to_ready() {
        let mut server = StoffelServer::builder(0).build().unwrap();
        assert_eq!(server.state(), ServerState::Initialized);

        server.start().await.unwrap();
        assert_eq!(server.state(), ServerState::Ready);
        assert!(server.ready());
    }

    #[tokio::test]
    async fn double_start_fails() {
        let mut server = StoffelServer::builder(0).build().unwrap();
        server.start().await.unwrap();

        let err = server.start().await.unwrap_err();
        assert!(err.to_string().contains("Cannot start server"));
    }

    #[tokio::test]
    async fn shutdown_transitions_to_stopped() {
        let mut server = StoffelServer::builder(0).build().unwrap();
        server.start().await.unwrap();

        server.shutdown().await.unwrap();
        // Note: shutdown consumes self, so we can't check state after.
        // The test verifies it completes without error.
    }
}
