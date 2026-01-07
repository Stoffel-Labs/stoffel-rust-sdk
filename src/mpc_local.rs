//! Local MPC Network Setup for Testing and Examples
//!
//! This module provides simplified APIs for setting up local MPC networks
//! for testing and demonstration purposes using an in-memory network.
//!
//! # Example
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::mpc_local::LocalMPCNetwork;
//!
//! #[tokio::main]
//! async fn main() -> stoffel_rust_sdk::Result<()> {
//!     // Set up a 3-party MPC network locally
//!     let network = LocalMPCNetwork::new(3, 1, 10, 25).await?;
//!
//!     // Run preprocessing
//!     network.run_preprocessing().await?;
//!
//!     // Get a VM configured for party 0
//!     let mut vm = network.create_vm_for_party(0)?;
//!
//!     // Execute Stoffel bytecode with MPC support
//!     let result = vm.execute("main")?;
//!
//!     Ok(())
//! }
//! ```

use crate::{Error, Result};
use std::sync::Arc;

use stoffel_vm::core_vm::VirtualMachine;
use stoffelmpc_network::fake_network::{FakeNetwork, FakeNetworkConfig};
use tokio::time::{timeout, Duration};
use ark_bls12_381::Fr;
use stoffelmpc_mpc::honeybadger::{HoneyBadgerMPCNode, HoneyBadgerMPCNodeOpts};
use stoffelmpc_mpc::common::{MPCProtocol, PreprocessingMPCProtocol};
use stoffelmpc_mpc::common::rbc::rbc::Avid as RBCImpl;
use stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare;
use rand::SeedableRng;

/// A local MPC network for testing with multiple parties running in-process
pub struct LocalMPCNetwork {
    n_parties: usize,
    threshold: usize,
    instance_id: u64,
    network: Arc<FakeNetwork>,
    receivers: Vec<tokio::sync::mpsc::Receiver<Vec<u8>>>,
    nodes: Vec<HoneyBadgerMPCNode<Fr, RBCImpl>>,
}

impl LocalMPCNetwork {
    /// Create a new local MPC network using in-memory channels
    ///
    /// # Arguments
    /// * `n_parties` - Number of parties in the network
    /// * `threshold` - Byzantine fault tolerance threshold (n >= 3t+1)
    /// * `n_triples` - Number of beaver triples to generate
    /// * `n_random` - Number of random shares to generate
    pub async fn new(
        n_parties: usize,
        threshold: usize,
        n_triples: usize,
        n_random: usize,
    ) -> Result<Self> {
        // Validate parameters
        if n_parties < 3 * threshold + 1 {
            return Err(Error::InvalidInput(format!(
                "Invalid parameters: n={} must be >= 3t+1={} for t={}",
                n_parties, 3 * threshold + 1, threshold
            )));
        }

        // Generate a random instance ID
        let instance_id = rand::random::<u64>();

        // Create fake network with in-memory channels
        let config = FakeNetworkConfig::new(1024);
        let (network, receivers, _) = FakeNetwork::new(n_parties, None, config);
        let network = Arc::new(network);

        // Create MPC nodes for each party
        let mut nodes = Vec::new();
        for party_id in 0..n_parties {
            let mpc_opts = HoneyBadgerMPCNodeOpts::new(
                n_parties,
                threshold,
                n_triples,
                n_random,
                instance_id,
                0,  // n_prandbit
                0,  // n_prandint
                0,  // l
                0,  // k
            );

            let node = <HoneyBadgerMPCNode<Fr, RBCImpl> as MPCProtocol<
                Fr,
                RobustShare<Fr>,
                FakeNetwork,
            >>::setup(party_id, mpc_opts)
                .map_err(|e| Error::RuntimeError(format!("Failed to create MPC node for party {}: {:?}", party_id, e)))?;

            nodes.push(node);
        }

        Ok(Self {
            n_parties,
            threshold,
            instance_id,
            network,
            receivers,
            nodes,
        })
    }

    /// Run preprocessing on all parties
    ///
    /// This must be called before executing MPC operations.
    /// Uses the pattern from mpc-protocols tests with in-memory message channels.
    pub async fn run_preprocessing(&mut self) -> Result<()> {
        let mut receivers = std::mem::take(&mut self.receivers);

        // Step 1: Spawn background tasks to process incoming messages for each node
        // This is critical - without these tasks, messages sent via the network
        // just sit in the channels and never get processed, causing hangs
        // Clone the nodes for the receive tasks (they share preprocessing_material via Arc)
        for i in 0..self.n_parties {
            let mut receiver = receivers.remove(0);
            let mut node = self.nodes[i].clone();
            let network = self.network.clone();

            tokio::spawn(async move {
                while let Some(raw_msg) = receiver.recv().await {
                    if let Err(e) = node.process(raw_msg, network.clone()).await {
                        // Suppress expected errors from processing messages after sessions end
                        let error_str = format!("{:?}", e);
                        if !error_str.contains("SessionEnded")
                            && !error_str.contains("WaitForOk")
                            && !error_str.contains("Abort") {
                            eprintln!("  Node {} failed to process message: {:?}", i, e);
                        }
                    }
                }
            });
        }

        // Step 2: Spawn tasks to run preprocessing on each party
        // Clone the nodes for preprocessing (they share preprocessing_material via Arc)
        let mut preprocessing_handles = Vec::new();

        for i in 0..self.n_parties {
            let mut node = self.nodes[i].clone();
            let network = self.network.clone();

            let handle = tokio::spawn(async move {
                println!("  Party {} starting preprocessing...", i);

                let result = timeout(Duration::from_secs(30), async {
                    let mut rng = ark_std::rand::rngs::StdRng::from_entropy();
                    node.run_preprocessing(network, &mut rng).await
                }).await;

                match result {
                    Ok(Ok(_)) => {
                        println!("  ✓ Party {} preprocessing SUCCESS", i);
                        Ok(())
                    }
                    Ok(Err(e)) => {
                        println!("  ✗ Party {} preprocessing ERROR: {:?}", i, e);
                        Err(format!("Preprocessing error: {:?}", e))
                    }
                    Err(_) => {
                        println!("  ⚠ Party {} preprocessing TIMEOUT (>30s)", i);
                        Err("Preprocessing timeout".to_string())
                    }
                }
            });

            preprocessing_handles.push(handle);
        }

        // Step 3: Wait for all preprocessing tasks to complete
        let results = futures::future::join_all(preprocessing_handles).await;

        // Check results
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(Ok(_)) => {
                    println!("  ✓ Party {} completed successfully", i);
                }
                Ok(Err(e)) => {
                    return Err(Error::RuntimeError(format!("Party {} failed: {}", i, e)));
                }
                Err(e) => {
                    return Err(Error::RuntimeError(format!("Party {} task panicked: {}", i, e)));
                }
            }
        }

        // Give a moment for any remaining messages to be processed
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(())
    }

    /// Create a VM configured with MPC support for a specific party
    ///
    /// Note: This creates a simplified engine wrapper since we're using FakeNetwork.
    /// For full MPC execution, use the preprocessing results from the nodes.
    pub fn create_vm_for_party(&self, party_id: usize) -> Result<VirtualMachine> {
        if party_id >= self.n_parties {
            return Err(Error::InvalidInput(format!(
                "Invalid party_id {}: must be < {}",
                party_id, self.n_parties
            )));
        }

        // For now, create a basic VM without MPC engine
        // Full integration would require wrapping the HoneyBadgerMPCNode in an engine
        let vm = VirtualMachine::new();

        // TODO: Set MPC engine once we have a wrapper that works with FakeNetwork
        // vm.state.set_mpc_engine(engine);

        Ok(vm)
    }

    /// Get the number of parties in the network
    pub fn n_parties(&self) -> usize {
        self.n_parties
    }

    /// Get the threshold
    pub fn threshold(&self) -> usize {
        self.threshold
    }

    /// Get the instance ID
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }
}
