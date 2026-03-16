//! AVSS MPC engine re-export (RFC-005).
//!
//! The real `AvssMpcEngine` lives in StoffelVM's `net::avss_engine` module.
//! This module re-exports it for SDK users.
//!
//! # Protocol properties
//!
//! - **Secret sharing**: Feldman-verifiable shares with polynomial commitments.
//! - **EC support**: operations on elliptic curve points (threshold signatures, DKG).
//! - **Fault tolerance**: `n >= 3t + 1` Byzantine tolerance.

pub use stoffel_vm::net::avss_engine::AvssMpcEngine;
