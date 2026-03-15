//! MPC Backend trait and implementations (RFC-004, RFC-005).
//!
//! This module provides the [`MpcBackend`] enum for protocol selection,
//! the [`MpcEngine`] and [`MpcEngineAsync`] traits that all backend
//! implementations must satisfy, and concrete implementations for
//! HoneyBadger ([`honeybadger::HoneyBadgerEngine`]) and AVSS
//! ([`avss::AvssEngine`]).
//!
//! # Architecture
//!
//! ```text
//! MpcEngine (sync operations: add, sub, scalar_mul, input, random)
//!     |
//!     v
//! MpcEngineAsync (async operations: open, multiply -- require network)
//!     |
//!     +-- HoneyBadgerEngine  (RFC-004)
//!     +-- AvssEngine          (RFC-005)
//! ```
//!
//! All engine implementations are currently stubs that establish the correct
//! type signatures and trait structure. Real protocol logic will be wired in
//! once the networking and preprocessing layers are integrated.

pub mod avss;
pub mod honeybadger;

use std::future::Future;
use std::pin::Pin;

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
/// engine's internal state. They are produced by [`MpcEngine::input_share`]
/// and consumed by arithmetic operations and [`MpcEngineAsync::open_share`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShareHandle(pub u64);

// ---------------------------------------------------------------------------
// MpcEngine trait (synchronous)
// ---------------------------------------------------------------------------

/// Trait for synchronous MPC protocol engine operations.
///
/// Implementors provide local arithmetic on secret shares. These operations
/// do not require network communication and can be performed immediately.
///
/// See also [`MpcEngineAsync`] for operations that require communication.
pub trait MpcEngine: Send + Sync {
    /// Import an external share into the engine, returning a handle.
    ///
    /// # Errors
    ///
    /// Returns an error if the share format is incompatible with this engine.
    fn input_share(&self, share: ShareType) -> crate::Result<ShareHandle>;

    /// Compute the sum of two secret-shared values.
    ///
    /// This is a local operation (no communication required).
    fn add_share(&self, a: ShareHandle, b: ShareHandle) -> ShareHandle;

    /// Compute the difference of two secret-shared values.
    ///
    /// This is a local operation (no communication required).
    fn sub_share(&self, a: ShareHandle, b: ShareHandle) -> ShareHandle;

    /// Multiply a secret-shared value by a public scalar.
    ///
    /// The `scalar` is a serialized field element. This is a local operation.
    fn scalar_mul(&self, share: ShareHandle, scalar: Vec<u8>) -> ShareHandle;

    /// Generate a fresh random secret share.
    ///
    /// Consumes one preprocessed random share from the engine's pool.
    ///
    /// # Errors
    ///
    /// Returns an error if no random shares are available.
    fn random_share(&self) -> crate::Result<ShareHandle>;
}

/// Trait for asynchronous MPC engine operations that require network
/// communication.
///
/// These operations involve inter-party communication (e.g., opening a
/// share requires collecting partial openings from all parties, and
/// multiplication requires a Beaver-triple-based protocol round).
///
/// # Note on async
///
/// Since `async_trait` is not available in this crate, methods return
/// boxed futures. A future version may switch to native `async fn in trait`
/// once stabilized.
pub trait MpcEngineAsync: MpcEngine {
    /// Open (reconstruct) a secret-shared value, revealing the plaintext.
    ///
    /// All parties must participate in this operation. Returns the serialized
    /// field element.
    ///
    /// # Errors
    ///
    /// Returns an error if reconstruction fails (e.g., insufficient shares
    /// or network timeout).
    fn open_share(
        &self,
        handle: ShareHandle,
    ) -> Pin<Box<dyn Future<Output = crate::Result<Vec<u8>>> + Send + '_>>;

    /// Multiply two secret-shared values using a Beaver triple.
    ///
    /// Requires one round of communication to execute the multiplication
    /// sub-protocol. Consumes one preprocessed Beaver triple.
    ///
    /// # Errors
    ///
    /// Returns an error if no triples are available or communication fails.
    fn multiply_share(
        &self,
        a: ShareHandle,
        b: ShareHandle,
    ) -> Pin<Box<dyn Future<Output = crate::Result<ShareHandle>> + Send + '_>>;
}

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
