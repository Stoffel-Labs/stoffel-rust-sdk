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

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

// Re-export the real coordinator from stoffel-mpc-coordinator crate
pub use stoffel_mpc_coordinator::off_chain::OffChainCoordinator as RealOffChainCoordinator;

// Re-export the Coordinator trait for generic usage
pub use stoffel_mpc_coordinator::Coordinator;

// Re-export self-signed cert utilities
pub use stoffel_mpc_coordinator::self_signed_certs;

use crate::config::{CoordinatorConfig, TlsConfig};
use super::{MaskIndex, Round};
use crate::error::{Error, Result};

// ---------------------------------------------------------------------------
// StoffelCoordinator (production wrapper)
// ---------------------------------------------------------------------------

/// Builder for constructing a [`StoffelCoordinator`] that wraps the real
/// off-chain coordinator RPC server.
///
/// # Example
///
/// ```rust,no_run
/// use stoffel_rust_sdk::coordinator::offchain::StoffelCoordinator;
///
/// # async fn example() -> stoffel_rust_sdk::error::Result<()> {
/// let coordinator = StoffelCoordinator::builder()
///     .bind("0.0.0.0:31415")
///     .expected_parties(5)
///     .threshold(1)
///     .build()
///     .await?;
///
/// println!("Coordinator listening on {}", coordinator.addr());
/// coordinator.run_forever().await?;
/// # Ok(())
/// # }
/// ```
pub struct CoordinatorBuilder {
    bind_addr: SocketAddr,
    expected_parties: usize,
    threshold: usize,
    n_outputs: u64,
    tls_mode: TlsConfig,
    program_bytecode: Option<Vec<u8>>,
}

impl CoordinatorBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self {
            bind_addr: "0.0.0.0:31415".parse().unwrap(),
            expected_parties: 5,
            threshold: 1,
            n_outputs: 1,
            tls_mode: TlsConfig::SelfSigned,
            program_bytecode: None,
        }
    }

    /// Set the bind address for the coordinator RPC server.
    pub fn bind(mut self, addr: &str) -> Self {
        if let Ok(parsed) = addr.parse() {
            self.bind_addr = parsed;
        }
        self
    }

    /// Set the number of expected MPC server parties.
    pub fn expected_parties(mut self, n: usize) -> Self {
        self.expected_parties = n;
        self
    }

    /// Set the fault-tolerance threshold.
    pub fn threshold(mut self, t: usize) -> Self {
        self.threshold = t;
        self
    }

    /// Set the number of expected computation outputs.
    pub fn n_outputs(mut self, n: u64) -> Self {
        self.n_outputs = n;
        self
    }

    /// Set the TLS mode.
    pub fn tls(mut self, tls: TlsConfig) -> Self {
        self.tls_mode = tls;
        self
    }

    /// Set the program bytecode. If not provided, clients can submit later.
    pub fn program(mut self, bytecode: Vec<u8>) -> Self {
        self.program_bytecode = Some(bytecode);
        self
    }

    /// Build and start the coordinator RPC server.
    ///
    /// The RPC server starts in a background Tokio task. Call
    /// [`StoffelCoordinator::run_forever`] to block until shutdown.
    pub async fn build(self) -> Result<StoffelCoordinator> {
        let prog_hash = match &self.program_bytecode {
            Some(bytes) => stoffel_mpc_coordinator::compute_prog_hash(bytes),
            None => [0u8; 32], // deferred submission
        };

        let addr_str = self.bind_addr.ip().to_string();
        let port = self.bind_addr.port();

        let cert = match &self.tls_mode {
            TlsConfig::SelfSigned => self_signed_certs::server_cert(),
            TlsConfig::Custom { .. } => {
                // For now, custom certs use self-signed as fallback.
                // Full custom cert support requires loading PEM files.
                // TODO: load custom certs from cert_path/key_path
                self_signed_certs::server_cert()
            }
        };

        let inner = RealOffChainCoordinator::start_coord_from_cert(
            &addr_str,
            port,
            prog_hash,
            self.expected_parties as u64,
            self.threshold as u64,
            vec![], // no initial MPC nodes — they register dynamically
            self.n_outputs,
            cert,
        )
        .await;

        Ok(StoffelCoordinator {
            bind_addr: self.bind_addr,
            inner,
        })
    }
}

impl Default for CoordinatorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A running off-chain MPC coordinator that wraps the real
/// `OffChainCoordinator` from `stoffel-mpc-coordinator`.
///
/// The coordinator provides a JSON-RPC server over TLS that MPC servers
/// and clients connect to for round management, input masking, and
/// output distribution.
pub struct StoffelCoordinator {
    bind_addr: SocketAddr,
    inner: RealOffChainCoordinator,
}

impl StoffelCoordinator {
    /// Create a new [`CoordinatorBuilder`].
    pub fn builder() -> CoordinatorBuilder {
        CoordinatorBuilder::new()
    }

    /// Build a coordinator from a [`CoordinatorConfig`].
    ///
    /// Reads configuration from a TOML file and starts the RPC server.
    pub async fn from_config(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;

        // Try parsing as a standalone coordinator config (with [coordinator] section)
        #[derive(serde::Deserialize)]
        struct CoordWrapper {
            coordinator: CoordinatorConfig,
        }

        let config: CoordinatorConfig = if let Ok(wrapper) = toml::from_str::<CoordWrapper>(&content) {
            wrapper.coordinator
        } else {
            toml::from_str(&content)
                .map_err(|e| Error::Configuration(format!("TOML parse error: {e}")))?
        };

        // Apply PARTY_ID-style env overrides
        let bind = if let Ok(val) = std::env::var("BIND_ADDRESS") {
            val.parse().map_err(|e| {
                Error::Configuration(format!("BIND_ADDRESS: invalid address: {}", e))
            })?
        } else {
            config.bind_address
        };

        let n = config.expected_parties.unwrap_or(5);
        let t = config.threshold.unwrap_or(1);

        Self::builder()
            .bind(&bind.to_string())
            .expected_parties(n)
            .threshold(t)
            .n_outputs(config.n_outputs)
            .tls(config.tls)
            .build()
            .await
    }

    /// Return the address the coordinator is listening on.
    pub fn addr(&self) -> SocketAddr {
        self.bind_addr
    }

    /// Return a reference to the underlying coordinator for direct RPC access.
    pub fn inner(&self) -> &RealOffChainCoordinator {
        &self.inner
    }

    /// Block until the process receives a shutdown signal (Ctrl+C).
    ///
    /// The RPC server is already running in a background task; this method
    /// simply awaits `tokio::signal::ctrl_c()`.
    pub async fn run_forever(self) -> Result<()> {
        tokio::signal::ctrl_c()
            .await
            .map_err(|e| Error::Runtime(format!("signal handler error: {}", e)))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// OffChainCoordinator (in-memory test coordinator)
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
