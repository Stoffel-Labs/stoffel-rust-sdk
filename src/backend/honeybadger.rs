//! HoneyBadger MPC engine implementation (RFC-004).
//!
//! This module provides [`HoneyBadgerEngine`], a Byzantine fault-tolerant MPC
//! engine based on the HoneyBadger protocol. It uses robust secret sharing
//! with Reed-Solomon error correction and Beaver triples for secure
//! multiplication.
//!
//! # Protocol properties
//!
//! - **Fault tolerance**: tolerates up to `t` Byzantine parties where
//!   `n >= 3t + 1`.
//! - **Communication model**: fully asynchronous (no timing assumptions).
//! - **Secret sharing**: [`RobustShare`] with Reed-Solomon codewords.
//! - **Multiplication**: Beaver-triple-based protocol.
//!
//! # Current status
//!
//! All trait methods are stubs that establish correct type signatures. Real
//! protocol logic will be wired in once the networking layer is integrated.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use super::{
    MpcEngine, MpcEngineAsync, PartyId, RobustShare, ShareHandle, ShareType,
};
use crate::error::Error;

// ---------------------------------------------------------------------------
// HoneyBadgerEngine
// ---------------------------------------------------------------------------

/// HoneyBadger MPC engine.
///
/// Manages secret shares, preprocessing material (Beaver triples and random
/// shares), and provides the arithmetic interface required by [`MpcEngine`]
/// and [`MpcEngineAsync`].
///
/// # Construction
///
/// ```
/// use stoffel_rust_sdk::backend::honeybadger::HoneyBadgerEngine;
/// use stoffel_rust_sdk::backend::PartyId;
///
/// let engine = HoneyBadgerEngine::new(PartyId(0), 5, 1);
/// assert_eq!(engine.triples_remaining(), 0);
/// ```
pub struct HoneyBadgerEngine {
    /// This party's identifier.
    party_id: PartyId,
    /// Total number of parties in the MPC network.
    parties: usize,
    /// Byzantine fault-tolerance threshold.
    threshold: usize,
    /// Number of unused Beaver triples available for multiplication.
    triples_remaining: AtomicUsize,
    /// Number of unused random shares available.
    random_shares_remaining: AtomicUsize,
    /// Monotonically increasing handle counter for share allocation.
    next_handle: AtomicU64,
}

impl HoneyBadgerEngine {
    /// Create a new HoneyBadger engine.
    ///
    /// # Arguments
    ///
    /// * `party_id` - This party's identifier in the MPC network.
    /// * `parties` - Total number of parties (`n`).
    /// * `threshold` - Maximum number of Byzantine faults tolerated (`t`).
    ///
    /// # Panics
    ///
    /// Does **not** panic; validation of `n >= 3t + 1` is expected to be
    /// performed at a higher layer (e.g., the `Stoffel` builder).
    pub fn new(party_id: PartyId, parties: usize, threshold: usize) -> Self {
        Self {
            party_id,
            parties,
            threshold,
            triples_remaining: AtomicUsize::new(0),
            random_shares_remaining: AtomicUsize::new(0),
            next_handle: AtomicU64::new(1),
        }
    }

    /// Return the number of unused Beaver triples.
    pub fn triples_remaining(&self) -> usize {
        self.triples_remaining.load(Ordering::Relaxed)
    }

    /// Return the number of unused random shares.
    pub fn random_shares_remaining(&self) -> usize {
        self.random_shares_remaining.load(Ordering::Relaxed)
    }

    /// Return this engine's party identifier.
    pub fn party_id(&self) -> PartyId {
        self.party_id
    }

    /// Return the total number of parties.
    pub fn parties(&self) -> usize {
        self.parties
    }

    /// Return the fault-tolerance threshold.
    pub fn threshold(&self) -> usize {
        self.threshold
    }

    /// Allocate a fresh [`ShareHandle`].
    fn alloc_handle(&self) -> ShareHandle {
        ShareHandle(self.next_handle.fetch_add(1, Ordering::Relaxed))
    }
}

// ---------------------------------------------------------------------------
// MpcEngine implementation
// ---------------------------------------------------------------------------

impl MpcEngine for HoneyBadgerEngine {
    fn input_share(&self, share: ShareType) -> crate::Result<ShareHandle> {
        match share {
            ShareType::Robust(RobustShare { .. }) => Ok(self.alloc_handle()),
            ShareType::Feldman(_) => Err(Error::Computation(
                "HoneyBadgerEngine does not accept Feldman shares; use RobustShare".into(),
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
        let remaining = self.random_shares_remaining.load(Ordering::Relaxed);
        if remaining == 0 {
            return Err(Error::Preprocessing(
                "no random shares remaining; run preprocessing first".into(),
            ));
        }
        self.random_shares_remaining.fetch_sub(1, Ordering::Relaxed);
        Ok(self.alloc_handle())
    }
}

// ---------------------------------------------------------------------------
// MpcEngineAsync implementation
// ---------------------------------------------------------------------------

impl MpcEngineAsync for HoneyBadgerEngine {
    fn open_share(
        &self,
        _handle: ShareHandle,
    ) -> Pin<Box<dyn Future<Output = crate::Result<Vec<u8>>> + Send + '_>> {
        Box::pin(async {
            // TODO: broadcast partial opening, collect from peers, reconstruct
            Err(Error::Computation(
                "HoneyBadgerEngine::open_share not yet implemented".into(),
            ))
        })
    }

    fn multiply_share(
        &self,
        _a: ShareHandle,
        _b: ShareHandle,
    ) -> Pin<Box<dyn Future<Output = crate::Result<ShareHandle>> + Send + '_>> {
        let triples = self.triples_remaining.load(Ordering::Relaxed);
        Box::pin(async move {
            if triples == 0 {
                return Err(Error::Preprocessing(
                    "no Beaver triples remaining; run preprocessing first".into(),
                ));
            }
            // TODO: consume a triple, run the multiplication sub-protocol
            Err(Error::Computation(
                "HoneyBadgerEngine::multiply_share not yet implemented".into(),
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

    #[test]
    fn test_engine_construction() {
        let engine = HoneyBadgerEngine::new(PartyId(0), 5, 1);
        assert_eq!(engine.party_id(), PartyId(0));
        assert_eq!(engine.parties(), 5);
        assert_eq!(engine.threshold(), 1);
        assert_eq!(engine.triples_remaining(), 0);
        assert_eq!(engine.random_shares_remaining(), 0);
    }

    #[test]
    fn test_input_robust_share_ok() {
        let engine = HoneyBadgerEngine::new(PartyId(0), 5, 1);
        let share = ShareType::Robust(RobustShare {
            value: vec![1, 2, 3],
            codeword: vec![4, 5, 6],
        });
        let handle = engine.input_share(share);
        assert!(handle.is_ok());
    }

    #[test]
    fn test_input_feldman_share_rejected() {
        use super::super::{FeldmanCommitment, FeldmanShare};

        let engine = HoneyBadgerEngine::new(PartyId(0), 5, 1);
        let share = ShareType::Feldman(FeldmanShare {
            value: vec![1],
            commitment: FeldmanCommitment {
                points: vec![vec![2]],
            },
            index: 0,
        });
        let result = engine.input_share(share);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_sub_scalar_produce_handles() {
        let engine = HoneyBadgerEngine::new(PartyId(0), 5, 1);
        let a = ShareHandle(1);
        let b = ShareHandle(2);

        let c = engine.add_share(a, b);
        let d = engine.sub_share(a, b);
        let e = engine.scalar_mul(a, vec![42]);

        // All returned handles should be distinct
        assert_ne!(c, d);
        assert_ne!(d, e);
    }

    #[test]
    fn test_random_share_fails_when_empty() {
        let engine = HoneyBadgerEngine::new(PartyId(0), 5, 1);
        let result = engine.random_share();
        assert!(result.is_err());
    }

    #[test]
    fn test_random_share_succeeds_with_stock() {
        let engine = HoneyBadgerEngine::new(PartyId(0), 5, 1);
        engine
            .random_shares_remaining
            .store(3, Ordering::Relaxed);
        assert!(engine.random_share().is_ok());
        assert_eq!(engine.random_shares_remaining(), 2);
    }

    #[test]
    fn test_handles_are_unique() {
        let engine = HoneyBadgerEngine::new(PartyId(0), 5, 1);
        let h1 = engine.alloc_handle();
        let h2 = engine.alloc_handle();
        let h3 = engine.alloc_handle();
        assert_ne!(h1, h2);
        assert_ne!(h2, h3);
    }
}
