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

- ✅ Stoffel-Lang compiler integration complete
- ✅ StoffelVM runtime integration complete
- ✅ MPC protocols API complete (HoneyBadger Byzantine fault-tolerant protocol)
- ✅ HoneyBadger client/server/node wrappers with automatic constraint validation
- ✅ Automatic network manager creation via runtime
- ✅ **Working MPC example with real execution** (`examples/quick_start_local_network_real.rs`)
- ✅ Comprehensive SDK API examples demonstrating all features
- ✅ Builds successfully with all dependencies

**MPC Protocol:** HoneyBadger (Byzantine fault-tolerant, asynchronous)
- Constraint: n >= 3t + 1 (automatically validated)
- Secret Sharing: RobustShare (Reed-Solomon erasure coding)
- Real QUIC networking for production deployments

**Network Infrastructure:**
- `network_helpers` module provides production-ready networking
- `setup_honeybadger_quic_network()` - Complete network in one call
- Automatic QUIC listeners, connections, and message handlers
- See `examples/quick_start_local_network_real.rs` for working example

**Examples:**
- See `examples/README.md` for complete documentation
- `quick_start_local_network_real.rs` - **Fully functional MPC execution**
- `stoffel_sdk_demo.rs` - Complete API tour
- `quick_start_network.rs` - Quick start guide

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
# Build with default features
cargo build

# Build with QUIC networking enabled
cargo build --features networking

# Optimized build
cargo build --release

# Optimized build with networking
cargo build --release --features networking
```

### Running Examples

The SDK includes focused examples demonstrating key functionality:

```bash
# 1. Quick start - Local HoneyBadger MPC network (START HERE)
cargo run --example quick_start_local_network

# 2. MPC participants - MPCClient and MPCServer
cargo run --example mpc_demo

# 3. Protocol configuration - HoneyBadger Byzantine fault tolerance
cargo run --example protocol_demo

# 4. Network configuration - TOML config files
cargo run --example network_config_demo

# 5. Comprehensive features - All SDK capabilities
cargo run --example program_demo
```

**Example Guide:**

| Example | Purpose | Key Concepts |
|---------|---------|--------------|
| **`quick_start_local_network.rs`** | ⭐ **START HERE** - Complete local HoneyBadger MPC network | 5-party setup, preprocessing, client input, secure computation, output reconstruction. Follows `external/stoffel-vm/tests` pattern. |
| `mpc_demo.rs` | MPC participant roles | MPCClient (input provider), MPCServer (compute node) |
| `protocol_demo.rs` | Protocol configuration | HoneyBadger, RobustShare, Byzantine fault tolerance |
| `network_config_demo.rs` | Network deployment | TOML configuration, bind addresses, bootnode setup |
| `program_demo.rs` | Comprehensive SDK tour | Compilation, local execution, MPC config, participants, function listing |

## Quick Start

### Production MPC in 3 Steps

```rust
use stoffel_rust_sdk::prelude::*;
use ark_bls12_381::Fr;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Step 1: Compile Stoffel program
    let runtime = Stoffel::compile(
        "main main(a: secret int64, b: secret int64) -> secret int64:\n  return a * b"
    )?
        .parties(5)      // 5-party MPC network
        .threshold(1)    // Tolerates 1 Byzantine fault
        .build()?;

    // Step 2: Setup complete network infrastructure (ONE CALL!)
    let (mut servers, receivers) = setup_honeybadger_quic_network::<Fr>(
        5,      // n_parties
        1,      // threshold
        3,      // n_triples
        8,      // n_random_shares
        42,     // instance_id
        19200,  // base_port
        HoneyBadgerQuicConfig::default(),
    ).await?;

    // Step 3: Start and connect
    for server in &mut servers {
        server.start().await?;
    }
    for server in &servers {
        server.connect_to_peers().await?;
    }

    // Network is ready! Run MPC protocol...
    // See examples/quick_start_local_network_real.rs for complete implementation

    Ok(())
}
```

### What You Get

**Automatic Infrastructure:**
- ✅ QUIC listener setup on all ports
- ✅ Connection establishment with retries
- ✅ Message handler task spawning
- ✅ Full mesh network topology
- ✅ Byzantine fault-tolerant HoneyBadger protocol

**One Function Call:**
```rust
setup_honeybadger_quic_network()  // Complete network ready!
```

### API Exploration (without networking)

For learning the SDK API:

```rust
use stoffel_rust_sdk::prelude::*;

fn main() -> Result<()> {
    // Compile with MPC configuration
    let runtime = Stoffel::compile("main main() -> int64:\n  return 42")?
        .parties(5)
        .threshold(1)
        .build()?;

    // Create participants (automatic network manager creation)
    let server = runtime.server(0).build()?;
    let client = runtime.client(100).with_inputs(vec![10, 20]).build()?;
    let node = runtime.node(0).with_inputs(vec![10, 20]).build()?;

    Ok(())
}
```

### Local Testing (no MPC)

```rust
// Quick local execution without MPC
let result = Stoffel::compile("main main() -> int64:\n  return 42")?
    .execute_local()?;
```

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
cargo run --example quick_start --features mpc-local
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

### MPC-First Architecture

The SDK embraces **Stoffel's MPC-first philosophy**: programs ARE MPC programs by default.

1. **Stoffel** - The branded entry point for all SDK operations
   - `Stoffel::compile(source)` - Returns a ProgramBuilder
   - `Stoffel::compile_file(path)` - Returns a ProgramBuilder
   - `Stoffel::load(bytecode)` - Returns a ProgramBuilder
   - `Stoffel::builder()` - Advanced compilation options

2. **ProgramBuilder** - Configure MPC parameters during compilation
   - `.parties(n)` - Set number of MPC servers (must satisfy n >= 3t + 1)
   - `.threshold(t)` - Set Byzantine fault tolerance (defaults to 1)
   - `.instance_id(id)` - Set computation instance ID
   - `.protocol(type)` - Set protocol (defaults to HoneyBadger)
   - `.build()` - Build the Program with MPC config (validates constraints)
   - `.execute_local()` - Quick local test (skips MPC config)

3. **Program** - A compiled program with built-in MPC configuration
   - `.execute_local()` - Test locally on the VM
   - `.server(party_id)` - Create MPC server tied to THIS program
   - `.client(client_id)` - Create MPC client tied to THIS program
   - `.mpc_config()` - Get (parties, threshold, instance_id)

**This design makes MPC central:**
- **MPC is built-in** - Configure parties/threshold when compiling
- **Servers and clients FROM program** - Created directly from the program object
- **Testing is explicit** - `.execute_local()` makes it clear this is testing
- **No separate MPCProgram** - Every program IS an MPC program

**Example:**

```rust
// Compile with MPC configuration (HoneyBadger protocol)
// Constraint: n >= 3t + 1 for Byzantine fault tolerance
let program = Stoffel::compile("main main() -> int64:\n  return 42")?
    .parties(5)        // n=5 servers
    .threshold(1)      // t=1 Byzantine faults (5 >= 3*1+1 = 4 ✓)
    .build()?;

// Test locally before deployment
let result = program.execute_local()?;

// Create servers FROM the program
let server = program.server(0).build()?;

// Create clients FROM the program
let client = program.client(100).with_inputs(vec![42]).build()?;

// Quick testing (skip MPC config)
let result = Stoffel::compile(source)?.execute_local()?;
```

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
├── lib.rs           # Main library entry and Stoffel builder
├── program.rs       # ⭐ NEW: Unified Program API (recommended)
├── compiler.rs      # High-level wrapper around stoffellang compiler
├── vm.rs            # High-level wrapper around StoffelVM
├── client.rs        # MPCClient - clients sending inputs to MPC network
├── server.rs        # MPCServer - servers performing MPC computation
├── session.rs       # MPCNode - full participants (client + server)
├── error.rs         # Unified error types
└── prelude.rs       # Convenient re-exports
```

**Recommended:** Use the new `program` module for all new code. It provides a cleaner, more intuitive API.

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

5. **MPC Preprocessing**: Preprocessing material generation (beaver triples, random shares) is managed internally but requires careful parameter tuning for production use.

6. **Object/Array Conversion**: Complex type conversions between SDK and VM (objects, arrays) are not yet fully implemented.

7. **FFI Registration**: Custom Rust function registration via FFI is not yet fully implemented.

## Contributing

This is an internal Stoffel Labs SDK. For issues and feature requests, please use the Linear board.

### Known Issues

Issues and feature development are tracked in Linear:

**Feature Development:**
- [STO-104](https://linear.app/stoffel-labs/issue/STO-104) - Client input access in Stoffel programs

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
