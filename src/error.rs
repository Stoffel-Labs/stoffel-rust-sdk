//! Error types for the Stoffel SDK

use thiserror::Error;

/// Result type alias for Stoffel SDK operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur when using the Stoffel SDK
#[derive(Error, Debug)]
pub enum Error {
    /// Compilation error
    #[error("Compilation error: {0}")]
    CompilationError(String),

    /// VM runtime error
    #[error("VM runtime error: {0}")]
    RuntimeError(String),

    /// MPC protocol error
    #[error("MPC protocol error: {0}")]
    MPCError(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Function not found
    #[error("Function not found: {0}")]
    FunctionNotFound(String),

    /// Network error (for MPC)
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Generic error
    #[error("{0}")]
    Other(String),
}
