//! Error types for the Stoffel SDK.
//!
//! This module provides a comprehensive error hierarchy with:
//! - A top-level [`Error`] enum covering all SDK failure modes
//! - Specialized [`NetworkError`] and [`ConsensusError`] enums for MPC operations
//! - A [`RetryConfig`] struct for exponential backoff retry logic
//! - A [`ResultExt`] trait for adding contextual information to errors
//!
//! # Error Chain
//!
//! The error hierarchy is designed for progressive specificity:
//!
//! ```text
//! Error
//! ├── Network(NetworkError)       ─── bind failures, peer disconnects, TLS, etc.
//! ├── Consensus(ConsensusError)   ─── node mismatch, timeouts, invalid messages
//! ├── Compilation(String)         ─── Stoffel-Lang compilation failures
//! ├── Configuration(String)       ─── invalid parameters/configuration
//! ├── Runtime(String)             ─── StoffelVM execution errors
//! ├── Preprocessing(String)       ─── MPC preprocessing errors
//! ├── Computation(String)         ─── MPC computation errors
//! ├── InvalidInput(String)        ─── invalid user-provided parameters
//! ├── FunctionNotFound(String)    ─── missing function in bytecode
//! └── Io(std::io::Error)         ─── file/network IO errors
//! ```

use std::fmt;
use std::time::Duration;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Result alias
// ---------------------------------------------------------------------------

/// Convenience result type defaulting the error to [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

// ---------------------------------------------------------------------------
// Top-level Error
// ---------------------------------------------------------------------------

/// Top-level error type for all Stoffel SDK operations.
#[derive(Error, Debug)]
pub enum Error {
    /// Stoffel-Lang compilation failure.
    #[error("Compilation error: {0}")]
    Compilation(String),

    /// Invalid SDK or MPC configuration.
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Network-layer error (QUIC, TLS, peer connectivity).
    #[error(transparent)]
    Network(#[from] NetworkError),

    /// MPC consensus-layer error (HoneyBadger protocol).
    #[error(transparent)]
    Consensus(#[from] ConsensusError),

    /// MPC preprocessing error (beaver triples, random shares).
    #[error("Preprocessing error: {0}")]
    Preprocessing(String),

    /// MPC computation error during secure evaluation.
    #[error("Computation error: {0}")]
    Computation(String),

    /// Requested function was not found in compiled bytecode.
    #[error("Function not found: {0}")]
    FunctionNotFound(String),

    /// Invalid input provided by the caller.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// StoffelVM runtime error.
    #[error("Runtime error: {0}")]
    Runtime(String),

    /// IO error (files, sockets).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// Backward-compatible aliases so existing `Error::IoError(e)` and
// `Error::RuntimeError(s)` call-sites keep compiling.  These are
// constructor functions that mirror the old variant names.
impl Error {
    /// Alias for [`Error::Io`] — keeps old `Error::IoError(e)` call-sites working.
    #[inline]
    pub fn IoError(source: std::io::Error) -> Self {
        Self::Io(source)
    }

    /// Alias for [`Error::Runtime`] — keeps old `Error::RuntimeError(s)` call-sites working.
    #[inline]
    pub fn RuntimeError(msg: String) -> Self {
        Self::Runtime(msg)
    }

    /// Alias for [`Error::Compilation`] — keeps old `Error::CompilationError(s)` call-sites working.
    #[inline]
    pub fn CompilationError(msg: String) -> Self {
        Self::Compilation(msg)
    }

    /// Alias for [`Error::Network`] variant wrapping a plain string (legacy).
    #[inline]
    pub fn MPCError(msg: String) -> Self {
        Self::Computation(msg)
    }

    /// Return the full error chain as a single string, one cause per line.
    pub fn chain(&self) -> String {
        use std::error::Error as _;
        let mut parts = vec![self.to_string()];
        let mut current: &dyn std::error::Error = self;
        while let Some(source) = current.source() {
            parts.push(source.to_string());
            current = source;
        }
        parts.join("\n  caused by: ")
    }

    /// Log this error via the `tracing` crate at the ERROR level, including
    /// the full causal chain.
    pub fn log(&self) {
        tracing::error!(error = %self, chain = %self.chain(), "stoffel sdk error");
    }
}

// ---------------------------------------------------------------------------
// NetworkError
// ---------------------------------------------------------------------------

/// Errors originating from the networking layer (QUIC / TLS / peer management).
#[derive(Error, Debug)]
pub enum NetworkError {
    /// Failed to bind a listener to the requested address.
    #[error("Failed to bind to {addr}: {source}")]
    BindFailed {
        addr: String,
        source: std::io::Error,
    },

    /// Could not connect to a peer MPC server.
    #[error("Failed to connect to party {party_id} at {addr}: {reason}")]
    PeerConnectionFailed {
        party_id: u64,
        addr: String,
        reason: String,
    },

    /// Timed out while connecting to a server.
    #[error("Connection to {server} timed out after {timeout:?}")]
    ConnectionTimeout {
        server: String,
        timeout: Duration,
    },

    /// None of the attempted bootstrap/peer servers could be reached.
    #[error("All {attempted} servers unreachable")]
    AllServersUnreachable {
        attempted: usize,
    },

    /// A previously-connected peer disconnected unexpectedly.
    #[error("Party {party_id} disconnected: {reason}")]
    PeerDisconnected {
        party_id: u64,
        reason: String,
    },

    /// TLS handshake or certificate error.
    #[error("TLS error: {0}")]
    Tls(String),

    /// Message serialization / deserialization failure on the wire.
    #[error("Serialization error: {0}")]
    Serialization(String),
}

// ---------------------------------------------------------------------------
// ConsensusError
// ---------------------------------------------------------------------------

/// Errors originating from the MPC consensus layer (HoneyBadger protocol).
#[derive(Error, Debug)]
pub enum ConsensusError {
    /// A node reported a different participant list than expected.
    #[error("Node list mismatch from {node_address}")]
    NodeListMismatch {
        node_address: String,
    },

    /// A client's input-mask digest does not match the coordinator's record.
    #[error("Client list digest mismatch for party {party_id}")]
    ClientListDigestMismatch {
        party_id: u64,
    },

    /// Not enough clients connected before the readiness deadline.
    #[error("Client readiness timeout: expected {expected}, got {connected}")]
    ClientReadinessTimeout {
        expected: usize,
        connected: usize,
    },

    /// The consensus round did not complete in time.
    #[error("Consensus timeout: {missing_count} parties missing")]
    ConsensusTimeout {
        missing_count: usize,
    },

    /// A party disconnected during the consensus round.
    #[error("Party {party_id} disconnected during consensus")]
    PartyDisconnected {
        party_id: u64,
    },

    /// Received an invalid or malformed protocol message.
    #[error("Invalid message from party {party_id}: {reason}")]
    InvalidMessage {
        party_id: u64,
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// RetryConfig
// ---------------------------------------------------------------------------

/// Configuration for exponential-backoff retry logic.
///
/// # Example
///
/// ```
/// use stoffel_rust_sdk::error::RetryConfig;
///
/// let cfg = RetryConfig::exponential_backoff(5);
/// assert_eq!(cfg.max_attempts, 5);
/// ```
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of attempts (including the initial one).
    pub max_attempts: u32,
    /// Delay before the first retry.
    pub initial_delay: Duration,
    /// Upper bound on the delay between retries.
    pub max_delay: Duration,
    /// Multiplicative factor applied to the delay after each attempt.
    pub backoff_multiplier: f64,
}

impl RetryConfig {
    /// Create a config with exponential backoff.
    ///
    /// Uses sensible defaults: 100 ms initial delay, 30 s max delay, 2x multiplier.
    pub fn exponential_backoff(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
        }
    }

    /// Create a config that disables retrying (single attempt).
    pub fn none() -> Self {
        Self {
            max_attempts: 1,
            initial_delay: Duration::ZERO,
            max_delay: Duration::ZERO,
            backoff_multiplier: 1.0,
        }
    }

    /// Compute the delay for the `n`-th retry (0-indexed).
    ///
    /// Returns `None` if `n >= max_attempts - 1` (no more retries allowed).
    pub fn delay_for(&self, n: u32) -> Option<Duration> {
        if n + 1 >= self.max_attempts {
            return None;
        }
        let raw = self.initial_delay.as_secs_f64() * self.backoff_multiplier.powi(n as i32);
        let clamped = raw.min(self.max_delay.as_secs_f64());
        Some(Duration::from_secs_f64(clamped))
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self::exponential_backoff(3)
    }
}

// ---------------------------------------------------------------------------
// ResultExt trait
// ---------------------------------------------------------------------------

/// Extension trait for adding context to any `Result` whose error can be
/// converted into [`Error`].
pub trait ResultExt<T> {
    /// Wrap the error with a static context message, producing [`Error::Runtime`]
    /// if the original error is not already an [`Error`].
    fn context(self, msg: &str) -> Result<T>;

    /// Wrap the error with a lazily-evaluated context message.
    fn with_context<F: FnOnce() -> String>(self, f: F) -> Result<T>;
}

impl<T, E> ResultExt<T> for std::result::Result<T, E>
where
    E: Into<Error>,
{
    fn context(self, msg: &str) -> Result<T> {
        self.map_err(|e| {
            let inner = e.into();
            Error::Runtime(format!("{}: {}", msg, inner))
        })
    }

    fn with_context<F: FnOnce() -> String>(self, f: F) -> Result<T> {
        self.map_err(|e| {
            let inner = e.into();
            Error::Runtime(format!("{}: {}", f(), inner))
        })
    }
}

impl<T> ResultExt<T> for Option<T> {
    fn context(self, msg: &str) -> Result<T> {
        self.ok_or_else(|| Error::Runtime(msg.to_string()))
    }

    fn with_context<F: FnOnce() -> String>(self, f: F) -> Result<T> {
        self.ok_or_else(|| Error::Runtime(f()))
    }
}

// ---------------------------------------------------------------------------
// Display helpers (already derived via thiserror, but we add From impls
// for ergonomic conversions from common external error types)
// ---------------------------------------------------------------------------

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Error::Runtime(msg)
    }
}

impl From<&str> for Error {
    fn from(msg: &str) -> Self {
        Error::Runtime(msg.to_string())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_alias_defaults_to_error() {
        let ok: Result<i32> = Ok(42);
        assert_eq!(ok.unwrap(), 42);

        let err: Result<i32> = Err(Error::InvalidInput("bad".into()));
        assert!(err.is_err());
    }

    #[test]
    fn network_error_converts_to_error() {
        let net = NetworkError::Tls("cert expired".into());
        let err: Error = net.into();
        assert!(matches!(err, Error::Network(NetworkError::Tls(_))));
        assert!(err.to_string().contains("cert expired"));
    }

    #[test]
    fn consensus_error_converts_to_error() {
        let con = ConsensusError::ConsensusTimeout { missing_count: 2 };
        let err: Error = con.into();
        assert!(matches!(err, Error::Consensus(ConsensusError::ConsensusTimeout { .. })));
    }

    #[test]
    fn error_chain_includes_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let net = NetworkError::BindFailed {
            addr: "0.0.0.0:1234".into(),
            source: io_err,
        };
        let err: Error = net.into();
        let chain = err.chain();
        assert!(chain.contains("bind"));
        assert!(chain.contains("gone"));
    }

    #[test]
    fn retry_config_exponential_backoff() {
        let cfg = RetryConfig::exponential_backoff(4);
        assert_eq!(cfg.max_attempts, 4);

        let d0 = cfg.delay_for(0).unwrap();
        assert_eq!(d0, Duration::from_millis(100));

        let d1 = cfg.delay_for(1).unwrap();
        assert_eq!(d1, Duration::from_millis(200));

        let d2 = cfg.delay_for(2).unwrap();
        assert_eq!(d2, Duration::from_millis(400));

        // No more retries after max_attempts - 1
        assert!(cfg.delay_for(3).is_none());
    }

    #[test]
    fn retry_config_none_single_attempt() {
        let cfg = RetryConfig::none();
        assert_eq!(cfg.max_attempts, 1);
        assert!(cfg.delay_for(0).is_none());
    }

    #[test]
    fn result_ext_context_on_option() {
        let none: Option<i32> = None;
        let err = none.context("missing value").unwrap_err();
        assert!(err.to_string().contains("missing value"));
    }

    #[test]
    fn result_ext_context_on_result() {
        let bad: std::result::Result<i32, Error> = Err(Error::InvalidInput("x".into()));
        let err = bad.context("during setup").unwrap_err();
        assert!(err.to_string().contains("during setup"));
        assert!(err.to_string().contains("x"));
    }

    #[test]
    fn backward_compat_aliases() {
        // Ensure the old constructor-style calls still work
        let e1 = Error::IoError(std::io::Error::new(std::io::ErrorKind::Other, "oops"));
        assert!(matches!(e1, Error::Io(_)));

        let e2 = Error::RuntimeError("boom".into());
        assert!(matches!(e2, Error::Runtime(_)));

        let e3 = Error::CompilationError("parse fail".into());
        assert!(matches!(e3, Error::Compilation(_)));
    }
}
