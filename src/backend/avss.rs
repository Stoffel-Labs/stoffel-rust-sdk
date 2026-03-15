//! Asynchronous Verifiable Secret Sharing (AVSS) engine (RFC-005).
//!
//! This module provides [`AvssEngine`], an MPC engine backed by Feldman VSS
//! commitments. It is designed for scenarios where verifiability of the
//! sharing polynomial is required (e.g., distributed key generation).
//!
//! # Key management
//!
//! The AVSS engine includes a [`KeyStore`] that persists Feldman shares and
//! associated public keys across protocol rounds.
//!
//! # Current status
//!
//! All trait methods are stubs. Real AVSS protocol logic (dealing, verification,
//! reconstruction) will be integrated in a future iteration.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    Curve, FeldmanShare, MpcEngine, MpcEngineAsync, PartyId, RobustShare,
    ShareHandle, ShareType,
};
use crate::error::Error;

// ---------------------------------------------------------------------------
// KeyStore
// ---------------------------------------------------------------------------

/// Persistent store for Feldman VSS shares and public keys.
///
/// The key store maps human-readable names (e.g., `"dkg_round_1"`) to
/// [`FeldmanShare`] values and their associated public keys. This allows
/// the AVSS engine to retain shares across multiple protocol rounds.
///
/// # Examples
///
/// ```
/// use stoffel_rust_sdk::backend::avss::KeyStore;
/// use stoffel_rust_sdk::backend::{FeldmanShare, FeldmanCommitment};
///
/// let mut store = KeyStore::new();
/// store.store("my_key".into(), FeldmanShare {
///     value: vec![1, 2, 3],
///     commitment: FeldmanCommitment { points: vec![vec![4, 5]] },
///     index: 0,
/// });
///
/// assert!(store.has_key("my_key"));
/// assert_eq!(store.list_keys(), vec!["my_key".to_string()]);
/// ```
pub struct KeyStore {
    /// Named Feldman shares.
    shares: HashMap<String, FeldmanShare>,
    /// Named public keys (serialized group elements).
    public_keys: HashMap<String, Vec<u8>>,
}

impl KeyStore {
    /// Create an empty key store.
    pub fn new() -> Self {
        Self {
            shares: HashMap::new(),
            public_keys: HashMap::new(),
        }
    }

    /// Store a Feldman share under the given name.
    ///
    /// If a public key can be derived from the share's commitment, it is
    /// automatically stored alongside the share.
    pub fn store(&mut self, name: String, share: FeldmanShare) {
        if let Some(pk) = share.commitment.public_key() {
            self.public_keys.insert(name.clone(), pk.clone());
        }
        self.shares.insert(name, share);
    }

    /// Retrieve a reference to the share stored under `name`.
    pub fn get(&self, name: &str) -> Option<&FeldmanShare> {
        self.shares.get(name)
    }

    /// Retrieve the public key associated with `name`.
    pub fn public_key(&self, name: &str) -> Option<&Vec<u8>> {
        self.public_keys.get(name)
    }

    /// Remove and return the share stored under `name`.
    pub fn remove(&mut self, name: &str) -> Option<FeldmanShare> {
        self.public_keys.remove(name);
        self.shares.remove(name)
    }

    /// Check whether a share with the given name exists.
    pub fn has_key(&self, name: &str) -> bool {
        self.shares.contains_key(name)
    }

    /// List all stored key names, sorted alphabetically.
    pub fn list_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.shares.keys().cloned().collect();
        keys.sort();
        keys
    }
}

impl Default for KeyStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AvssEngine
// ---------------------------------------------------------------------------

/// AVSS MPC engine.
///
/// Provides Feldman-commitment-based verifiable secret sharing backed by a
/// configurable elliptic curve. Shares and public keys are persisted in an
/// internal [`KeyStore`].
///
/// # Construction
///
/// ```
/// use stoffel_rust_sdk::backend::avss::AvssEngine;
/// use stoffel_rust_sdk::backend::{Curve, PartyId};
///
/// let engine = AvssEngine::new(PartyId(0), 5, 1, Curve::Bls12_381);
/// assert_eq!(engine.curve(), Curve::Bls12_381);
/// ```
pub struct AvssEngine {
    /// This party's identifier.
    party_id: PartyId,
    /// Total number of parties in the MPC network.
    parties: usize,
    /// Sharing threshold (degree of the polynomial).
    threshold: usize,
    /// Elliptic curve used for commitments and group operations.
    curve: Curve,
    /// Persistent key/share storage.
    key_store: KeyStore,
    /// Monotonically increasing handle counter.
    next_handle: AtomicU64,
}

impl AvssEngine {
    /// Create a new AVSS engine.
    ///
    /// # Arguments
    ///
    /// * `party_id` - This party's identifier.
    /// * `parties` - Total number of parties.
    /// * `threshold` - Sharing threshold (polynomial degree).
    /// * `curve` - Elliptic curve for group operations.
    pub fn new(party_id: PartyId, parties: usize, threshold: usize, curve: Curve) -> Self {
        Self {
            party_id,
            parties,
            threshold,
            curve,
            key_store: KeyStore::new(),
            next_handle: AtomicU64::new(1),
        }
    }

    /// Return the elliptic curve used by this engine.
    pub fn curve(&self) -> Curve {
        self.curve
    }

    /// Return this engine's party identifier.
    pub fn party_id(&self) -> PartyId {
        self.party_id
    }

    /// Return the total number of parties.
    pub fn parties(&self) -> usize {
        self.parties
    }

    /// Return the sharing threshold.
    pub fn threshold(&self) -> usize {
        self.threshold
    }

    /// Retrieve a reference to a stored Feldman share by key name.
    ///
    /// # Errors
    ///
    /// Returns `Error::Computation` if no share with the given name exists.
    pub fn get_share(&self, key_name: &str) -> crate::Result<&FeldmanShare> {
        self.key_store.get(key_name).ok_or_else(|| {
            Error::Computation(format!("no share found for key '{}'", key_name))
        })
    }

    /// Check whether the key store contains a share with the given name.
    pub fn has_key(&self, key_name: &str) -> bool {
        self.key_store.has_key(key_name)
    }

    /// List all key names in the key store.
    pub fn list_keys(&self) -> Vec<String> {
        self.key_store.list_keys()
    }

    /// Get a mutable reference to the internal key store.
    ///
    /// This is useful for storing shares produced by external AVSS dealing
    /// rounds.
    pub fn key_store_mut(&mut self) -> &mut KeyStore {
        &mut self.key_store
    }

    /// Allocate a fresh [`ShareHandle`].
    fn alloc_handle(&self) -> ShareHandle {
        ShareHandle(self.next_handle.fetch_add(1, Ordering::Relaxed))
    }
}

// ---------------------------------------------------------------------------
// MpcEngine implementation
// ---------------------------------------------------------------------------

impl MpcEngine for AvssEngine {
    fn input_share(&self, share: ShareType) -> crate::Result<ShareHandle> {
        match share {
            ShareType::Feldman(fs) => {
                if !fs.verify() {
                    return Err(Error::Computation(
                        "Feldman share verification failed".into(),
                    ));
                }
                Ok(self.alloc_handle())
            }
            ShareType::Robust(RobustShare { .. }) => Err(Error::Computation(
                "AvssEngine does not accept RobustShares; use FeldmanShare".into(),
            )),
        }
    }

    fn add_share(&self, _a: ShareHandle, _b: ShareHandle) -> ShareHandle {
        // TODO: look up shares, compute element-wise addition
        self.alloc_handle()
    }

    fn sub_share(&self, _a: ShareHandle, _b: ShareHandle) -> ShareHandle {
        // TODO: look up shares, compute element-wise subtraction
        self.alloc_handle()
    }

    fn scalar_mul(&self, _share: ShareHandle, _scalar: Vec<u8>) -> ShareHandle {
        // TODO: multiply the share value by the public scalar
        self.alloc_handle()
    }

    fn random_share(&self) -> crate::Result<ShareHandle> {
        // TODO: run AVSS dealing round to produce a fresh random share
        Err(Error::Computation(
            "AvssEngine::random_share not yet implemented".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// MpcEngineAsync implementation
// ---------------------------------------------------------------------------

impl MpcEngineAsync for AvssEngine {
    fn open_share(
        &self,
        _handle: ShareHandle,
    ) -> Pin<Box<dyn Future<Output = crate::Result<Vec<u8>>> + Send + '_>> {
        Box::pin(async {
            // TODO: collect partial openings from peers, verify, reconstruct
            Err(Error::Computation(
                "AvssEngine::open_share not yet implemented".into(),
            ))
        })
    }

    fn multiply_share(
        &self,
        _a: ShareHandle,
        _b: ShareHandle,
    ) -> Pin<Box<dyn Future<Output = crate::Result<ShareHandle>> + Send + '_>> {
        Box::pin(async {
            // TODO: use AVSS-based multiplication sub-protocol
            Err(Error::Computation(
                "AvssEngine::multiply_share not yet implemented".into(),
            ))
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::FeldmanCommitment;

    #[test]
    fn test_engine_construction() {
        let engine = AvssEngine::new(PartyId(2), 7, 2, Curve::Bn254);
        assert_eq!(engine.party_id(), PartyId(2));
        assert_eq!(engine.parties(), 7);
        assert_eq!(engine.threshold(), 2);
        assert_eq!(engine.curve(), Curve::Bn254);
    }

    #[test]
    fn test_input_feldman_share_ok() {
        let engine = AvssEngine::new(PartyId(0), 5, 1, Curve::Bls12_381);
        let share = ShareType::Feldman(FeldmanShare {
            value: vec![1],
            commitment: FeldmanCommitment {
                points: vec![vec![2]],
            },
            index: 0,
        });
        assert!(engine.input_share(share).is_ok());
    }

    #[test]
    fn test_input_robust_share_rejected() {
        let engine = AvssEngine::new(PartyId(0), 5, 1, Curve::Bls12_381);
        let share = ShareType::Robust(super::super::RobustShare {
            value: vec![1],
            codeword: vec![2],
        });
        assert!(engine.input_share(share).is_err());
    }

    #[test]
    fn test_key_store_lifecycle() {
        let mut engine = AvssEngine::new(PartyId(0), 5, 1, Curve::Bls12_381);
        let share = FeldmanShare {
            value: vec![10, 20],
            commitment: FeldmanCommitment {
                points: vec![vec![30, 40]],
            },
            index: 0,
        };

        engine.key_store_mut().store("test_key".into(), share);
        assert!(engine.has_key("test_key"));
        assert_eq!(engine.list_keys(), vec!["test_key".to_string()]);

        let retrieved = engine.get_share("test_key").unwrap();
        assert_eq!(retrieved.value, vec![10, 20]);
        assert_eq!(retrieved.index, 0);
    }

    #[test]
    fn test_key_store_missing_key_error() {
        let engine = AvssEngine::new(PartyId(0), 5, 1, Curve::Bls12_381);
        let result = engine.get_share("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_key_store_remove() {
        let mut store = KeyStore::new();
        store.store(
            "k1".into(),
            FeldmanShare {
                value: vec![1],
                commitment: FeldmanCommitment {
                    points: vec![vec![2]],
                },
                index: 0,
            },
        );
        assert!(store.has_key("k1"));
        let removed = store.remove("k1");
        assert!(removed.is_some());
        assert!(!store.has_key("k1"));
        assert!(store.public_key("k1").is_none());
    }

    #[test]
    fn test_key_store_public_key_auto_stored() {
        let mut store = KeyStore::new();
        store.store(
            "pk_test".into(),
            FeldmanShare {
                value: vec![1],
                commitment: FeldmanCommitment {
                    points: vec![vec![99, 100]],
                },
                index: 0,
            },
        );
        assert_eq!(store.public_key("pk_test"), Some(&vec![99, 100]));
    }

    #[test]
    fn test_random_share_not_implemented() {
        let engine = AvssEngine::new(PartyId(0), 5, 1, Curve::Bls12_381);
        assert!(engine.random_share().is_err());
    }
}
