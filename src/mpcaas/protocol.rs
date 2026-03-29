//! MPCaaS Protocol Messages
//!
//! This module defines the message types for MPCaaS (MPC as a Service)
//! client-server communication. These messages are serialized and wrapped
//! in `NetEnvelope::HoneyBadger` for transport over QUIC.
//!
//! # Architecture
//!
//! The MPCaaS protocol separates concerns:
//! - **Clients**: App developers who submit inputs and receive outputs
//! - **Servers**: Infrastructure operators running MPC nodes
//!
//! Clients don't need to understand MPC internals - they just connect,
//! send inputs, and get results.
//!
//! # Message Flow
//!
//! ```text
//! Client                          Server
//!   |                                |
//!   |--- QUIC Connect -------------->|
//!   |<-- ServerInfo -----------------|
//!   |                                |
//!   |--- ClientReady --------------->|
//!   |                                |
//!   |<-- HoneyBadger messages ------>| (masked input protocol)
//!   |                                |
//!   |<-- ComputationComplete --------|
//!   |                                |
//!   |<-- HoneyBadger messages -------| (output shares)
//!   |                                |
//! ```

use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// Messages for MPCaaS client-server communication
///
/// These messages are exchanged between MPCaaS clients (app developers)
/// and MPCaaS servers (infrastructure operators).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MPCaaSMessage {
    /// Server information sent after client connects
    ///
    /// This allows the client to verify it's connecting to a properly
    /// configured MPC network and to learn network parameters.
    ServerInfo {
        /// Total number of MPC parties in the network
        n_parties: usize,
        /// Byzantine fault tolerance threshold (n >= 3t + 1)
        threshold: usize,
        /// Unique identifier for this computation instance
        instance_id: u64,
        /// This server's party ID (0..n_parties-1)
        party_id: usize,
    },

    /// Client announces readiness with input count
    ///
    /// Sent by the client after receiving ServerInfo from all servers.
    /// This tells the servers how many inputs to expect.
    ClientReady {
        /// Unique identifier for this client
        client_id: usize,
        /// Number of inputs the client will provide
        num_inputs: usize,
    },

    /// Trigger computation (sent by coordinating party)
    ///
    /// In HoneyBadger, party 0 typically coordinates. This message
    /// signals all parties to begin the computation phase.
    ComputationTrigger {
        /// Session identifier for this computation
        session_id: u64,
    },

    /// Computation complete notification
    ///
    /// Sent by servers to inform clients that the MPC computation
    /// has finished and output shares are available.
    ComputationComplete {
        /// Session identifier for this computation
        session_id: u64,
    },

    /// Wrapped HoneyBadger protocol message
    ///
    /// These are the actual MPC protocol messages for:
    /// - Masked input protocol (client -> servers)
    /// - Preprocessing coordination
    /// - Output share distribution (servers -> client)
    HoneyBadger(Vec<u8>),

    /// Error message
    ///
    /// Sent when an error occurs during the protocol.
    Error {
        /// Error code for categorization
        code: ErrorCode,
        /// Human-readable error message
        message: String,
    },

    /// Heartbeat/keepalive
    ///
    /// Used to maintain connection liveness.
    Ping,

    /// Heartbeat response
    Pong,
}

/// Error codes for MPCaaS protocol errors
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    /// Invalid message format or sequence
    InvalidMessage,
    /// MPC configuration mismatch
    ConfigurationMismatch,
    /// Too many clients connected
    TooManyClients,
    /// Server not ready for computation
    NotReady,
    /// Computation timed out
    Timeout,
    /// Internal server error
    InternalError,
    /// Client disconnected unexpectedly
    ClientDisconnected,
    /// Preprocessing material exhausted
    PreprocessingExhausted,
}

/// Magic bytes for identifying MPCaaS messages in the stream
const MPCAAS_MAGIC: [u8; 4] = [0x4D, 0x50, 0x43, 0x53]; // "MPCS"

/// Current protocol version
const PROTOCOL_VERSION: u8 = 1;

/// Serialize an MPCaaS message for transport
///
/// Format: [MAGIC:4][VERSION:1][LENGTH:4][PAYLOAD:LENGTH]
///
/// # Arguments
///
/// * `msg` - The message to serialize
///
/// # Returns
///
/// Serialized bytes ready for transport
pub fn serialize_message(msg: &MPCaaSMessage) -> Result<Vec<u8>> {
    let payload = bincode::serialize(msg)
        .map_err(|e| Error::Network(format!("Failed to serialize MPCaaS message: {}", e)))?;

    let length = payload.len() as u32;

    let mut result = Vec::with_capacity(4 + 1 + 4 + payload.len());
    result.extend_from_slice(&MPCAAS_MAGIC);
    result.push(PROTOCOL_VERSION);
    result.extend_from_slice(&length.to_be_bytes());
    result.extend_from_slice(&payload);

    Ok(result)
}

/// Deserialize an MPCaaS message from transport bytes
///
/// # Arguments
///
/// * `data` - Raw bytes from transport
///
/// # Returns
///
/// Parsed message and the number of bytes consumed
pub fn deserialize_message(data: &[u8]) -> Result<(MPCaaSMessage, usize)> {
    // Minimum size: magic (4) + version (1) + length (4) = 9 bytes
    if data.len() < 9 {
        return Err(Error::Network("Message too short".to_string()));
    }

    // Check magic bytes
    if &data[0..4] != &MPCAAS_MAGIC {
        return Err(Error::Network("Invalid MPCaaS magic bytes".to_string()));
    }

    // Check version
    let version = data[4];
    if version != PROTOCOL_VERSION {
        return Err(Error::Network(format!(
            "Unsupported protocol version: {} (expected {})",
            version, PROTOCOL_VERSION
        )));
    }

    // Read length
    let length = u32::from_be_bytes([data[5], data[6], data[7], data[8]]) as usize;

    // Check we have enough data
    let total_size = 9 + length;
    if data.len() < total_size {
        return Err(Error::Network(format!(
            "Incomplete message: expected {} bytes, got {}",
            total_size,
            data.len()
        )));
    }

    // Deserialize payload
    let payload = &data[9..total_size];
    let msg: MPCaaSMessage = bincode::deserialize(payload)
        .map_err(|e| Error::Network(format!("Failed to deserialize MPCaaS message: {}", e)))?;

    Ok((msg, total_size))
}

/// Wrap an MPCaaS message for transport over NetEnvelope::HoneyBadger
///
/// This is a convenience function that serializes the message for use
/// with the stoffelnet transport layer.
pub fn wrap_for_transport(msg: &MPCaaSMessage) -> Result<Vec<u8>> {
    serialize_message(msg)
}

/// Unwrap an MPCaaS message from transport bytes
///
/// This is a convenience function for deserializing messages received
/// from the stoffelnet transport layer.
pub fn unwrap_from_transport(data: &[u8]) -> Result<MPCaaSMessage> {
    let (msg, _) = deserialize_message(data)?;
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_server_info() {
        let msg = MPCaaSMessage::ServerInfo {
            n_parties: 4,
            threshold: 1,
            instance_id: 42,
            party_id: 0,
        };

        let data = serialize_message(&msg).unwrap();
        let (parsed, consumed) = deserialize_message(&data).unwrap();

        assert_eq!(consumed, data.len());
        match parsed {
            MPCaaSMessage::ServerInfo {
                n_parties,
                threshold,
                instance_id,
                party_id,
            } => {
                assert_eq!(n_parties, 4);
                assert_eq!(threshold, 1);
                assert_eq!(instance_id, 42);
                assert_eq!(party_id, 0);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_serialize_deserialize_client_ready() {
        let msg = MPCaaSMessage::ClientReady {
            client_id: 100,
            num_inputs: 2,
        };

        let data = wrap_for_transport(&msg).unwrap();
        let parsed = unwrap_from_transport(&data).unwrap();

        match parsed {
            MPCaaSMessage::ClientReady {
                client_id,
                num_inputs,
            } => {
                assert_eq!(client_id, 100);
                assert_eq!(num_inputs, 2);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_serialize_deserialize_honeybadger() {
        let msg = MPCaaSMessage::HoneyBadger(vec![1, 2, 3, 4, 5]);

        let data = serialize_message(&msg).unwrap();
        let (parsed, _) = deserialize_message(&data).unwrap();

        match parsed {
            MPCaaSMessage::HoneyBadger(payload) => {
                assert_eq!(payload, vec![1, 2, 3, 4, 5]);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_invalid_magic() {
        let data = vec![0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00];
        let result = deserialize_message(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_message_too_short() {
        let data = vec![0x4D, 0x50, 0x43, 0x53];
        let result = deserialize_message(&data);
        assert!(result.is_err());
    }
}
