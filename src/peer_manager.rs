//! Peer Manager for Mesh Topology Management
//!
//! This module provides the `PeerManager` type for managing peer-to-peer
//! connections between MPC servers in a full mesh topology.
//!
//! # Design
//!
//! MPC servers form a full mesh topology where every server is connected
//! to every other server. This is required for the HoneyBadger protocol's
//! broadcast and agreement phases.
//!
//! Peer discovery can happen via:
//! - **Explicit configuration**: Peers are specified at build time
//! - **Signaling server**: Peers are discovered dynamically via a central server

use crate::{Error, Result};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Unique identifier for a party (server) in the MPC network
pub type PartyId = usize;

/// State of a peer connection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    /// Peer discovered but not connected
    Discovered,
    /// Connection attempt in progress
    Connecting,
    /// Successfully connected
    Connected,
    /// Connection failed
    Failed,
    /// Disconnected (was connected before)
    Disconnected,
}

impl std::fmt::Display for PeerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PeerState::Discovered => write!(f, "Discovered"),
            PeerState::Connecting => write!(f, "Connecting"),
            PeerState::Connected => write!(f, "Connected"),
            PeerState::Failed => write!(f, "Failed"),
            PeerState::Disconnected => write!(f, "Disconnected"),
        }
    }
}

/// Information about a peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Party ID of this peer
    pub party_id: PartyId,
    /// Network address
    pub address: SocketAddr,
    /// Current connection state
    pub state: PeerState,
}

impl PeerInfo {
    /// Create a new peer info
    pub fn new(party_id: PartyId, address: SocketAddr) -> Self {
        Self {
            party_id,
            address,
            state: PeerState::Discovered,
        }
    }
}

/// Mode for discovering peers
#[derive(Debug, Clone)]
pub enum DiscoveryMode {
    /// Peers are explicitly configured
    Explicit(Vec<(PartyId, SocketAddr)>),
    /// Peers are discovered via signaling server
    SignalingServer {
        /// Signaling server address
        address: String,
        /// Optional STUN server for NAT traversal
        stun: Option<String>,
    },
}

/// Manager for peer connections in the MPC network
///
/// Handles peer discovery, connection management, and mesh topology maintenance.
pub struct PeerManager {
    /// This server's party ID
    party_id: PartyId,
    /// Known peers
    peers: Arc<Mutex<HashMap<PartyId, PeerInfo>>>,
    /// Discovery mode
    discovery_mode: DiscoveryMode,
}

impl PeerManager {
    /// Create a new peer manager
    ///
    /// # Arguments
    ///
    /// * `party_id` - This server's party ID
    /// * `discovery_mode` - How to discover peers
    pub fn new(party_id: PartyId, discovery_mode: DiscoveryMode) -> Self {
        let mut peers = HashMap::new();

        // Pre-populate peers if using explicit mode
        if let DiscoveryMode::Explicit(ref peer_list) = discovery_mode {
            for (id, addr) in peer_list {
                if *id != party_id {
                    peers.insert(*id, PeerInfo::new(*id, *addr));
                }
            }
        }

        Self {
            party_id,
            peers: Arc::new(Mutex::new(peers)),
            discovery_mode,
        }
    }

    /// Get this server's party ID
    pub fn party_id(&self) -> PartyId {
        self.party_id
    }

    /// Get the discovery mode
    pub fn discovery_mode(&self) -> &DiscoveryMode {
        &self.discovery_mode
    }

    /// Connect to all peers (mesh topology)
    ///
    /// Establishes connections to all known peers. In explicit mode, this
    /// connects to the pre-configured peers. In signaling mode, this first
    /// discovers peers via the signaling server.
    pub async fn connect_mesh(&self) -> Result<()> {
        match &self.discovery_mode {
            DiscoveryMode::Explicit(_) => {
                self.connect_explicit_peers().await
            }
            DiscoveryMode::SignalingServer { address, stun } => {
                self.discover_and_connect(address, stun.as_deref()).await
            }
        }
    }

    /// Connect to explicitly configured peers
    async fn connect_explicit_peers(&self) -> Result<()> {
        let mut peers = self.peers.lock().await;

        for (_party_id, peer) in peers.iter_mut() {
            peer.state = PeerState::Connecting;
            tracing::info!(
                "Server {} connecting to peer {} at {}",
                self.party_id,
                peer.party_id,
                peer.address
            );

            // TODO: Implement actual QUIC connection
            // For now, just mark as connected
            peer.state = PeerState::Connected;
        }

        Ok(())
    }

    /// Discover peers via signaling server and connect
    async fn discover_and_connect(&self, signaling_addr: &str, stun_addr: Option<&str>) -> Result<()> {
        tracing::info!(
            "Server {} discovering peers via signaling server {}",
            self.party_id,
            signaling_addr
        );

        // TODO: Implement signaling server protocol
        // 1. Connect to signaling server
        // 2. Register ourselves
        // 3. Get list of other peers
        // 4. If STUN is configured, perform ICE negotiation
        // 5. Connect to each peer

        Err(Error::Network(
            "Signaling server discovery not yet implemented".to_string()
        ))
    }

    /// Check if all peers are connected
    pub async fn is_mesh_complete(&self) -> bool {
        let peers = self.peers.lock().await;
        peers.values().all(|p| p.state == PeerState::Connected)
    }

    /// Get connected peer count
    pub async fn connected_peer_count(&self) -> usize {
        let peers = self.peers.lock().await;
        peers.values().filter(|p| p.state == PeerState::Connected).count()
    }

    /// Get total peer count (including disconnected)
    pub async fn total_peer_count(&self) -> usize {
        let peers = self.peers.lock().await;
        peers.len()
    }

    /// Get a peer's info
    pub async fn get_peer(&self, party_id: PartyId) -> Option<PeerInfo> {
        let peers = self.peers.lock().await;
        peers.get(&party_id).cloned()
    }

    /// Get all peers
    pub async fn get_all_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.lock().await;
        peers.values().cloned().collect()
    }

    /// Handle peer disconnection (reconnect logic)
    pub async fn handle_disconnection(&self, peer_id: PartyId) -> Result<()> {
        let mut peers = self.peers.lock().await;

        if let Some(peer) = peers.get_mut(&peer_id) {
            peer.state = PeerState::Disconnected;
            tracing::warn!(
                "Server {} lost connection to peer {}",
                self.party_id,
                peer_id
            );

            // TODO: Implement reconnection logic
            // - Attempt to reconnect
            // - If using signaling server, re-discover the peer
            // - Notify other components of the disconnection
        }

        Ok(())
    }

    /// Add a peer dynamically
    pub async fn add_peer(&self, party_id: PartyId, address: SocketAddr) {
        let mut peers = self.peers.lock().await;
        if !peers.contains_key(&party_id) {
            peers.insert(party_id, PeerInfo::new(party_id, address));
            tracing::info!(
                "Server {} added peer {} at {}",
                self.party_id,
                party_id,
                address
            );
        }
    }

    /// Remove a peer
    pub async fn remove_peer(&self, party_id: PartyId) {
        let mut peers = self.peers.lock().await;
        if peers.remove(&party_id).is_some() {
            tracing::info!(
                "Server {} removed peer {}",
                self.party_id,
                party_id
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_state_display() {
        assert_eq!(format!("{}", PeerState::Connected), "Connected");
        assert_eq!(format!("{}", PeerState::Disconnected), "Disconnected");
    }

    #[tokio::test]
    async fn test_peer_manager_explicit() {
        let peers = vec![
            (1, "127.0.0.1:19201".parse().unwrap()),
            (2, "127.0.0.1:19202".parse().unwrap()),
        ];

        let manager = PeerManager::new(0, DiscoveryMode::Explicit(peers));

        assert_eq!(manager.party_id(), 0);
        assert_eq!(manager.total_peer_count().await, 2);
    }

    #[tokio::test]
    async fn test_add_peer() {
        let manager = PeerManager::new(0, DiscoveryMode::Explicit(vec![]));

        assert_eq!(manager.total_peer_count().await, 0);

        manager.add_peer(1, "127.0.0.1:19201".parse().unwrap()).await;
        assert_eq!(manager.total_peer_count().await, 1);

        let peer = manager.get_peer(1).await;
        assert!(peer.is_some());
        assert_eq!(peer.unwrap().state, PeerState::Discovered);
    }
}
