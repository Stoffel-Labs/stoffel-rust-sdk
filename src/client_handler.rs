//! Client Handler for Dynamic Client Management
//!
//! This module provides the `ClientHandler` type for managing dynamic client
//! connections on the server side. Servers accept any client connection without
//! prior registration.
//!
//! # Design
//!
//! The client handler maintains:
//! - A registry of connected clients
//! - Input buffers for received shares
//! - Output buffers for computed results

use crate::{Error, Result};
use ark_bls12_381::Fr;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

/// Unique identifier for a client
pub type ClientId = usize;

/// State of a connected client
#[derive(Debug, Clone)]
pub struct ClientState {
    /// Client ID
    pub client_id: ClientId,
    /// When the client connected
    pub connected_at: Instant,
    /// Whether inputs have been received
    pub inputs_received: bool,
    /// Whether outputs have been sent
    pub outputs_sent: bool,
}

impl ClientState {
    /// Create a new client state
    pub fn new(client_id: ClientId) -> Self {
        Self {
            client_id,
            connected_at: Instant::now(),
            inputs_received: false,
            outputs_sent: false,
        }
    }
}

/// Handler for managing client connections
///
/// This is used by `StoffelServer` to manage dynamic client registration,
/// input reception, and output distribution.
pub struct ClientHandler {
    /// Registered clients and their state
    registered_clients: Arc<Mutex<HashMap<ClientId, ClientState>>>,
    /// Input shares received from clients
    /// Key: client_id, Value: shares for that client
    input_buffer: Arc<Mutex<HashMap<ClientId, Vec<Vec<u8>>>>>,
    /// Output shares to send to clients
    output_buffer: Arc<Mutex<HashMap<ClientId, Vec<Vec<u8>>>>>,
}

impl ClientHandler {
    /// Create a new client handler
    pub fn new() -> Self {
        Self {
            registered_clients: Arc::new(Mutex::new(HashMap::new())),
            input_buffer: Arc::new(Mutex::new(HashMap::new())),
            output_buffer: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Accept a new client connection (dynamic registration)
    ///
    /// Clients are registered when they first connect. The server doesn't
    /// require pre-registration of clients.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The client's unique identifier
    ///
    /// # Returns
    ///
    /// The assigned client ID (same as input for now).
    pub async fn accept_client(&self, client_id: ClientId) -> Result<ClientId> {
        let mut clients = self.registered_clients.lock().await;

        if clients.contains_key(&client_id) {
            return Err(Error::InvalidInput(format!(
                "Client {} is already registered",
                client_id
            )));
        }

        clients.insert(client_id, ClientState::new(client_id));
        tracing::info!("Accepted client {}", client_id);

        Ok(client_id)
    }

    /// Store input shares received from a client
    ///
    /// # Arguments
    ///
    /// * `client_id` - The client's ID
    /// * `shares` - Serialized input shares
    pub async fn store_inputs(&self, client_id: ClientId, shares: Vec<u8>) -> Result<()> {
        let mut inputs = self.input_buffer.lock().await;
        let entry = inputs.entry(client_id).or_insert_with(Vec::new);
        entry.push(shares);

        // Mark client as having received inputs
        let mut clients = self.registered_clients.lock().await;
        if let Some(state) = clients.get_mut(&client_id) {
            state.inputs_received = true;
        }

        tracing::debug!("Stored input shares from client {}", client_id);
        Ok(())
    }

    /// Get all input shares for a client
    ///
    /// # Arguments
    ///
    /// * `client_id` - The client's ID
    ///
    /// # Returns
    ///
    /// All stored input shares for this client.
    pub async fn get_inputs(&self, client_id: ClientId) -> Option<Vec<Vec<u8>>> {
        let inputs = self.input_buffer.lock().await;
        inputs.get(&client_id).cloned()
    }

    /// Store output shares to send to a client
    ///
    /// # Arguments
    ///
    /// * `client_id` - The client's ID
    /// * `shares` - Serialized output shares
    pub async fn store_outputs(&self, client_id: ClientId, shares: Vec<u8>) -> Result<()> {
        let mut outputs = self.output_buffer.lock().await;
        let entry = outputs.entry(client_id).or_insert_with(Vec::new);
        entry.push(shares);

        tracing::debug!("Stored output shares for client {}", client_id);
        Ok(())
    }

    /// Get all output shares for a client
    ///
    /// # Arguments
    ///
    /// * `client_id` - The client's ID
    ///
    /// # Returns
    ///
    /// All stored output shares for this client.
    pub async fn get_outputs(&self, client_id: ClientId) -> Option<Vec<Vec<u8>>> {
        let outputs = self.output_buffer.lock().await;
        outputs.get(&client_id).cloned()
    }

    /// Mark that outputs have been sent to a client
    pub async fn mark_outputs_sent(&self, client_id: ClientId) {
        let mut clients = self.registered_clients.lock().await;
        if let Some(state) = clients.get_mut(&client_id) {
            state.outputs_sent = true;
        }
    }

    /// Get list of connected clients
    pub async fn connected_clients(&self) -> Vec<ClientId> {
        let clients = self.registered_clients.lock().await;
        clients.keys().copied().collect()
    }

    /// Check if a client is registered
    pub async fn is_registered(&self, client_id: ClientId) -> bool {
        let clients = self.registered_clients.lock().await;
        clients.contains_key(&client_id)
    }

    /// Get the number of connected clients
    pub async fn client_count(&self) -> usize {
        let clients = self.registered_clients.lock().await;
        clients.len()
    }

    /// Remove a client (on disconnect)
    pub async fn remove_client(&self, client_id: ClientId) {
        let mut clients = self.registered_clients.lock().await;
        clients.remove(&client_id);

        let mut inputs = self.input_buffer.lock().await;
        inputs.remove(&client_id);

        let mut outputs = self.output_buffer.lock().await;
        outputs.remove(&client_id);

        tracing::info!("Removed client {}", client_id);
    }

    /// Clear all buffers for a fresh computation
    pub async fn clear_buffers(&self) {
        let mut inputs = self.input_buffer.lock().await;
        inputs.clear();

        let mut outputs = self.output_buffer.lock().await;
        outputs.clear();

        let mut clients = self.registered_clients.lock().await;
        for state in clients.values_mut() {
            state.inputs_received = false;
            state.outputs_sent = false;
        }

        tracing::debug!("Cleared all client buffers");
    }
}

impl Default for ClientHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_accept_client() {
        let handler = ClientHandler::new();

        let client_id = handler.accept_client(100).await.unwrap();
        assert_eq!(client_id, 100);
        assert!(handler.is_registered(100).await);
    }

    #[tokio::test]
    async fn test_duplicate_client() {
        let handler = ClientHandler::new();

        handler.accept_client(100).await.unwrap();
        let result = handler.accept_client(100).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_store_and_get_inputs() {
        let handler = ClientHandler::new();

        handler.accept_client(100).await.unwrap();
        handler.store_inputs(100, vec![1, 2, 3]).await.unwrap();

        let inputs = handler.get_inputs(100).await;
        assert!(inputs.is_some());
        assert_eq!(inputs.unwrap(), vec![vec![1, 2, 3]]);
    }
}
