//! Configuration system for the Stoffel SDK (RFC-008).
//!
//! This module provides serializable configuration types for MPC computations,
//! networking, and preprocessing. All types support TOML serialization via serde,
//! sensible defaults, and validation.
//!
//! # Top-level configuration
//!
//! The [`StoffelConfig`] struct is the top-level configuration that can be loaded
//! from a TOML file. It aggregates [`MpcConfig`], [`NetworkConfig`], and
//! [`PreprocessingConfig`].
//!
//! ```toml
//! [mpc]
//! parties = 5
//! threshold = 1
//!
//! [mpc.backend]
//! protocol = "honeybadger"
//!
//! [network]
//! party_id = 0
//! bind_address = "127.0.0.1:9000"
//! expected_parties = 5
//! consensus_timeout_ms = 30000
//!
//! [network.peers]
//! 1 = "127.0.0.1:9001"
//! 2 = "127.0.0.1:9002"
//!
//! [preprocessing]
//! triples = 1000
//! random_shares = 500
//! ```
//!
//! # Environment variable overrides
//!
//! Use [`StoffelConfig::load_with_env`] to apply environment variable overrides
//! on top of file-based configuration. Override priority: Env > File > Default.

pub mod validation;

use crate::error::Error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;

// ---------------------------------------------------------------------------
// Curve
// ---------------------------------------------------------------------------

/// Elliptic curve selection for MPC backends that require one.
///
/// The default curve is BLS12-381, which provides 128-bit security and is
/// widely used in zero-knowledge and MPC protocols.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Curve {
    /// BLS12-381 curve (default). 381-bit field, 128-bit security.
    #[default]
    Bls12_381,
    /// BN254 curve. 254-bit field, ~100-bit security.
    Bn254,
    /// Curve25519. 255-bit field, 128-bit security.
    Curve25519,
    /// Ed25519 (Edwards form of Curve25519). 255-bit field, 128-bit security.
    Ed25519,
}

impl Curve {
    /// Returns the number of bits in the base field of this curve.
    pub fn field_bits(&self) -> usize {
        match self {
            Curve::Bls12_381 => 381,
            Curve::Bn254 => 254,
            Curve::Curve25519 => 255,
            Curve::Ed25519 => 255,
        }
    }

    /// Returns the estimated security level in bits.
    pub fn security_bits(&self) -> usize {
        match self {
            Curve::Bls12_381 => 128,
            Curve::Bn254 => 100,
            Curve::Curve25519 => 128,
            Curve::Ed25519 => 128,
        }
    }
}

// ---------------------------------------------------------------------------
// MpcBackendConfig
// ---------------------------------------------------------------------------

/// Serializable MPC backend configuration for config files.
///
/// This is a tagged enum so that the TOML representation clearly indicates
/// which protocol is selected. The default backend is HoneyBadger.
///
/// # TOML examples
///
/// ```toml
/// # HoneyBadger (default)
/// [mpc.backend]
/// protocol = "honeybadger"
///
/// # AVSS with explicit curve
/// [mpc.backend]
/// protocol = "avss"
/// curve = "bn254"
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "protocol", rename_all = "lowercase")]
pub enum MpcBackendConfig {
    /// HoneyBadger Byzantine fault-tolerant protocol (default).
    #[default]
    HoneyBadger,
    /// Asynchronous Verifiable Secret Sharing protocol.
    Avss {
        /// The elliptic curve to use. Defaults to BLS12-381.
        #[serde(default)]
        curve: Curve,
    },
}

// ---------------------------------------------------------------------------
// MpcConfig
// ---------------------------------------------------------------------------

/// Returns the default number of parties (5).
fn default_parties() -> usize {
    5
}

/// Returns the default threshold (1).
fn default_threshold() -> usize {
    1
}

/// Generates a random instance ID using the `rand` crate.
fn generate_instance_id() -> u64 {
    use rand::Rng;
    rand::thread_rng().gen()
}

/// MPC computation configuration.
///
/// Controls the number of parties, fault-tolerance threshold, instance
/// identifier, and which MPC backend protocol to use.
///
/// # Defaults
///
/// | Field         | Default       |
/// |---------------|---------------|
/// | `parties`     | 5             |
/// | `threshold`   | 1             |
/// | `instance_id` | random `u64`  |
/// | `backend`     | HoneyBadger   |
///
/// # Validation
///
/// Call [`MpcConfig::validate`] to check:
/// - `parties >= 4`
/// - `parties >= 3 * threshold + 1`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MpcConfig {
    /// Total number of MPC server parties. Must be >= 4.
    #[serde(default = "default_parties")]
    pub parties: usize,

    /// Fault-tolerance threshold. The protocol tolerates up to `threshold`
    /// Byzantine parties, requiring `parties >= 3 * threshold + 1`.
    #[serde(default = "default_threshold")]
    pub threshold: usize,

    /// Unique identifier for this MPC computation instance.
    /// Defaults to a random value.
    #[serde(default = "generate_instance_id")]
    pub instance_id: u64,

    /// MPC backend protocol configuration.
    #[serde(default)]
    pub backend: MpcBackendConfig,
}

impl Default for MpcConfig {
    fn default() -> Self {
        Self {
            parties: default_parties(),
            threshold: default_threshold(),
            instance_id: generate_instance_id(),
            backend: MpcBackendConfig::default(),
        }
    }
}

impl MpcConfig {
    /// Validate this MPC configuration.
    ///
    /// Returns `Error::Configuration` if any constraint is violated.
    pub fn validate(&self) -> Result<(), Error> {
        validation::validate_mpc(self.parties, self.threshold)
    }
}

// ---------------------------------------------------------------------------
// NetworkConfig
// ---------------------------------------------------------------------------

/// Returns the default bind address (`0.0.0.0:9000`).
fn default_bind_address() -> SocketAddr {
    "0.0.0.0:9000".parse().unwrap()
}

/// Returns the default consensus timeout in milliseconds (30 000 ms = 30 s).
fn default_consensus_timeout() -> u64 {
    30_000
}

/// Network configuration for an MPC party.
///
/// Describes how this party binds, which peers it knows about, and
/// timeout parameters for the consensus protocol.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// This party's numeric identifier (0-indexed).
    pub party_id: usize,

    /// The local address to bind to. Defaults to `0.0.0.0:9000`.
    #[serde(default = "default_bind_address")]
    pub bind_address: SocketAddr,

    /// Optional dedicated MPC protocol port. When set, MPC traffic is
    /// separated from other services.
    #[serde(default)]
    pub mpc_port: Option<u16>,

    /// Number of server parties expected in the MPC network.
    pub expected_parties: usize,

    /// Optional number of clients expected to submit inputs.
    #[serde(default)]
    pub expected_clients: Option<usize>,

    /// Consensus round timeout in milliseconds. Defaults to 30 000 ms.
    #[serde(default = "default_consensus_timeout")]
    pub consensus_timeout_ms: u64,

    /// Map from party ID to socket address string for all known peers.
    #[serde(default)]
    pub peers: HashMap<String, String>,
}

impl NetworkConfig {
    /// Validate this network configuration.
    ///
    /// Returns `Error::Configuration` if any constraint is violated.
    pub fn validate(&self) -> Result<(), Error> {
        validation::validate_network(self.expected_parties)
    }
}

// ---------------------------------------------------------------------------
// PreprocessingConfig
// ---------------------------------------------------------------------------

/// Configuration for MPC preprocessing material generation.
///
/// Preprocessing creates cryptographic material (Beaver triples, random shares)
/// that is consumed during secure computation.
///
/// # Defaults
///
/// | Field                 | Default |
/// |-----------------------|---------|
/// | `triples`             | 1000    |
/// | `random_shares`       | 500     |
/// | `min_triples`         | 100     |
/// | `generate_on_startup` | true    |
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreprocessingConfig {
    /// Number of Beaver triples to generate. Default: 1000.
    #[serde(default = "default_triples")]
    pub triples: usize,

    /// Number of random shares to generate. Default: 500.
    #[serde(default = "default_random_shares")]
    pub random_shares: usize,

    /// Minimum number of triples before triggering regeneration. Default: 100.
    #[serde(default = "default_min_triples")]
    pub min_triples: usize,

    /// Whether to generate preprocessing material on startup. Default: true.
    #[serde(default = "default_generate_on_startup")]
    pub generate_on_startup: bool,
}

fn default_triples() -> usize {
    1000
}
fn default_random_shares() -> usize {
    500
}
fn default_min_triples() -> usize {
    100
}
fn default_generate_on_startup() -> bool {
    true
}

impl Default for PreprocessingConfig {
    fn default() -> Self {
        Self {
            triples: default_triples(),
            random_shares: default_random_shares(),
            min_triples: default_min_triples(),
            generate_on_startup: default_generate_on_startup(),
        }
    }
}

// ---------------------------------------------------------------------------
// StoffelConfig
// ---------------------------------------------------------------------------

/// Top-level Stoffel configuration, loadable from a TOML file.
///
/// Aggregates MPC, network, and preprocessing settings into a single
/// configuration structure.
///
/// # Loading
///
/// ```rust,no_run
/// # use stoffel_rust_sdk::config::StoffelConfig;
/// // From a TOML file
/// let config = StoffelConfig::load("stoffel.toml").unwrap();
///
/// // From a TOML file with environment variable overrides
/// let config = StoffelConfig::load_with_env("stoffel.toml").unwrap();
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoffelConfig {
    /// MPC computation parameters.
    #[serde(default)]
    pub mpc: MpcConfig,

    /// Network configuration for this party.
    pub network: NetworkConfig,

    /// Preprocessing material generation settings.
    #[serde(default)]
    pub preprocessing: PreprocessingConfig,
}

impl StoffelConfig {
    /// Load configuration from a TOML file.
    ///
    /// # Errors
    ///
    /// Returns `Error::Io` if the file cannot be read, or
    /// `Error::Configuration` if the TOML is malformed.
    pub fn load(path: &str) -> Result<Self, Error> {
        let content = std::fs::read_to_string(path)?;
        let config: Self =
            toml::from_str(&content).map_err(|e| Error::Configuration(format!("TOML parse error: {e}")))?;
        Ok(config)
    }

    /// Load configuration from a TOML file with environment variable overrides.
    ///
    /// Override priority: **Env > File > Default**.
    ///
    /// # Supported environment variables
    ///
    /// | Variable                | Overrides                     |
    /// |-------------------------|-------------------------------|
    /// | `STOFFEL_PARTIES`       | `mpc.parties`                 |
    /// | `STOFFEL_THRESHOLD`     | `mpc.threshold`               |
    /// | `STOFFEL_INSTANCE_ID`   | `mpc.instance_id`             |
    /// | `STOFFEL_BACKEND`       | `mpc.backend` protocol tag    |
    /// | `STOFFEL_CURVE`         | `mpc.backend.curve` (AVSS)    |
    /// | `STOFFEL_PARTY_ID`      | `network.party_id`            |
    /// | `STOFFEL_BIND_ADDRESS`  | `network.bind_address`        |
    /// | `STOFFEL_MPC_PORT`      | `network.mpc_port`            |
    /// | `STOFFEL_EXPECTED_PARTIES` | `network.expected_parties`  |
    /// | `STOFFEL_CONSENSUS_TIMEOUT_MS` | `network.consensus_timeout_ms` |
    /// | `STOFFEL_TRIPLES`       | `preprocessing.triples`       |
    /// | `STOFFEL_RANDOM_SHARES` | `preprocessing.random_shares` |
    /// | `STOFFEL_MIN_TRIPLES`   | `preprocessing.min_triples`   |
    /// | `STOFFEL_GENERATE_ON_STARTUP` | `preprocessing.generate_on_startup` |
    ///
    /// # Errors
    ///
    /// Returns `Error::Configuration` if an environment variable value cannot
    /// be parsed into the expected type.
    pub fn load_with_env(path: &str) -> Result<Self, Error> {
        let mut config = Self::load(path)?;
        config.apply_env_overrides()?;
        Ok(config)
    }

    /// Apply environment variable overrides to this configuration.
    fn apply_env_overrides(&mut self) -> Result<(), Error> {
        // --- MPC overrides ---
        if let Ok(val) = std::env::var("STOFFEL_PARTIES") {
            self.mpc.parties = parse_env("STOFFEL_PARTIES", &val)?;
        }
        if let Ok(val) = std::env::var("STOFFEL_THRESHOLD") {
            self.mpc.threshold = parse_env("STOFFEL_THRESHOLD", &val)?;
        }
        if let Ok(val) = std::env::var("STOFFEL_INSTANCE_ID") {
            self.mpc.instance_id = parse_env("STOFFEL_INSTANCE_ID", &val)?;
        }
        if let Ok(val) = std::env::var("STOFFEL_BACKEND") {
            self.mpc.backend = match val.to_lowercase().as_str() {
                "honeybadger" => MpcBackendConfig::HoneyBadger,
                "avss" => {
                    // Preserve existing curve if already AVSS, otherwise default
                    let curve = match &self.mpc.backend {
                        MpcBackendConfig::Avss { curve } => *curve,
                        _ => Curve::default(),
                    };
                    MpcBackendConfig::Avss { curve }
                }
                other => {
                    return Err(Error::Configuration(format!(
                        "STOFFEL_BACKEND: unknown backend '{}', expected 'honeybadger' or 'avss'",
                        other
                    )));
                }
            };
        }
        if let Ok(val) = std::env::var("STOFFEL_CURVE") {
            let curve = match val.to_lowercase().as_str() {
                "bls12-381" | "bls12_381" => Curve::Bls12_381,
                "bn254" => Curve::Bn254,
                "curve25519" => Curve::Curve25519,
                "ed25519" => Curve::Ed25519,
                other => {
                    return Err(Error::Configuration(format!(
                        "STOFFEL_CURVE: unknown curve '{}', expected one of: \
                         bls12-381, bn254, curve25519, ed25519",
                        other
                    )));
                }
            };
            // Only applies to AVSS backend; promote to AVSS if needed
            self.mpc.backend = match self.mpc.backend {
                MpcBackendConfig::Avss { .. } => MpcBackendConfig::Avss { curve },
                _ => MpcBackendConfig::Avss { curve },
            };
        }

        // --- Network overrides ---
        if let Ok(val) = std::env::var("STOFFEL_PARTY_ID") {
            self.network.party_id = parse_env("STOFFEL_PARTY_ID", &val)?;
        }
        if let Ok(val) = std::env::var("STOFFEL_BIND_ADDRESS") {
            self.network.bind_address = val.parse().map_err(|e| {
                Error::Configuration(format!("STOFFEL_BIND_ADDRESS: invalid address '{}': {}", val, e))
            })?;
        }
        if let Ok(val) = std::env::var("STOFFEL_MPC_PORT") {
            self.network.mpc_port = Some(parse_env("STOFFEL_MPC_PORT", &val)?);
        }
        if let Ok(val) = std::env::var("STOFFEL_EXPECTED_PARTIES") {
            self.network.expected_parties = parse_env("STOFFEL_EXPECTED_PARTIES", &val)?;
        }
        if let Ok(val) = std::env::var("STOFFEL_CONSENSUS_TIMEOUT_MS") {
            self.network.consensus_timeout_ms = parse_env("STOFFEL_CONSENSUS_TIMEOUT_MS", &val)?;
        }

        // --- Preprocessing overrides ---
        if let Ok(val) = std::env::var("STOFFEL_TRIPLES") {
            self.preprocessing.triples = parse_env("STOFFEL_TRIPLES", &val)?;
        }
        if let Ok(val) = std::env::var("STOFFEL_RANDOM_SHARES") {
            self.preprocessing.random_shares = parse_env("STOFFEL_RANDOM_SHARES", &val)?;
        }
        if let Ok(val) = std::env::var("STOFFEL_MIN_TRIPLES") {
            self.preprocessing.min_triples = parse_env("STOFFEL_MIN_TRIPLES", &val)?;
        }
        if let Ok(val) = std::env::var("STOFFEL_GENERATE_ON_STARTUP") {
            self.preprocessing.generate_on_startup = match val.to_lowercase().as_str() {
                "true" | "1" | "yes" => true,
                "false" | "0" | "no" => false,
                other => {
                    return Err(Error::Configuration(format!(
                        "STOFFEL_GENERATE_ON_STARTUP: expected boolean, got '{}'",
                        other
                    )));
                }
            };
        }

        Ok(())
    }

    /// Validate the entire configuration.
    ///
    /// Runs validation on all sub-configurations.
    pub fn validate(&self) -> Result<(), Error> {
        self.mpc.validate()?;
        self.network.validate()?;
        Ok(())
    }
}

/// Helper to parse an environment variable value into a type that implements
/// `FromStr`, returning a `Configuration` error on failure.
fn parse_env<T: std::str::FromStr>(var_name: &str, val: &str) -> Result<T, Error>
where
    T::Err: std::fmt::Display,
{
    val.parse().map_err(|e| {
        Error::Configuration(format!("{}: invalid value '{}': {}", var_name, val, e))
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mpc_config_defaults() {
        let config = MpcConfig::default();
        assert_eq!(config.parties, 5);
        assert_eq!(config.threshold, 1);
        assert_eq!(config.backend, MpcBackendConfig::HoneyBadger);
        // instance_id is random, just check it exists
        let _ = config.instance_id;
    }

    #[test]
    fn test_mpc_config_validate_ok() {
        let config = MpcConfig {
            parties: 5,
            threshold: 1,
            instance_id: 42,
            backend: MpcBackendConfig::HoneyBadger,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_mpc_config_validate_too_few_parties() {
        let config = MpcConfig {
            parties: 3,
            threshold: 1,
            instance_id: 0,
            backend: MpcBackendConfig::HoneyBadger,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_mpc_config_validate_threshold_too_high() {
        let config = MpcConfig {
            parties: 5,
            threshold: 2,
            instance_id: 0,
            backend: MpcBackendConfig::HoneyBadger,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_preprocessing_defaults() {
        let config = PreprocessingConfig::default();
        assert_eq!(config.triples, 1000);
        assert_eq!(config.random_shares, 500);
        assert_eq!(config.min_triples, 100);
        assert!(config.generate_on_startup);
    }

    #[test]
    fn test_curve_field_bits() {
        assert_eq!(Curve::Bls12_381.field_bits(), 381);
        assert_eq!(Curve::Bn254.field_bits(), 254);
        assert_eq!(Curve::Curve25519.field_bits(), 255);
        assert_eq!(Curve::Ed25519.field_bits(), 255);
    }

    #[test]
    fn test_curve_security_bits() {
        assert_eq!(Curve::Bls12_381.security_bits(), 128);
        assert_eq!(Curve::Bn254.security_bits(), 100);
        assert_eq!(Curve::Curve25519.security_bits(), 128);
        assert_eq!(Curve::Ed25519.security_bits(), 128);
    }

    #[test]
    fn test_curve_default() {
        assert_eq!(Curve::default(), Curve::Bls12_381);
    }

    #[test]
    fn test_backend_default() {
        assert_eq!(MpcBackendConfig::default(), MpcBackendConfig::HoneyBadger);
    }

    #[test]
    fn test_mpc_backend_serde_honeybadger() {
        let backend = MpcBackendConfig::HoneyBadger;
        let toml_str = toml::to_string(&backend).unwrap();
        assert!(toml_str.contains("honeybadger"));
        let parsed: MpcBackendConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed, MpcBackendConfig::HoneyBadger);
    }

    #[test]
    fn test_mpc_backend_serde_avss() {
        let backend = MpcBackendConfig::Avss {
            curve: Curve::Bn254,
        };
        let toml_str = toml::to_string(&backend).unwrap();
        assert!(toml_str.contains("avss"));
        assert!(toml_str.contains("bn254"));
        let parsed: MpcBackendConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed, backend);
    }

    #[test]
    fn test_curve_serde_roundtrip() {
        // TOML can't serialize a bare enum; wrap in a struct
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct W { curve: Curve }
        for curve in [Curve::Bls12_381, Curve::Bn254, Curve::Curve25519, Curve::Ed25519] {
            let w = W { curve };
            let s = toml::to_string(&w).unwrap();
            let parsed: W = toml::from_str(&s).unwrap();
            assert_eq!(parsed.curve, curve);
        }
    }

    #[test]
    fn test_network_config_validate_ok() {
        let config = NetworkConfig {
            party_id: 0,
            bind_address: "127.0.0.1:9000".parse().unwrap(),
            mpc_port: None,
            expected_parties: 5,
            expected_clients: None,
            consensus_timeout_ms: 30_000,
            peers: HashMap::new(),
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_full_toml_roundtrip() {
        let toml_input = r#"
[mpc]
parties = 7
threshold = 2
instance_id = 12345

[mpc.backend]
protocol = "honeybadger"

[network]
party_id = 0
bind_address = "127.0.0.1:9000"
expected_parties = 7
consensus_timeout_ms = 60000

[network.peers]
1 = "127.0.0.1:9001"
2 = "127.0.0.1:9002"

[preprocessing]
triples = 2000
random_shares = 1000
min_triples = 200
generate_on_startup = false
"#;

        let config: StoffelConfig = toml::from_str(toml_input).unwrap();
        assert_eq!(config.mpc.parties, 7);
        assert_eq!(config.mpc.threshold, 2);
        assert_eq!(config.mpc.instance_id, 12345);
        assert_eq!(config.mpc.backend, MpcBackendConfig::HoneyBadger);
        assert_eq!(config.network.party_id, 0);
        assert_eq!(config.network.expected_parties, 7);
        assert_eq!(config.network.consensus_timeout_ms, 60_000);
        assert_eq!(config.network.peers.get("1").unwrap(), "127.0.0.1:9001");
        assert_eq!(config.preprocessing.triples, 2000);
        assert_eq!(config.preprocessing.random_shares, 1000);
        assert_eq!(config.preprocessing.min_triples, 200);
        assert!(!config.preprocessing.generate_on_startup);

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_toml_defaults_applied() {
        // Minimal TOML: only required fields
        let toml_input = r#"
[network]
party_id = 0
expected_parties = 5
"#;
        let config: StoffelConfig = toml::from_str(toml_input).unwrap();
        assert_eq!(config.mpc.parties, 5);
        assert_eq!(config.mpc.threshold, 1);
        assert_eq!(config.mpc.backend, MpcBackendConfig::HoneyBadger);
        assert_eq!(config.preprocessing.triples, 1000);
        assert_eq!(config.preprocessing.random_shares, 500);
        assert_eq!(config.preprocessing.min_triples, 100);
        assert!(config.preprocessing.generate_on_startup);
        assert_eq!(
            config.network.bind_address,
            "0.0.0.0:9000".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(config.network.consensus_timeout_ms, 30_000);
    }

    #[test]
    fn test_toml_avss_backend() {
        let toml_input = r#"
[mpc.backend]
protocol = "avss"
curve = "curve25519"

[network]
party_id = 0
expected_parties = 5
"#;
        let config: StoffelConfig = toml::from_str(toml_input).unwrap();
        assert_eq!(
            config.mpc.backend,
            MpcBackendConfig::Avss {
                curve: Curve::Curve25519
            }
        );
    }

    #[test]
    fn test_env_override_parties() {
        // Create a minimal config
        let mut config = StoffelConfig {
            mpc: MpcConfig {
                parties: 5,
                threshold: 1,
                instance_id: 1,
                backend: MpcBackendConfig::HoneyBadger,
            },
            network: NetworkConfig {
                party_id: 0,
                bind_address: "127.0.0.1:9000".parse().unwrap(),
                mpc_port: None,
                expected_parties: 5,
                expected_clients: None,
                consensus_timeout_ms: 30_000,
                peers: HashMap::new(),
            },
            preprocessing: PreprocessingConfig::default(),
        };

        // Set env var and apply
        std::env::set_var("STOFFEL_PARTIES", "7");
        std::env::set_var("STOFFEL_THRESHOLD", "2");
        config.apply_env_overrides().unwrap();
        assert_eq!(config.mpc.parties, 7);
        assert_eq!(config.mpc.threshold, 2);

        // Clean up
        std::env::remove_var("STOFFEL_PARTIES");
        std::env::remove_var("STOFFEL_THRESHOLD");
    }

    #[test]
    fn test_env_override_backend() {
        let mut config = StoffelConfig {
            mpc: MpcConfig::default(),
            network: NetworkConfig {
                party_id: 0,
                bind_address: "127.0.0.1:9000".parse().unwrap(),
                mpc_port: None,
                expected_parties: 5,
                expected_clients: None,
                consensus_timeout_ms: 30_000,
                peers: HashMap::new(),
            },
            preprocessing: PreprocessingConfig::default(),
        };

        std::env::set_var("STOFFEL_BACKEND", "avss");
        std::env::set_var("STOFFEL_CURVE", "bn254");
        config.apply_env_overrides().unwrap();
        assert_eq!(
            config.mpc.backend,
            MpcBackendConfig::Avss {
                curve: Curve::Bn254
            }
        );

        std::env::remove_var("STOFFEL_BACKEND");
        std::env::remove_var("STOFFEL_CURVE");
    }

    #[test]
    fn test_env_override_bind_address() {
        let mut config = StoffelConfig {
            mpc: MpcConfig::default(),
            network: NetworkConfig {
                party_id: 0,
                bind_address: "127.0.0.1:9000".parse().unwrap(),
                mpc_port: None,
                expected_parties: 5,
                expected_clients: None,
                consensus_timeout_ms: 30_000,
                peers: HashMap::new(),
            },
            preprocessing: PreprocessingConfig::default(),
        };

        std::env::set_var("STOFFEL_BIND_ADDRESS", "0.0.0.0:8080");
        config.apply_env_overrides().unwrap();
        assert_eq!(
            config.network.bind_address,
            "0.0.0.0:8080".parse::<SocketAddr>().unwrap()
        );

        std::env::remove_var("STOFFEL_BIND_ADDRESS");
    }

    #[test]
    fn test_env_override_invalid_value() {
        let mut config = StoffelConfig {
            mpc: MpcConfig::default(),
            network: NetworkConfig {
                party_id: 0,
                bind_address: "127.0.0.1:9000".parse().unwrap(),
                mpc_port: None,
                expected_parties: 5,
                expected_clients: None,
                consensus_timeout_ms: 30_000,
                peers: HashMap::new(),
            },
            preprocessing: PreprocessingConfig::default(),
        };

        std::env::set_var("STOFFEL_PARTIES", "not_a_number");
        let result = config.apply_env_overrides();
        assert!(result.is_err());

        std::env::remove_var("STOFFEL_PARTIES");
    }
}
