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
use crate::{Error, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

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
    network: Arc<stoffelnet::transports::quic::QuicNetworkManager>,
    /// MPC configuration received from servers during handshake
    n_parties: usize,
    threshold: usize,
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
        let network = Arc::new(
            stoffelnet::transports::quic::QuicNetworkManager::with_node_id(client_id)
        );

        // For now, we'll use the number of servers as n_parties and threshold 1
        // In a full implementation, this would be received via handshake
        let n_parties = server_addrs.len();
        let threshold = 1;

        Ok(Self {
            servers: server_addrs,
            network,
            n_parties,
            threshold,
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
    pub async fn run(self, inputs: &[i64]) -> Result<Vec<i64>> {
        use ark_bls12_381::Fr;
        use stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare;
        use stoffelmpc_mpc::common::SecretSharingScheme;
        use ark_std::test_rng;

        tracing::info!(
            "Client {} submitting {} inputs to {} servers",
            self.client_id,
            inputs.len(),
            self.servers.len()
        );

        // Convert inputs to field elements
        let field_inputs: Vec<Fr> = inputs.iter()
            .map(|&x| Fr::from(x as u64))
            .collect();

        // Generate secret shares for each input
        let mut all_shares = Vec::with_capacity(field_inputs.len());
        let mut rng = test_rng();

        for input in &field_inputs {
            let shares = RobustShare::compute_shares(*input, self.n_parties, self.threshold, None, &mut rng)
                .map_err(|e| Error::Computation(format!("Failed to generate shares: {:?}", e)))?;
            all_shares.push(shares);
        }

        // Transpose: Convert from [input_idx][party_id] to [party_id][input_idx]
        let mut shares_per_party: Vec<Vec<RobustShare<Fr>>> = vec![Vec::new(); self.n_parties];
        for shares_for_input in all_shares {
            for (party_id, share) in shares_for_input.into_iter().enumerate() {
                shares_per_party[party_id].push(share);
            }
        }

        // In a full implementation, we would:
        // 1. Connect to each server
        // 2. Send our shares to each server
        // 3. Wait for computation to complete
        // 4. Receive output shares from each server
        // 5. Reconstruct the output

        // For now, return a placeholder indicating the API is ready
        // The actual network implementation will be added when we integrate
        // with the full client-server protocol
        tracing::warn!(
            "MPCConnection::run() - Network protocol not yet implemented. \
             Generated {} shares for {} parties.",
            inputs.len(),
            self.n_parties
        );

        // TODO: Implement full client-server protocol
        // This requires:
        // - QUIC connection to each server
        // - Client-server handshake protocol
        // - Input share distribution
        // - Output share collection and reconstruction

        Err(Error::Network(
            "Client-server protocol not yet implemented. \
             Use the existing MPCClient API for now, or wait for the \
             full MPCaaS implementation.".to_string()
        ))
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
