//! HoneyBadger MPC engine re-export (RFC-004).
//!
//! The real `HoneyBadgerMpcEngine` lives in StoffelVM's `net::hb_engine` module.
//! This module re-exports it for SDK users.
//!
//! # Protocol properties
//!
//! - **Fault tolerance**: tolerates up to `t` Byzantine parties where `n >= 3t + 1`.
//! - **Communication model**: fully asynchronous (no timing assumptions).
//! - **Secret sharing**: RobustShare with Reed-Solomon codewords.
//! - **Multiplication**: Beaver-triple-based protocol.

pub use stoffel_vm::net::hb_engine::HoneyBadgerMpcEngine;
