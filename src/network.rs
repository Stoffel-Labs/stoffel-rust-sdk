//! StoffelNetwork — orchestrator for deployable MPC networks.
//!
//! This module provides [`StoffelNetwork`], the top-level type that defines an
//! entire MPC network (coordinator + N servers) and can either run it in-process
//! for development/testing or scaffold deployment artifacts for production.
//!
//! # Architecture
//!
//! ```text
//! StoffelNetwork (orchestrator)
//!   ├── .execute_local()      → runs everything in-process (dev/testing)
//!   ├── .scaffold("./deploy") → generates deployment artifacts (production)
//!   └── Composes:
//!         ├── StoffelCoordinator  → off-chain JSON-RPC coordinator
//!         ├── StoffelServer × N   → MPC compute servers
//!         └── StoffelClient       → input provider + result retriever
//! ```
//!
//! # Examples
//!
//! ## Development: In-Process Execution
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::network::StoffelNetwork;
//! use stoffel_rust_sdk::backend::MpcBackend;
//!
//! # async fn run() -> stoffel_rust_sdk::error::Result<()> {
//! let results = StoffelNetwork::builder()
//!     .program_file("program.stfl")
//!     .parties(5)
//!     .threshold(1)
//!     .backend(MpcBackend::HoneyBadger)
//!     .build()?
//!     .with_inputs(&[("a", 42i64), ("b", 58i64)])
//!     .execute_local()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Production: Generate Deployment Artifacts
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::network::StoffelNetwork;
//!
//! # fn run() -> stoffel_rust_sdk::error::Result<()> {
//! StoffelNetwork::builder()
//!     .program_file("program.stfl")
//!     .parties(5)
//!     .threshold(1)
//!     .build()?
//!     .scaffold("./deployment")?;
//! # Ok(())
//! # }
//! ```

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ark_bls12_381::Fr;
use ark_serialize::CanonicalSerialize;
use ark_std::rand::SeedableRng;

use crate::backend::MpcBackend;
use crate::config::{
    CoordinatorConfig, MpcBackendConfig, NetworkDeployConfig, NetworkParams,
    PreprocessingConfig, TlsConfig,
};
use crate::error::{Error, Result};
use crate::types::Value;
use crate::vm::{self, Value as VmValue};

// ---------------------------------------------------------------------------
// NetworkBuilder
// ---------------------------------------------------------------------------

/// Builder for constructing a [`StoffelNetwork`].
pub struct NetworkBuilder {
    bytecode: Option<Vec<u8>>,
    source_path: Option<PathBuf>,
    n_parties: usize,
    threshold: usize,
    backend: MpcBackend,
    preprocessing: (usize, usize),
    tls_mode: TlsConfig,
    coordinator_bind: Option<SocketAddr>,
}

impl NetworkBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self {
            bytecode: None,
            source_path: None,
            n_parties: 5,
            threshold: 1,
            backend: MpcBackend::HoneyBadger,
            preprocessing: (1000, 500),
            tls_mode: TlsConfig::SelfSigned,
            coordinator_bind: None,
        }
    }

    /// Set the program bytecode directly.
    pub fn program(mut self, bytecode: Vec<u8>) -> Self {
        self.bytecode = Some(bytecode);
        self
    }

    /// Set the program source file path (will be compiled).
    pub fn program_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    /// Set the number of MPC server parties.
    pub fn parties(mut self, n: usize) -> Self {
        self.n_parties = n;
        self
    }

    /// Set the fault-tolerance threshold.
    pub fn threshold(mut self, t: usize) -> Self {
        self.threshold = t;
        self
    }

    /// Set the MPC backend protocol.
    pub fn backend(mut self, backend: MpcBackend) -> Self {
        self.backend = backend;
        self
    }

    /// Set the number of preprocessing triples and random shares.
    pub fn with_preprocessing(mut self, triples: usize, random_shares: usize) -> Self {
        self.preprocessing = (triples, random_shares);
        self
    }

    /// Set the TLS mode for all network participants.
    pub fn tls(mut self, tls: TlsConfig) -> Self {
        self.tls_mode = tls;
        self
    }

    /// Set the coordinator bind address.
    pub fn coordinator_bind(mut self, addr: SocketAddr) -> Self {
        self.coordinator_bind = Some(addr);
        self
    }

    /// Validate configuration and build a [`StoffelNetwork`].
    pub fn build(self) -> Result<StoffelNetwork> {
        // Resolve bytecode
        let bytecode = if let Some(bc) = self.bytecode {
            bc
        } else if let Some(ref path) = self.source_path {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            match ext {
                "stfl" => {
                    let compiler = crate::compiler::Compiler::new();
                    compiler.compile_file(path.to_str().unwrap_or(""))?
                }
                "stfb" | "stfbin" => std::fs::read(path)?,
                _ => {
                    // Try as bytecode first, fall back to source
                    let data = std::fs::read(path)?;
                    data
                }
            }
        } else {
            return Err(Error::Configuration(
                "program bytecode or source file is required".into(),
            ));
        };

        // Validate MPC parameters
        crate::config::validation::validate_mpc(self.n_parties, self.threshold)?;

        let backend_config = match self.backend {
            MpcBackend::HoneyBadger => MpcBackendConfig::HoneyBadger,
            MpcBackend::Avss { curve } => MpcBackendConfig::Avss { curve },
        };

        let coordinator_bind = self
            .coordinator_bind
            .unwrap_or_else(|| "127.0.0.1:0".parse().unwrap()); // OS-assigned port

        let config = NetworkDeployConfig {
            network: NetworkParams {
                parties: self.n_parties,
                threshold: self.threshold,
                backend: backend_config,
                program: None,
            },
            coordinator: CoordinatorConfig {
                bind_address: coordinator_bind,
                expected_parties: Some(self.n_parties),
                threshold: Some(self.threshold),
                n_outputs: 1,
                tls: self.tls_mode.clone(),
            },
            server: (0..self.n_parties)
                .map(|i| crate::config::ServerDeployConfig {
                    party_id: Some(i),
                    bind_address: None,
                    coordinator: coordinator_bind.to_string(),
                    backend: match self.backend {
                        MpcBackend::HoneyBadger => MpcBackendConfig::HoneyBadger,
                        MpcBackend::Avss { curve } => MpcBackendConfig::Avss { curve },
                    },
                    preprocessing: PreprocessingConfig {
                        triples: self.preprocessing.0,
                        random_shares: self.preprocessing.1,
                        ..PreprocessingConfig::default()
                    },
                    tls: self.tls_mode.clone(),
                })
                .collect(),
            client: None,
        };

        Ok(StoffelNetwork {
            bytecode,
            config,
            inputs: Vec::new(),
            client_inputs: Vec::new(),
        })
    }
}

impl Default for NetworkBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// StoffelNetwork
// ---------------------------------------------------------------------------

/// An MPC network definition that can be executed locally or scaffolded
/// for production deployment.
///
/// Created via [`StoffelNetwork::builder()`] or [`StoffelNetwork::from_config()`].
#[derive(Debug)]
pub struct StoffelNetwork {
    bytecode: Vec<u8>,
    config: NetworkDeployConfig,
    inputs: Vec<(String, Value)>,
    client_inputs: Vec<(u64, Vec<i64>)>,
}

impl StoffelNetwork {
    /// Create a new [`NetworkBuilder`].
    pub fn builder() -> NetworkBuilder {
        NetworkBuilder::new()
    }

    /// Load a network definition from a `stoffel-network.toml` file.
    pub fn from_config(path: &str) -> Result<Self> {
        let config = NetworkDeployConfig::load_with_env(path)?;
        config.validate()?;

        let bytecode = if let Some(ref prog_path) = config.network.program {
            std::fs::read(prog_path)?
        } else {
            return Err(Error::Configuration(
                "network config must specify a program path".into(),
            ));
        };

        Ok(Self {
            bytecode,
            config,
            inputs: Vec::new(),
            client_inputs: Vec::new(),
        })
    }

    /// Provide named inputs for the computation.
    pub fn with_inputs(mut self, inputs: &[(&str, impl Into<Value> + Clone)]) -> Self {
        self.inputs = inputs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone().into()))
            .collect();
        self
    }

    /// Provide inputs from an existing vector (used internally by Stoffel builder).
    pub fn with_inputs_from(mut self, inputs: Vec<(String, Value)>) -> Self {
        self.inputs = inputs;
        self
    }

    /// Provide client inputs for local execution.
    ///
    /// Each tuple is `(client_id, list_of_integer_values)`.
    /// During [`execute_local()`](Self::execute_local), these are secret-shared
    /// and injected into each party's VM `ClientInputStore`, enabling programs
    /// that use `ClientStore.take_share()`.
    pub fn with_client_inputs(mut self, inputs: Vec<(u64, Vec<i64>)>) -> Self {
        self.client_inputs = inputs;
        self
    }

    /// Return a reference to the network configuration.
    pub fn config(&self) -> &NetworkDeployConfig {
        &self.config
    }

    /// Return a reference to the program bytecode.
    pub fn bytecode(&self) -> &[u8] {
        &self.bytecode
    }

    /// Execute the computation locally with a full MPC network on localhost.
    ///
    /// Spins up a real HoneyBadger MPC network with N parties communicating
    /// over QUIC on localhost ports. Each party runs preprocessing, loads the
    /// program bytecode, and executes it with full secret-sharing semantics.
    ///
    /// 1. Install TLS crypto provider
    /// 2. Create N `HoneyBadgerQuicServer` instances in a mesh topology
    /// 3. Start servers and connect peers
    /// 4. Create `MpcRunner` per party (wraps VM + MPC engine)
    /// 5. Run HoneyBadger preprocessing via the engine
    /// 6. Load bytecode and inject client inputs (if any)
    /// 7. Execute `main` on all parties in parallel
    /// 8. Return party 0's result
    ///
    /// **WARNING:** This is for development and testing only. All parties share
    /// a process address space, violating MPC party isolation.
    pub async fn execute_local(self) -> Result<Vec<VmValue>> {
        use std::sync::Once;
        use std::time::Duration;

        use tokio::sync::mpsc;

        use stoffel_vm::net::{
            honeybadger_node_opts,
            HoneyBadgerQuicConfig, HoneyBadgerQuicServer, MpcRunner,
        };
        use stoffelmpc_mpc::common::SecretSharingScheme;
        use stoffelmpc_mpc::honeybadger::robust_interpolate::robust_interpolate::RobustShare;

        let n = self.config.network.parties;
        let t = self.config.network.threshold;
        let bytecode = self.bytecode.clone();

        // Extract preprocessing parameters from server config
        let (n_triples, n_random_shares) = self
            .config
            .server
            .first()
            .map(|s| (s.preprocessing.triples, s.preprocessing.random_shares))
            .unwrap_or((1000, 500));

        tracing::info!(
            parties = n,
            threshold = t,
            triples = n_triples,
            random_shares = n_random_shares,
            bytecode_len = bytecode.len(),
            "Starting local MPC network with full HoneyBadger protocol"
        );

        // Step 1: Install rustls crypto provider (idempotent)
        static INIT_CRYPTO: Once = Once::new();
        INIT_CRYPTO.call_once(|| {
            if rustls::crypto::CryptoProvider::get_default().is_none() {
                let _ = rustls::crypto::ring::default_provider().install_default();
            }
        });

        // Step 2: Derive instance_id from bytecode hash and pick base port
        let instance_id = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            bytecode.hash(&mut hasher);
            hasher.finish()
        };
        let base_port: u16 = 19200 + (instance_id % 10000) as u16;

        let quic_config = HoneyBadgerQuicConfig {
            mpc_timeout: Duration::from_secs(120),
            connection_retry_delay: Duration::from_millis(100),
            ..Default::default()
        };

        // Determine client IDs for input injection
        let client_ids: Vec<usize> = if !self.client_inputs.is_empty() {
            self.client_inputs.iter().map(|(id, _)| *id as usize).collect()
        } else if !self.inputs.is_empty() {
            (0..self.inputs.len()).collect()
        } else {
            vec![]
        };

        // Step 3: Create MPC options and server addresses
        let mpc_opts = honeybadger_node_opts(n, t, n_triples, n_random_shares, instance_id);

        let addresses: Vec<SocketAddr> = (0..n)
            .map(|i| {
                format!("127.0.0.1:{}", base_port + i as u16)
                    .parse()
                    .expect("valid localhost address")
            })
            .collect();

        // Step 4: Create all HoneyBadger QUIC servers
        tracing::info!("Creating {} HoneyBadger QUIC servers", n);
        let mut servers: Vec<HoneyBadgerQuicServer<Fr>> = Vec::with_capacity(n);

        for i in 0..n {
            let (tx, _rx) = mpsc::channel(1500);
            let mut server = HoneyBadgerQuicServer::new(
                i,
                addresses[i],
                mpc_opts.clone(),
                quic_config.clone(),
                tx,
                client_ids.clone(),
            )
            .await
            .map_err(|e| Error::Runtime(format!(
                "Failed to create server {i} at {}: {e:?}", addresses[i]
            )))?;

            // Add all other servers as peers
            for j in 0..n {
                if i != j {
                    server.add_peer(j, addresses[j]).await;
                }
            }

            servers.push(server);
        }

        // Step 5: Start servers and connect peers
        tracing::info!("Starting servers and connecting peers");
        for (i, server) in servers.iter_mut().enumerate() {
            server.start().await.map_err(|e| {
                Error::Runtime(format!("Server {i} failed to start: {e:?}"))
            })?;
        }

        for server in &servers {
            server.connect_to_peers().await.map_err(|e| {
                Error::Runtime(format!("Peer connection failed: {e:?}"))
            })?;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Step 6: Create MpcRunner per party (wraps VM + MPC engine)
        // MpcRunner::from_node() creates an HoneyBadgerMpcEngine from the
        // server's node and attaches it to a fresh VirtualMachine.
        tracing::info!("Creating MPC runners");
        let mut runners: Vec<MpcRunner> = Vec::with_capacity(n);
        for (i, server) in servers.iter().enumerate() {
            let network = server.network.clone().ok_or_else(|| {
                Error::Runtime(format!("Server {i} network not set after start"))
            })?;
            let node = server.node.clone();

            let runner = MpcRunner::from_node(instance_id, i, n, t, network, node);

            // Load bytecode into the runner's VM
            {
                let vm_lock = runner.vm();
                let mut vm = vm_lock.lock();
                vm::load_bytecode_into_vm(&mut vm, &bytecode)?;
            }

            runners.push(runner);
        }

        // Step 7: Run preprocessing on all parties in parallel via the engine
        tracing::info!("Running HoneyBadger preprocessing");
        let preprocessing_handles: Vec<_> = runners
            .iter()
            .enumerate()
            .map(|(i, runner)| {
                let engine = runner.mpc_engine().clone();
                tokio::spawn(async move {
                    engine
                        .preprocess()
                        .await
                        .map_err(|e| format!("Party {i} preprocessing failed: {e}"))
                })
            })
            .collect();

        let preprocessing_results = futures::future::join_all(preprocessing_handles).await;
        for result in preprocessing_results {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => return Err(Error::Preprocessing(e)),
                Err(e) => return Err(Error::Preprocessing(format!("Task panicked: {e}"))),
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        tracing::info!("Preprocessing complete");

        // Step 8: Inject client inputs
        let has_client_inputs = !self.client_inputs.is_empty();
        let has_named_inputs = !self.inputs.is_empty();

        if has_client_inputs {
            tracing::info!(
                clients = self.client_inputs.len(),
                "Injecting client inputs via secret sharing"
            );
            let mut rng = ark_std::rand::rngs::StdRng::from_entropy();

            for (client_id, values) in &self.client_inputs {
                for (input_idx, val) in values.iter().enumerate() {
                    let secret = Fr::from(*val as u64);
                    let shares: Vec<_> = RobustShare::compute_shares(secret, n, t, None, &mut rng)
                        .map_err(|e| Error::Computation(format!(
                            "Failed to compute shares for client {client_id} input {input_idx}: {e:?}"
                        )))?;

                    for (party_id, runner) in runners.iter().enumerate() {
                        let mut bytes = Vec::new();
                        shares[party_id]
                            .serialize_compressed(&mut bytes)
                            .map_err(|e| Error::Computation(format!(
                                "Share serialization failed: {e}"
                            )))?;

                        let vm_lock = runner.vm();
                        let vm = vm_lock.lock();
                        let store = vm.state.client_store();
                        store.store_client_input_bytes(*client_id as usize, vec![bytes]);
                    }
                }
            }
        } else if has_named_inputs {
            tracing::info!(
                inputs = self.inputs.len(),
                "Injecting named inputs as client inputs via secret sharing"
            );
            let mut rng = ark_std::rand::rngs::StdRng::from_entropy();

            for (client_idx, (_name, value)) in self.inputs.iter().enumerate() {
                let secret = match value {
                    Value::Int64(v) => Fr::from(*v as u64),
                    _ => continue,
                };
                let shares: Vec<_> = RobustShare::compute_shares(secret, n, t, None, &mut rng)
                    .map_err(|e| Error::Computation(format!(
                        "Failed to compute shares for input {client_idx}: {e:?}"
                    )))?;

                for (party_id, runner) in runners.iter().enumerate() {
                    let mut bytes = Vec::new();
                    shares[party_id]
                        .serialize_compressed(&mut bytes)
                        .map_err(|e| Error::Computation(format!(
                            "Share serialization failed: {e}"
                        )))?;

                    let vm_lock = runner.vm();
                    let vm = vm_lock.lock();
                    let store = vm.state.client_store();
                    store.store_client_input_bytes(client_idx, vec![bytes]);
                }
            }
        }

        // Step 9: Execute main on all parties in parallel via MpcRunner
        // MpcRunner::execute_function uses parking_lot::Mutex internally which
        // makes its future non-Send. We use LocalSet to run these non-Send
        // futures concurrently on the current thread.
        tracing::info!("Executing program on {} parties", n);

        let local_set = tokio::task::LocalSet::new();
        let execution_timeout = Duration::from_secs(120);

        let runners: Vec<Arc<MpcRunner>> = runners.into_iter().map(Arc::new).collect();

        let result = local_set.run_until(async {
            let execution_handles: Vec<_> = runners
                .iter()
                .enumerate()
                .map(|(i, runner)| {
                    let runner = Arc::clone(runner);
                    tokio::task::spawn_local(async move {
                        runner
                            .execute_function("main")
                            .await
                            .map_err(|e| format!("Party {i} execution failed: {e}"))
                    })
                })
                .collect();

            tokio::time::timeout(
                execution_timeout,
                futures::future::join_all(execution_handles),
            )
            .await
        }).await
        .map_err(|_| Error::Computation(format!(
            "MPC execution timed out after {execution_timeout:?}"
        )))?;

        // Take party 0's result
        let party0_result = result
            .into_iter()
            .next()
            .ok_or_else(|| Error::Computation("No party results".into()))?
            .map_err(|e| Error::Computation(format!("Party 0 task panicked: {e}")))?
            .map_err(|e| Error::Computation(e))?;

        let sdk_value = vm::convert_vm_value_to_sdk_value(party0_result.value);

        // Step 10: Clean up - stop servers
        for mut server in servers {
            server.stop().await;
        }

        tracing::info!("Local MPC execution complete");
        Ok(vec![sdk_value])
    }

    /// Generate deployment artifacts for the MPC network.
    ///
    /// Creates a directory structure with:
    /// - `stoffel-network.toml` — full network topology
    /// - `config/coordinator.toml` — coordinator config
    /// - `config/server.toml` — server config (PARTY_ID from env)
    /// - `config/client.toml` — client config
    /// - `coordinator/main.rs` + `Cargo.toml`
    /// - `server/main.rs` + `Cargo.toml`
    /// - `client/main.rs` + `Cargo.toml`
    /// - `program.stfb` — compiled bytecode
    /// - `docker-compose.yml`
    /// - `Dockerfile.coordinator`, `Dockerfile.server`, `Dockerfile.client`
    pub fn scaffold(&self, output_dir: impl AsRef<Path>) -> Result<()> {
        let dir = output_dir.as_ref();
        let n = self.config.network.parties;
        let t = self.config.network.threshold;

        // Create directory structure
        std::fs::create_dir_all(dir.join("config"))?;
        std::fs::create_dir_all(dir.join("coordinator/src"))?;
        std::fs::create_dir_all(dir.join("server/src"))?;
        std::fs::create_dir_all(dir.join("client/src"))?;

        // Write program bytecode
        std::fs::write(dir.join("program.stfb"), &self.bytecode)?;

        // Write network config
        let network_toml = toml::to_string_pretty(&self.config)
            .map_err(|e| Error::Configuration(format!("TOML serialization error: {e}")))?;
        std::fs::write(dir.join("stoffel-network.toml"), network_toml)?;

        // Write coordinator config
        let coord_config = toml::to_string_pretty(&self.config.coordinator)
            .map_err(|e| Error::Configuration(format!("TOML serialization error: {e}")))?;
        std::fs::write(
            dir.join("config/coordinator.toml"),
            format!(
                "# Coordinator configuration\n\
                 # Bind address and TLS can be overridden via BIND_ADDRESS and STOFFEL_TLS_MODE env vars\n\n\
                 [coordinator]\n\
                 expected_parties = {n}\n\
                 threshold = {t}\n\n\
                 {coord_config}"
            ),
        )?;

        // Write server config
        std::fs::write(
            dir.join("config/server.toml"),
            format!(
                "# Server configuration\n\
                 # PARTY_ID is set via env var per container\n\
                 # COORDINATOR_ADDR overrides the coordinator address\n\n\
                 [server]\n\
                 coordinator = \"coordinator:31415\"\n\n\
                 [server.backend]\n\
                 protocol = \"honeybadger\"\n\n\
                 [server.preprocessing]\n\
                 triples = {triples}\n\
                 random_shares = {random}\n\n\
                 [server.tls]\n\
                 mode = \"self-signed\"\n",
                triples = self.config.server.first().map(|s| s.preprocessing.triples).unwrap_or(1000),
                random = self.config.server.first().map(|s| s.preprocessing.random_shares).unwrap_or(500),
            ),
        )?;

        // Write client config
        std::fs::write(
            dir.join("config/client.toml"),
            "# Client configuration\n\
             # COORDINATOR_ADDR overrides the coordinator address\n\n\
             [client]\n\
             coordinator = \"coordinator:31415\"\n\
             program = \"program.stfb\"\n\n\
             [client.tls]\n\
             mode = \"self-signed\"\n",
        )?;

        // Write coordinator main.rs
        std::fs::write(
            dir.join("coordinator/src/main.rs"),
            "use stoffel_rust_sdk::coordinator::offchain::StoffelCoordinator;\n\
             \n\
             #[tokio::main]\n\
             async fn main() -> stoffel_rust_sdk::error::Result<()> {\n\
             \x20   let coordinator = StoffelCoordinator::from_config(\"config/coordinator.toml\").await?;\n\
             \x20   println!(\"Coordinator listening on {}\", coordinator.addr());\n\
             \x20   coordinator.run_forever().await\n\
             }\n",
        )?;

        // Write coordinator Cargo.toml
        std::fs::write(
            dir.join("coordinator/Cargo.toml"),
            "[package]\n\
             name = \"stoffel-coordinator\"\n\
             version = \"0.1.0\"\n\
             edition = \"2021\"\n\n\
             [dependencies]\n\
             stoffel-rust-sdk = { git = \"https://github.com/Stoffel-Labs/stoffel-rust-sdk.git\" }\n\
             tokio = { version = \"1.0\", features = [\"full\"] }\n",
        )?;

        // Write server main.rs
        std::fs::write(
            dir.join("server/src/main.rs"),
            "use stoffel_rust_sdk::server::StoffelServer;\n\
             \n\
             #[tokio::main]\n\
             async fn main() -> stoffel_rust_sdk::error::Result<()> {\n\
             \x20   let server = StoffelServer::from_config(\"config/server.toml\")?;\n\
             \x20   println!(\"Server party {} starting...\", server.party_id());\n\
             \x20   server.run_forever().await\n\
             }\n",
        )?;

        // Write server Cargo.toml
        std::fs::write(
            dir.join("server/Cargo.toml"),
            "[package]\n\
             name = \"stoffel-server\"\n\
             version = \"0.1.0\"\n\
             edition = \"2021\"\n\n\
             [dependencies]\n\
             stoffel-rust-sdk = { git = \"https://github.com/Stoffel-Labs/stoffel-rust-sdk.git\" }\n\
             tokio = { version = \"1.0\", features = [\"full\"] }\n",
        )?;

        // Write client main.rs
        std::fs::write(
            dir.join("client/src/main.rs"),
            "use stoffel_rust_sdk::client::StoffelClient;\n\
             \n\
             #[tokio::main]\n\
             async fn main() -> stoffel_rust_sdk::error::Result<()> {\n\
             \x20   let client = StoffelClient::from_config(\"config/client.toml\").await?;\n\
             \x20   let results = client.run(&[42, 58]).await?;\n\
             \x20   println!(\"Result: {:?}\", results);\n\
             \x20   client.disconnect().await\n\
             }\n",
        )?;

        // Write client Cargo.toml
        std::fs::write(
            dir.join("client/Cargo.toml"),
            "[package]\n\
             name = \"stoffel-client\"\n\
             version = \"0.1.0\"\n\
             edition = \"2021\"\n\n\
             [dependencies]\n\
             stoffel-rust-sdk = { git = \"https://github.com/Stoffel-Labs/stoffel-rust-sdk.git\" }\n\
             tokio = { version = \"1.0\", features = [\"full\"] }\n",
        )?;

        // Write docker-compose.yml
        let mut compose = String::from(
            "services:\n\
             \x20 coordinator:\n\
             \x20   build: { dockerfile: Dockerfile.coordinator }\n\
             \x20   ports: [\"31415:31415\"]\n\
             \x20   volumes:\n\
             \x20     - ./config:/app/config\n\
             \x20     - ./program.stfb:/app/program.stfb\n\n",
        );

        for i in 0..n {
            compose.push_str(&format!(
                "\x20 server-{i}:\n\
                 \x20   build: {{ dockerfile: Dockerfile.server }}\n\
                 \x20   environment:\n\
                 \x20     PARTY_ID: \"{i}\"\n\
                 \x20     COORDINATOR_ADDR: \"coordinator:31415\"\n\
                 \x20   volumes:\n\
                 \x20     - ./config:/app/config\n\n",
            ));
        }

        compose.push_str(&format!(
            "\x20 client:\n\
             \x20   build: {{ dockerfile: Dockerfile.client }}\n\
             \x20   environment:\n\
             \x20     COORDINATOR_ADDR: \"coordinator:31415\"\n\
             \x20   volumes:\n\
             \x20     - ./config:/app/config\n\
             \x20     - ./program.stfb:/app/program.stfb\n\
             \x20   depends_on:\n"
        ));
        for i in 0..n {
            compose.push_str(&format!("\x20     - server-{i}\n"));
        }
        compose.push_str("\x20     - coordinator\n");

        std::fs::write(dir.join("docker-compose.yml"), compose)?;

        // Write Dockerfiles
        let dockerfile_template = |name: &str| {
            format!(
                "FROM rust:1.78 AS builder\n\
                 WORKDIR /build\n\
                 COPY {name}/ ./\n\
                 RUN cargo build --release\n\n\
                 FROM debian:bookworm-slim\n\
                 RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*\n\
                 WORKDIR /app\n\
                 COPY --from=builder /build/target/release/stoffel-{name} /app/stoffel-{name}\n\
                 COPY config/ /app/config/\n\
                 CMD [\"/app/stoffel-{name}\"]\n"
            )
        };

        std::fs::write(dir.join("Dockerfile.coordinator"), dockerfile_template("coordinator"))?;
        std::fs::write(dir.join("Dockerfile.server"), dockerfile_template("server"))?;
        std::fs::write(dir.join("Dockerfile.client"), dockerfile_template("client"))?;

        tracing::info!(
            output_dir = %dir.display(),
            parties = n,
            threshold = t,
            "Scaffolded MPC network deployment"
        );

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_defaults() {
        let builder = NetworkBuilder::new();
        assert_eq!(builder.n_parties, 5);
        assert_eq!(builder.threshold, 1);
        assert!(matches!(builder.backend, MpcBackend::HoneyBadger));
    }

    #[test]
    fn builder_requires_program() {
        let result = StoffelNetwork::builder().build();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("program"));
    }

    #[test]
    fn builder_validates_mpc_params() {
        let result = StoffelNetwork::builder()
            .program(vec![0u8; 10])
            .parties(3) // too few
            .threshold(1)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn builder_creates_network() {
        let network = StoffelNetwork::builder()
            .program(vec![0u8; 10])
            .parties(5)
            .threshold(1)
            .build()
            .unwrap();

        assert_eq!(network.config.network.parties, 5);
        assert_eq!(network.config.network.threshold, 1);
        assert_eq!(network.config.server.len(), 5);
        assert_eq!(network.bytecode.len(), 10);
    }

    #[test]
    fn with_inputs() {
        let network = StoffelNetwork::builder()
            .program(vec![0u8; 10])
            .build()
            .unwrap()
            .with_inputs(&[("a", 42i64), ("b", 58i64)]);

        assert_eq!(network.inputs.len(), 2);
    }

    #[test]
    fn scaffold_creates_files() {
        let tmp = std::env::temp_dir().join("stoffel_scaffold_test");
        let _ = std::fs::remove_dir_all(&tmp);

        let network = StoffelNetwork::builder()
            .program(vec![0u8; 10])
            .parties(4)
            .threshold(1)
            .build()
            .unwrap();

        network.scaffold(&tmp).unwrap();

        // Verify key files exist
        assert!(tmp.join("stoffel-network.toml").exists());
        assert!(tmp.join("program.stfb").exists());
        assert!(tmp.join("config/coordinator.toml").exists());
        assert!(tmp.join("config/server.toml").exists());
        assert!(tmp.join("config/client.toml").exists());
        assert!(tmp.join("coordinator/src/main.rs").exists());
        assert!(tmp.join("server/src/main.rs").exists());
        assert!(tmp.join("client/src/main.rs").exists());
        assert!(tmp.join("docker-compose.yml").exists());
        assert!(tmp.join("Dockerfile.coordinator").exists());
        assert!(tmp.join("Dockerfile.server").exists());
        assert!(tmp.join("Dockerfile.client").exists());

        // Verify docker-compose has correct number of servers
        let compose = std::fs::read_to_string(tmp.join("docker-compose.yml")).unwrap();
        assert!(compose.contains("server-0"));
        assert!(compose.contains("server-3"));

        // Clean up
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
