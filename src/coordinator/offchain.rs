//! Off-chain coordinator for local testing and development.
//!
//! This module provides:
//! - [`OffChainCoordinator`]: Simple in-memory round state machine for testing
//! - [`RealOffChainCoordinator`]: Re-export of the full `stoffel-mpc-coordinator`
//!   implementation with RPC server, TLS, and input masking protocol
//!
//! # Example
//!
//! ```rust
//! use stoffel_rust_sdk::coordinator::offchain::OffChainCoordinator;
//! use stoffel_rust_sdk::coordinator::Round;
//!
//! let coord = OffChainCoordinator::new();
//! assert_eq!(coord.current_round().unwrap(), Round::Preprocessing);
//!
//! let next = coord.advance_round().unwrap();
//! assert_eq!(next, Round::InputMaskReservation);
//! ```

use std::sync::{Arc, Mutex};

// Re-export the real coordinator from stoffel-mpc-coordinator crate
pub use stoffel_mpc_coordinator::off_chain::OffChainCoordinator as RealOffChainCoordinator;

// Re-export the Coordinator trait for generic usage
pub use stoffel_mpc_coordinator::Coordinator;

use super::{MaskIndex, Round};
use crate::error::{Error, Result};

// ---------------------------------------------------------------------------
// OffChainCoordinator
// ---------------------------------------------------------------------------

/// In-memory coordinator that tracks the round state machine locally.
///
/// Thread-safe: the internal state is protected by a [`Mutex`], so multiple
/// participants (or test threads) can share a single coordinator instance via
/// `Arc<OffChainCoordinator>`.
pub struct OffChainCoordinator {
    round: Arc<Mutex<Round>>,
    next_mask_index: Arc<Mutex<u64>>,
}

impl OffChainCoordinator {
    /// Create a new coordinator starting at [`Round::Preprocessing`].
    pub fn new() -> Self {
        Self {
            round: Arc::new(Mutex::new(Round::Preprocessing)),
            next_mask_index: Arc::new(Mutex::new(0)),
        }
    }

    /// Return the current round.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Computation`] if the internal mutex is poisoned.
    pub fn current_round(&self) -> Result<Round> {
        let guard = self
            .round
            .lock()
            .map_err(|e| Error::Computation(format!("coordinator lock poisoned: {}", e)))?;
        Ok(*guard)
    }

    /// Advance to the next round in the state machine.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Computation`] if the current round is
    /// [`Round::OutputCollection`] (terminal) or the lock is poisoned.
    pub fn advance_round(&self) -> Result<Round> {
        let mut guard = self
            .round
            .lock()
            .map_err(|e| Error::Computation(format!("coordinator lock poisoned: {}", e)))?;

        let next = guard.next().ok_or_else(|| {
            Error::Computation("cannot advance past OutputCollection round".into())
        })?;

        *guard = next;
        Ok(next)
    }

    /// Force-set the current round (useful in tests).
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn set_round(&self, round: Round) {
        let mut guard = self.round.lock().expect("coordinator lock poisoned");
        *guard = round;
    }

    /// Reserve the next available input-mask index.
    ///
    /// Each call returns a monotonically increasing [`MaskIndex`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Computation`] if the lock is poisoned.
    pub fn reserve_input_mask(&self) -> Result<MaskIndex> {
        let mut guard = self
            .next_mask_index
            .lock()
            .map_err(|e| Error::Computation(format!("mask index lock poisoned: {}", e)))?;

        let idx = *guard;
        *guard = idx + 1;
        Ok(MaskIndex(idx))
    }
}

impl Default for OffChainCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_preprocessing() {
        let c = OffChainCoordinator::new();
        assert_eq!(c.current_round().unwrap(), Round::Preprocessing);
    }

    #[test]
    fn advance_all_rounds() {
        let c = OffChainCoordinator::new();
        let expected = [
            Round::InputMaskReservation,
            Round::CollectingInputs,
            Round::InputsCollectionEnd,
            Round::Execution,
            Round::ExecutionEnd,
            Round::OutputCollection,
        ];
        for exp in &expected {
            let r = c.advance_round().unwrap();
            assert_eq!(r, *exp);
        }
        // Terminal round
        assert!(c.advance_round().is_err());
    }

    #[test]
    fn set_round_overrides() {
        let c = OffChainCoordinator::new();
        c.set_round(Round::Execution);
        assert_eq!(c.current_round().unwrap(), Round::Execution);
    }

    #[test]
    fn reserve_mask_increments() {
        let c = OffChainCoordinator::new();
        assert_eq!(c.reserve_input_mask().unwrap(), MaskIndex(0));
        assert_eq!(c.reserve_input_mask().unwrap(), MaskIndex(1));
        assert_eq!(c.reserve_input_mask().unwrap(), MaskIndex(2));
    }

    #[test]
    fn thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let c = Arc::new(OffChainCoordinator::new());
        let mut handles = vec![];

        for _ in 0..4 {
            let coord = Arc::clone(&c);
            handles.push(thread::spawn(move || {
                coord.reserve_input_mask().unwrap()
            }));
        }

        let mut indices: Vec<u64> = handles
            .into_iter()
            .map(|h| h.join().unwrap().0)
            .collect();
        indices.sort();
        assert_eq!(indices, vec![0, 1, 2, 3]);
    }
}
