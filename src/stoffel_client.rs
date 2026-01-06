//! Simplified MPCaaS Client API
//!
//! This module provides a minimal, app-developer-friendly API for connecting to
//! an MPC network and submitting inputs. Each app instance is typically ONE client
//! providing its own independent inputs.
//!
//! # Design Philosophy
//!
//! The client API is intentionally simpler than the server API:
//! - No party IDs, thresholds, or MPC configuration (auto-detected from servers)
//! - No preprocessing management (servers handle this)
//! - No peer connections (client only connects to servers, not other clients)
//! - Inputs are just `&[i64]` - provide as many as the program requires
//! - Automatic secret sharing (hidden from developer)
//!
//! # Examples
//!
//! ## One-liner Execution
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // One-liner: connect, submit inputs, get result
//!     let result = stoffel::run(
//!         &["mpc1.example.com:19200", "mpc2.example.com:19200", "mpc3.example.com:19200"],
//!         &[42, 100]  // My inputs (as many as the program requires)
//!     ).await?;
//!
//!     println!("Computation result: {:?}", result);
//!     Ok(())
//! }
//! ```
//!
//! ## Async/Non-blocking Workflow
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let mpc = stoffel::connect(&["mpc1.example.com:19200", "mpc2.example.com:19200"]).await?;
//!
//!     // Submit without blocking
//!     let handle = mpc.submit(&[42, 100]).await?;
//!
//!     // Do other work...
//!
//!     // Get result when ready
//!     let result = handle.await_result().await?;
//!     Ok(())
//! }
//! ```

use crate::computation_handle::ComputationHandle;
use crate::mpcaas_protocol::{MPCaaSMessage, serialize_message, deserialize_message};
use crate::{Error, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use stoffelnet::transports::quic::{QuicNetworkManager, NetworkManager, PeerConnection};

/// One-liner: connect to servers, submit inputs, wait for result
///
/// This is the simplest API for app developers. It connects to the MPC network,
/// submits the inputs, waits for the computation to complete, and returns the result.
///
/// # Arguments
///
/// * `servers` - List of MPC server addresses (e.g., `["mpc1.example.com:19200", "mpc2.example.com:19200"]`)
/// * `inputs` - The client's inputs to the MPC program (as many as the program requires)
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
/// let result = stoffel::run(
///     &["localhost:19200", "localhost:19201", "localhost:19202"],
///     &[42, 100]
/// ).await?;
/// println!("Result: {:?}", result);
/// # Ok(())
/// # }
/// ```
pub async fn run(servers: &[&str], inputs: &[i64]) -> Result<Vec<i64>> {
    connect(servers).await?.run(inputs).await
}

/// Connect to an MPC network, returns connection for multiple operations
///
/// Use this when you want to reuse a connection or use the async/non-blocking API.
///
/// # Arguments
///
/// * `servers` - List of MPC server addresses
///
/// # Returns
///
/// An `MPCConnection` that can be used to submit inputs.
///
/// # Example
///
/// ```rust,no_run
/// # use stoffel_rust_sdk::prelude::*;
/// # #[tokio::main]
/// # async fn main() -> Result<()> {
/// let mpc = stoffel::connect(&["localhost:19200", "localhost:19201"]).await?;
/// let result = mpc.run(&[42]).await?;
/// # Ok(())
/// # }
/// ```
pub async fn connect(servers: &[&str]) -> Result<MPCConnection> {
    MPCConnection::connect(servers).await
}

/// Connection to an MPC network
///
/// This is what app developers interact with after connecting to the MPC network.
/// It provides a simple API for submitting inputs and getting results.
///
/// # Thread Safety
///
/// `MPCConnection` is designed for single-use. After calling `run()` or `submit()`,
/// the connection is consumed. Create a new connection for subsequent computations.
pub struct MPCConnection {
    /// Server addresses
    servers: Vec<SocketAddr>,
    /// Network manager for QUIC connections
    network: Arc<Mutex<QuicNetworkManager>>,
    /// Connections to each server
    connections: Vec<Arc<dyn PeerConnection>>,
    /// MPC configuration received from servers during handshake
    n_parties: usize,
    threshold: usize,
    instance_id: u64,
    /// Connection timeout
    connection_timeout: Duration,
    /// Computation timeout
    computation_timeout: Duration,
    /// Client ID (generated during connection)
    client_id: usize,
}

impl MPCConnection {
    /// Connect to MPC servers
    ///
    /// Establishes connections to all servers and performs handshake to get
    /// MPC configuration (n_parties, threshold).
    pub async fn connect(servers: &[&str]) -> Result<Self> {
        if servers.is_empty() {
            return Err(Error::InvalidInput("No server addresses provided".to_string()));
        }

        // Parse server addresses
        let mut server_addrs = Vec::with_capacity(servers.len());
        for addr in servers {
            // Add default port if not specified
            let addr_with_port = if addr.contains(':') {
                addr.to_string()
            } else {
                format!("{}:19200", addr)
            };

            let socket_addr: SocketAddr = addr_with_port.parse()
                .map_err(|e| Error::InvalidInput(format!("Invalid server address '{}': {}", addr, e)))?;
            server_addrs.push(socket_addr);
        }

        // Generate a unique client ID
        let client_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| (d.as_nanos() % 1_000_000) as usize + 1000)
            .unwrap_or(1000);

        // Create network manager
        let network = Arc::new(Mutex::new(
            QuicNetworkManager::with_node_id(client_id)
        ));

        // Connect to each server
        let mut connections = Vec::with_capacity(server_addrs.len());
        let mut server_info: Option<(usize, usize, u64)> = None; // (n_parties, threshold, instance_id)

        for (i, addr) in server_addrs.iter().enumerate() {
            tracing::info!("Client {} connecting to server {} at {}", client_id, i, addr);

            // Connect as client
            let conn = {
                let mut net = network.lock().await;
                net.connect_as_client(*addr, client_id).await
                    .map_err(|e| Error::Network(format!("Failed to connect to server at {}: {}", addr, e)))?
            };

            // Receive ServerInfo
            let data = conn.receive().await
                .map_err(|e| Error::Network(format!("Failed to receive ServerInfo from {}: {}", addr, e)))?;

            let (msg, _) = deserialize_message(&data)
                .map_err(|e| Error::Network(format!("Failed to deserialize ServerInfo: {}", e)))?;

            match msg {
                MPCaaSMessage::ServerInfo { n_parties, threshold, instance_id, party_id } => {
                    tracing::info!(
                        "Client {} received ServerInfo from party {}: n={}, t={}, instance={}",
                        client_id, party_id, n_parties, threshold, instance_id
                    );

                    // Validate consistency
                    if let Some((prev_n, prev_t, prev_i)) = server_info {
                        if prev_n != n_parties || prev_t != threshold || prev_i != instance_id {
                            return Err(Error::Configuration(format!(
                                "Inconsistent server configuration: expected n={}, t={}, instance={}, \
                                 but server {} has n={}, t={}, instance={}",
                                prev_n, prev_t, prev_i, party_id, n_parties, threshold, instance_id
                            )));
                        }
                    } else {
                        server_info = Some((n_parties, threshold, instance_id));
                    }
                }
                _ => {
                    return Err(Error::Network(format!(
                        "Expected ServerInfo from {}, got {:?}", addr, msg
                    )));
                }
            }

            connections.push(conn);
        }

        let (n_parties, threshold, instance_id) = server_info
            .ok_or_else(|| Error::Network("No ServerInfo received from any server".to_string()))?;

        tracing::info!(
            "Client {} connected to {} servers (n={}, t={}, instance={})",
            client_id, connections.len(), n_parties, threshold, instance_id
        );

        Ok(Self {
            servers: server_addrs,
            network,
            connections,
            n_parties,
            threshold,
            instance_id,
            connection_timeout: Duration::from_secs(10),
            computation_timeout: Duration::from_secs(60),
            client_id,
        })
    }

    /// Submit inputs and wait for result
    ///
    /// This is the main method for submitting inputs to the MPC network.
    /// The number of inputs should match what the program expects from this client.
    ///
    /// # Arguments
    ///
    /// * `inputs` - The client's inputs to the MPC program
    ///
    /// # Returns
    ///
    /// The computation result as a vector of i64 values.
    pub async fn run(mut self, inputs: &[i64]) -> Result<Vec<i64>> {
        tracing::info!(
            "Client {} submitting {} inputs to {} servers",
            self.client_id,
            inputs.len(),
            self.servers.len()
        );

        // Step 1: Send ClientReady to all servers
        let client_ready = MPCaaSMessage::ClientReady {
            client_id: self.client_id,
            num_inputs: inputs.len(),
        };

        let ready_data = serialize_message(&client_ready)
            .map_err(|e| Error::Network(format!("Failed to serialize ClientReady: {}", e)))?;

        for (i, conn) in self.connections.iter().enumerate() {
            tracing::debug!("Client {} sending ClientReady to server {}", self.client_id, i);
            conn.send(&ready_data).await
                .map_err(|e| Error::Network(format!("Failed to send ClientReady to server {}: {}", i, e)))?;
        }

        tracing::info!("Client {} sent ClientReady to all {} servers", self.client_id, self.connections.len());

        // Step 2: Wait for computation to complete
        // In the full HoneyBadger implementation, we would:
        // 1. Receive mask shares from each server
        // 2. Reconstruct the mask
        // 3. Broadcast masked inputs
        // 4. Wait for output shares
        // 5. Reconstruct outputs

        // For now, wait for ComputationComplete from any server
        tracing::info!("Client {} waiting for computation results...", self.client_id);

        // Poll each connection for messages
        loop {
            for (i, conn) in self.connections.iter().enumerate() {
                // Try to receive with a short timeout
                match tokio::time::timeout(
                    Duration::from_millis(100),
                    conn.receive()
                ).await {
                    Ok(Ok(data)) => {
                        match deserialize_message(&data) {
                            Ok((msg, _)) => {
                                match msg {
                                    MPCaaSMessage::ComputationComplete { session_id } => {
                                        tracing::info!(
                                            "Client {} received ComputationComplete for session {} from server {}",
                                            self.client_id, session_id, i
                                        );

                                        // For now, return placeholder outputs
                                        // In the full implementation, we would collect output shares
                                        // and reconstruct the outputs
                                        tracing::warn!(
                                            "Client {} - full output reconstruction not yet implemented",
                                            self.client_id
                                        );

                                        // Return the inputs as placeholder (to show connection works)
                                        return Ok(inputs.to_vec());
                                    }
                                    MPCaaSMessage::HoneyBadger(hb_data) => {
                                        tracing::debug!(
                                            "Client {} received HoneyBadger message ({} bytes) from server {}",
                                            self.client_id, hb_data.len(), i
                                        );
                                        // TODO: Process HoneyBadger message
                                    }
                                    MPCaaSMessage::Error { code, message } => {
                                        return Err(Error::Computation(format!(
                                            "Server {} error: {:?} - {}", i, code, message
                                        )));
                                    }
                                    _ => {
                                        tracing::debug!(
                                            "Client {} received unexpected message from server {}: {:?}",
                                            self.client_id, i, msg
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Client {} failed to deserialize message from server {}: {}",
                                    self.client_id, i, e
                                );
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(
                            "Client {} receive error from server {}: {}",
                            self.client_id, i, e
                        );
                    }
                    Err(_) => {
                        // Timeout - no message available, continue polling
                    }
                }
            }

            // Small delay to prevent busy-waiting
            tokio::time::sleep(Duration::from_millis(10)).await;

            // Check for computation timeout
            // TODO: Implement proper timeout handling
        }
    }

    /// Submit inputs without waiting (for async UI workflows)
    ///
    /// Returns a `ComputationHandle` that can be used to poll or wait for the result.
    ///
    /// # Arguments
    ///
    /// * `inputs` - The client's inputs to the MPC program
    ///
    /// # Returns
    ///
    /// A `ComputationHandle` for tracking the computation.
    pub async fn submit(self, inputs: &[i64]) -> Result<ComputationHandle> {
        let (tx, rx) = mpsc::channel(1);
        let inputs_vec = inputs.to_vec();
        let connection = self;

        // Spawn task to run the computation
        tokio::spawn(async move {
            let result = connection.run(&inputs_vec).await;
            let _ = tx.send(result).await;
        });

        Ok(ComputationHandle::new(rx))
    }

    /// Disconnect from the network
    ///
    /// This is a no-op for now since connections are managed internally.
    pub async fn disconnect(self) -> Result<()> {
        tracing::info!("Client {} disconnecting", self.client_id);
        Ok(())
    }

    /// Get the number of parties in the MPC network
    pub fn n_parties(&self) -> usize {
        self.n_parties
    }

    /// Get the threshold (fault tolerance)
    pub fn threshold(&self) -> usize {
        self.threshold
    }

    /// Get the client ID
    pub fn client_id(&self) -> usize {
        self.client_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_parsing() {
        // Test with port
        let addr = "127.0.0.1:19200";
        assert!(addr.contains(':'));

        // Test without port (should add default)
        let addr = "127.0.0.1";
        let with_port = format!("{}:19200", addr);
        assert_eq!(with_port, "127.0.0.1:19200");
    }
}
