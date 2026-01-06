//! Computation Handle for Async/Non-blocking MPC Workflows
//!
//! This module provides the `ComputationHandle` type for tracking and retrieving
//! results from asynchronous MPC computations.
//!
//! # Example
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let mpc = stoffel::connect(&["localhost:19200", "localhost:19201"]).await?;
//!
//!     // Submit without blocking
//!     let handle = mpc.submit(&[42, 100]).await?;
//!
//!     // Do other work while computation runs...
//!
//!     // Poll without blocking
//!     if let Some(result) = handle.try_result() {
//!         println!("Done: {:?}", result?);
//!     }
//!
//!     // Or wait for result
//!     // let result = handle.await_result().await?;
//!
//!     Ok(())
//! }
//! ```

use crate::{Error, Result};
use tokio::sync::mpsc;
use std::sync::{Arc, Mutex};

/// Cached result state (avoids cloning Error which doesn't implement Clone)
#[derive(Clone)]
enum CachedResult {
    /// No result received yet
    Pending,
    /// Successful result
    Ok(Vec<i64>),
    /// Error occurred (stored as message string)
    Err(String),
}

/// Handle for tracking an asynchronous MPC computation
///
/// This is returned by `MPCConnection::submit()` and allows the caller to
/// poll for results or wait for the computation to complete.
pub struct ComputationHandle {
    /// Receiver for the computation result
    output_rx: mpsc::Receiver<Result<Vec<i64>>>,
    /// Cached result (once received)
    cached_result: Arc<Mutex<CachedResult>>,
}

impl ComputationHandle {
    /// Create a new computation handle
    pub(crate) fn new(output_rx: mpsc::Receiver<Result<Vec<i64>>>) -> Self {
        Self {
            output_rx,
            cached_result: Arc::new(Mutex::new(CachedResult::Pending)),
        }
    }

    /// Wait for computation to complete and return result
    ///
    /// This blocks until the computation is complete.
    ///
    /// # Returns
    ///
    /// The computation result as a vector of i64 values.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let mpc = stoffel::connect(&["localhost:19200"]).await?;
    /// let handle = mpc.submit(&[42]).await?;
    /// let result = handle.await_result().await?;
    /// println!("Result: {:?}", result);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn await_result(mut self) -> Result<Vec<i64>> {
        // Check if we already have a cached result
        {
            let cached = self.cached_result.lock().unwrap();
            match &*cached {
                CachedResult::Ok(v) => return Ok(v.clone()),
                CachedResult::Err(e) => return Err(Error::Computation(e.clone())),
                CachedResult::Pending => {}
            }
        }

        // Wait for result from channel
        match self.output_rx.recv().await {
            Some(result) => {
                // Cache the result
                let mut cached = self.cached_result.lock().unwrap();
                match &result {
                    Ok(v) => *cached = CachedResult::Ok(v.clone()),
                    Err(e) => *cached = CachedResult::Err(format!("{}", e)),
                }
                result
            }
            None => Err(Error::Computation("Computation task was dropped".to_string())),
        }
    }

    /// Poll for result without blocking (for UI apps)
    ///
    /// Returns `None` if the computation is still in progress.
    /// Returns `Some(result)` if the computation is complete.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::prelude::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let mpc = stoffel::connect(&["localhost:19200"]).await?;
    /// let mut handle = mpc.submit(&[42]).await?;
    ///
    /// loop {
    ///     if let Some(result) = handle.try_result() {
    ///         println!("Done: {:?}", result?);
    ///         break;
    ///     }
    ///     // Do other work...
    ///     tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_result(&mut self) -> Option<Result<Vec<i64>>> {
        // Check if we already have a cached result
        {
            let cached = self.cached_result.lock().unwrap();
            match &*cached {
                CachedResult::Ok(v) => return Some(Ok(v.clone())),
                CachedResult::Err(e) => return Some(Err(Error::Computation(e.clone()))),
                CachedResult::Pending => {}
            }
        }

        // Try to receive without blocking
        match self.output_rx.try_recv() {
            Ok(result) => {
                // Cache the result
                let mut cached = self.cached_result.lock().unwrap();
                match &result {
                    Ok(v) => *cached = CachedResult::Ok(v.clone()),
                    Err(e) => *cached = CachedResult::Err(format!("{}", e)),
                }
                Some(result)
            }
            Err(mpsc::error::TryRecvError::Empty) => None,
            Err(mpsc::error::TryRecvError::Disconnected) => {
                Some(Err(Error::Computation("Computation task was dropped".to_string())))
            }
        }
    }

    /// Cancel the computation (best-effort, may still complete)
    ///
    /// This drops the handle, which will cause the computation task to
    /// eventually clean up. The computation may still complete if it was
    /// already in progress.
    pub fn cancel(self) {
        // Simply dropping the handle will signal cancellation
        // The spawned task will detect that the sender is dropped
        tracing::info!("Computation cancelled");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_computation_handle_await() {
        let (tx, rx) = mpsc::channel(1);
        let handle = ComputationHandle::new(rx);

        // Simulate computation completing
        tx.send(Ok(vec![42, 100])).await.unwrap();

        let result = handle.await_result().await.unwrap();
        assert_eq!(result, vec![42, 100]);
    }

    #[tokio::test]
    async fn test_computation_handle_try_result() {
        let (tx, rx) = mpsc::channel(1);
        let mut handle = ComputationHandle::new(rx);

        // Should be None initially
        assert!(handle.try_result().is_none());

        // Simulate computation completing
        tx.send(Ok(vec![42])).await.unwrap();

        // Now should have result
        let result = handle.try_result();
        assert!(result.is_some());
        assert_eq!(result.unwrap().unwrap(), vec![42]);
    }
}
