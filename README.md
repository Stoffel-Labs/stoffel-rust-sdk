# Stoffel Rust SDK

The Rust SDK for building Multi-Party Computation (MPC) applications with [Stoffel](https://stoffel.ai).

Write programs in [StoffelLang](https://github.com/Stoffel-Labs/Stoffel-Lang), compile them to bytecode, and execute them across a distributed MPC network where no single party ever sees the plaintext data.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
stoffel-rust-sdk = { git = "https://github.com/Stoffel-Labs/stoffel-rust-sdk.git", branch = "feature/sdk-v0.1.0-rewrite" }
```

If the repo is private, add to `~/.cargo/config.toml`:

```toml
[net]
git-fetch-with-cli = true
```

**Requirements:** Rust 1.75+, Tokio runtime

## Quick Start

```rust
use stoffel_rust_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Compile, configure, and run — three lines
    let results = Stoffel::compile("
        main main(a: secret int64, b: secret int64) -> secret int64:
            return a + b
    ")?
    .parties(5)
    .threshold(1)
    .with_inputs(&[("a", 42i64), ("b", 58i64)])
    .execute_local()
    .await?;

    println!("Result: {:?}", results);
    Ok(())
}
```

`execute_local()` spins up a full MPC network on localhost — coordinator, servers, the works. For production, use `scaffold()` to generate deployable artifacts.

## Deploy to Production

When you're ready to go beyond localhost, `StoffelNetwork` generates everything you need:

```rust
use stoffel_rust_sdk::prelude::*;

fn main() -> Result<()> {
    StoffelNetwork::builder()
        .program_file("program.stfl")
        .parties(5)
        .threshold(1)
        .build()?
        .scaffold("./deployment")?;
    Ok(())
}
```

This creates a complete deployment directory:

```
deployment/
├── coordinator/src/main.rs    # Coordinator binary
├── server/src/main.rs         # Server binary (PARTY_ID from env)
├── client/src/main.rs         # Client binary
├── config/*.toml              # Per-actor TOML configs
├── docker-compose.yml         # One coordinator + N servers + client
├── Dockerfile.*               # Multi-stage builds
└── program.stfb               # Compiled bytecode
```

Then deploy:

```bash
cd deployment
docker-compose up
```

Each actor loads its config from TOML and supports env var overrides (`PARTY_ID`, `COORDINATOR_ADDR`, `STOFFEL_TLS_MODE`).

## Actors

The SDK exposes three actor types that compose into an MPC network:

### StoffelCoordinator

Off-chain coordinator that manages rounds, input masking, and output distribution via JSON-RPC over TLS.

```rust
let coordinator = StoffelCoordinator::builder()
    .bind("0.0.0.0:31415")
    .expected_parties(5)
    .threshold(1)
    .build()
    .await?;

coordinator.run_forever().await?;
```

### StoffelServer

MPC compute node. Connects to peers, runs preprocessing, executes computations.

```rust
let server = StoffelServer::builder(0)   // party_id = 0
    .bind("127.0.0.1:9000")
    .coordinator("127.0.0.1:31415")
    .with_preprocessing(1000, 500)
    .build()?;

server.run_forever().await?;
```

### StoffelClient

Input provider. Submits masked inputs to the coordinator and retrieves results.

```rust
let client = StoffelClient::builder()
    .coordinator("127.0.0.1:31415")
    .with_program(bytecode)
    .connect()
    .await?;

let results = client.run(&[42, 58]).await?;
```

All three support `from_config("path.toml")` for config-driven deployment.

## Configuration

### Programmatic

```rust
StoffelNetwork::builder()
    .program_file("program.stfl")    // or .program(bytecode)
    .parties(5)                       // n >= 3t + 1
    .threshold(1)
    .backend(MpcBackend::HoneyBadger) // default
    .with_preprocessing(1000, 500)
    .tls(TlsConfig::SelfSigned)       // or TlsConfig::Custom { cert_path, key_path }
    .build()?
```

### TOML

```toml
# stoffel-network.toml
[network]
parties = 5
threshold = 1
program = "program.stfb"

[coordinator]
bind_address = "0.0.0.0:31415"

[coordinator.tls]
mode = "self-signed"

[[server]]
coordinator = "coordinator:31415"

[client]
coordinator = "coordinator:31415"
```

```rust
let network = StoffelNetwork::from_config("stoffel-network.toml")?;
```

### Environment Overrides

All config values can be overridden via env vars. Priority: **Env > File > Default**.

| Variable | Overrides | Used By |
|----------|-----------|---------|
| `PARTY_ID` | `server.party_id` | Server |
| `COORDINATOR_ADDR` | `*.coordinator` | Server, Client |
| `BIND_ADDRESS` | `*.bind_address` | Coordinator |
| `STOFFEL_TLS_MODE` | `*.tls.mode` | All |
| `STOFFEL_PARTIES` | `network.parties` | Network |
| `STOFFEL_THRESHOLD` | `network.threshold` | Network |

## How It Works

```
StoffelNetwork
  ├── StoffelCoordinator     JSON-RPC/TLS — manages rounds
  ├── StoffelServer × N      QUIC P2P — HoneyBadger MPC engine
  └── StoffelClient           submits inputs, retrieves outputs

Compile → Distribute → Preprocess → Mask Inputs → Execute → Reconstruct
```

The SDK compiles your program, the coordinator distributes it and orchestrates rounds, and the VM executes it using secret-shared arithmetic. No single party ever sees the plaintext inputs.

## Building Locally

```bash
git clone https://github.com/Stoffel-Labs/stoffel-rust-sdk.git
cd stoffel-rust-sdk

cargo build          # Build
cargo test           # Run tests
cargo clippy         # Lint
cargo doc --open     # API docs
```

## Contributing

1. Fork the repo and create a feature branch
2. Make your changes with tests
3. Run `cargo fmt && cargo clippy && cargo test`
4. Open a pull request

Keep PRs focused on a single change.

## Security

If you discover a security vulnerability, **do not open a public issue**. Email [security@stoffel.ai](mailto:security@stoffel.ai) instead.

`execute_local()` runs all MPC parties in one process — this violates party isolation and is for development only.

## License

Apache-2.0
