//! On-chain and off-chain coordination (RFC-007)
//!
//! This module implements the round-based state machine that orchestrates MPC
//! computations. Two concrete coordinators are provided:
//!
//! - [`offchain::OffChainCoordinator`] -- an in-memory coordinator for local
//!   testing and development.
//! - [`onchain::OnChainCoordinator`] -- a placeholder for future on-chain
//!   coordination via the StoffelCoordinator Solidity contract.
//!
//! # Round State Machine
//!
//! The computation progresses through a fixed sequence of rounds that mirrors
//! the on-chain `StoffelCoordinator` contract:
//!
//! ```text
//! Preprocessing
//!     -> InputMaskReservation
//!         -> CollectingInputs
//!             -> InputsCollectionEnd
//!                 -> Execution
//!                     -> ExecutionEnd
//!                         -> OutputCollection
//! ```
//!
//! Each round is represented by the [`Round`] enum. Transitions are validated
//! by [`Round::next`]; skipping rounds is not allowed.

pub mod offchain;
pub mod onchain;

// ---------------------------------------------------------------------------
// Round
// ---------------------------------------------------------------------------

/// MPC coordination round.
///
/// Represents one step in the linear state machine that drives an MPC
/// computation from preprocessing through to output delivery.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Round {
    /// Generate beaver triples, random shares, and other preprocessing material.
    Preprocessing,
    /// Clients reserve input-mask indices from the coordinator.
    InputMaskReservation,
    /// Clients submit their masked inputs.
    CollectingInputs,
    /// All client inputs have been collected; no more submissions accepted.
    InputsCollectionEnd,
    /// Servers execute the MPC protocol on the masked inputs.
    Execution,
    /// Execution has completed; servers submit result shares.
    ExecutionEnd,
    /// Clients reconstruct their outputs from the result shares.
    OutputCollection,
}

impl Round {
    /// Return the next round in the state machine, or `None` if this is the
    /// terminal round ([`OutputCollection`](Round::OutputCollection)).
    pub fn next(&self) -> Option<Round> {
        match self {
            Round::Preprocessing => Some(Round::InputMaskReservation),
            Round::InputMaskReservation => Some(Round::CollectingInputs),
            Round::CollectingInputs => Some(Round::InputsCollectionEnd),
            Round::InputsCollectionEnd => Some(Round::Execution),
            Round::Execution => Some(Round::ExecutionEnd),
            Round::ExecutionEnd => Some(Round::OutputCollection),
            Round::OutputCollection => None,
        }
    }

    /// Returns `true` if clients are expected to act during this round.
    ///
    /// Client rounds are:
    /// - [`InputMaskReservation`](Round::InputMaskReservation)
    /// - [`CollectingInputs`](Round::CollectingInputs)
    /// - [`OutputCollection`](Round::OutputCollection)
    pub fn is_client_round(&self) -> bool {
        matches!(
            self,
            Round::InputMaskReservation | Round::CollectingInputs | Round::OutputCollection
        )
    }

    /// Returns `true` if servers are expected to act during this round.
    ///
    /// Server rounds are:
    /// - [`Preprocessing`](Round::Preprocessing)
    /// - [`Execution`](Round::Execution)
    pub fn is_server_round(&self) -> bool {
        matches!(self, Round::Preprocessing | Round::Execution)
    }
}

// ---------------------------------------------------------------------------
// MaskIndex
// ---------------------------------------------------------------------------

/// Index into the input-mask table managed by the coordinator.
///
/// Each client receives a unique `MaskIndex` during the
/// [`InputMaskReservation`](Round::InputMaskReservation) round.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MaskIndex(pub u64);

// ---------------------------------------------------------------------------
// CoordinatorEvent
// ---------------------------------------------------------------------------

/// Events emitted by coordinators to notify participants of state changes.
#[derive(Clone, Debug)]
pub enum CoordinatorEvent {
    /// The coordination round changed.
    RoundChanged {
        /// The round we just left.
        from: Round,
        /// The round we entered.
        to: Round,
    },
    /// A client successfully reserved an input mask.
    MaskReserved {
        /// The reserved mask index.
        index: MaskIndex,
    },
    /// A client submitted a masked input.
    InputSubmitted {
        /// The mask index used for this input.
        index: MaskIndex,
    },
    /// Preprocessing completed successfully.
    PreprocessingDone,
    /// A server submitted a result share.
    ResultSubmitted,
    /// Output is ready for client retrieval.
    OutputReady,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_progression() {
        let mut r = Round::Preprocessing;
        let expected = [
            Round::InputMaskReservation,
            Round::CollectingInputs,
            Round::InputsCollectionEnd,
            Round::Execution,
            Round::ExecutionEnd,
            Round::OutputCollection,
        ];
        for exp in &expected {
            r = r.next().expect("should have a next round");
            assert_eq!(r, *exp);
        }
        assert!(r.next().is_none(), "OutputCollection is terminal");
    }

    #[test]
    fn client_rounds() {
        assert!(!Round::Preprocessing.is_client_round());
        assert!(Round::InputMaskReservation.is_client_round());
        assert!(Round::CollectingInputs.is_client_round());
        assert!(!Round::InputsCollectionEnd.is_client_round());
        assert!(!Round::Execution.is_client_round());
        assert!(!Round::ExecutionEnd.is_client_round());
        assert!(Round::OutputCollection.is_client_round());
    }

    #[test]
    fn server_rounds() {
        assert!(Round::Preprocessing.is_server_round());
        assert!(!Round::InputMaskReservation.is_server_round());
        assert!(!Round::CollectingInputs.is_server_round());
        assert!(!Round::InputsCollectionEnd.is_server_round());
        assert!(Round::Execution.is_server_round());
        assert!(!Round::ExecutionEnd.is_server_round());
        assert!(!Round::OutputCollection.is_server_round());
    }

    #[test]
    fn mask_index_equality() {
        assert_eq!(MaskIndex(0), MaskIndex(0));
        assert_ne!(MaskIndex(0), MaskIndex(1));
    }

    #[test]
    fn coordinator_event_debug() {
        let evt = CoordinatorEvent::RoundChanged {
            from: Round::Preprocessing,
            to: Round::InputMaskReservation,
        };
        let s = format!("{:?}", evt);
        assert!(s.contains("RoundChanged"));
    }
}
