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

1. **Stoffel-Lang Integration**: Compiler and language functionality (✅ Integrated)
2. **StoffelVM Integration**: VM runtime and execution environment (✅ Integrated)
3. **MPC Protocols Integration**: Multi-party computation primitives (✅ API complete - HoneyBadger Byzantine fault-tolerant protocol)

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

#### Known Issues
- **MPC Network Integration**: Full MPC network integration requires setting up stoffelnet networking layer. The API wrappers are complete but need network connectivity.

#### Resolved Issues
- **StoffelVM Build Errors** (Closed STO-231): Fixed by switching to the `runner` branch instead of `main`. Added stub `client_store.rs` module for client-server MPC scenarios.

### Design Goals

- **Progressive Disclosure**: Simple by default, powerful when needed
- **Clean Abstractions**: NO raw internal types exposed, even in advanced API
- **Application-Specific SDKs**: Support building specialized SDKs on top of the core functionality
- **Rust-Native**: Idiomatic Rust patterns and best practices

### API Architecture

```rust
use stoffel_rust_sdk::prelude::*;

// Compile with MPC configuration
let runtime = Stoffel::compile(source)?
    .parties(5)
    .threshold(1)
    .backend(MpcBackend::HoneyBadger)
    .build()?;

// Query MPC config
let mpc = runtime.mpc_config().unwrap();

// Test locally
let result = runtime.program().execute_local()?;
```

**Prelude re-exports:**
- `Stoffel`, `StoffelRuntime` - Main entry point and runtime
- `Program`, `VM`, `Compiler` - Compilation and execution
- `MpcConfig`, `MpcBackendConfig`, `Curve`, `StoffelConfig` - Configuration
- `MpcBackend` - Backend protocol selection
- `PartyId`, `ClientId`, `ComputationId` - Shared types
- `Error`, `Result` - Error handling

### Module Structure

```
src/
├── lib.rs              # Stoffel builder - main entry point
├── prelude.rs          # Convenient re-exports
├── error.rs            # Error types (Error, NetworkError, ConsensusError)
├── types.rs            # Shared types (PartyId, ClientId, ComputationId)
├── config/             # Configuration system (RFC-008)
│   ├── mod.rs          # MpcConfig, NetworkConfig, StoffelConfig
│   └── validation.rs   # Parameter validation
├── compiler.rs         # Stoffel-Lang compiler wrapper
├── vm.rs               # StoffelVM execution wrapper
├── program.rs          # Compiled bytecode container
├── runtime.rs          # StoffelRuntime (Program + MpcConfig)
├── backend/            # MPC backend engines (RFC-004, RFC-005)
│   ├── mod.rs          # MpcBackend enum, MpcEngine traits
│   ├── honeybadger.rs  # HoneyBadger engine
│   └── avss.rs         # AVSS engine
├── client.rs           # MPC client (input provider)
├── server.rs           # MPC server (compute node)
├── consensus.rs        # Consensus primitives
├── coordinator/        # Round coordination
│   ├── mod.rs          # Round state machine
│   ├── offchain.rs     # Off-chain coordinator
│   └── onchain.rs      # On-chain coordinator (placeholder)
└── observability/      # Metrics and telemetry
    ├── mod.rs          # Counter, Gauge, Histogram, ServerMetrics
    └── otel.rs         # OpenTelemetry config
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

1. **MPCClient** (`src/client.rs`)
   - For clients sending private inputs to MPC network
   - Wraps `HoneyBadgerMPCClient` from mpc-protocols
   - Handles input secret-sharing and output reconstruction
   - Does not participate in computation

2. **MPCServer** (`src/server.rs`)
   - For MPC network servers performing computation
   - Wraps `HoneyBadgerMPCNode` from mpc-protocols
   - Manages preprocessing material (beaver triples, random shares)
   - Performs secure multiparty computation using HoneyBadger
   - Cannot learn individual client inputs
   - Byzantine fault-tolerant

3. **MPCNode**
   - For full participants in collaborative MPC scenarios
   - Combines client and server functionality
   - Provides inputs AND participates in computation
   - Also uses HoneyBadger protocol

#### Design Rationale

- **Separation of concerns**: Clients don't need to understand MPC internals
- **Scalability**: Many clients can use a fixed MPC network of servers
- **Security**: Clear boundaries between data providers and compute servers
- **Byzantine fault tolerance**: HoneyBadger handles malicious parties (not just crash failures)
- **Automatic validation**: SDK validates n >= 3t + 1 constraint at build time
- **Flexibility**: Full participant mode (MPCNode) available for collaborative scenarios

#### Configuration Requirements

When configuring MPC networks with HoneyBadger:
- Minimum: 4 parties with threshold 1 (4 >= 3*1 + 1)
- Common: 5 parties with threshold 1 (5 >= 3*1 + 1) - used in examples
- Higher tolerance: 7 parties with threshold 2 (7 >= 3*2 + 1)
- Always ensure: n >= 3t + 1 for Byzantine fault tolerance

As the architecture evolves, document key patterns for:
- How the three components interact
- Module boundaries and public API surface
- Integration patterns for application-specific SDKs
