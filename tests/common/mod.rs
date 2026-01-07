//! Common test utilities for HoneyBadger MPC integration tests
//!
//! This module provides shared test infrastructure including:
//! - Test program constants
//! - Network setup helpers
//! - Port allocation utilities

use stoffel_rust_sdk::prelude::*;
use std::sync::atomic::{AtomicU16, Ordering};

// ============================================================================
// Test Program Constants
// ============================================================================

/// Simple addition program using ClientStore for MPC inputs
/// Expected: a + b where a and b come from client inputs
pub const TEST_PROGRAM_ADD: &str = r#"
main main() -> secret int64:
  var a: secret int64 = ClientStore.take_share(0, 0)
  var b: secret int64 = ClientStore.take_share(0, 1)
  return a + b
"#;

/// Simple program that returns a constant (for basic tests)
pub const TEST_PROGRAM_CONSTANT: &str = "main main() -> int64:\n  return 42\n";

/// Program with three inputs for extended testing
pub const TEST_PROGRAM_THREE_INPUTS: &str = r#"
main main() -> secret int64:
  var a: secret int64 = ClientStore.take_share(0, 0)
  var b: secret int64 = ClientStore.take_share(0, 1)
  var c: secret int64 = ClientStore.take_share(0, 2)
  return a + b + c
"#;

/// Program with multiplication (requires Beaver triples)
pub const TEST_PROGRAM_MULTIPLY: &str = r#"
main main() -> secret int64:
  var a: secret int64 = ClientStore.take_share(0, 0)
  var b: secret int64 = ClientStore.take_share(0, 1)
  return a * b
"#;

// ============================================================================
// Port Allocation
// ============================================================================

/// Global atomic counter for allocating unique port ranges per test
static PORT_COUNTER: AtomicU16 = AtomicU16::new(30000);

/// Get a unique base port for a test
///
/// Each call returns a base port that is at least 100 ports apart from
/// the previous call to avoid port conflicts between parallel tests.
///
/// # Example
///
/// ```
/// let base_port = get_test_port_base();
/// // Use ports base_port, base_port+1, ..., base_port+n for n parties
/// ```
pub fn get_test_port_base() -> u16 {
    PORT_COUNTER.fetch_add(100, Ordering::SeqCst)
}

// ============================================================================
// MPC Configuration Helpers
// ============================================================================

/// Default number of parties for most tests (minimum for HoneyBadger with t=1)
pub const DEFAULT_N_PARTIES: usize = 5;

/// Default threshold for most tests
pub const DEFAULT_THRESHOLD: usize = 1;

/// Default number of Beaver triples for preprocessing
pub const DEFAULT_N_TRIPLES: usize = 3;

/// Default number of random shares for preprocessing
pub const DEFAULT_N_RANDOM_SHARES: usize = 8;

/// Calculate preprocessing start time (seconds from now)
///
/// Returns an absolute epoch time (seconds since Unix epoch) that is
/// `delay_seconds` in the future from now. All servers should use the
/// same value for coordinated preprocessing start.
pub fn calculate_preprocessing_start_time(delay_seconds: u64) -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + delay_seconds
}

// ============================================================================
// Test Validation Helpers
// ============================================================================

/// Validate that an MPC configuration meets HoneyBadger requirements
///
/// HoneyBadger requires n >= 3t + 1 (basic Byzantine)
/// TripleGen requires n >= 4t + 1 (for preprocessing)
pub fn validate_mpc_config(n_parties: usize, threshold: usize) -> Result<()> {
    // Basic HoneyBadger constraint
    if n_parties < 3 * threshold + 1 {
        return Err(Error::Configuration(format!(
            "HoneyBadger requires n >= 3t + 1: {} < {}",
            n_parties,
            3 * threshold + 1
        )));
    }

    // TripleGen preprocessing constraint
    if n_parties < 4 * threshold + 1 {
        return Err(Error::Configuration(format!(
            "TripleGen requires n >= 4t + 1: {} < {}",
            n_parties,
            4 * threshold + 1
        )));
    }

    Ok(())
}

/// Generate peer addresses for a test network
///
/// Returns a vector of (party_id, address) tuples for all parties.
pub fn generate_peer_addresses(n_parties: usize, base_port: u16) -> Vec<(usize, String)> {
    (0..n_parties)
        .map(|i| (i, format!("127.0.0.1:{}", base_port + i as u16)))
        .collect()
}

/// Generate peers list for a specific party (excluding self)
pub fn generate_peers_for_party(
    party_id: usize,
    n_parties: usize,
    base_port: u16,
) -> Vec<(usize, String)> {
    (0..n_parties)
        .filter(|&p| p != party_id)
        .map(|p| (p, format!("127.0.0.1:{}", base_port + p as u16)))
        .collect()
}

// ============================================================================
// Compile Test Helpers
// ============================================================================

/// Compile a test program and return the runtime
pub fn compile_test_program(source: &str) -> Result<StoffelRuntime> {
    Stoffel::compile(source)?
        .parties(DEFAULT_N_PARTIES)
        .threshold(DEFAULT_THRESHOLD)
        .build()
}

/// Compile the default addition test program
pub fn compile_addition_program() -> Result<StoffelRuntime> {
    compile_test_program(TEST_PROGRAM_ADD)
}

// ============================================================================
// Test Result Verification
// ============================================================================

/// Verify that an MPC result matches the expected value
pub fn verify_result(result: &[i64], expected: i64) -> bool {
    result.first() == Some(&expected)
}

/// Verify addition result: a + b should equal expected
pub fn verify_addition_result(result: &[i64], a: i64, b: i64) -> bool {
    verify_result(result, a + b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_allocation_is_unique() {
        let port1 = get_test_port_base();
        let port2 = get_test_port_base();
        let port3 = get_test_port_base();

        assert!(port2 >= port1 + 100);
        assert!(port3 >= port2 + 100);
    }

    #[test]
    fn test_validate_mpc_config_valid() {
        // Valid: 5 >= 4*1 + 1
        assert!(validate_mpc_config(5, 1).is_ok());
        // Valid: 9 >= 4*2 + 1
        assert!(validate_mpc_config(9, 2).is_ok());
    }

    #[test]
    fn test_validate_mpc_config_invalid() {
        // Invalid: 4 < 4*1 + 1
        assert!(validate_mpc_config(4, 1).is_err());
        // Invalid: 7 < 4*2 + 1
        assert!(validate_mpc_config(7, 2).is_err());
    }

    #[test]
    fn test_generate_peer_addresses() {
        let peers = generate_peer_addresses(3, 19200);
        assert_eq!(peers.len(), 3);
        assert_eq!(peers[0], (0, "127.0.0.1:19200".to_string()));
        assert_eq!(peers[1], (1, "127.0.0.1:19201".to_string()));
        assert_eq!(peers[2], (2, "127.0.0.1:19202".to_string()));
    }

    #[test]
    fn test_generate_peers_for_party() {
        let peers = generate_peers_for_party(1, 3, 19200);
        assert_eq!(peers.len(), 2);
        assert!(peers.iter().all(|(id, _)| *id != 1));
    }

    #[test]
    fn test_verify_addition_result() {
        assert!(verify_addition_result(&[142], 42, 100));
        assert!(!verify_addition_result(&[100], 42, 100));
    }

    #[test]
    fn test_compile_test_program() {
        let result = compile_test_program(TEST_PROGRAM_CONSTANT);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_addition_program() {
        let result = compile_addition_program();
        assert!(result.is_ok());
    }
}
