//! Shared types used across the Stoffel SDK
//!
//! This module contains common type aliases and newtypes used throughout the SDK.

/// Party identifier for MPC compute nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PartyId(pub usize);

impl From<usize> for PartyId {
    fn from(id: usize) -> Self {
        PartyId(id)
    }
}

impl From<PartyId> for usize {
    fn from(id: PartyId) -> Self {
        id.0
    }
}

/// Client identifier for MPC input providers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClientId(pub u64);

impl From<u64> for ClientId {
    fn from(id: u64) -> Self {
        ClientId(id)
    }
}

/// Computation identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ComputationId(pub u64);

/// MPC protocol value - represents inputs and outputs.
///
/// This wraps the VM's value type for use in MPC operations.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int64(i64),
    Bool(bool),
    /// Raw field element bytes
    FieldElement(Vec<u8>),
}

impl Value {
    /// Try to extract as i64.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int64(v) => Some(*v),
            _ => None,
        }
    }

    /// Try to extract as bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(v) => Some(*v),
            _ => None,
        }
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int64(v)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}
