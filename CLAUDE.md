# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is the Rust SDK for Stoffel, designed to expose key functionality from:
- **Stoffel-Lang**: The core language
- **StoffelVM**: The virtual machine runtime
- **mpc-protocols**: Multi-party computation protocols

The SDK serves two primary purposes:
1. Provide an easy-to-use API for external developers to use the Stoffel stack in Rust
2. Enable internal development of application-specific SDKs in Rust for specific Stoffel-Lang programs

## Development Commands

### Initial Setup
```bash
# Initialize Rust project structure
cargo init --lib

# Add development dependencies
cargo add --dev tokio --features full
```

### Building and Testing
```bash
# Build the project
cargo build

# Build with release optimizations
cargo build --release

# Run all tests
cargo test

# Run a specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run tests in a specific module
cargo test module_name::

# Check code without building
cargo check
```

### Code Quality
```bash
# Format code
cargo fmt

# Check formatting without making changes
cargo fmt -- --check

# Run linter
cargo clippy

# Run linter with all warnings
cargo clippy -- -W clippy::all
```

### Documentation
```bash
# Build documentation
cargo doc

# Build and open documentation in browser
cargo doc --open
```

## Architecture Notes

### Core Components

The SDK is organized around three main integration points:

1. **Stoffel-Lang Integration**: Compiler and language functionality (Integrated)
2. **StoffelVM Integration**: VM runtime and execution environment (Integrated)
3. **MPC Protocols Integration**: Multi-party computation primitives (API complete - HoneyBadger Byzantine fault-tolerant protocol)

### Integration Status

#### Git Submodules
The SDK uses git submodules to pin exact versions of dependencies:
- `external/stoffel-lang` - Stoffel language compiler
- `external/stoffel-vm` - VM runtime and types
- `external/mpc-protocols` - MPC protocol implementations

To initialize submodules:
```bash
git submodule update --init --recursive
```

### Design Goals

- **Progressive Disclosure**: Simple by default, powerful when needed
- **Clean Abstractions**: NO raw internal types exposed, even in advanced API
- **Application-Specific SDKs**: Support building specialized SDKs on top of the core functionality
- **Rust-Native**: Idiomatic Rust patterns and best practices

### API Architecture: MPCaaS (MPC as a Service)

The SDK provides a production-ready **client-server architecture** where:
- **Clients** (app developers): Connect to an MPC network, provide inputs, receive outputs
- **Servers** (infrastructure operators): Form the MPC network, perform computations

#### Client API (For App Developers)

```rust
use stoffel_rust_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Connect to MPC network
    let client = StoffelClient::builder()
        .with_servers(&["server1:19200", "server2:19200", "server3:19200"])
        .connect()
        .await?;

    // Submit computation and get result
    let result = client.run(&[42, 100]).await?;
    println!("Result: {:?}", result);
    Ok(())
}
```

#### Server API (For Infrastructure Operators)

```rust
use stoffel_rust_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Compile the program
    let program = Stoffel::compile("main main() -> secret int64:\n  ...")?
        .build()?;

    // Create MPC server
    let server = Stoffel::server(0)
        .bind("0.0.0.0:19200")
        .with_peers(&[(1, "server2:19200"), (2, "server3:19200")])
        .with_program(program.program().clone())
        .with_preprocessing(10, 20)
        .build()?;

    server.start().await?;
    server.run_forever().await
}
```

**Re-exports (from `prelude`):**
- `Stoffel`, `StoffelRuntime` - Main API
- `StoffelClient`, `StoffelClientBuilder`, `ClientState` - Client API
- `StoffelServer`, `StoffelServerBuilder`, `ServerState` - Server API
- `ComputationHandle` - Async computation tracking
- `Program`, `VM`, `Compiler` - Core types
- `Error`, `Result` - Error handling

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

### MPC Architecture

The SDK implements an **MPC as a Service** architecture using the **HoneyBadger protocol**:

#### Protocol: HoneyBadger

- **Type**: Byzantine fault-tolerant, asynchronous MPC protocol
- **Constraint**: n >= 3t + 1 (where n = number of parties, t = threshold of faulty parties)
- **Secret Sharing**: RobustShare (Reed-Solomon erasure coding for error correction)
- **Fault Tolerance**: Tolerates up to t Byzantine (malicious or crashed) parties
- **Communication**: Asynchronous (no timing assumptions required)

#### Components

1. **StoffelClient** (`src/mpcaas/client.rs`)
   - For app developers connecting to an existing MPC network
   - Simple API: connect, submit inputs, receive outputs
   - Handles QUIC networking automatically
   - Does not participate in computation

2. **StoffelServer** (`src/mpcaas/server.rs`)
   - For infrastructure operators running MPC compute nodes
   - Manages peer connections, preprocessing, and client handling
   - Performs secure multiparty computation using HoneyBadger
   - Cannot learn individual client inputs
   - Byzantine fault-tolerant

#### Design Rationale

- **Separation of concerns**: Clients don't need to understand MPC internals
- **Scalability**: Many clients can use a fixed MPC network of servers
- **Security**: Clear boundaries between data providers and compute servers
- **Byzantine fault tolerance**: HoneyBadger handles malicious parties (not just crash failures)
- **Automatic validation**: SDK validates n >= 4t + 1 constraint at build time

#### Configuration Requirements

When configuring MPC networks with HoneyBadger:
- **TripleGen constraint**: n >= 4t + 1 (stricter than basic Byzantine n >= 3t + 1)
- Minimum: 5 parties with threshold 1 (5 >= 4*1 + 1)
- Higher tolerance: 9 parties with threshold 2 (9 >= 4*2 + 1)
- The TripleGen preprocessing uses degree-2t shares requiring more parties for robust interpolation

### Examples

Available examples in `examples/`:
- `honeybadger_mpc_demo.rs` - Full E2E MPC demonstration
- `mpcaas_server.rs` - Running an MPC server node
- `mpcaas_client.rs` - Connecting as an MPC client
- `mpc_e2e.rs` - End-to-end MPC with unified API
