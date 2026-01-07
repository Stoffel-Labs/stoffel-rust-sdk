# Stoffel Rust SDK

A production-ready Rust SDK for the Stoffel ecosystem, providing easy access to Stoffel-Lang compilation, StoffelVM execution, and Multi-Party Computation (MPC) protocols.

## Overview

The Stoffel Rust SDK brings together three core components:

- **Stoffel-Lang**: Compile Stoffel programs to bytecode
- **StoffelVM**: Execute bytecode in the Stoffel virtual machine
- **MPC Protocols**: Multi-party computation primitives for secure distributed computation

The SDK provides an **MPCaaS (MPC as a Service)** architecture where:
- **Clients** (app developers): Connect to an MPC network, provide inputs, receive outputs
- **Servers** (infrastructure operators): Form the MPC network, perform secure computations

## Installation

### Prerequisites

- Rust 1.70 or later
- Git with submodule support

### Setup

Clone the repository with submodules:

```bash
git clone --recurse-submodules https://github.com/Stoffel-Labs/stoffel-rust-sdk.git
cd stoffel-rust-sdk
```

Or if you already cloned without submodules:

```bash
git submodule update --init --recursive
```

### Build

```bash
cargo build
cargo build --release  # Optimized build
```

## Quick Start

### For App Developers (Client)

Connect to an existing MPC network and run computations:

```rust
use stoffel_rust_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Connect to MPC network
    let client = StoffelClient::builder()
        .with_servers(&["server1:19200", "server2:19200", "server3:19200"])
        .connect()
        .await?;

    // Submit inputs and get result
    let result = client.run(&[42, 100]).await?;
    println!("Result: {:?}", result);
    Ok(())
}
```

### For Infrastructure Operators (Server)

Run MPC compute nodes:

```rust
use stoffel_rust_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Compile the program
    let program = Stoffel::compile("main main() -> secret int64:\n  return 42")?
        .parties(5)
        .threshold(1)
        .build()?;

    // Create and start server
    let server = Stoffel::server(0)
        .bind("0.0.0.0:19200")
        .with_peers(&[
            (1, "server2:19201"),
            (2, "server3:19202"),
            (3, "server4:19203"),
            (4, "server5:19204"),
        ])
        .with_program(program.program().clone())
        .with_preprocessing(10, 20)
        .build()?;

    server.start().await?;
    server.run_forever().await
}
```

### Local Testing

Test programs locally before MPC deployment:

```rust
use stoffel_rust_sdk::prelude::*;

fn main() -> Result<()> {
    let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
        .parties(5)
        .threshold(1)
        .build()?;

    // Test locally
    let result = runtime.program().execute_local()?;
    println!("Result: {:?}", result);
    Ok(())
}
```

## Examples

Run the included examples:

```bash
# Full E2E MPC demonstration
cargo run --example honeybadger_mpc_demo

# Run an MPC server
cargo run --example mpcaas_server

# Connect as an MPC client
cargo run --example mpcaas_client
```

See [examples/README.md](examples/README.md) for detailed documentation.

## Architecture

### MPCaaS (MPC as a Service)

The SDK implements a client-server architecture:

- **StoffelClient**: For app developers connecting to an MPC network
  - Simple API: connect, submit inputs, receive outputs
  - Handles QUIC networking automatically
  - Does not participate in computation

- **StoffelServer**: For infrastructure operators running MPC nodes
  - Manages peer connections and preprocessing
  - Performs secure multiparty computation using HoneyBadger protocol
  - Cannot learn individual client inputs
  - Byzantine fault-tolerant

### HoneyBadger Protocol

The SDK uses the HoneyBadger MPC protocol:
- **Byzantine fault-tolerant**: Handles up to t malicious parties
- **Constraint**: n >= 3t + 1 (validated at build time)
- **Asynchronous**: No timing assumptions required
- **Secret sharing**: RobustShare with Reed-Solomon error correction

Common configurations:
- 5 parties, threshold 1 (minimum)
- 7 parties, threshold 2 (higher fault tolerance)

### Module Structure

```
src/
├── lib.rs              # Stoffel builder - main entry point
├── prelude.rs          # Simple API exports
├── advanced.rs         # Advanced abstractions
├── network_helpers.rs  # Production infrastructure
│
├── mpcaas/             # MPCaaS API (Primary)
│   ├── mod.rs          # Module exports
│   ├── client.rs       # StoffelClient implementation
│   ├── server.rs       # StoffelServer implementation
│   ├── protocol.rs     # MPCaaS wire protocol
│   ├── client_handler.rs # Server-side client handling
│   ├── peer_manager.rs # Peer connection management
│   └── handle.rs       # ComputationHandle for async
│
├── program.rs          # Compiled program
├── compiler.rs         # Stoffel-Lang compiler wrapper
├── vm.rs               # StoffelVM execution wrapper
├── network_config.rs   # Network configuration types
├── secret_sharing.rs   # Secret sharing utilities
├── mpc_network.rs      # MPC network execution
├── mpc_local.rs        # Local MPC testing
└── error.rs            # Unified error types
```

### Git Submodules

The SDK uses git submodules for dependencies:

```
external/
├── stoffel-lang/       # Stoffel language compiler
├── stoffel-vm/         # VM runtime and types
├── mpc-protocols/      # MPC protocol implementations
└── stoffel-networking/ # QUIC-based networking
```

## Network Configuration

Configure MPC deployment using TOML files:

```toml
# stoffel.toml
[network]
party_id = 0
bind_address = "127.0.0.1:19200"
bootstrap_address = "127.0.0.1:19200"
min_parties = 5

[mpc]
n_parties = 5
threshold = 1
instance_id = 12345
```

```rust
let runtime = Stoffel::compile(source)?
    .network_config_file("stoffel.toml")?
    .build()?;
```

## Development

### Running Tests

```bash
cargo test
```

### Code Quality

```bash
cargo fmt      # Format code
cargo clippy   # Run linter
```

### Building Documentation

```bash
cargo doc --open
```

## Known Limitations

1. **MPC Engine Configuration**: VM execution of secret operations requires MPC network setup
2. **Client Input Access**: Stoffel programs cannot yet access client inputs directly (tracked in STO-104)
3. **MPC Preprocessing**: Requires coordinator service for orchestration (tracked in STO-245)

## License

Apache-2.0

## Links

- [Stoffel-Lang Repository](https://github.com/Stoffel-Labs/Stoffel-Lang)
- [StoffelVM Repository](https://github.com/Stoffel-Labs/StoffelVM)
- [MPC Protocols Repository](https://github.com/Stoffel-Labs/mpc-protocols)
- [Linear Project Board](https://linear.app/stoffel-labs)

---

For detailed development guidance, see [CLAUDE.md](CLAUDE.md).
