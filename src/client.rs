//! StoffelClient API (RFC-002)
//!
//! This module provides the high-level [`StoffelClient`] abstraction for connecting
//! to an MPC network, submitting inputs, and retrieving computation results.
//!
//! # Architecture
//!
//! The client connects to one or more MPC servers, submits secret-shared inputs,
//! and waits for the computation to complete. The [`ClientBuilder`] provides a
//! fluent interface for configuring connection parameters, timeouts, and retry
//! policies before establishing a connection.
//!
//! # Example
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::client::{StoffelClient, ClientBuilder};
//! use std::time::Duration;
//!
//! # async fn example() -> stoffel_rust_sdk::error::Result<()> {
//! let client = StoffelClient::builder()
//!     .server("127.0.0.1:9000")
//!     .server("127.0.0.1:9001")
//!     .timeout(Duration::from_secs(30))
//!     .connect()
//!     .await?;
//!
//! let results = client.run(&[42, 17]).await?;
//! client.disconnect().await?;
//! # Ok(())
//! # }
//! ```

use std::time::Duration;

use crate::error::{Error, Result, RetryConfig};
use crate::types::{ClientId, ComputationId, Value};

// ---------------------------------------------------------------------------
// ClientState
// ---------------------------------------------------------------------------

/// The lifecycle state of a [`StoffelClient`] connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientState {
    /// Not connected to any server.
    Disconnected,
    /// Actively establishing connections to MPC servers.
    Connecting,
    /// TCP/QUIC connections established, awaiting protocol handshake.
    Connected,
    /// Waiting for all MPC servers to reach consensus on the participant set.
    AwaitingConsensus,
    /// Fully initialised and ready to submit inputs.
    Ready,
    /// A computation is currently in progress.
    Computing,
}

// ---------------------------------------------------------------------------
// ClientBuilder
// ---------------------------------------------------------------------------

/// Builder for constructing and connecting a [`StoffelClient`].
///
/// Use [`StoffelClient::builder()`] to obtain an instance.
///
/// # Example
///
/// ```rust,no_run
/// # use stoffel_rust_sdk::client::StoffelClient;
/// # use std::time::Duration;
/// # async fn example() -> stoffel_rust_sdk::error::Result<()> {
/// let client = StoffelClient::builder()
///     .server("127.0.0.1:9000")
///     .server("127.0.0.1:9001")
///     .client_id(stoffel_rust_sdk::types::ClientId(42))
///     .timeout(Duration::from_secs(60))
///     .connect()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct ClientBuilder {
    servers: Vec<String>,
    client_id: Option<ClientId>,
    timeout: Duration,
    retry_config: RetryConfig,
}

impl ClientBuilder {
    /// Create a new builder with default settings.
    ///
    /// Defaults:
    /// - No servers (at least one must be added before [`connect`](Self::connect))
    /// - Random client ID (assigned by the server)
    /// - 30-second timeout
    /// - Default retry config (3 attempts, exponential backoff)
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
            client_id: None,
            timeout: Duration::from_secs(30),
            retry_config: RetryConfig::default(),
        }
    }

    /// Add a single MPC server address.
    ///
    /// May be called multiple times to add multiple servers.
    pub fn server(mut self, addr: &str) -> Self {
        self.servers.push(addr.to_string());
        self
    }

    /// Add multiple MPC server addresses at once.
    pub fn servers(mut self, addrs: &[&str]) -> Self {
        self.servers.extend(addrs.iter().map(|a| a.to_string()));
        self
    }

    /// Set an explicit client ID.
    ///
    /// If not set, the server will assign one during the handshake.
    pub fn client_id(mut self, id: ClientId) -> Self {
        self.client_id = Some(id);
        self
    }

    /// Set the connection timeout.
    ///
    /// This timeout applies to the initial connection phase. Individual
    /// computation timeouts are separate.
    pub fn timeout(mut self, duration: Duration) -> Self {
        self.timeout = duration;
        self
    }

    /// Set the retry configuration for transient failures.
    pub fn retry(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    /// Validate the builder configuration and connect to the MPC network.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Configuration`] if no servers have been added.
    /// Returns [`Error::Computation`] as a placeholder until real networking
    /// is implemented.
    pub async fn connect(self) -> Result<StoffelClient> {
        if self.servers.is_empty() {
            return Err(Error::Configuration(
                "at least one server address is required".into(),
            ));
        }

        let client_id = self.client_id.unwrap_or(ClientId(0));

        // TODO: establish real QUIC connections here
        Ok(StoffelClient {
            client_id,
            state: ClientState::Ready,
            servers: self.servers,
        })
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// StoffelClient
// ---------------------------------------------------------------------------

/// A connected MPC client that can submit inputs and retrieve results.
///
/// Obtain an instance via [`StoffelClient::builder()`].
///
/// # Lifecycle
///
/// ```text
/// builder() -> ClientBuilder
///   .server(...)
///   .connect().await  -> StoffelClient (state = Ready)
///   .run(inputs)      -> Vec<Value>
///   .disconnect()     -> ()
/// ```
pub struct StoffelClient {
    client_id: ClientId,
    state: ClientState,
    servers: Vec<String>,
}

impl StoffelClient {
    /// Create a new [`ClientBuilder`].
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Return the current connection state.
    pub fn state(&self) -> ClientState {
        self.state
    }

    /// Return this client's identifier.
    pub fn client_id(&self) -> ClientId {
        self.client_id
    }

    /// Returns `true` when the client is in the [`ClientState::Ready`] state.
    pub fn is_ready(&self) -> bool {
        self.state == ClientState::Ready
    }

    /// Submit inputs and synchronously wait for the computation result.
    ///
    /// This is a convenience wrapper around [`submit`](Self::submit) followed
    /// by [`ComputationHandle::await_result`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Computation`] (stub) until real networking is wired up.
    pub async fn run(&self, inputs: &[i64]) -> Result<Vec<Value>> {
        let handle = self.submit(inputs).await?;
        handle.await_result().await
    }

    /// Submit inputs for a named function and synchronously wait for the result.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Computation`] (stub) until real networking is wired up.
    pub async fn run_function(&self, _name: &str, inputs: &[i64]) -> Result<Vec<Value>> {
        // TODO: encode function name in the submission
        self.run(inputs).await
    }

    /// Submit inputs and return a [`ComputationHandle`] for async tracking.
    ///
    /// The handle can be used to poll status, await results, or cancel the
    /// computation.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Computation`] (stub) until real networking is wired up.
    pub async fn submit(&self, _inputs: &[i64]) -> Result<ComputationHandle> {
        // TODO: secret-share inputs and send to servers
        Err(Error::Computation("not yet implemented".into()))
    }

    /// Gracefully disconnect from all MPC servers.
    ///
    /// After this call the client is no longer usable.
    pub async fn disconnect(self) -> Result<()> {
        // TODO: send disconnect messages to servers
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ComputationStatus
// ---------------------------------------------------------------------------

/// Status of an in-flight MPC computation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComputationStatus {
    /// The computation request has been created but not yet submitted.
    Pending,
    /// Client inputs have been secret-shared and sent to servers.
    InputsSubmitted,
    /// The MPC protocol is actively computing.
    Computing,
    /// The computation finished successfully; results are available.
    Completed,
    /// The computation failed.
    Failed,
    /// The computation was cancelled by the client.
    Cancelled,
}

// ---------------------------------------------------------------------------
// ComputationHandle
// ---------------------------------------------------------------------------

/// A handle to a running or completed MPC computation.
///
/// Returned by [`StoffelClient::submit`]. Use [`await_result`](Self::await_result)
/// to block until the computation finishes, or [`status`](Self::status) to poll.
pub struct ComputationHandle {
    computation_id: ComputationId,
    status: ComputationStatus,
}

impl ComputationHandle {
    /// Return the current status of this computation.
    pub fn status(&self) -> ComputationStatus {
        self.status
    }

    /// Return the unique identifier for this computation.
    pub fn computation_id(&self) -> ComputationId {
        self.computation_id
    }

    /// Block until the computation completes and return the result.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Computation`] (stub) until real networking is wired up.
    pub async fn await_result(self) -> Result<Vec<Value>> {
        // TODO: poll servers for result shares and reconstruct
        Err(Error::Computation("not yet implemented".into()))
    }

    /// Request cancellation of this computation.
    ///
    /// Cancellation is best-effort; the servers may have already completed.
    pub fn cancel(self) {
        // TODO: send cancellation to servers
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_defaults() {
        let b = ClientBuilder::new();
        assert!(b.servers.is_empty());
        assert!(b.client_id.is_none());
        assert_eq!(b.timeout, Duration::from_secs(30));
    }

    #[test]
    fn builder_add_servers() {
        let b = ClientBuilder::new()
            .server("127.0.0.1:9000")
            .servers(&["127.0.0.1:9001", "127.0.0.1:9002"]);
        assert_eq!(b.servers.len(), 3);
    }

    #[tokio::test]
    async fn connect_requires_servers() {
        let result = ClientBuilder::new().connect().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn connect_succeeds_with_server() {
        let client = ClientBuilder::new()
            .server("127.0.0.1:9000")
            .connect()
            .await
            .unwrap();
        assert!(client.is_ready());
        assert_eq!(client.state(), ClientState::Ready);
    }

    #[tokio::test]
    async fn submit_returns_not_implemented() {
        let client = ClientBuilder::new()
            .server("127.0.0.1:9000")
            .connect()
            .await
            .unwrap();
        let err = client.submit(&[1, 2, 3]).await.unwrap_err();
        assert!(err.to_string().contains("not yet implemented"));
    }

    #[tokio::test]
    async fn disconnect_succeeds() {
        let client = ClientBuilder::new()
            .server("127.0.0.1:9000")
            .connect()
            .await
            .unwrap();
        assert!(client.disconnect().await.is_ok());
    }

    #[test]
    fn client_state_variants() {
        assert_ne!(ClientState::Disconnected, ClientState::Ready);
        assert_eq!(ClientState::Ready, ClientState::Ready);
    }

    #[test]
    fn computation_status_variants() {
        assert_ne!(ComputationStatus::Pending, ComputationStatus::Completed);
    }
}
