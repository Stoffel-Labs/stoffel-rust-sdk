//! On-chain coordination via the StoffelCoordinator Solidity contract.
//!
//! This module is a **placeholder**. Full implementation requires integration
//! with an Ethereum client library (ethers-rs or alloy) and the generated
//! contract bindings from `Stoffel-solidity-SDK`.
//!
//! # Future Work
//!
//! When wired up, [`OnChainCoordinator`] will:
//! 1. Read the current round from the on-chain state machine.
//! 2. Submit transactions to advance rounds.
//! 3. Watch for `RoundChanged` events to notify local participants.
//! 4. Reserve input-mask indices via contract calls.
//!
//! The on-chain round state machine mirrors [`super::Round`] exactly:
//!
//! ```text
//! Preprocessing -> InputMaskReservation -> CollectingInputs
//!   -> InputsCollectionEnd -> Execution -> ExecutionEnd -> OutputCollection
//! ```

use super::Round;
use crate::error::{Error, Result};
use crate::types::ComputationId;

// ---------------------------------------------------------------------------
// OnChainCoordinator
// ---------------------------------------------------------------------------

/// On-chain coordinator backed by the StoffelCoordinator Solidity contract.
///
/// This is a placeholder implementation. All async methods currently return
/// `Err(Error::Computation("not yet implemented"))`.
///
/// # Construction
///
/// ```rust
/// use stoffel_rust_sdk::coordinator::onchain::OnChainCoordinator;
/// use stoffel_rust_sdk::types::ComputationId;
///
/// let coord = OnChainCoordinator::new(
///     "0xDEADBEEF...".to_string(),
///     ComputationId(1),
/// );
/// ```
pub struct OnChainCoordinator {
    /// The Ethereum address of the deployed StoffelCoordinator contract.
    contract_address: String,
    /// The computation this coordinator instance tracks.
    computation_id: ComputationId,
}

impl OnChainCoordinator {
    /// Create a new on-chain coordinator targeting the given contract and
    /// computation.
    pub fn new(contract_address: String, computation_id: ComputationId) -> Self {
        Self {
            contract_address,
            computation_id,
        }
    }

    /// Return the contract address this coordinator is bound to.
    pub fn contract_address(&self) -> &str {
        &self.contract_address
    }

    /// Return the computation ID this coordinator is tracking.
    pub fn computation_id(&self) -> ComputationId {
        self.computation_id
    }

    /// Query the current round from the on-chain contract.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Computation`] (stub) until contract integration is complete.
    pub async fn current_round(&self) -> Result<Round> {
        Err(Error::Computation(
            "on-chain coordinator not yet implemented".into(),
        ))
    }

    /// Block until the on-chain state machine reaches the specified round.
    ///
    /// This will poll the contract (or subscribe to events) until the target
    /// round is reached.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Computation`] (stub) until contract integration is complete.
    pub async fn await_round(&self, _round: Round) -> Result<()> {
        Err(Error::Computation(
            "on-chain coordinator not yet implemented".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction() {
        let coord = OnChainCoordinator::new(
            "0x1234abcd".to_string(),
            ComputationId(99),
        );
        assert_eq!(coord.contract_address(), "0x1234abcd");
        assert_eq!(coord.computation_id(), ComputationId(99));
    }

    #[tokio::test]
    async fn current_round_not_implemented() {
        let coord = OnChainCoordinator::new("0x0".to_string(), ComputationId(1));
        let err = coord.current_round().await.unwrap_err();
        assert!(err.to_string().contains("not yet implemented"));
    }

    #[tokio::test]
    async fn await_round_not_implemented() {
        let coord = OnChainCoordinator::new("0x0".to_string(), ComputationId(1));
        let err = coord.await_round(Round::Execution).await.unwrap_err();
        assert!(err.to_string().contains("not yet implemented"));
    }
}
