# Stoffel Rust SDK

The reference Rust SDK for building Multi-Party Computation (MPC) applications with the Stoffel framework.

## Overview

The Stoffel Rust SDK v0.1.0 provides a unified API for:

- **Stoffel-Lang** &mdash; Compile Stoffel programs to bytecode
- **MPC Backends** &mdash; HoneyBadger (Byzantine fault-tolerant) and AVSS (threshold crypto) via StoffelVM
- **Coordinator-Centric Architecture** &mdash; Program distribution and round orchestration via coordinator
- **Client/Server Model** &mdash; Clients submit inputs to coordinator, servers execute MPC computation
- **On-Chain Coordination** &mdash; Optional blockchain-based orchestration via Solidity contracts
- **Observability** &mdash; Metrics, health checks, and OpenTelemetry integration

## Architecture

```
SDK (thin layer)                    Coordinator (control plane)         StoffelVM (data plane)
  Stoffel::compile() -> bytecode      Receives program from client       MpcRunner executes bytecode
  StoffelClient -> coordinator        Distributes to MPC network         HoneyBadger or AVSS engine
  StoffelServer -> registers          Manages round state machine        Returns output shares
```

The SDK compiles, the coordinator orchestrates, the VM executes. The coordinator distributes the program to the MPC network, enforcing bytecode consistency by construction.

## Quick Start

### Build an MPC Runtime

```rust
use stoffel_rust_sdk::prelude::*;

fn main() -> Result<()> {
    let runtime = Stoffel::compile("
        main main(a: secret int64, b: secret int64) -> secret int64:
            return a + b
    ")?
    .parties(5)        // 5-party MPC network
    .threshold(1)      // Tolerates 1 Byzantine fault
    .build()?;

    // Access configuration
    let mpc = runtime.mpc_config().unwrap();
    assert_eq!(mpc.parties, 5);

    // Create participants from runtime
    let server = runtime.server(0).bind("0.0.0.0:19200").build()?;
    let client = runtime.client().build();

    Ok(())
}
```

### Full MPC on Localhost (Development)

```rust
use stoffel_rust_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let results = Stoffel::compile("
        main main(a: secret int64, b: secret int64) -> secret int64:
            return a + b
    ")?
    .parties(5)
    .threshold(1)
    .with_inputs(&[("a", 42i64), ("b", 58i64)])
    .execute_local()  // Spawns coordinator + N servers as Tokio tasks
    .await?;

    println!("Result: {:?}", results);
    Ok(())
}
```

> **Note:** `execute_local()` runs all parties in a single process for development convenience. For production, deploy separate processes via the Stoffel CLI (`stoffel deploy`). See [security analysis](https://hackmd.io/@stoffel-labs/_6iDFAwMSOm-QDua12Wleg) for details on shared-memory implications.

### Production Server

```rust
use stoffel_rust_sdk::prelude::*;
use stoffel_rust_sdk::server::StoffelServer;

#[tokio::main]
async fn main() -> Result<()> {
    // Server registers with coordinator and receives program from it
    let server = StoffelServer::builder(0)
        .bind("0.0.0.0:19200")
        .coordinator("coordinator.example.com:31415")
        .with_peers(&[
            (1, "192.168.1.2:19200"),
            (2, "192.168.1.3:19200"),
            (3, "192.168.1.4:19200"),
        ])
        .with_preprocessing(1000, 500)
        .build()?;

    server.start().await?;
    server.run_forever().await
}
```

### Client

```rust
use stoffel_rust_sdk::client::StoffelClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Client connects to coordinator, not individual servers
    let client = StoffelClient::builder()
        .coordinator("coordinator.example.com:31415")
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

Requires `~/.cargo/config.toml` for private repo access:

```toml
[net]
git-fetch-with-cli = true
```

**Requirements:** Rust 1.75+ (edition 2021)

## Module Structure

```
src/
├── lib.rs              # Stoffel entry point & builder
├── runtime.rs          # StoffelRuntime (program + MpcConfig + inputs)
├── error.rs            # Error, NetworkError, ConsensusError
├── types.rs            # PartyId, ClientId, ComputationId, Value
├── config/             # Configuration system (RFC-008)
│   ├── mod.rs          # MpcConfig, NetworkConfig, StoffelConfig, Curve
│   └── validation.rs   # n >= 4, n >= 3t+1
├── backend/            # MPC backends — re-exports from StoffelVM
│   ├── mod.rs          # MpcBackend enum, MpcEngine trait, share types
│   ├── honeybadger.rs  # HoneyBadgerMpcEngine (re-export)
│   └── avss.rs         # AvssMpcEngine (re-export)
├── client.rs           # StoffelClient (connects to coordinator)
├── server.rs           # StoffelServer (registers with coordinator)
├── consensus.rs        # ConsensusGate, VerifiedOrdering
├── coordinator/        # Coordination layer
│   ├── mod.rs          # Round state machine (7 phases)
│   ├── offchain.rs     # OffChainCoordinator + real coordinator re-export
│   └── onchain.rs      # OnChainCoordinator + real coordinator re-export
├── observability/      # Metrics & health
│   ├── mod.rs          # Counter, Gauge, Histogram, ServerMetrics, HealthStatus
│   └── otel.rs         # OtelConfig
├── compiler.rs         # Stoffel-Lang compiler wrapper
├── vm.rs               # StoffelVM execution wrapper
├── program.rs          # Program (pure bytecode container)
└── prelude.rs          # Convenient re-exports
```

## Protocol Backends

The SDK re-exports real MPC engines from StoffelVM:

| Feature | HoneyBadger | AVSS |
|---------|:-:|:-:|
| Secret Sharing | RobustShare (Reed-Solomon) | Feldman-verifiable |
| Byzantine Tolerance | n >= 3t + 1 | n >= 3t + 1 |
| EC Operations | No | Yes |
| Threshold Signatures | No | Yes |
| Default | Yes | No |

```rust
// Select protocol at build time
let runtime = Stoffel::compile(source)?
    .backend(MpcBackend::Avss { curve: Curve::Bn254 })
    .parties(5)
    .build()?;
```

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
1 = "192.168.1.2:19200"
2 = "192.168.1.3:19200"

[preprocessing]
triples = 1000
random_shares = 500
```

```rust
let runtime = Stoffel::compile_file("program.stfl")?
    .config_file("stoffel.toml")?
    .build()?;
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

### Valid Party/Threshold Configurations

| Parties | Threshold | Valid | Notes |
|---------|-----------|:-----:|-------|
| 4 | 1 | Yes | Minimum (4 >= 3(1)+1) |
| 5 | 1 | Yes | Default |
| 7 | 2 | Yes | 7 >= 3(2)+1 |
| 10 | 3 | Yes | 10 >= 3(3)+1 |
| 3 | 1 | No | Below minimum |
| 5 | 2 | No | 5 < 3(2)+1 = 7 |

## Coordination

The coordinator manages the MPC round state machine and distributes the program to the network:

```
Preprocessing -> InputMaskReservation -> CollectingInputs
    -> InputsCollectionEnd -> Execution -> ExecutionEnd -> OutputCollection
```

Off-chain (testing) and on-chain (production via Solidity contract) coordinators are both supported.

## Observability

```rust
use stoffel_rust_sdk::observability::{ServerMetrics, HealthStatus};

let metrics = ServerMetrics::new();
metrics.connected_peers.inc();
metrics.computation_latency_ms.observe(42.0);
```

## Error Handling

```rust
use stoffel_rust_sdk::error::{Error, NetworkError, ConsensusError, ResultExt};

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
```

## Development

```bash
cargo build          # Build
cargo test           # Run all tests (101 unit + 29 doc)
cargo fmt            # Format
cargo clippy         # Lint
cargo doc --open     # Documentation
```

## Related Documentation

- [SDK PRD](https://hackmd.io/@stoffel-labs/5c0V-JoJTbeH7YqiHdDFpA)
- [CLI PRD](https://hackmd.io/@stoffel-labs/bTPfeukzQceym6OES8Gvzg)
- [RFC Book](https://hackmd.io/@stoffel-labs/4S4LBWzwS_aznua_GWSY4g) (14 RFCs)
- [In-Process MPC Security Analysis](https://hackmd.io/@stoffel-labs/_6iDFAwMSOm-QDua12Wleg)

## License

Apache-2.0
