//! HTTP client for the MPC coordinator API
//!
//! This module provides a high-level client for interacting with the
//! stoffel-mpc-coordinator REST API.

use std::time::Duration;
use reqwest::Client;
use uuid::Uuid;
use tracing::{debug, info, warn};

use super::types::*;
use crate::error::{Error, Result};

/// Default timeout for HTTP requests
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default polling interval for job status
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Configuration for the coordinator client
#[derive(Debug, Clone)]
pub struct CoordinatorClientConfig {
    /// Base URL of the coordinator (e.g., "http://localhost:8080")
    pub base_url: String,
    /// Request timeout
    pub timeout: Duration,
    /// Polling interval for status checks
    pub poll_interval: Duration,
}

impl CoordinatorClientConfig {
    /// Create a new configuration with the given base URL
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            timeout: DEFAULT_TIMEOUT,
            poll_interval: DEFAULT_POLL_INTERVAL,
        }
    }

    /// Set the request timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the polling interval
    pub fn poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }
}

impl Default for CoordinatorClientConfig {
    fn default() -> Self {
        Self::new("http://localhost:8080")
    }
}

/// Client for interacting with the MPC coordinator
pub struct CoordinatorClient {
    config: CoordinatorClientConfig,
    http_client: Client,
}

impl CoordinatorClient {
    /// Create a new coordinator client with the given configuration
    pub fn new(config: CoordinatorClientConfig) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| Error::Network(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { config, http_client })
    }

    /// Create a new client with just a base URL
    pub fn with_url(base_url: impl Into<String>) -> Result<Self> {
        Self::new(CoordinatorClientConfig::new(base_url))
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    /// Submit a new MPC job
    pub async fn submit_job(&self, request: JobRequest) -> Result<JobSubmitResponse> {
        let url = format!("{}/mpc/jobs", self.config.base_url);

        debug!(
            job_type = ?request.job_type,
            client_id = %request.client_id,
            "Submitting MPC job"
        );

        let response = self
            .http_client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Network(format!("Failed to submit job: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Network(format!(
                "Job submission failed with status {}: {}",
                status, body
            )));
        }

        let submit_response: JobSubmitResponse = response
            .json()
            .await
            .map_err(|e| Error::Network(format!("Failed to parse response: {}", e)))?;

        info!(
            job_id = %submit_response.job_id,
            status = ?submit_response.status,
            "Job submitted successfully"
        );

        Ok(submit_response)
    }

    /// Get the status of a job
    pub async fn get_job_status(&self, job_id: Uuid) -> Result<JobStatusResponse> {
        let url = format!("{}/mpc/jobs/{}", self.config.base_url, job_id);

        debug!(job_id = %job_id, "Fetching job status");

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Network(format!("Failed to get job status: {}", e)))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(Error::Other(format!("Job not found: {}", job_id)));
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Network(format!(
                "Failed to get job status with status {}: {}",
                status, body
            )));
        }

        let status_response: JobStatusResponse = response
            .json()
            .await
            .map_err(|e| Error::Network(format!("Failed to parse response: {}", e)))?;

        Ok(status_response)
    }

    /// Poll for job completion with a timeout
    ///
    /// Returns the final job status when complete or failed, or an error if timeout.
    pub async fn poll_until_complete(
        &self,
        job_id: Uuid,
        timeout: Duration,
    ) -> Result<JobStatusResponse> {
        let start = std::time::Instant::now();
        let poll_interval = self.config.poll_interval;

        info!(job_id = %job_id, timeout_secs = timeout.as_secs(), "Polling for job completion");

        loop {
            let status = self.get_job_status(job_id).await?;

            if status.status.is_terminal() {
                info!(
                    job_id = %job_id,
                    status = ?status.status,
                    "Job reached terminal state"
                );
                return Ok(status);
            }

            if start.elapsed() >= timeout {
                warn!(job_id = %job_id, "Job polling timed out");
                return Err(Error::Other(format!(
                    "Timeout waiting for job {} to complete (status: {:?})",
                    job_id, status.status
                )));
            }

            debug!(
                job_id = %job_id,
                status = ?status.status,
                elapsed_secs = start.elapsed().as_secs(),
                "Job still in progress, waiting..."
            );

            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Submit a job and wait for completion
    ///
    /// Convenience method that combines submit_job and poll_until_complete.
    pub async fn submit_and_wait(
        &self,
        request: JobRequest,
        timeout: Duration,
    ) -> Result<JobStatusResponse> {
        let submit_response = self.submit_job(request).await?;
        self.poll_until_complete(submit_response.job_id, timeout).await
    }

    /// Get the coordinator service status
    pub async fn get_status(&self) -> Result<CoordinatorStatus> {
        let url = format!("{}/mpc/status", self.config.base_url);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Network(format!("Failed to get status: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Network(format!(
                "Failed to get status with status {}: {}",
                status, body
            )));
        }

        let status: CoordinatorStatus = response
            .json()
            .await
            .map_err(|e| Error::Network(format!("Failed to parse response: {}", e)))?;

        Ok(status)
    }

    /// Get the aggregated encryption keys
    pub async fn get_keys(&self) -> Result<KeysResponse> {
        let url = format!("{}/mpc/keys", self.config.base_url);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Network(format!("Failed to get keys: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Network(format!(
                "Failed to get keys with status {}: {}",
                status, body
            )));
        }

        let keys: KeysResponse = response
            .json()
            .await
            .map_err(|e| Error::Network(format!("Failed to parse response: {}", e)))?;

        Ok(keys)
    }

    /// Get the list of registered parties
    pub async fn get_parties(&self) -> Result<Vec<PartyInfo>> {
        let url = format!("{}/mpc/parties", self.config.base_url);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Network(format!("Failed to get parties: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Network(format!(
                "Failed to get parties with status {}: {}",
                status, body
            )));
        }

        let parties: Vec<PartyInfo> = response
            .json()
            .await
            .map_err(|e| Error::Network(format!("Failed to parse response: {}", e)))?;

        Ok(parties)
    }

    /// Check if the coordinator is healthy
    pub async fn is_healthy(&self) -> bool {
        match self.get_status().await {
            Ok(status) => status.healthy,
            Err(_) => false,
        }
    }
}

/// Builder for creating coordinator clients
pub struct CoordinatorClientBuilder {
    config: CoordinatorClientConfig,
}

impl CoordinatorClientBuilder {
    /// Create a new builder with the given base URL
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            config: CoordinatorClientConfig::new(base_url),
        }
    }

    /// Set the request timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    /// Set the polling interval
    pub fn poll_interval(mut self, interval: Duration) -> Self {
        self.config.poll_interval = interval;
        self
    }

    /// Build the client
    pub fn build(self) -> Result<CoordinatorClient> {
        CoordinatorClient::new(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = CoordinatorClientConfig::default();
        assert_eq!(config.base_url, "http://localhost:8080");
        assert_eq!(config.timeout, DEFAULT_TIMEOUT);
        assert_eq!(config.poll_interval, DEFAULT_POLL_INTERVAL);
    }

    #[test]
    fn test_config_builder() {
        let config = CoordinatorClientConfig::new("http://example.com:9000")
            .timeout(Duration::from_secs(60))
            .poll_interval(Duration::from_secs(1));

        assert_eq!(config.base_url, "http://example.com:9000");
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.poll_interval, Duration::from_secs(1));
    }

    #[test]
    fn test_client_builder() {
        let client = CoordinatorClientBuilder::new("http://localhost:8080")
            .timeout(Duration::from_secs(10))
            .poll_interval(Duration::from_millis(100))
            .build()
            .unwrap();

        assert_eq!(client.base_url(), "http://localhost:8080");
    }

    #[test]
    fn test_client_with_url() {
        let client = CoordinatorClient::with_url("http://localhost:8080").unwrap();
        assert_eq!(client.base_url(), "http://localhost:8080");
    }

    // Integration tests would require a running coordinator
    // See tests/coordinator_integration.rs for those
}
