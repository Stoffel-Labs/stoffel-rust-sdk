//! Coordinator client for MPC job submission and management
//!
//! This module provides a high-level client for interacting with the
//! stoffel-mpc-coordinator REST API. It handles job submission, status
//! polling, and result retrieval.
//!
//! # Example
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::coordinator::{CoordinatorClient, JobRequest, JobType, ClientInput};
//! use std::time::Duration;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a client
//! let client = CoordinatorClient::with_url("http://localhost:8080")?;
//!
//! // Build a job request
//! let request = JobRequest::builder(JobType::AuthorizeTransfer, "my-client")
//!     .program_hash("0x1234abcd")
//!     .input(ClientInput::secret("my-client", 0, &[100u8; 32]))
//!     .build();
//!
//! // Submit and wait for completion
//! let result = client.submit_and_wait(request, Duration::from_secs(60)).await?;
//!
//! if let Some(outputs) = result.outputs_bytes() {
//!     println!("Job outputs: {} bytes", outputs.len());
//! }
//! # Ok(())
//! # }
//! ```

mod types;
mod client;

pub use types::*;
pub use client::*;
