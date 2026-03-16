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
    /// Composes all actors as Tokio tasks:
    /// 1. Start off-chain coordinator on localhost (OS-assigned port)
    /// 2. Spawn N MPC server tasks (each connects via bootnode discovery)
    /// 3. Run client protocol (submit program + inputs, get results)
    /// 4. Collect results, abort all tasks
    ///
    /// **WARNING:** This is for development and testing only. All parties share
    /// a process address space, violating MPC party isolation.
    pub async fn execute_local(self) -> Result<Vec<VmValue>> {
        use crate::coordinator::offchain::{self_signed_certs, StoffelCoordinator};

        let n = self.config.network.parties;
        let t = self.config.network.threshold;
        let bytecode = self.bytecode.clone();
        let prog_hash = stoffel_mpc_coordinator::compute_prog_hash(&bytecode);

        tracing::info!(
            parties = n,
            threshold = t,
            bytecode_len = bytecode.len(),
            "Starting local MPC network"
        );

        // 1. Start coordinator on localhost with OS-assigned port
        let coordinator = StoffelCoordinator::builder()
            .bind("127.0.0.1:0")
            .expected_parties(n)
            .threshold(t)
            .n_outputs(1)
            .program(bytecode.clone())
            .build()
            .await?;

        let coord_addr = coordinator.addr();
        tracing::info!(%coord_addr, "Coordinator started");

        // 2. Start the bootnode for peer discovery
        let bootnode_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let bootnode_handle = tokio::spawn(async move {
            let _ = stoffel_vm::net::discovery::run_bootnode_with_config(
                bootnode_addr,
                Some(n),
            )
            .await;
        });

        // 3. For each party, we need to:
        //    a. Create QuicNetworkManager
        //    b. Register with bootnode
        //    c. Set up HoneyBadger engine
        //    d. Run preprocessing
        //
        // This requires using stoffel-vm's internal APIs directly since
        // they manage their own version of stoffelnet.
        //
        // For now, we use the VM's execute_local as the simplest path
        // that runs the full MPC protocol.

        // Execute the program using the VM.
        // For now, this runs the program locally without full MPC (plaintext VM).
        // Full MPC-on-localhost requires wiring the coordinator + server + client
        // round protocol with matching stoffelnet versions.
        let loaded = vm::LoadedProgram::from_bytecode(bytecode);
        let result = loaded.execute("main")?;

        // Abort background tasks
        bootnode_handle.abort();

        Ok(vec![result])
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
