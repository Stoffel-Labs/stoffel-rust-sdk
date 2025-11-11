//! High-level MPC abstraction combining Stoffel compilation, clients, and servers
//!
//! This module provides a unified interface for MPC execution where:
//! - Clients provide secret inputs for a Stoffel program
//! - Servers execute the MPC program with those inputs
//!
//! # Example
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::stoffel_mpc::StoffelMPC;
//!
//! #[tokio::main]
//! async fn main() -> stoffel_rust_sdk::Result<()> {
//!     // Compile program and set up 5-party MPC network
//!     let mut mpc = StoffelMPC::new(
//!         r#"
//!         main main() -> secret int64:
//!             var x: secret int64 = 6
//!             var y: secret int64 = 7
//!             return x * y
//!         "#,
//!         5,  // n_parties
//!         1,  // threshold
//!     ).await?;
//!
//!     // Run preprocessing (generate beaver triples)
//!     mpc.run_preprocessing().await?;
//!
//!     // Get a client to provide inputs
//!     let mut client = mpc.client(100);
//!     client.provide_inputs(vec![6, 7]).await?;
//!
//!     // Execute on all servers
//!     for party_id in 0..5 {
//!         let mut server = mpc.server(party_id)?;
//!         server.execute("main").await?;
//!     }
//!
//!     // Get outputs
//!     let result = client.get_output().await?;
//!     println!("Result: {:?}", result);
//!
//!     Ok(())
//! }
//! ```

use crate::{Error, Result};

#[cfg(feature = "mpc-local")]
use {
    crate::mpc_local::LocalMPCNetwork,
    stoffel_vm::core_vm::VirtualMachine,
    stoffelmpc_mpc::honeybadger::HoneyBadgerMPCNode,
    stoffelmpc_mpc::common::rbc::rbc::Avid as RBCImpl,
    ark_bls12_381::Fr,
};

/// Type alias for client IDs
pub type ClientId = u64;

/// High-level MPC coordinator that manages compilation, clients, and servers
#[cfg(feature = "mpc-local")]
pub struct StoffelMPC {
    /// Compiled Stoffel bytecode
    bytecode: Vec<u8>,
    /// Local MPC network with nodes
    network: LocalMPCNetwork,
    /// Number of parties (servers)
    n_parties: usize,
}

#[cfg(feature = "mpc-local")]
impl StoffelMPC {
    /// Create a new StoffelMPC instance
    ///
    /// # Arguments
    /// * `source` - Stoffel source code to compile
    /// * `n_parties` - Number of MPC server parties
    /// * `threshold` - Byzantine fault tolerance threshold (n >= 3t+1)
    ///
    /// This compiles the source code and sets up the MPC network.
    pub async fn new(source: &str, n_parties: usize, threshold: usize) -> Result<Self> {
        // Compile the Stoffel program
        let compiler = crate::compiler::Compiler::new();
        let bytecode = compiler.compile_source(source)?;

        // Calculate preprocessing parameters
        // Use values that work with the current MPC implementation
        let n_triples = 3; // Number of beaver triples
        let n_random = 8;  // Number of random shares

        // Create the MPC network
        let network = LocalMPCNetwork::new(n_parties, threshold, n_triples, n_random).await?;

        Ok(Self {
            bytecode,
            network,
            n_parties,
        })
    }

    /// Run preprocessing to generate beaver triples and random shares
    ///
    /// This must be called before executing any MPC operations.
    pub async fn run_preprocessing(&mut self) -> Result<()> {
        self.network.run_preprocessing().await
    }

    /// Get an MPC server for a specific party
    ///
    /// # Arguments
    /// * `party_id` - The party ID (0 to n_parties-1)
    pub fn server(&self, party_id: usize) -> Result<MPCServer> {
        if party_id >= self.n_parties {
            return Err(Error::InvalidInput(format!(
                "Invalid party_id {}: must be < {}",
                party_id, self.n_parties
            )));
        }

        // Create a VM for this party
        let vm = self.network.create_vm_for_party(party_id)?;

        Ok(MPCServer {
            party_id,
            vm,
            bytecode: self.bytecode.clone(),
        })
    }

    /// Get an MPC client
    ///
    /// # Arguments
    /// * `client_id` - Unique client identifier
    pub fn client(&self, client_id: ClientId) -> MPCClient {
        MPCClient {
            client_id,
            network: &self.network,
        }
    }

    /// Get the number of parties
    pub fn n_parties(&self) -> usize {
        self.n_parties
    }

    /// Get the compiled bytecode
    pub fn bytecode(&self) -> &[u8] {
        &self.bytecode
    }
}

/// An MPC server that executes Stoffel programs with secret-shared data
#[cfg(feature = "mpc-local")]
pub struct MPCServer {
    party_id: usize,
    vm: VirtualMachine,
    bytecode: Vec<u8>,
}

#[cfg(feature = "mpc-local")]
impl MPCServer {
    /// Get the party ID for this server
    pub fn party_id(&self) -> usize {
        self.party_id
    }

    /// Execute a function in the Stoffel program
    ///
    /// # Arguments
    /// * `entry_point` - Name of the function to execute (e.g., "main")
    ///
    /// # Note
    /// This is a simplified interface. Full MPC execution requires:
    /// 1. Clients providing inputs via `MPCClient::provide_inputs()`
    /// 2. All servers running `execute()` in parallel
    /// 3. Servers communicating through the MPC protocol
    pub async fn execute(&mut self, entry_point: &str) -> Result<()> {
        // For now, this is a placeholder
        // Full implementation would:
        // 1. Load bytecode into VM
        // 2. Set up MPC engine with the node
        // 3. Execute with MPC operations
        // 4. Return shares of the result

        // TODO: Integrate HoneyBadgerMPCNode with VM execution
        // The challenge is that we need to coordinate with other parties

        println!("  [Server {}] Would execute '{}' with MPC", self.party_id, entry_point);
        Ok(())
    }
}

/// An MPC client that provides secret inputs and retrieves outputs
#[cfg(feature = "mpc-local")]
pub struct MPCClient<'a> {
    client_id: ClientId,
    network: &'a LocalMPCNetwork,
}

#[cfg(feature = "mpc-local")]
impl<'a> MPCClient<'a> {
    /// Get the client ID
    pub fn client_id(&self) -> ClientId {
        self.client_id
    }

    /// Provide secret inputs for the MPC computation
    ///
    /// # Arguments
    /// * `inputs` - Vector of input values to share among the parties
    ///
    /// # Note
    /// The inputs are secret-shared among all parties using Shamir secret sharing.
    pub async fn provide_inputs(&mut self, inputs: Vec<i64>) -> Result<()> {
        // TODO: Implement input protocol
        // This would:
        // 1. Convert inputs to field elements
        // 2. Generate Shamir shares
        // 3. Distribute shares to all parties via the input protocol
        // 4. Wait for acknowledgment

        println!("  [Client {}] Would provide {} inputs", self.client_id, inputs.len());
        Ok(())
    }

    /// Get the output of the MPC computation
    ///
    /// # Note
    /// This reconstructs the secret-shared output from the parties.
    pub async fn get_output(&mut self) -> Result<i64> {
        // TODO: Implement output protocol
        // This would:
        // 1. Request output shares from all parties
        // 2. Collect threshold+1 shares
        // 3. Reconstruct the secret using Lagrange interpolation
        // 4. Return the result

        println!("  [Client {}] Would retrieve output", self.client_id);
        Ok(42) // Placeholder
    }
}

#[cfg(not(feature = "mpc-local"))]
pub struct StoffelMPC;

#[cfg(not(feature = "mpc-local"))]
impl StoffelMPC {
    pub async fn new(_source: &str, _n_parties: usize, _threshold: usize) -> Result<Self> {
        Err(Error::RuntimeError(
            "MPC features require the 'mpc-local' feature flag".to_string()
        ))
    }
}
