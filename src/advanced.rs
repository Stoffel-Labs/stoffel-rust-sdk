//! Advanced APIs with proper abstractions
//!
//! This module provides advanced functionality with clean abstractions.
//! Unlike raw internal types, these are designed for extensibility.
//!
//! # When to use this module
//!
//! - Building custom MPC applications
//! - Integrating with existing infrastructure
//! - Creating domain-specific SDKs
//! - Custom VM execution contexts
//!
//! # Example: Share Management
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::advanced::*;
//!
//! // Store shares for a client
//! ShareManager::store_shares(100, &[10, 20], 5, 1)?;
//!
//! // Check if shares exist
//! if ShareManager::has_shares(100) {
//!     println!("Client 100 has {} shares", ShareManager::share_count(100));
//! }
//! # Ok::<(), stoffel_rust_sdk::Error>(())
//! ```

use crate::{Error, Result};

/// Share management for MPC operations
///
/// Provides clean interface for managing secret shares without
/// exposing the underlying ClientInputStore implementation.
pub struct ShareManager;

impl ShareManager {
    /// Store secret shares for a client
    ///
    /// # Arguments
    /// * `client_id` - Unique identifier for the client
    /// * `values` - The secret values to share
    /// * `n_parties` - Number of MPC parties
    /// * `threshold` - Fault tolerance threshold
    pub fn store_shares(
        client_id: u64,
        values: &[i64],
        n_parties: usize,
        threshold: usize,
    ) -> Result<()> {
        use ark_ff::PrimeField;
        use stoffelmpc_mpc::common::SecretSharingScheme;
        use stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare;

        let store = stoffel_vm::net::client_store::get_global_store();
        let mut rng = ark_std::test_rng();

        for &value in values {
            let field_value = ark_bls12_381::Fr::from(value as u64);
            let shares = RobustShare::compute_shares(
                field_value,
                n_parties,
                threshold,
                None,
                &mut rng,
            )
            .map_err(|e| Error::Computation(format!("Failed to compute shares: {:?}", e)))?;

            store.store_client_input(client_id as usize, shares);
        }

        Ok(())
    }

    /// Check if shares exist for a client
    pub fn has_shares(client_id: u64) -> bool {
        let store = stoffel_vm::net::client_store::get_global_store();
        store.has_client_input(client_id as usize)
    }

    /// Get the number of shares for a client
    pub fn share_count(client_id: u64) -> usize {
        let store = stoffel_vm::net::client_store::get_global_store();
        store.get_client_input_count(client_id as usize)
    }

    /// Clear shares for a client
    pub fn clear_shares(client_id: u64) -> bool {
        let store = stoffel_vm::net::client_store::get_global_store();
        store.remove_client_input(client_id as usize).is_some()
    }

    /// List all clients with shares
    pub fn list_clients() -> Vec<u64> {
        let store = stoffel_vm::net::client_store::get_global_store();
        store.list_clients().into_iter().map(|id| id as u64).collect()
    }
}

/// Network configuration builder
///
/// Provides clean abstraction for network setup without exposing
/// internal transport details.
pub struct NetworkBuilder {
    n_parties: usize,
    threshold: usize,
    base_port: u16,
    instance_id: u64,
}

impl NetworkBuilder {
    /// Create a new network builder
    pub fn new(n_parties: usize, threshold: usize) -> Self {
        Self {
            n_parties,
            threshold,
            base_port: 19200,
            instance_id: rand::random(),
        }
    }

    /// Set the base port for the network
    pub fn base_port(mut self, port: u16) -> Self {
        self.base_port = port;
        self
    }

    /// Set the instance ID
    pub fn instance_id(mut self, id: u64) -> Self {
        self.instance_id = id;
        self
    }

    /// Build and return network configuration
    ///
    /// Returns configuration that can be used with the network_helpers module.
    pub fn build(self) -> NetworkConfig {
        NetworkConfig {
            n_parties: self.n_parties,
            threshold: self.threshold,
            base_port: self.base_port,
            instance_id: self.instance_id,
        }
    }
}

/// Network configuration
///
/// Encapsulates all parameters needed for MPC network setup.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub n_parties: usize,
    pub threshold: usize,
    pub base_port: u16,
    pub instance_id: u64,
}

impl NetworkConfig {
    /// Validate network configuration
    pub fn validate(&self) -> Result<()> {
        if self.n_parties < 3 * self.threshold + 1 {
            return Err(Error::Configuration(format!(
                "Invalid configuration: n_parties ({}) must be >= 3*threshold+1 ({})",
                self.n_parties,
                3 * self.threshold + 1
            )));
        }
        Ok(())
    }
}
