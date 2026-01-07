//! Network configuration for Stoffel MPC programs
//!
//! This module provides configuration for MPC network deployment, including:
//! - Network topology (bootnode, parties)
//! - Party identification
//! - Network addresses
//! - MPC parameters (parties, threshold)
//!
//! Configuration can be provided manually or loaded from a TOML file.
//!
//! # Example TOML Configuration
//!
//! ```toml
//! # stoffel.toml
//! [network]
//! # This party's ID (0 to n_parties-1)
//! party_id = 0
//!
//! # Address to bind for incoming connections
//! bind_address = "127.0.0.1:9001"
//!
//! # Bootnode address for discovery
//! bootstrap_address = "127.0.0.1:9000"
//!
//! # Minimum parties required before starting
//! min_parties = 3
//!
//! [mpc]
//! # Total number of MPC parties
//! n_parties = 5
//!
//! # Fault tolerance threshold (n >= 3t + 1)
//! threshold = 1
//!
//! # Instance ID for this computation (optional, random if not set)
//! instance_id = 12345
//! ```

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::Path;

/// Complete network configuration for Stoffel MPC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Network-specific configuration
    pub network: NetworkSettings,

    /// MPC protocol configuration
    pub mpc: MPCSettings,
}

/// Network topology and addressing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSettings {
    /// This party's ID (0 to n_parties-1)
    pub party_id: usize,

    /// Address to bind for incoming connections
    pub bind_address: String,

    /// Bootnode address for party discovery
    pub bootstrap_address: String,

    /// Minimum number of parties required before starting
    #[serde(default = "default_min_parties")]
    pub min_parties: usize,
}

fn default_min_parties() -> usize {
    3
}

/// MPC protocol parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MPCSettings {
    /// Total number of MPC parties
    pub n_parties: usize,

    /// Fault tolerance threshold (n >= 3t + 1)
    #[serde(default = "default_threshold")]
    pub threshold: usize,

    /// Instance ID for this computation (random if not set)
    #[serde(default)]
    pub instance_id: Option<u64>,
}

fn default_threshold() -> usize {
    1
}

impl NetworkConfig {
    /// Load configuration from a TOML file
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::network_config::NetworkConfig;
    /// # fn main() -> stoffel_rust_sdk::Result<()> {
    /// let config = NetworkConfig::from_file("stoffel.toml")?;
    /// println!("Party ID: {}", config.network.party_id);
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| Error::IoError(e))?;

        toml::from_str(&contents)
            .map_err(|e| Error::InvalidInput(format!("Failed to parse TOML config: {}", e)))
    }

    /// Create a new network configuration manually
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::network_config::{NetworkConfig, NetworkSettings, MPCSettings};
    /// # fn main() -> stoffel_rust_sdk::Result<()> {
    /// let config = NetworkConfig::new(
    ///     NetworkSettings {
    ///         party_id: 0,
    ///         bind_address: "127.0.0.1:9001".to_string(),
    ///         bootstrap_address: "127.0.0.1:9000".to_string(),
    ///         min_parties: 3,
    ///     },
    ///     MPCSettings {
    ///         n_parties: 5,
    ///         threshold: 1,
    ///         instance_id: None,
    ///     },
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(network: NetworkSettings, mpc: MPCSettings) -> Self {
        Self { network, mpc }
    }

    /// Validate the configuration
    ///
    /// Ensures that:
    /// - n_parties >= 4 * threshold + 1 (for HoneyBadger TripleGen batch reconstruction)
    /// - party_id < n_parties
    /// - Addresses are valid
    ///
    /// Note: The basic Byzantine constraint is n >= 3t + 1, but HoneyBadger's TripleGen
    /// preprocessing uses degree-2t shares which require n >= 4t + 1 for robust
    /// interpolation in batch reconstruction.
    pub fn validate(&self) -> Result<()> {
        // Validate MPC parameters - TripleGen requires n >= 4t + 1
        if self.mpc.n_parties < 4 * self.mpc.threshold + 1 {
            return Err(Error::InvalidInput(format!(
                "Invalid MPC parameters: n_parties={} must be >= 4*threshold+1={} for threshold={} (HoneyBadger TripleGen constraint)",
                self.mpc.n_parties,
                4 * self.mpc.threshold + 1,
                self.mpc.threshold
            )));
        }

        // Validate party ID
        if self.network.party_id >= self.mpc.n_parties {
            return Err(Error::InvalidInput(format!(
                "Invalid party_id={}: must be < n_parties={}",
                self.network.party_id,
                self.mpc.n_parties
            )));
        }

        // Validate addresses
        self.network.bind_address.parse::<SocketAddr>()
            .map_err(|e| Error::InvalidInput(format!("Invalid bind_address: {}", e)))?;

        self.network.bootstrap_address.parse::<SocketAddr>()
            .map_err(|e| Error::InvalidInput(format!("Invalid bootstrap_address: {}", e)))?;

        Ok(())
    }

    /// Get the bind address as a SocketAddr
    pub fn bind_addr(&self) -> Result<SocketAddr> {
        self.network.bind_address.parse::<SocketAddr>()
            .map_err(|e| Error::InvalidInput(format!("Invalid bind_address: {}", e)))
    }

    /// Get the bootstrap address as a SocketAddr
    pub fn bootstrap_addr(&self) -> Result<SocketAddr> {
        self.network.bootstrap_address.parse::<SocketAddr>()
            .map_err(|e| Error::InvalidInput(format!("Invalid bootstrap_address: {}", e)))
    }

    /// Save this configuration to a TOML file
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stoffel_rust_sdk::network_config::{NetworkConfig, NetworkSettings, MPCSettings};
    /// # fn main() -> stoffel_rust_sdk::Result<()> {
    /// let config = NetworkConfig::new(
    ///     NetworkSettings {
    ///         party_id: 0,
    ///         bind_address: "127.0.0.1:9001".to_string(),
    ///         bootstrap_address: "127.0.0.1:9000".to_string(),
    ///         min_parties: 3,
    ///     },
    ///     MPCSettings {
    ///         n_parties: 5,
    ///         threshold: 1,
    ///         instance_id: Some(12345),
    ///     },
    /// );
    /// config.save("stoffel.toml")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let contents = toml::to_string_pretty(self)
            .map_err(|e| Error::InvalidInput(format!("Failed to serialize config: {}", e)))?;

        std::fs::write(path, contents)
            .map_err(|e| Error::IoError(e))
    }
}

/// Builder for creating network configuration
pub struct NetworkConfigBuilder {
    party_id: Option<usize>,
    bind_address: Option<String>,
    bootstrap_address: Option<String>,
    min_parties: usize,
    n_parties: Option<usize>,
    threshold: usize,
    instance_id: Option<u64>,
}

impl NetworkConfigBuilder {
    /// Create a new network configuration builder
    pub fn new() -> Self {
        Self {
            party_id: None,
            bind_address: None,
            bootstrap_address: None,
            min_parties: 3,
            n_parties: None,
            threshold: 1,
            instance_id: None,
        }
    }

    /// Set the party ID for this node
    pub fn party_id(mut self, id: usize) -> Self {
        self.party_id = Some(id);
        self
    }

    /// Set the bind address
    pub fn bind_address(mut self, addr: impl Into<String>) -> Self {
        self.bind_address = Some(addr.into());
        self
    }

    /// Set the bootstrap (bootnode) address
    pub fn bootstrap_address(mut self, addr: impl Into<String>) -> Self {
        self.bootstrap_address = Some(addr.into());
        self
    }

    /// Set the minimum number of parties required
    pub fn min_parties(mut self, n: usize) -> Self {
        self.min_parties = n;
        self
    }

    /// Set the total number of MPC parties
    pub fn n_parties(mut self, n: usize) -> Self {
        self.n_parties = Some(n);
        self
    }

    /// Set the fault tolerance threshold
    pub fn threshold(mut self, t: usize) -> Self {
        self.threshold = t;
        self
    }

    /// Set the instance ID
    pub fn instance_id(mut self, id: u64) -> Self {
        self.instance_id = Some(id);
        self
    }

    /// Build the network configuration
    pub fn build(self) -> Result<NetworkConfig> {
        let config = NetworkConfig {
            network: NetworkSettings {
                party_id: self.party_id.ok_or_else(||
                    Error::InvalidInput("party_id not set".to_string()))?,
                bind_address: self.bind_address.ok_or_else(||
                    Error::InvalidInput("bind_address not set".to_string()))?,
                bootstrap_address: self.bootstrap_address.ok_or_else(||
                    Error::InvalidInput("bootstrap_address not set".to_string()))?,
                min_parties: self.min_parties,
            },
            mpc: MPCSettings {
                n_parties: self.n_parties.ok_or_else(||
                    Error::InvalidInput("n_parties not set".to_string()))?,
                threshold: self.threshold,
                instance_id: self.instance_id,
            },
        };

        config.validate()?;
        Ok(config)
    }
}

impl Default for NetworkConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_config() {
        let config = NetworkConfigBuilder::new()
            .party_id(0)
            .bind_address("127.0.0.1:9001")
            .bootstrap_address("127.0.0.1:9000")
            .n_parties(5)
            .threshold(1)
            .build()
            .unwrap();

        assert_eq!(config.network.party_id, 0);
        assert_eq!(config.mpc.n_parties, 5);
        assert_eq!(config.mpc.threshold, 1);
    }

    #[test]
    fn test_invalid_mpc_params() {
        let result = NetworkConfigBuilder::new()
            .party_id(0)
            .bind_address("127.0.0.1:9001")
            .bootstrap_address("127.0.0.1:9000")
            .n_parties(3)  // Too few for threshold=1 (need >= 4)
            .threshold(1)
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_party_id() {
        let result = NetworkConfigBuilder::new()
            .party_id(5)  // >= n_parties
            .bind_address("127.0.0.1:9001")
            .bootstrap_address("127.0.0.1:9000")
            .n_parties(5)
            .threshold(1)
            .build();

        assert!(result.is_err());
    }
}
