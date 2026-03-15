# Stoffel Rust SDK

The reference Rust SDK for building Multi-Party Computation (MPC) applications with the Stoffel framework.

## Overview

The Stoffel Rust SDK v0.1.0 provides a unified API for:

- **Stoffel-Lang** &mdash; Compile Stoffel programs to bytecode
- **StoffelVM** &mdash; Execute bytecode locally for testing
- **MPC Backends** &mdash; HoneyBadger (Byzantine fault-tolerant) and AVSS (threshold crypto)
- **Client/Server Architecture** &mdash; Production-ready MPCaaS model
- **On-Chain Coordination** &mdash; Optional blockchain-based orchestration
- **Observability** &mdash; Metrics, health checks, and OpenTelemetry integration

## Quick Start

### Local Execution

```rust
use stoffel_rust_sdk::prelude::*;

fn main() -> Result<()> {
    // Compile and execute locally (no MPC)
    let result = Stoffel::compile("
        main main() -> int64:
            return 42
    ")?
    .execute_local()?;

    println!("Result: {:?}", result);
    Ok(())
}
```

### Building an MPC Runtime

```rust
use stoffel_rust_sdk::prelude::*;

fn main() -> Result<()> {
    let runtime = Stoffel::compile("
        main main(a: secret int64, b: secret int64) -> secret int64:
            return a + b
    ")?
    .parties(5)        // 5-party MPC network
    .threshold(1)      // Tolerates 1 Byzantine fault
    .instance_id(42)   // Unique computation ID
    .build()?;

    // Test locally before deploying
    let result = runtime.program().execute_local()?;
    println!("Result: {:?}", result);
    Ok(())
}
```

### Production Server

```rust
use stoffel_rust_sdk::prelude::*;
use stoffel_rust_sdk::server::StoffelServer;

#[tokio::main]
async fn main() -> Result<()> {
    let server = StoffelServer::builder(0)
        .bind("0.0.0.0:19200")
        .with_peers(&[
            (1, "192.168.1.2:19300"),
            (2, "192.168.1.3:19300"),
            (3, "192.168.1.4:19300"),
        ])
        .with_preprocessing(1000, 500)
        .build()?;

    server.start().await?;
    server.run_forever().await
}
```

### Client

```rust
use stoffel_rust_sdk::client::{StoffelClient, ClientBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = StoffelClient::builder()
        .servers(&["server1:19200", "server2:19200", "server3:19200"])
        .connect()
        .await?;

    let result = client.run(&[42, 58]).await?;
    println!("Sum: {:?}", result);

    client.disconnect().await?;
    Ok(())
}
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
stoffel-rust-sdk = { git = "https://github.com/Stoffel-Labs/stoffel-rust-sdk.git", branch = "feature/sdk-v0.1.0-rewrite" }
```

**Requirements:** Rust 1.75+ (edition 2021)

## Architecture

```
Stoffel::compile(source)        # Entry point
    .parties(5).threshold(1)    # MPC configuration
    .build()                    # Validates and produces StoffelRuntime
        |
        v
StoffelRuntime                  # Compiled program + MPC config
    .program()                  # Access bytecode for local testing
    .execute_local()            # Quick VM execution
```

### Module Structure

```
src/
├── lib.rs              # Stoffel entry point & builder (RFC-001)
├── runtime.rs          # StoffelRuntime
├── error.rs            # Error types: Error, NetworkError, ConsensusError (RFC-009)
├── types.rs            # PartyId, ClientId, ComputationId, Value
├── config/             # Configuration system (RFC-008)
│   ├── mod.rs          # MpcConfig, NetworkConfig, PreprocessingConfig, Curve
│   └── validation.rs   # n >= 4, n >= 3t+1
├── backend/            # MPC protocol backends (RFC-004, RFC-005)
│   ├── mod.rs          # MpcBackend enum, MpcEngine trait, share types
│   ├── honeybadger.rs  # HoneyBadgerEngine
│   └── avss.rs         # AvssEngine + KeyStore
├── client.rs           # StoffelClient API (RFC-002)
├── server.rs           # StoffelServer API (RFC-003)
├── consensus.rs        # ConsensusGate, VerifiedOrdering (RFC-006)
├── coordinator/        # On-chain / off-chain coordination (RFC-007)
│   ├── mod.rs          # Round state machine (7 phases)
│   ├── offchain.rs     # OffChainCoordinator (local testing)
│   └── onchain.rs      # OnChainCoordinator (Solidity contract)
├── observability/      # Metrics & health (RFC-010)
│   ├── mod.rs          # Counter, Gauge, Histogram, ServerMetrics, HealthStatus
│   └── otel.rs         # OtelConfig placeholder
├── compiler.rs         # Stoffel-Lang compiler wrapper
├── vm.rs               # StoffelVM execution wrapper
├── program.rs          # Program (pure bytecode container)
└── prelude.rs          # Convenient re-exports
```

### MPC Participant Roles

| Role | Provides Inputs | Computes | Receives Outputs |
|------|:-:|:-:|:-:|
| **StoffelClient** | Yes | No | Yes |
| **StoffelServer** | No | Yes | No |

### Protocol Backends

| Feature | HoneyBadger | AVSS |
|---------|:-:|:-:|
| Secret Sharing | RobustShare (Reed-Solomon) | Feldman-verifiable |
| Byzantine Tolerance | n >= 3t + 1 | n >= 3t + 1 |
| EC Operations | No | Yes |
| Threshold Signatures | No | Yes |
| Default | Yes | No |

## Configuration

### TOML File

```toml
[mpc]
parties = 7
threshold = 2

[mpc.backend]
protocol = "honeybadger"

[network]
party_id = 0
bind_address = "0.0.0.0:19200"
expected_parties = 7
consensus_timeout_ms = 60000

[network.peers]
1 = "192.168.1.2:19300"
2 = "192.168.1.3:19300"

[preprocessing]
triples = 1000
random_shares = 500
```

### Environment Variable Overrides

Priority: **Env > File > Default**

| Variable | Overrides |
|----------|-----------|
| `STOFFEL_PARTIES` | `mpc.parties` |
| `STOFFEL_THRESHOLD` | `mpc.threshold` |
| `STOFFEL_BACKEND` | `mpc.backend` (honeybadger/avss) |
| `STOFFEL_CURVE` | `mpc.backend.curve` (bls12-381/bn254) |
| `STOFFEL_PARTY_ID` | `network.party_id` |
| `STOFFEL_BIND_ADDRESS` | `network.bind_address` |

```rust
use stoffel_rust_sdk::config::StoffelConfig;

let config = StoffelConfig::load_with_env("stoffel.toml")?;
```

### Valid Party/Threshold Configurations

| Parties | Threshold | Valid | Notes |
|---------|-----------|:-----:|-------|
| 4 | 1 | Yes | Minimum (4 >= 3(1)+1) |
| 5 | 1 | Yes | Default |
| 7 | 2 | Yes | 7 >= 3(2)+1 |
| 10 | 3 | Yes | 10 >= 3(3)+1 |
| 3 | 1 | No | Below minimum |
| 5 | 2 | No | 5 < 3(2)+1 = 7 |

## On-Chain Coordination

The SDK supports blockchain-based MPC orchestration via a round state machine that mirrors the `StoffelCoordinator` Solidity contract:

```
Preprocessing -> InputMaskReservation -> CollectingInputs
    -> InputsCollectionEnd -> Execution -> ExecutionEnd -> OutputCollection
```

```rust
use stoffel_rust_sdk::coordinator::offchain::OffChainCoordinator;
use stoffel_rust_sdk::coordinator::Round;

let coordinator = OffChainCoordinator::new();
assert_eq!(coordinator.current_round()?, Round::Preprocessing);

let mask = coordinator.reserve_input_mask()?;
coordinator.advance_round()?;
```

## Observability

```rust
use stoffel_rust_sdk::observability::{ServerMetrics, HealthStatus};

let metrics = ServerMetrics::new();
metrics.connected_peers.inc();
metrics.computation_latency_ms.observe(42.0);

assert_eq!(metrics.connected_peers.get(), 1);
```

Health checks:

```rust
use stoffel_rust_sdk::server::StoffelServer;

let server = StoffelServer::builder(0).build()?;
match server.health() {
    HealthStatus::Healthy => println!("OK"),
    HealthStatus::Degraded { reason } => println!("Degraded: {}", reason),
    HealthStatus::Unhealthy { reason } => eprintln!("Down: {}", reason),
}
```

## Error Handling

The SDK provides structured error types with context chaining:

```rust
use stoffel_rust_sdk::error::{Error, NetworkError, ConsensusError, ResultExt};

// Match on specific error types
match result {
    Err(Error::Network(NetworkError::ConnectionTimeout { server, .. })) => {
        eprintln!("Server {} not responding", server);
    }
    Err(Error::Consensus(ConsensusError::ClientListDigestMismatch { party_id })) => {
        eprintln!("SECURITY: Party {} has different view!", party_id);
    }
    Err(e) => eprintln!("Error chain: {}", e.chain()),
    Ok(_) => {}
}

// Add context to errors
let data = std::fs::read("program.stfb")
    .context("reading bytecode file")?;
```

## Development

```bash
cargo build          # Build
cargo test           # Run all tests (120 unit + 38 doc)
cargo fmt            # Format
cargo clippy         # Lint
cargo doc --open     # Documentation
```

## RFC Specifications

This SDK was designed from 10 RFCs in the [Stoffel Rust SDK RFC Book](https://hackmd.io/@stoffel-labs/4S4LBWzwS_aznua_GWSY4g):

| RFC | Title |
|-----|-------|
| [RFC-001](https://hackmd.io/@stoffel-labs/g4wwhVD2Rfe5ijQB4FStVw) | Core API & Builder Pattern |
| [RFC-002](https://hackmd.io/@stoffel-labs/NjFzeISZRK6P85IM5NSDhQ) | StoffelClient API |
| [RFC-003](https://hackmd.io/@stoffel-labs/Z0S5rR8USc2sjta0BSU9lw) | StoffelServer API |
| [RFC-004](https://hackmd.io/@stoffel-labs/iQ9OmZXcTSSqyhcRJ6LZEQ) | MPC Backend Trait & HoneyBadger |
| [RFC-005](https://hackmd.io/@stoffel-labs/hYo5e0J3RN6NzNPrbi_Lsg) | AVSS Backend Integration |
| [RFC-006](https://hackmd.io/@stoffel-labs/LsXI2ZCGTBC6GoGmQ1fFYg) | Consensus Protocol |
| [RFC-007](https://hackmd.io/@stoffel-labs/kCYNMTDFQ1-R9OGHc8UmUg) | On-Chain Coordination |
| [RFC-008](https://hackmd.io/@stoffel-labs/lVUx5QUsQSCiGea7m0na_A) | Configuration System |
| [RFC-009](https://hackmd.io/@stoffel-labs/4iCPjSX_SYOPdxtkKLB3Jw) | Error Handling & Recovery |
| [RFC-010](https://hackmd.io/@stoffel-labs/G1jNuDdLQuenpPomm-9bbQ) | Observability & Metrics |

## License

Apache-2.0
