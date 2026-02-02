//! API types for the MPC coordinator
//!
//! These types mirror the coordinator's REST API contract.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Types of MPC jobs supported by the coordinator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobType {
    /// Generate MPC authorization for a confidential transfer
    AuthorizeTransfer,
    /// Execute a full confidential transfer
    ConfidentialTransfer,
    /// Create a new confidential account
    CreateConfidentialAccount,
    /// Claim a confidential account as recipient
    ClaimConfidentialAccount,
    /// Wrap an SPL mint as a confidential mint
    WrapMint,
    /// Set the auditor for a confidential mint
    SetAuditor,
    /// Perform distributed decryption
    DistributedDecrypt,
    /// Generate a zero-knowledge range proof
    GenerateRangeProof,
}

/// Status of a job in the coordinator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// Job is queued and waiting to be processed
    Queued,
    /// Job has been assigned to parties
    Assigned,
    /// Preprocessing phase (generating Beaver triples, etc.)
    Preprocessing,
    /// Main MPC execution phase
    Executing,
    /// Signing phase (collecting threshold signatures)
    Signing,
    /// Job completed successfully
    Complete,
    /// Job failed with an error
    Failed,
}

impl JobStatus {
    /// Check if the job is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, JobStatus::Complete | JobStatus::Failed)
    }

    /// Check if the job is still in progress
    pub fn is_in_progress(&self) -> bool {
        !self.is_terminal()
    }
}

/// A client input for an MPC job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInput {
    /// Client identifier
    pub client_id: String,
    /// Input index (for ordering multiple inputs)
    pub index: u32,
    /// Base64-encoded input data
    pub data: String,
    /// Whether this input should be treated as secret
    pub is_secret: bool,
}

impl ClientInput {
    /// Create a new secret input
    pub fn secret(client_id: impl Into<String>, index: u32, data: &[u8]) -> Self {
        use base64::Engine;
        Self {
            client_id: client_id.into(),
            index,
            data: base64::engine::general_purpose::STANDARD.encode(data),
            is_secret: true,
        }
    }

    /// Create a new public input
    pub fn public(client_id: impl Into<String>, index: u32, data: &[u8]) -> Self {
        use base64::Engine;
        Self {
            client_id: client_id.into(),
            index,
            data: base64::engine::general_purpose::STANDARD.encode(data),
            is_secret: false,
        }
    }
}

/// Request to submit a new MPC job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRequest {
    /// Type of job to execute
    pub job_type: JobType,
    /// Hash of the program to execute (for verification)
    pub program_hash: String,
    /// Client inputs for the computation
    pub inputs: Vec<ClientInput>,
    /// Idempotency key to prevent duplicate submissions
    pub idempotency_key: String,
    /// Key ID for encryption/decryption
    pub key_id: String,
    /// Client submitting the job
    pub client_id: String,
    /// Request timestamp
    pub request_ts: u64,
}

impl JobRequest {
    /// Create a new job request builder
    pub fn builder(job_type: JobType, client_id: impl Into<String>) -> JobRequestBuilder {
        JobRequestBuilder::new(job_type, client_id)
    }
}

/// Builder for constructing job requests
#[derive(Debug, Clone)]
pub struct JobRequestBuilder {
    job_type: JobType,
    program_hash: String,
    inputs: Vec<ClientInput>,
    idempotency_key: Option<String>,
    key_id: String,
    client_id: String,
}

impl JobRequestBuilder {
    /// Create a new builder
    pub fn new(job_type: JobType, client_id: impl Into<String>) -> Self {
        Self {
            job_type,
            program_hash: String::new(),
            inputs: Vec::new(),
            idempotency_key: None,
            key_id: "default-key-v1".to_string(),
            client_id: client_id.into(),
        }
    }

    /// Set the program hash
    pub fn program_hash(mut self, hash: impl Into<String>) -> Self {
        self.program_hash = hash.into();
        self
    }

    /// Add an input
    pub fn input(mut self, input: ClientInput) -> Self {
        self.inputs.push(input);
        self
    }

    /// Add multiple inputs
    pub fn inputs(mut self, inputs: impl IntoIterator<Item = ClientInput>) -> Self {
        self.inputs.extend(inputs);
        self
    }

    /// Set the idempotency key
    pub fn idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }

    /// Set the encryption key ID
    pub fn key_id(mut self, key_id: impl Into<String>) -> Self {
        self.key_id = key_id.into();
        self
    }

    /// Build the job request
    pub fn build(self) -> JobRequest {
        JobRequest {
            job_type: self.job_type,
            program_hash: self.program_hash,
            inputs: self.inputs,
            idempotency_key: self.idempotency_key.unwrap_or_else(|| Uuid::new_v4().to_string()),
            key_id: self.key_id,
            client_id: self.client_id,
            request_ts: chrono::Utc::now().timestamp() as u64,
        }
    }
}

/// Response when submitting a job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSubmitResponse {
    /// Unique job identifier
    pub job_id: Uuid,
    /// Current status of the job
    pub status: JobStatus,
    /// Position in the queue (if queued)
    pub queue_position: Option<usize>,
}

/// Response when querying job status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStatusResponse {
    /// Job identifier
    pub job_id: Uuid,
    /// Current status
    pub status: JobStatus,
    /// Type of job
    pub job_type: JobType,
    /// When the job was submitted
    pub submitted_at: DateTime<Utc>,
    /// When the job completed (if complete)
    pub completed_at: Option<DateTime<Utc>>,
    /// Job outputs (if complete)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outputs: Option<String>,
    /// Error message (if failed)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl JobStatusResponse {
    /// Get the outputs as decoded bytes
    pub fn outputs_bytes(&self) -> Option<Vec<u8>> {
        use base64::Engine;
        self.outputs.as_ref().and_then(|s| {
            base64::engine::general_purpose::STANDARD.decode(s).ok()
        })
    }
}

/// Status of the coordinator service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorStatus {
    /// Whether the service is healthy
    pub healthy: bool,
    /// Number of registered parties
    pub party_count: usize,
    /// Number of jobs in the queue
    pub queue_depth: usize,
    /// Number of jobs currently being processed
    pub active_jobs: usize,
    /// Server uptime in seconds
    pub uptime_seconds: u64,
}

/// Information about an MPC party
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyInfo {
    /// Party identifier
    pub party_id: usize,
    /// Party's public key (hex-encoded)
    pub public_key: String,
    /// Party's network endpoint
    pub endpoint: String,
    /// Whether the party is currently online
    pub online: bool,
}

/// Response containing encryption keys
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeysResponse {
    /// Key identifier
    pub key_id: String,
    /// Aggregated encryption public key (hex-encoded)
    pub encryption_pubkey: String,
    /// Number of parties for this key
    pub n_parties: usize,
    /// Threshold for this key
    pub threshold: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_status_terminal() {
        assert!(JobStatus::Complete.is_terminal());
        assert!(JobStatus::Failed.is_terminal());
        assert!(!JobStatus::Queued.is_terminal());
        assert!(!JobStatus::Executing.is_terminal());
    }

    #[test]
    fn test_client_input_secret() {
        let input = ClientInput::secret("client1", 0, &[1, 2, 3]);
        assert!(input.is_secret);
        assert_eq!(input.client_id, "client1");
        assert_eq!(input.index, 0);
    }

    #[test]
    fn test_client_input_public() {
        let input = ClientInput::public("client1", 0, &[1, 2, 3]);
        assert!(!input.is_secret);
    }

    #[test]
    fn test_job_request_builder() {
        let request = JobRequest::builder(JobType::AuthorizeTransfer, "client1")
            .program_hash("0x1234")
            .input(ClientInput::secret("client1", 0, &[10]))
            .idempotency_key("unique-key")
            .build();

        assert_eq!(request.job_type, JobType::AuthorizeTransfer);
        assert_eq!(request.program_hash, "0x1234");
        assert_eq!(request.inputs.len(), 1);
        assert_eq!(request.idempotency_key, "unique-key");
        assert_eq!(request.client_id, "client1");
    }

    #[test]
    fn test_job_request_builder_auto_idempotency() {
        let request1 = JobRequest::builder(JobType::AuthorizeTransfer, "client1").build();
        let request2 = JobRequest::builder(JobType::AuthorizeTransfer, "client1").build();

        // Auto-generated idempotency keys should be unique
        assert_ne!(request1.idempotency_key, request2.idempotency_key);
    }

    #[test]
    fn test_job_type_serialization() {
        let json = serde_json::to_string(&JobType::AuthorizeTransfer).unwrap();
        assert_eq!(json, "\"authorize_transfer\"");

        let parsed: JobType = serde_json::from_str("\"confidential_transfer\"").unwrap();
        assert_eq!(parsed, JobType::ConfidentialTransfer);
    }
}
