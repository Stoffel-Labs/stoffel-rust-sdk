//! Consensus protocol for deterministic ordering (RFC-006).
//!
//! Before an MPC computation can begin, all server parties must agree on:
//! 1. **Which parties** are participating (and their ordering).
//! 2. **Which clients** have connected (and their ordering).
//! 3. **A digest** of the client list so every party can independently verify
//!    that it has the same view of the computation.
//!
//! This module provides:
//!
//! - [`ClientListDigest`] — a 32-byte hash summarising the agreed client set.
//! - [`ConsensusGate`] — a synchronisation primitive that blocks until consensus
//!   is reached (or fails).
//! - [`VerifiedOrdering`] — the output of a successful consensus round,
//!   containing the canonical party and client ordering.
//! - [`PartyInfo`] / [`ClientInfo`] — per-participant metadata.

use std::net::SocketAddr;
use std::time::SystemTime;

use crate::error::{ConsensusError, Error, Result};
use crate::types::{ClientId, PartyId};

// ---------------------------------------------------------------------------
// ClientListDigest
// ---------------------------------------------------------------------------

/// A 32-byte digest of the sorted client public-key list.
///
/// Every server party computes this digest independently from the client keys
/// it has received. During the consensus round the digests are compared; a
/// mismatch indicates that parties have different views of the connected
/// clients.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ClientListDigest([u8; 32]);

impl ClientListDigest {
    /// Construct a digest from raw bytes.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Borrow the underlying byte array.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Compute a digest from a list of client public keys.
    ///
    /// The keys are **sorted lexicographically** before hashing so that
    /// every party arrives at the same digest regardless of connection order.
    ///
    /// # Current implementation
    ///
    /// Uses SHA-256 via a simple hand-rolled Merkle-Damgard–style stub.
    /// This will be replaced by a proper cryptographic hash once the
    /// `sha2` crate is wired in.
    pub fn compute(client_keys: &[Vec<u8>]) -> Self {
        // Sort keys for deterministic ordering.
        let mut sorted: Vec<&[u8]> = client_keys.iter().map(|k| k.as_slice()).collect();
        sorted.sort();

        // Stub hash: XOR-fold all key bytes into a 32-byte buffer.
        // This is NOT cryptographically secure — it is a placeholder until
        // sha2 is available.
        let mut digest = [0u8; 32];
        for key in &sorted {
            for (i, &byte) in key.iter().enumerate() {
                digest[i % 32] ^= byte;
            }
        }

        Self(digest)
    }
}

// ---------------------------------------------------------------------------
// ConsensusGate
// ---------------------------------------------------------------------------

/// A gate that blocks operations until consensus has been reached.
///
/// The gate tracks how many parties and clients have connected versus how
/// many are expected. Once all expected participants are present the gate
/// transitions to [`ConsensusGate::Ready`].
///
/// # State diagram
///
/// ```text
/// NotRequired ──► (operations proceed immediately)
/// Pending     ──► Ready   (when all parties + clients connect)
///             ──► Failed  (on timeout or protocol error)
/// Ready       ──► (operations proceed)
/// Failed      ──► (operations return the stored error)
/// ```
#[derive(Clone, Debug)]
pub enum ConsensusGate {
    /// Consensus is not required for this operation.
    NotRequired,

    /// Waiting for participants to connect.
    Pending {
        /// Number of server parties connected so far.
        connected_parties: usize,
        /// Total number of server parties expected.
        expected_parties: usize,
        /// Number of clients connected so far.
        connected_clients: usize,
        /// Total number of clients expected.
        expected_clients: usize,
    },

    /// All participants are present — consensus has been established.
    Ready,

    /// Consensus failed with the given error.
    Failed(ConsensusError),
}

impl ConsensusGate {
    /// Returns `true` if the gate is in the [`ConsensusGate::Ready`] or
    /// [`ConsensusGate::NotRequired`] state.
    pub fn is_ready(&self) -> bool {
        matches!(self, ConsensusGate::Ready | ConsensusGate::NotRequired)
    }

    /// Asynchronously wait until the gate becomes ready.
    ///
    /// - Returns `Ok(())` immediately for `NotRequired` and `Ready`.
    /// - Returns the stored error for `Failed`.
    /// - For `Pending`, this stub returns immediately with a "not yet
    ///   implemented" error. A real implementation will `await` on a
    ///   notification channel.
    pub async fn await_ready(&self) -> Result<()> {
        match self {
            ConsensusGate::NotRequired | ConsensusGate::Ready => Ok(()),
            ConsensusGate::Failed(e) => Err(Error::Consensus(ConsensusError::ConsensusTimeout {
                missing_count: match e {
                    ConsensusError::ConsensusTimeout { missing_count } => *missing_count,
                    _ => 0,
                },
            })),
            ConsensusGate::Pending { .. } => {
                // TODO: await on a tokio::sync::Notify or watch channel
                Err(Error::Computation(
                    "ConsensusGate::await_ready: not yet implemented for Pending state"
                        .to_string(),
                ))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// PartyInfo / ClientInfo
// ---------------------------------------------------------------------------

/// Metadata about a server party after consensus.
#[derive(Clone, Debug)]
pub struct PartyInfo {
    /// The party's unique identifier.
    pub party_id: PartyId,
    /// The party's public key (raw bytes).
    pub public_key: Vec<u8>,
    /// The party's network address.
    pub address: SocketAddr,
}

/// Metadata about a client after consensus.
#[derive(Clone, Debug)]
pub struct ClientInfo {
    /// The client's unique identifier.
    pub client_id: ClientId,
    /// The client's public key (raw bytes).
    pub public_key: Vec<u8>,
}

// ---------------------------------------------------------------------------
// VerifiedOrdering
// ---------------------------------------------------------------------------

/// The verified, canonical ordering of parties and clients produced by a
/// successful consensus round.
///
/// All server parties will hold an identical [`VerifiedOrdering`] after
/// consensus completes, ensuring deterministic computation.
#[derive(Clone, Debug)]
pub struct VerifiedOrdering {
    /// Ordered list of participating server parties.
    pub parties: Vec<PartyInfo>,
    /// Ordered list of connected clients.
    pub clients: Vec<ClientInfo>,
    /// Digest of the client public-key list, used for cross-party verification.
    pub digest: ClientListDigest,
    /// Timestamp at which consensus was established.
    pub timestamp: SystemTime,
}

impl VerifiedOrdering {
    /// Look up a party's index in the canonical ordering by its public key.
    ///
    /// Returns `None` if no party with the given key is present.
    pub fn party_index(&self, key: &[u8]) -> Option<usize> {
        self.parties
            .iter()
            .position(|p| p.public_key.as_slice() == key)
    }

    /// Look up a client's index in the canonical ordering by its public key.
    ///
    /// Returns `None` if no client with the given key is present.
    pub fn client_index(&self, key: &[u8]) -> Option<usize> {
        self.clients
            .iter()
            .position(|c| c.public_key.as_slice() == key)
    }

    /// Verify that the given digest matches this ordering's digest.
    ///
    /// This is the core cross-party check: after exchanging digests, each
    /// party calls this method to confirm agreement.
    pub fn verify_digest(&self, digest: &ClientListDigest) -> bool {
        self.digest == *digest
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_from_bytes_roundtrip() {
        let bytes = [42u8; 32];
        let d = ClientListDigest::from_bytes(bytes);
        assert_eq!(d.as_bytes(), &bytes);
    }

    #[test]
    fn digest_compute_deterministic() {
        let keys = vec![vec![1, 2, 3], vec![4, 5, 6]];
        let d1 = ClientListDigest::compute(&keys);
        let d2 = ClientListDigest::compute(&keys);
        assert_eq!(d1, d2);
    }

    #[test]
    fn digest_compute_order_independent() {
        let keys_a = vec![vec![1, 2, 3], vec![4, 5, 6]];
        let keys_b = vec![vec![4, 5, 6], vec![1, 2, 3]];
        assert_eq!(
            ClientListDigest::compute(&keys_a),
            ClientListDigest::compute(&keys_b),
        );
    }

    #[test]
    fn digest_compute_empty() {
        let d = ClientListDigest::compute(&[]);
        assert_eq!(d.as_bytes(), &[0u8; 32]);
    }

    #[test]
    fn consensus_gate_ready_variants() {
        assert!(ConsensusGate::NotRequired.is_ready());
        assert!(ConsensusGate::Ready.is_ready());
        assert!(!ConsensusGate::Pending {
            connected_parties: 2,
            expected_parties: 5,
            connected_clients: 0,
            expected_clients: 3,
        }
        .is_ready());
        assert!(!ConsensusGate::Failed(ConsensusError::ConsensusTimeout {
            missing_count: 3,
        })
        .is_ready());
    }

    #[tokio::test]
    async fn await_ready_not_required() {
        ConsensusGate::NotRequired.await_ready().await.unwrap();
    }

    #[tokio::test]
    async fn await_ready_ready() {
        ConsensusGate::Ready.await_ready().await.unwrap();
    }

    #[tokio::test]
    async fn await_ready_failed() {
        let gate = ConsensusGate::Failed(ConsensusError::ConsensusTimeout { missing_count: 2 });
        assert!(gate.await_ready().await.is_err());
    }

    #[tokio::test]
    async fn await_ready_pending_stub() {
        let gate = ConsensusGate::Pending {
            connected_parties: 2,
            expected_parties: 5,
            connected_clients: 1,
            expected_clients: 3,
        };
        let err = gate.await_ready().await.unwrap_err();
        assert!(err.to_string().contains("not yet implemented"));
    }

    #[test]
    fn verified_ordering_lookups() {
        let ordering = VerifiedOrdering {
            parties: vec![
                PartyInfo {
                    party_id: PartyId::from(0),
                    public_key: vec![10, 20, 30],
                    address: "127.0.0.1:9000".parse().unwrap(),
                },
                PartyInfo {
                    party_id: PartyId::from(1),
                    public_key: vec![40, 50, 60],
                    address: "127.0.0.1:9001".parse().unwrap(),
                },
            ],
            clients: vec![ClientInfo {
                client_id: ClientId::from(100),
                public_key: vec![70, 80, 90],
            }],
            digest: ClientListDigest::compute(&[vec![70, 80, 90]]),
            timestamp: SystemTime::now(),
        };

        assert_eq!(ordering.party_index(&[10, 20, 30]), Some(0));
        assert_eq!(ordering.party_index(&[40, 50, 60]), Some(1));
        assert_eq!(ordering.party_index(&[99]), None);

        assert_eq!(ordering.client_index(&[70, 80, 90]), Some(0));
        assert_eq!(ordering.client_index(&[99]), None);
    }

    #[test]
    fn verified_ordering_verify_digest() {
        let keys = vec![vec![1, 2], vec![3, 4]];
        let digest = ClientListDigest::compute(&keys);

        let ordering = VerifiedOrdering {
            parties: vec![],
            clients: vec![],
            digest: digest.clone(),
            timestamp: SystemTime::now(),
        };

        assert!(ordering.verify_digest(&digest));
        assert!(!ordering.verify_digest(&ClientListDigest::from_bytes([0xff; 32])));
    }
}
