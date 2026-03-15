//! Validation logic for Stoffel configuration types.
//!
//! This module provides validation functions for MPC, network, and preprocessing
//! configurations. All validation errors are returned as `Error::Configuration`
//! with clear, actionable messages.

use crate::error::Error;

/// Validate MPC configuration parameters.
///
/// Checks:
/// - `parties >= 4` (minimum for HoneyBadger BFT)
/// - `parties >= 3 * threshold + 1` (HoneyBadger fault tolerance constraint)
pub fn validate_mpc(parties: usize, threshold: usize) -> Result<(), Error> {
    if parties < 4 {
        return Err(Error::Configuration(format!(
            "parties must be >= 4 for HoneyBadger BFT, got {}",
            parties
        )));
    }

    let min_parties = 3 * threshold + 1;
    if parties < min_parties {
        return Err(Error::Configuration(format!(
            "parties must be >= 3 * threshold + 1 for HoneyBadger BFT: \
             parties={} < 3*{}+1={}",
            parties, threshold, min_parties
        )));
    }

    Ok(())
}

/// Validate network configuration parameters.
///
/// Checks:
/// - `expected_parties >= 2` (need at least two parties to communicate)
pub fn validate_network(expected_parties: usize) -> Result<(), Error> {
    if expected_parties < 2 {
        return Err(Error::Configuration(format!(
            "expected_parties must be >= 2, got {}",
            expected_parties
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_mpc_configs() {
        // Minimum valid: 4 parties, threshold 1
        assert!(validate_mpc(4, 1).is_ok());
        // Standard: 5 parties, threshold 1
        assert!(validate_mpc(5, 1).is_ok());
        // Higher tolerance: 7 parties, threshold 2
        assert!(validate_mpc(7, 2).is_ok());
        // Large: 10 parties, threshold 3
        assert!(validate_mpc(10, 3).is_ok());
    }

    #[test]
    fn test_invalid_mpc_too_few_parties() {
        let err = validate_mpc(3, 1).unwrap_err();
        match err {
            Error::Configuration(msg) => assert!(msg.contains("parties must be >= 4")),
            _ => panic!("expected Configuration error"),
        }
    }

    #[test]
    fn test_invalid_mpc_threshold_too_high() {
        // 5 parties with threshold 2: 5 < 3*2+1=7
        let err = validate_mpc(5, 2).unwrap_err();
        match err {
            Error::Configuration(msg) => assert!(msg.contains("3 * threshold + 1")),
            _ => panic!("expected Configuration error"),
        }
    }

    #[test]
    fn test_valid_network() {
        assert!(validate_network(2).is_ok());
        assert!(validate_network(5).is_ok());
    }

    #[test]
    fn test_invalid_network() {
        let err = validate_network(1).unwrap_err();
        match err {
            Error::Configuration(msg) => assert!(msg.contains("expected_parties must be >= 2")),
            _ => panic!("expected Configuration error"),
        }
    }
}
