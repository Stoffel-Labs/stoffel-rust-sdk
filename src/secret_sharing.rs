///! Secret Sharing Module
///!
///! This module provides high-level wrappers around the mpc-protocols secret sharing schemes.
///! It exposes RobustShare (Reed-Solomon based) for Byzantine fault-tolerant MPC.

use ark_bls12_381::Fr;
use ark_std::rand::rngs::StdRng;
use ark_std::rand::SeedableRng;
use stoffelmpc_mpc::common::SecretSharingScheme;
use stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare;

use crate::{Error, Result};

/// A secret share using the Robust (Reed-Solomon) secret sharing scheme.
/// This is the default scheme used by HoneyBadger MPC for Byzantine fault tolerance.
#[derive(Clone, Debug)]
pub struct SecretShare {
    inner: RobustShare<Fr>,
}

impl SecretShare {
    /// Get the share value
    pub fn value(&self) -> Fr {
        self.inner.share[0]
    }

    /// Get the share ID (party index)
    pub fn id(&self) -> usize {
        self.inner.id
    }

    /// Get the polynomial degree (threshold)
    pub fn degree(&self) -> usize {
        self.inner.degree
    }
}

/// Secret sharing operations for distributing values among MPC parties
pub struct SecretSharing;

impl SecretSharing {
    /// Share a secret value among n parties with threshold t.
    ///
    /// Uses RobustShare (Reed-Solomon) for error correction, which is required
    /// for HoneyBadger's Byzantine fault tolerance.
    ///
    /// # Arguments
    /// * `secret` - The secret value to share (as i64)
    /// * `n_parties` - Total number of parties
    /// * `threshold` - Maximum number of faulty parties to tolerate
    ///
    /// # Returns
    /// A vector of n shares, one for each party
    ///
    /// # Example
    /// ```no_run
    /// use stoffel_rust_sdk::secret_sharing::SecretSharing;
    ///
    /// let secret = 42;
    /// let shares = SecretSharing::share_secret(secret, 5, 1).unwrap();
    /// assert_eq!(shares.len(), 5);
    /// ```
    pub fn share_secret(secret: i64, n_parties: usize, threshold: usize) -> Result<Vec<SecretShare>> {
        // Convert i64 to field element
        let secret_field = Fr::from(secret as u64);

        // Create RNG
        let mut rng = StdRng::from_entropy();

        // Generate shares using RobustShare
        let shares = RobustShare::compute_shares(secret_field, n_parties, threshold, None, &mut rng)
            .map_err(|e| Error::Other(format!("Secret sharing failed: {:?}", e)))?;

        // Wrap in our SecretShare type
        Ok(shares.into_iter().map(|inner| SecretShare { inner }).collect())
    }

    /// Reconstruct a secret from a collection of shares.
    ///
    /// This uses robust interpolation with error correction, so it can tolerate
    /// up to t corrupted shares (where t is the threshold).
    ///
    /// # Arguments
    /// * `shares` - Collection of shares (at least 2*t+1 needed)
    /// * `n_parties` - Total number of parties in the network
    ///
    /// # Returns
    /// The reconstructed secret value
    ///
    /// # Example
    /// ```no_run
    /// use stoffel_rust_sdk::secret_sharing::SecretSharing;
    ///
    /// let secret = 42;
    /// let shares = SecretSharing::share_secret(secret, 5, 1).unwrap();
    ///
    /// // Reconstruct from all shares
    /// let reconstructed = SecretSharing::reconstruct_secret(&shares, 5).unwrap();
    /// assert_eq!(reconstructed, secret);
    /// ```
    pub fn reconstruct_secret(shares: &[SecretShare], n_parties: usize) -> Result<i64> {
        // Extract inner RobustShare objects
        let inner_shares: Vec<RobustShare<Fr>> = shares.iter().map(|s| s.inner.clone()).collect();

        // Reconstruct using robust interpolation
        let (_coeffs, secret_field) = RobustShare::recover_secret(&inner_shares, n_parties)
            .map_err(|e| Error::Other(format!("Secret reconstruction failed: {:?}", e)))?;

        // Convert field element back to i64
        // Note: This is a simplified conversion. In production, you'd want proper field->int conversion
        let secret_bytes = secret_field.to_string();
        let secret_value = secret_bytes.parse::<i64>()
            .unwrap_or_else(|_| {
                // If parsing fails, try to extract the numeric part
                // Field elements are printed as "Fr(value)"
                if let Some(start) = secret_bytes.find('(') {
                    if let Some(end) = secret_bytes.find(')') {
                        if let Ok(val) = secret_bytes[start+1..end].parse::<i64>() {
                            return val;
                        }
                    }
                }
                0
            });

        Ok(secret_value)
    }

    /// Share multiple secrets at once.
    ///
    /// This is more efficient than calling share_secret multiple times.
    ///
    /// # Arguments
    /// * `secrets` - Vector of secret values to share
    /// * `n_parties` - Total number of parties
    /// * `threshold` - Maximum number of faulty parties to tolerate
    ///
    /// # Returns
    /// A vector where each element contains the shares for one secret
    pub fn share_multiple_secrets(
        secrets: &[i64],
        n_parties: usize,
        threshold: usize,
    ) -> Result<Vec<Vec<SecretShare>>> {
        secrets
            .iter()
            .map(|&secret| Self::share_secret(secret, n_parties, threshold))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_share_and_reconstruct() {
        let secret = 42;
        let n_parties = 5;
        let threshold = 1;

        // Share the secret
        let shares = SecretSharing::share_secret(secret, n_parties, threshold)
            .expect("Failed to share secret");

        assert_eq!(shares.len(), n_parties);

        // Each share should have the correct metadata
        for (i, share) in shares.iter().enumerate() {
            assert_eq!(share.id(), i);
            assert_eq!(share.degree(), threshold);
        }

        // Reconstruct from all shares
        let reconstructed = SecretSharing::reconstruct_secret(&shares, n_parties)
            .expect("Failed to reconstruct secret");

        assert_eq!(reconstructed, secret);
    }

    #[test]
    fn test_share_multiple() {
        let secrets = vec![10, 20, 30];
        let n_parties = 5;
        let threshold = 1;

        let all_shares = SecretSharing::share_multiple_secrets(&secrets, n_parties, threshold)
            .expect("Failed to share secrets");

        assert_eq!(all_shares.len(), secrets.len());

        // Each secret should have n_parties shares
        for shares in &all_shares {
            assert_eq!(shares.len(), n_parties);
        }

        // Reconstruct each secret
        for (i, shares) in all_shares.iter().enumerate() {
            let reconstructed = SecretSharing::reconstruct_secret(shares, n_parties)
                .expect("Failed to reconstruct");
            assert_eq!(reconstructed, secrets[i]);
        }
    }

    #[test]
    fn test_reconstruct_with_subset() {
        let secret = 100;
        let n_parties = 5;
        let threshold = 1;

        let shares = SecretSharing::share_secret(secret, n_parties, threshold)
            .expect("Failed to share secret");

        // Should be able to reconstruct with just 2*t+1 = 3 shares
        let subset: Vec<SecretShare> = shares.iter().take(3).cloned().collect();

        let reconstructed = SecretSharing::reconstruct_secret(&subset, n_parties)
            .expect("Failed to reconstruct from subset");

        assert_eq!(reconstructed, secret);
    }
}
