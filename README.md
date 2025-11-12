# Stoffel Rust SDK

A friendly, high-level Rust SDK for the Stoffel ecosystem, providing easy access to Stoffel-Lang compilation, StoffelVM execution, and Multi-Party Computation (MPC) protocols.

## Overview

The Stoffel Rust SDK brings together three core components:

- **Stoffel-Lang**: Compile Stoffel programs to bytecode
- **StoffelVM**: Execute bytecode in the Stoffel virtual machine
- **MPC Protocols**: Multi-party computation primitives for secure distributed computation

The SDK is designed to:
1. Provide an easy-to-use API for external developers using the Stoffel stack in Rust
2. Enable internal development of application-specific SDKs for specific Stoffel-Lang programs

## Status

**Current Version:** 0.1.0 (Development)

### Implemented ✅

- **Stoffel-Lang Integration**
  - Compile Stoffel programs to bytecode
  - Full language support

- **StoffelVM Integration**
  - Execute bytecode on the VM
  - Local testing without networking

- **MPC Configuration API**
  - Configure parties, threshold, protocols
  - Builder pattern for participants (Client, Server, Node)
  - Automatic constraint validation (n ≥ 3t + 1 for Byzantine tolerance)

- **MPC Network Infrastructure**
  - `setup_mpc_network()` - High-level server network setup
  - `setup_mpc_clients()` - Automatic client configuration
  - QUIC transport integration
  - Server lifecycle management (start, connect, stop)
  - SDK-level wrappers around StoffelVM networking components
  - Message processor spawning (`spawn_message_processor()`) for protocol message routing
  - Node initialization API (`initialize_node()`) for proper setup order

### Roadmap 🎯

1. **Complete MPC Protocol Integration** - Full preprocessing and computation coordination
2. **Distributed Deployment Helpers** - Tools for multi-machine MPC deployments
3. **Advanced Protocol Support** - Additional MPC protocols beyond HoneyBadger

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
# Build (includes full MPC networking by default)
cargo build

# Optimized build
cargo build --release
```

### Examples

The SDK provides MPC network infrastructure examples running on localhost:

```bash
# ⭐ Complete MPC Workflow (START HERE)
cargo run --example complete_mpc_workflow

# Simple MPC network setup (all parties on 127.0.0.1)
cargo run --example simple_mpc_network

# Bytecode execution on MPC servers
cargo run --example bytecode_execution

# MPC compilation and configuration API
cargo run --example mpc_computation
```

**Recommended starting point**: `complete_mpc_workflow` demonstrates the full 11-step MPC workflow including:
- Node initialization before message processor spawning
- QUIC network setup and peer-to-peer connectivity
- Message processors for protocol message routing
- Concurrent preprocessing (following StoffelVM test patterns)
- Client connection and input distribution

**Note**: Examples run on localhost for testing. For distributed deployment, simply change IP addresses from `127.0.0.1` to actual machine IPs - no code changes needed!

For complete end-to-end MPC execution, see StoffelVM's integration tests:

```bash
cd external/stoffel-vm
cargo test --package stoffel-vm --lib tests::mpc_multiplication_integration -- --nocapture --test-threads=1
```

These tests demonstrate:
- Complete QUIC network setup
- HoneyBadger Byzantine fault-tolerant protocol
- Preprocessing (Beaver triple generation)
- Client input distribution
- Secure computation (10 × 20 = 200)
- Output reconstruction

See [examples/README.md](examples/README.md) for SDK roadmap and integration guidance.

## Quick Start

### SDK Usage

The SDK provides high-level APIs for Stoffel program compilation and VM execution:

```rust
use stoffel_rust_sdk::prelude::*;

fn main() -> Result<()> {
    // 1. Compile Stoffel program
    let source = r#"
main main() -> int64:
  return 42
    "#;

    let runtime = Stoffel::compile(source)?
        .parties(5)       // Configure for 5-party MPC
        .threshold(1)     // Byzantine fault tolerance
        .build()?;

    println!("Protocol: {:?}", runtime.protocol_type());

    // 2. Test locally before MPC deployment
    let result = runtime.program().execute_local()?;
    println!("Result: {:?}", result);

    // 3. Configure MPC participants
    let server = runtime.server(0).build()?;
    let client = runtime.client(100)
        .with_inputs(vec![10, 20])
        .build()?;

    Ok(())
}
```

**What the SDK provides:**
- ✅ Stoffel-Lang compilation
- ✅ VM execution
- ✅ MPC configuration types
- ✅ Participant builders (Client, Server, Node)

**What's next:**
- ⚠️ MPC network infrastructure (see roadmap in [examples/README.md](examples/README.md))

**Key features:**
- **MPC-first design** - Programs ARE MPC programs, not an afterthought
- **Built-in configuration** - Parties and threshold configured during compilation
- **HoneyBadger protocol** - Byzantine fault-tolerant, asynchronous (default)
- **Automatic validation** - Ensures n >= 3t + 1 constraint for Byzantine tolerance
- **Network configuration** - Deploy with TOML configs or manual setup
- **Tight coupling** - Servers and clients created FROM the program
- **Local testing** - `.execute_local()` for testing before deployment
- **Reasonable defaults** - MPC preprocessing calculated automatically

### Network Configuration

Stoffel programs can be configured for network deployment using TOML configuration files or manual setup:

```bash
# See network configuration in action
cargo run --example network_config_demo
```

**TOML Configuration** (`stoffel.toml`):
```toml
[network]
party_id = 0
bind_address = "127.0.0.1:9001"
bootstrap_address = "127.0.0.1:9000"
min_parties = 3

[mpc]
n_parties = 5
threshold = 1
instance_id = 12345
```

**Using Configuration:**
```rust
// From file
let program = Stoffel::compile(source)?
    .network_config_file("stoffel.toml")?
    .build()?;

// Manual configuration
use stoffel_rust_sdk::network_config::*;

let config = NetworkConfigBuilder::new()
    .party_id(0)
    .bind_address("127.0.0.1:9001")
    .bootstrap_address("127.0.0.1:9000")
    .n_parties(5)
    .threshold(1)
    .build()?;

let program = Stoffel::compile(source)?
    .network_config(config)?
    .build()?;
```

See `examples/configs/` for example configuration files.

### Advanced MPC Examples

The fastest way to see the SDK in action with full MPC networking is to run the quick_start example:

```bash
# Self-contained MPC network demonstration
cargo run --example quick_start
```

This example demonstrates:
- **LocalMPCNetwork** - SDK's high-level abstraction for local MPC networks
- Automatic QUIC setup and 3-party mesh network creation
- Compiling Stoffel programs with `secret` types to bytecode (178 bytes)
- MPC engine configuration and VM integration
- Preprocessing protocol initiation across all parties

**What the SDK successfully accomplishes:**
- ✅ Complete QUIC network infrastructure (endpoints, TLS, mesh topology)
- ✅ 3 parties connected over secure channels (ports 19000-19002)
- ✅ HoneyBadgerMpcEngine configured and initialized for all parties
- ✅ Network managers with correct party IDs and node registrations
- ✅ Stoffel compilation with secret types (178 bytes of bytecode)
- ✅ VM configuration with MPC engine support
- ✅ Preprocessing protocol initiation across all parties concurrently

**Architectural note:** The HoneyBadger MPC preprocessing protocol was designed for distributed execution (separate processes/machines). Single-process multi-party simulation has coordination challenges. In the distributed `stoffel-run` environment, preprocessing completes successfully using this same SDK infrastructure.

For integration into your own project, add to your `Cargo.toml`:

```toml
[dependencies]
stoffel-rust-sdk = { path = "../stoffel-rust-sdk" }
ark-bls12-381 = "0.5.0"  # Required for MPC field arithmetic

# Optional: Enable QUIC-based networking
stoffel-rust-sdk = { path = "../stoffel-rust-sdk", features = ["networking"] }
```

### Basic Example

```rust
use stoffel_rust_sdk::prelude::*;

fn main() -> Result<()> {
    // Create a compiler instance
    let compiler = Compiler::new()
        .optimize(true)
        .optimization_level(OptimizationLevel::O2);

    // Create a VM instance
    let vm = VM::new();

    // Create an MPC client (note: type parameter required)
    let client = MPCClient::<ark_bls12_381::Fr>::builder()
        .client_id(100)
        .parties(5)
        .threshold(1)
        .inputs(vec![42, 100, 200])
        .build()?;

    println!("SDK initialized successfully!");
    Ok(())
}
```

### Compiling Source Code

```rust
use stoffel_rust_sdk::compiler::Compiler;

fn main() -> stoffel_rust_sdk::Result<()> {
    let bytecode = Compiler::new()
        .optimize(true)
        .compile_source("fn add(a: i64, b: i64) -> i64 { return a + b; }")?;

    // Save bytecode to file
    std::fs::write("program.stfb", bytecode)?;
    Ok(())
}
```

### Running VM Programs

```rust
use stoffel_rust_sdk::vm::VM;

fn main() -> stoffel_rust_sdk::Result<()> {
    let vm = VM::new();

    // Load and execute bytecode
    let result = vm.load_bytecode("program.stfb")?
        .execute("main")?;

    println!("Result: {:?}", result);
    Ok(())
}
```

### MPC: Creating servers and clients from programs

```rust
use stoffel_rust_sdk::prelude::*;

fn example() -> Result<()> {
    // Compile an MPC program with HoneyBadger protocol
    // HoneyBadger constraint: n >= 3t + 1
    let program = Stoffel::compile("main main() -> int64:\n  return 42")?
        .parties(5)       // n=5 servers
        .threshold(1)     // t=1 Byzantine faults (5 >= 3*1+1 ✓)
        .build()?;

    // Create an MPC server (performs secure computation)
    let server = program.server(0).build()?;

    // Create an MPC client (sends private inputs)
    let client = program.client(100)
        .with_inputs(vec![42, 100, 25])
        .build()?;

    Ok(())
}
```

### MPC: Using network configuration files

```rust
use stoffel_rust_sdk::prelude::*;

fn example() -> Result<()> {
    // Load program with network configuration from file
    let program = Stoffel::compile("main main() -> int64:\n  return 42")?
        .network_config_file("stoffel.toml")?
        .build()?;

    // Create server - inherits network config from program
    let server = program.server(0).build()?;

    // Create client
    let client = program.client(100)
        .with_inputs(vec![6, 7])
        .build()?;

    Ok(())
}
```

### Advanced: MPC Engine Configuration

For executing Stoffel programs with `secret` types, the VM must be configured with an MPC engine. This requires network setup and is demonstrated in the `stoffel-run` binary in the StoffelVM repository.

**Requirements:**
- QUIC network manager for party-to-party communication
- HoneyBadger MPC engine configured with party parameters
- Preprocessing phase to generate beaver triples

**Example workflow** (see `stoffel-vm/src/bin/stoffel-run.rs`):

```rust
// Note: This is an advanced use case requiring network configuration
// The quick_start example demonstrates successful compilation without this setup

use stoffel_vm::net::hb_engine::HoneyBadgerMpcEngine;
use stoffelnet::transports::quic::QuicNetworkManager;
use std::sync::Arc;

async fn configure_mpc() -> Result<(), String> {
    // 1. Set up network manager
    let mut net_mgr = QuicNetworkManager::new();
    net_mgr.listen("127.0.0.1:9000".parse().unwrap()).await?;

    // 2. Create MPC engine
    let engine = HoneyBadgerMpcEngine::new(
        12345,           // instance_id
        0,               // party_id
        5,               // n_parties
        1,               // threshold
        10,              // num_triples
        25,              // num_random
        Arc::new(net_mgr)
    )?;

    // 3. Run preprocessing
    engine.start_async().await?;

    // 4. Configure VM (requires mutable VM instance)
    // vm.state.set_mpc_engine(engine);

    Ok(())
}
```

**Current status:** The SDK provides infrastructure for MPC but network configuration is left to the application. See the StoffelVM repository's `stoffel-run` binary for a complete example of distributed MPC execution.

## Architecture

### Design Philosophy

The SDK follows a **progressive disclosure** approach - simple by default, powerful when needed:

**Three API Levels:**

1. **`prelude` - Simple API** (Recommended for most users)
   - Clean, minimal API for shipping MPC applications
   - Everything needed for common use cases
   - Example: `simple_mpc.rs`

2. **`advanced` - Advanced Abstractions** (For custom applications)
   - Proper abstractions for fine-grained control
   - ShareManager, NetworkBuilder
   - NO raw internal types exposed
   - Example: `advanced_shares.rs`

3. **`network_helpers` - Production Infrastructure**
   - Complete network setup helpers
   - QUIC networking, message handlers
   - Example: `quick_start_local_network_real.rs`

### Core API Flow

```rust
use stoffel_rust_sdk::prelude::*;

// 1. Compile with MPC configuration
let runtime = Stoffel::compile(source)?
    .parties(5)       // 5-party MPC network
    .threshold(1)     // Byzantine fault tolerance
    .build()?;

// 2. Test locally before deployment
let result = runtime.program().execute_local()?;

// 3. Create MPC participants
let server = runtime.server(0).build()?;
let client = runtime.client(100).with_inputs(vec![10, 20]).build()?;
```

**Key Design Principles:**
- **MPC is built-in** - Configure parties/threshold when compiling
- **Testing is explicit** - `.execute_local()` for local testing
- **Clean abstractions** - Even advanced features use proper abstractions
- **Extensible** - Build custom SDKs on top of these primitives

### MPC as a Service Model

The SDK implements an **MPC as a Service** architecture with clear separation between clients and servers:

- **MPCClient**: Clients with private inputs that want computation performed
  - Sends secret-shared inputs to the MPC network
  - Receives computation results
  - Does not participate in the actual computation

- **MPCServer**: Server parties that form the MPC network
  - Receives inputs from clients
  - Performs secure multiparty computation using HoneyBadger protocol
  - Sends results back to clients
  - Cannot learn individual client inputs
  - Byzantine fault-tolerant: tolerates up to t malicious/crashed servers

- **MPCNode**: Full participant mode for collaborative computation
  - Entity acts as both client and server
  - Provides inputs AND participates in computation
  - Used for peer-to-peer collaborative MPC scenarios
  - Also uses HoneyBadger protocol with Byzantine fault tolerance
  - Should be used explicitly when this behavior is desired

This separation enables scalable MPC services where many clients can offload computation to a dedicated MPC network.

**Type Parameters:** All MPC types require a field element type parameter (defaults to `ark_bls12_381::Fr`). You must specify the type explicitly when using the builder pattern:

```rust
// Required: Explicit type parameter
let client = MPCClient::<ark_bls12_381::Fr>::builder()...

// Alternative field types can be used
let server = MPCServer::<ark_bn254::Fr>::builder()...
```

### Module Structure

```
src/
├── lib.rs              # Stoffel builder - main entry point
├── prelude.rs          # ⭐ Simple API (START HERE)
├── advanced.rs         # ⭐⭐ Advanced abstractions (ShareManager, NetworkBuilder)
├── network_helpers.rs  # ⭐⭐⭐ Production infrastructure
│
├── program.rs          # Compiled program with MPC config
├── compiler.rs         # Stoffel-Lang compiler wrapper
├── vm.rs               # StoffelVM execution wrapper
├── client.rs           # MPCClient - input providers
├── server.rs           # MPCServer - compute nodes
├── session.rs          # MPCNode - combined mode
├── network_config.rs   # Network configuration types
├── secret_sharing.rs   # Secret sharing utilities
└── error.rs            # Unified error types
```

**Module Usage:**
- **Most users**: Import `prelude` only
- **Custom applications**: Add `advanced` for ShareManager/NetworkBuilder
- **Production deployments**: Use `network_helpers` for complete infrastructure

### Git Submodules

The SDK uses git submodules to pin exact versions of dependencies:

```
external/
├── stoffel-lang/       # Stoffel language compiler
├── stoffel-vm/         # VM runtime and types (runner branch)
├── mpc-protocols/      # MPC protocol implementations
└── stoffel-networking/ # Modern QUIC-based networking (optional)
```

**Important:** The StoffelVM submodule uses the `runner` branch, not `main`.

### Networking Options

The SDK provides two networking layers:

1. **stoffelmpc-network** (default) - Basic networking from mpc-protocols
2. **stoffelnet** (optional) - Modern QUIC-based networking with enhanced features
   - Enable with the `networking` feature flag
   - Provides transport-agnostic API (`PeerConnection`, `NetworkManager`)
   - Uses QUIC protocol for secure, multiplexed communication
   - Includes connection state management and graceful shutdown

## Development

### Running Examples

Test the SDK with included examples:

```bash
# Run all examples
cargo run --example quick_start
cargo run --example mpc_demo
cargo run --example complete_workflow

# Build all examples
cargo build --examples
```

### Running Tests

```bash
cargo test
```

### Code Quality

Format code:
```bash
cargo fmt
```

Run linter:
```bash
cargo clippy
```

### Building Documentation

```bash
cargo doc --open
```

## Known Limitations

1. **Float Representation**: The runner branch uses fixed-point representation for floats (i64 scaled by 1000). The SDK converts between f64 and this representation.

2. **MPC Engine Configuration**:
   - ✅ Stoffel programs with `secret` types compile successfully to bytecode
   - ⚠️ VM execution of secret operations requires MPC engine configuration
   - MPC engine needs network setup (QUIC-based communication between parties)
   - Quick start examples show successful compilation but note execution requires networking
   - Full MPC execution demonstrated in `stoffel-run` binary (see StoffelVM repo)
   - VM must be configured with `HoneyBadgerMpcEngine` for secret-shared operations

3. **MPC Network Integration**:
   - The MPC client/server/node wrappers are complete with builder APIs
   - Basic networking available via `stoffelmpc-network` (default)
   - Modern QUIC networking available via `stoffelnet` (enable with `networking` feature)
   - Connecting VM to MPC network servers requires `QuicNetworkManager` setup

4. **Client Input Access in Stoffel Programs**:
   - MPC clients can be created with inputs via the builder API
   - However, Stoffel programs cannot yet access these client inputs directly
   - Programs currently use hardcoded secret values
   - Tracked in [Linear issue STO-104](https://linear.app/stoffel-labs/issue/STO-104)
   - Future: Programs will access client inputs via special syntax like `client_input(client_id, index)`

5. **MPC Preprocessing**:
   - Preprocessing material generation (beaver triples, random shares) is managed internally
   - Requires careful parameter tuning for production use
   - Currently requires a coordinator service to orchestrate preprocessing across servers (tracked in Linear issue STO-245)
   - Message processors are implemented and route protocol messages correctly
   - Full preprocessing will work once coordinator service is added

6. **Object/Array Conversion**: Complex type conversions between SDK and VM (objects, arrays) are not yet fully implemented.

7. **FFI Registration**: Custom Rust function registration via FFI is not yet fully implemented.

## Contributing

This is an internal Stoffel Labs SDK. For issues and feature requests, please use the Linear board.

### Known Issues

Issues and feature development are tracked in Linear:

**Feature Development:**
- [STO-104](https://linear.app/stoffel-labs/issue/STO-104) - Client input access in Stoffel programs
- [STO-245](https://linear.app/stoffel-labs/issue/STO-245) - Coordinator service for MPC preprocessing orchestration

**Compiler Warnings:**
- [STO-228](https://linear.app/stoffel-labs/issue/STO-228) - Unused imports
- [STO-229](https://linear.app/stoffel-labs/issue/STO-229) - Unused stub parameters
- [STO-230](https://linear.app/stoffel-labs/issue/STO-230) - Dead code in stubs

## License

Apache-2.0

## Links

- [Stoffel-Lang Repository](https://github.com/Stoffel-Labs/Stoffel-Lang)
- [StoffelVM Repository](https://github.com/Stoffel-Labs/StoffelVM)
- [MPC Protocols Repository](https://github.com/Stoffel-Labs/mpc-protocols)
- [Stoffel Networking Repository](https://github.com/Stoffel-Labs/stoffel-networking)
- [Linear Project Board](https://linear.app/stoffel-labs)

---

For detailed development guidance, see [CLAUDE.md](CLAUDE.md).
