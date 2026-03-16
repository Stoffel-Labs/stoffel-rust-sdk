//! MPC Backend selection and engine re-exports (RFC-004, RFC-005).
//!
//! This module provides:
//! - [`MpcBackend`] enum for protocol selection (HoneyBadger or AVSS)
//! - Re-exported [`MpcEngine`] trait from StoffelVM for engine operations
//! - Re-exported concrete engines: [`HoneyBadgerMpcEngine`] and [`AvssMpcEngine`]
//! - SDK-level share types ([`ShareType`], [`RobustShare`], [`FeldmanShare`])
//!
//! # Architecture
//!
//! The real engine trait and implementations live in StoffelVM's `net` module.
//! The SDK re-exports them and provides the `MpcBackend` enum as the
//! user-facing protocol selector.
//!
//! ```text
//! MpcEngine (from StoffelVM — sync + async operations)
//!     |
//!     +-- HoneyBadgerMpcEngine  (from StoffelVM, RFC-004)
//!     +-- AvssMpcEngine          (from StoffelVM, RFC-005)
//! ```

pub mod avss;
pub mod honeybadger;

// ---------------------------------------------------------------------------
// Re-exports from canonical locations
// ---------------------------------------------------------------------------

/// Party identifier for MPC compute nodes.
pub use crate::types::PartyId;

/// Elliptic curve selection for MPC backends that require one.
pub use crate::config::Curve;

// ---------------------------------------------------------------------------
// MpcBackend enum
// ---------------------------------------------------------------------------

/// MPC protocol backend selection.
///
/// Determines which protocol engine is instantiated at runtime. The default
/// backend is [`MpcBackend::HoneyBadger`], which provides Byzantine fault
/// tolerance without requiring any curve-specific configuration.
///
/// # Examples
///
/// ```
/// use stoffel_rust_sdk::backend::MpcBackend;
///
/// let backend = MpcBackend::default();
/// assert!(matches!(backend, MpcBackend::HoneyBadger));
/// ```
#[derive(Clone, Debug, Default)]
pub enum MpcBackend {
    /// HoneyBadger Byzantine fault-tolerant protocol (default).
    #[default]
    HoneyBadger,
    /// Asynchronous Verifiable Secret Sharing protocol.
    Avss {
        /// The elliptic curve to use for AVSS operations.
        curve: Curve,
    },
}

// ---------------------------------------------------------------------------
// Share types
// ---------------------------------------------------------------------------

/// Type of secret share produced or consumed by an MPC engine.
///
/// Variants correspond to different secret sharing schemes with distinct
/// security and performance characteristics.
#[derive(Clone, Debug)]
pub enum ShareType {
    /// A robust share with Reed-Solomon error correction, suitable for
    /// Byzantine fault-tolerant protocols like HoneyBadger.
    Robust(RobustShare),
    /// A Feldman VSS share with polynomial commitments, used by the AVSS
    /// backend.
    Feldman(FeldmanShare),
}

/// A robust secret share with error-correcting codeword.
///
/// Used by the HoneyBadger engine. The `codeword` field carries the
/// Reed-Solomon encoding that allows reconstruction even when some
/// shares are corrupted.
#[derive(Clone, Debug)]
pub struct RobustShare {
    /// Serialized field element representing the share value.
    pub value: Vec<u8>,
    /// Reed-Solomon codeword for error correction during reconstruction.
    pub codeword: Vec<u8>,
}

/// A Feldman VSS secret share with polynomial commitment.
///
/// Used by the AVSS engine. The commitment allows any party to verify
/// that the share is consistent with the dealer's polynomial without
/// learning the secret.
#[derive(Clone, Debug)]
pub struct FeldmanShare {
    /// Serialized field element representing the share value.
    pub value: Vec<u8>,
    /// Polynomial commitment that binds the dealer to the sharing polynomial.
    pub commitment: FeldmanCommitment,
    /// Zero-based index of this share (i.e., the evaluation point).
    pub index: usize,
}

impl FeldmanShare {
    /// Verify this share against its commitment.
    ///
    /// Currently a stub that always returns `true`. A real implementation
    /// would check `g^{share_value} == commitment.evaluate(index)`.
    pub fn verify(&self) -> bool {
        // TODO: implement actual Feldman verification
        true
    }
}

/// A Feldman polynomial commitment.
///
/// Stores serialized group elements `[g^{a_0}, g^{a_1}, ..., g^{a_t}]`
/// where `a_i` are the coefficients of the sharing polynomial and `g` is
/// the group generator.
#[derive(Clone, Debug)]
pub struct FeldmanCommitment {
    /// Serialized group elements forming the commitment.
    pub points: Vec<Vec<u8>>,
}

impl FeldmanCommitment {
    /// Return the public key (first commitment point, `g^{a_0}`).
    ///
    /// Returns `None` if the commitment has no points.
    pub fn public_key(&self) -> Option<&Vec<u8>> {
        self.points.first()
    }
}

// ---------------------------------------------------------------------------
// ShareHandle
// ---------------------------------------------------------------------------

/// Opaque handle to a share held inside an MPC engine.
///
/// Handles are lightweight identifiers that refer to shares stored in the
/// engine's internal state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShareHandle(pub u64);

// ---------------------------------------------------------------------------
// Re-exports from StoffelVM (real engine trait and implementations)
// ---------------------------------------------------------------------------

/// The real MPC engine trait from StoffelVM.
///
/// Provides sync operations (input, add, sub, scalar_mul, random, open, multiply)
/// and async operations for network communication.
pub use stoffel_vm::net::mpc_engine::MpcEngine;

/// MPC engine with consensus support.
pub use stoffel_vm::net::mpc_engine::MpcEngineConsensus;

/// MPC engine with client input hydration support.
pub use stoffel_vm::net::mpc_engine::MpcEngineClientOps;

/// The MPC runner that wraps a VM + engine for execution.
pub use stoffel_vm::net::mpc_runner::{MpcRunner, MpcRunnerConfig};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mpc_backend_default_is_honeybadger() {
        let backend = MpcBackend::default();
        assert!(matches!(backend, MpcBackend::HoneyBadger));
    }

    #[test]
    fn test_mpc_backend_avss_with_curve() {
        let backend = MpcBackend::Avss {
            curve: Curve::Bn254,
        };
        match backend {
            MpcBackend::Avss { curve } => assert_eq!(curve, Curve::Bn254),
            _ => panic!("expected Avss variant"),
        }
    }

    #[test]
    fn test_share_handle_equality() {
        let a = ShareHandle(42);
        let b = ShareHandle(42);
        let c = ShareHandle(99);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_robust_share_construction() {
        let share = RobustShare {
            value: vec![1, 2, 3],
            codeword: vec![4, 5, 6],
        };
        assert_eq!(share.value, vec![1, 2, 3]);
        assert_eq!(share.codeword, vec![4, 5, 6]);
    }

    #[test]
    fn test_feldman_share_verify_stub() {
        let share = FeldmanShare {
            value: vec![10],
            commitment: FeldmanCommitment {
                points: vec![vec![20], vec![30]],
            },
            index: 0,
        };
        // Stub always returns true
        assert!(share.verify());
    }

    #[test]
    fn test_feldman_commitment_public_key() {
        let commitment = FeldmanCommitment {
            points: vec![vec![1, 2], vec![3, 4]],
        };
        assert_eq!(commitment.public_key(), Some(&vec![1, 2]));

        let empty = FeldmanCommitment { points: vec![] };
        assert_eq!(empty.public_key(), None);
    }

    #[test]
    fn test_party_id_conversions() {
        let pid = PartyId::from(5usize);
        assert_eq!(pid.0, 5);
        let back: usize = pid.into();
        assert_eq!(back, 5);
    }

    #[test]
    fn test_curve_default() {
        assert_eq!(Curve::default(), Curve::Bls12_381);
    }

    #[test]
    fn test_share_type_variants() {
        let robust = ShareType::Robust(RobustShare {
            value: vec![1],
            codeword: vec![2],
        });
        assert!(matches!(robust, ShareType::Robust(_)));

        let feldman = ShareType::Feldman(FeldmanShare {
            value: vec![3],
            commitment: FeldmanCommitment {
                points: vec![vec![4]],
            },
            index: 0,
        });
        assert!(matches!(feldman, ShareType::Feldman(_)));
    }
}
