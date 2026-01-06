//! MPCaaS Server API
//!
//! This module provides the server-side API for infrastructure operators running
//! MPC compute nodes. Unlike the client API, the server API requires understanding
//! of MPC configuration (party ID, peers, program, preprocessing).
//!
//! # Design Philosophy
//!
//! The server API is more complex than the client API because infrastructure
//! operators need to configure:
//! - Party ID (unique identifier in the MPC network)
//! - Peer addresses (other servers in the network)
//! - The MPC program to execute
//! - Preprocessing parameters (Beaver triples, random shares)
//!
//! # Examples
//!
//! ## Explicit Peer Configuration
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let program = Stoffel::compile("main main(a: secret int64, b: secret int64) -> secret int64:\n  return a + b")?
//!         .build()?;
//!
//!     let server = Stoffel::server(0)  // Party ID
//!         .bind("0.0.0.0:19200")
//!         .with_peers(&[
//!             (1, "192.168.1.11:19200"),
//!             (2, "192.168.1.12:19200"),
//!         ])
//!         .with_program(program.program().clone())
//!         .with_preprocessing(10, 20)
//!         .build()?;
//!
//!     server.start().await?;
//!     server.run_forever().await
//! }
//! ```

use crate::client_handler::ClientHandler;
use crate::peer_manager::{DiscoveryMode, PeerManager};
use crate::program::Program;
use crate::{Error, Result};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

/// State of the MPC server
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerState {
    /// Server initialized but not started
    Initialized,
    /// Server is starting up
    Starting,
    /// Connecting to peer servers
    ConnectingPeers,
    /// Running preprocessing (generating cryptographic material)
    Preprocessing,
    /// Ready to accept client connections
    Ready,
    /// Currently processing a computation
    Computing,
    /// Shutting down
    ShuttingDown,
}

impl std::fmt::Display for ServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerState::Initialized => write!(f, "Initialized"),
            ServerState::Starting => write!(f, "Starting"),
            ServerState::ConnectingPeers => write!(f, "ConnectingPeers"),
            ServerState::Preprocessing => write!(f, "Preprocessing"),
            ServerState::Ready => write!(f, "Ready"),
            ServerState::Computing => write!(f, "Computing"),
            ServerState::ShuttingDown => write!(f, "ShuttingDown"),
        }
    }
}

/// Builder for creating an MPC server
///
/// Use `Stoffel::server(party_id)` to create a builder.
pub struct StoffelServerBuilder {
    party_id: usize,
    bind_address: Option<SocketAddr>,
    peers: Vec<(usize, SocketAddr)>,
    signaling_server: Option<String>,
    stun_server: Option<String>,
    program: Option<Program>,
    n_triples: usize,
    n_random_shares: usize,
}

impl StoffelServerBuilder {
    /// Create a new server builder with the given party ID
    pub fn new(party_id: usize) -> Self {
        Self {
            party_id,
            bind_address: None,
            peers: Vec::new(),
            signaling_server: None,
            stun_server: None,
            program: None,
            n_triples: 10,
            n_random_shares: 20,
        }
    }

    /// Set the address to bind to
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// let builder = Stoffel::server(0).bind("0.0.0.0:19200");
    /// # Ok(())
    /// # }
    /// ```
    pub fn bind(mut self, address: &str) -> Self {
        self.bind_address = address.parse().ok();
        self
    }

    /// Set the peer servers to connect to
    ///
    /// Each peer is specified as a tuple of (party_id, address).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// let builder = Stoffel::server(0)
    ///     .with_peers(&[
    ///         (1, "192.168.1.11:19200"),
    ///         (2, "192.168.1.12:19200"),
    ///     ]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_peers(mut self, peers: &[(usize, &str)]) -> Self {
        self.peers = peers
            .iter()
            .filter_map(|(id, addr)| {
                addr.parse::<SocketAddr>().ok().map(|a| (*id, a))
            })
            .collect();
        self
    }

    /// Set the signaling server for peer discovery
    ///
    /// Use this instead of `with_peers()` for dynamic peer discovery.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// let builder = Stoffel::server(0)
    ///     .with_signaling_server("signal.example.com:9000");
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_signaling_server(mut self, address: &str) -> Self {
        self.signaling_server = Some(address.to_string());
        self
    }

    /// Set the STUN server for NAT traversal
    ///
    /// Optional, used with signaling server for NAT traversal.
    pub fn with_stun_server(mut self, address: &str) -> Self {
        self.stun_server = Some(address.to_string());
        self
    }

    /// Set the MPC program to execute
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// let program = Stoffel::compile("main main() -> int64:\n  return 42")?
    ///     .build()?;
    ///
    /// let builder = Stoffel::server(0)
    ///     .with_program(program.program().clone());
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_program(mut self, program: Program) -> Self {
        self.program = Some(program);
        self
    }

    /// Set preprocessing parameters
    ///
    /// # Arguments
    ///
    /// * `n_triples` - Number of Beaver triples to generate
    /// * `n_random_shares` - Number of random shares to generate
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # fn main() -> Result<()> {
    /// let builder = Stoffel::server(0)
    ///     .with_preprocessing(10, 20);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_preprocessing(mut self, n_triples: usize, n_random_shares: usize) -> Self {
        self.n_triples = n_triples;
        self.n_random_shares = n_random_shares;
        self
    }

    /// Build the MPC server
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No bind address is specified
    /// - No peers are specified and no signaling server is set
    /// - No program is specified
    pub fn build(self) -> Result<StoffelServer> {
        let bind_address = self.bind_address
            .ok_or_else(|| Error::Configuration("No bind address specified".to_string()))?;

        // Determine peer discovery mode
        let discovery_mode = if let Some(signaling) = self.signaling_server {
            DiscoveryMode::SignalingServer {
                address: signaling,
                stun: self.stun_server,
            }
        } else if !self.peers.is_empty() {
            DiscoveryMode::Explicit(self.peers.clone())
        } else {
            return Err(Error::Configuration(
                "Either peers or signaling server must be specified".to_string()
            ));
        };

        let program = self.program
            .ok_or_else(|| Error::Configuration("No program specified".to_string()))?;

        // Calculate n_parties from peers + self
        let n_parties = self.peers.len() + 1;
        let threshold = 1; // Default threshold

        // Create peer manager
        let peer_manager = PeerManager::new(
            self.party_id,
            discovery_mode,
        );

        // Create client handler
        let client_handler = ClientHandler::new();

        Ok(StoffelServer {
            party_id: self.party_id,
            bind_address,
            program,
            peer_manager,
            client_handler,
            state: Arc::new(Mutex::new(ServerState::Initialized)),
            n_parties,
            threshold,
            n_triples: self.n_triples,
            n_random_shares: self.n_random_shares,
        })
    }
}

/// MPC Server
///
/// An MPC compute node that participates in secure multiparty computation.
/// Servers receive secret shares from clients, perform computation collaboratively
/// with other servers, and send result shares back to clients.
pub struct StoffelServer {
    /// This server's party ID
    party_id: usize,
    /// Address to bind to
    bind_address: SocketAddr,
    /// The MPC program to execute
    program: Program,
    /// Peer connection manager
    peer_manager: PeerManager,
    /// Client connection handler
    client_handler: ClientHandler,
    /// Current server state
    state: Arc<Mutex<ServerState>>,
    /// Number of parties
    n_parties: usize,
    /// Fault tolerance threshold
    threshold: usize,
    /// Number of Beaver triples for preprocessing
    n_triples: usize,
    /// Number of random shares for preprocessing
    n_random_shares: usize,
}

impl StoffelServer {
    /// Get the party ID
    pub fn party_id(&self) -> usize {
        self.party_id
    }

    /// Get the bind address
    pub fn bind_address(&self) -> SocketAddr {
        self.bind_address
    }

    /// Get the current server state
    pub fn state(&self) -> ServerState {
        *self.state.lock().unwrap()
    }

    /// Get the number of parties
    pub fn n_parties(&self) -> usize {
        self.n_parties
    }

    /// Get the threshold
    pub fn threshold(&self) -> usize {
        self.threshold
    }

    /// Start the server (bind, connect peers, preprocess)
    ///
    /// This performs the following steps:
    /// 1. Bind to the configured address
    /// 2. Connect to peer servers (mesh topology)
    /// 3. Run preprocessing to generate cryptographic material
    /// 4. Transition to Ready state
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let program = Stoffel::compile("main main() -> int64:\n  return 42")?.build()?;
    /// let server = Stoffel::server(0)
    ///     .bind("0.0.0.0:19200")
    ///     .with_peers(&[(1, "localhost:19201")])
    ///     .with_program(program.program().clone())
    ///     .build()?;
    ///
    /// server.start().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn start(&self) -> Result<()> {
        self.set_state(ServerState::Starting);
        tracing::info!("Server {} starting on {}", self.party_id, self.bind_address);

        // Step 1: Connect to peers
        self.set_state(ServerState::ConnectingPeers);
        tracing::info!("Server {} connecting to peers", self.party_id);

        // TODO: Implement actual peer connection
        // self.peer_manager.connect_mesh().await?;

        // Step 2: Run preprocessing
        self.set_state(ServerState::Preprocessing);
        tracing::info!(
            "Server {} running preprocessing ({} triples, {} random shares)",
            self.party_id,
            self.n_triples,
            self.n_random_shares
        );

        // TODO: Implement actual preprocessing
        // self.run_preprocessing().await?;

        // Step 3: Ready to accept clients
        self.set_state(ServerState::Ready);
        tracing::info!("Server {} ready to accept clients", self.party_id);

        Ok(())
    }

    /// Run forever, handling client requests
    ///
    /// This is the main server loop. It accepts client connections,
    /// coordinates with other servers, and processes computations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let program = Stoffel::compile("main main() -> int64:\n  return 42")?.build()?;
    /// let server = Stoffel::server(0)
    ///     .bind("0.0.0.0:19200")
    ///     .with_peers(&[(1, "localhost:19201")])
    ///     .with_program(program.program().clone())
    ///     .build()?;
    ///
    /// server.start().await?;
    /// server.run_forever().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run_forever(&self) -> Result<()> {
        tracing::info!("Server {} entering main loop", self.party_id);

        // TODO: Implement actual server loop
        // This should:
        // 1. Accept client connections
        // 2. Receive input shares from clients
        // 3. Coordinate with other servers for computation
        // 4. Send output shares to clients

        // For now, just wait forever
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;

            // Check if we should shut down
            if self.state() == ServerState::ShuttingDown {
                break;
            }
        }

        Ok(())
    }

    /// Run a single computation and return
    ///
    /// Unlike `run_forever()`, this processes one computation and returns.
    /// Useful for testing or batch processing.
    pub async fn run_once(&self) -> Result<crate::vm::Value> {
        self.set_state(ServerState::Computing);
        tracing::info!("Server {} running single computation", self.party_id);

        // TODO: Implement actual single computation
        // For now, just execute the program locally
        let result = self.program.execute_local()?;

        self.set_state(ServerState::Ready);
        Ok(result)
    }

    /// Graceful shutdown
    ///
    /// Signals the server to shut down and waits for cleanup.
    pub async fn shutdown(self) -> Result<()> {
        self.set_state(ServerState::ShuttingDown);
        tracing::info!("Server {} shutting down", self.party_id);

        // TODO: Implement graceful shutdown
        // - Finish any in-progress computations
        // - Close client connections
        // - Disconnect from peers

        Ok(())
    }

    /// Set the server state
    fn set_state(&self, state: ServerState) {
        let mut s = self.state.lock().unwrap();
        *s = state;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_state_display() {
        assert_eq!(format!("{}", ServerState::Ready), "Ready");
        assert_eq!(format!("{}", ServerState::Computing), "Computing");
    }
}
