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

### API Architecture: Progressive Disclosure

The SDK follows a **three-level architecture** to balance simplicity and power:

#### Level 1: Simple API (`src/prelude.rs`)
**For most users - building MPC applications easily**

```rust
use stoffel_rust_sdk::prelude::*;

// Compile with MPC configuration
let runtime = Stoffel::compile(source)?
    .parties(5)
    .threshold(1)
    .build()?;

// Create participants
let client = runtime.client(100).with_inputs(vec![10, 20]).build()?;
let server = runtime.server(0).build()?;
```

**Re-exports:**
- `Stoffel`, `StoffelRuntime` - Main API
- `MPCClient`, `MPCServer`, `MPCNode` - Participants
- `Program`, `VM`, `Compiler` - Core types
- `Error`, `Result` - Error handling

**Example:** `examples/simple_mpc.rs`

#### Level 2: Advanced Abstractions (`src/advanced.rs`)
**For custom MPC applications requiring fine-grained control**

```rust
use stoffel_rust_sdk::advanced::*;

// Store and manage secret shares
ShareManager::store_shares(100, &[10, 20], 5, 1)?;

// Build network configuration
let config = NetworkBuilder::new(5, 1)
    .base_port(19200)
    .instance_id(42)
    .build();
```

**Provides:**
- `ShareManager` - Clean abstraction over ClientInputStore
- `NetworkBuilder` - Clean abstraction for network configuration
- `NetworkConfig` - Configuration validation

**IMPORTANT:** No raw internal types exposed. All advanced functionality uses proper abstractions.

**Example:** `examples/advanced_shares.rs`

#### Level 3: Production Infrastructure (`src/network_helpers.rs`)
**For production deployments with real networking** (requires `mpc-local` feature)

```rust
use stoffel_rust_sdk::prelude::*;

// One-call network setup
let (servers, receivers) = setup_honeybadger_quic_network::<Fr>(
    5, 1, 3, 8, 42, 19200,
    HoneyBadgerQuicConfig::default(),
).await?;
```

**Provides:**
- `setup_honeybadger_quic_network()` - Complete network in one call
- `HoneyBadgerQuicServer`, `HoneyBadgerQuicClient` - Production wrappers
- QUIC listeners, connections, message handlers

**Example:** `examples/quick_start_local_network_real.rs`

### Module Structure

```
src/
├── lib.rs              # Stoffel builder - main entry point
├── prelude.rs          # ⭐ Simple API (Level 1)
├── advanced.rs         # ⭐⭐ Advanced abstractions (Level 2)
├── network_helpers.rs  # ⭐⭐⭐ Production infrastructure (Level 3)
│
├── program.rs          # Compiled program with MPC config
├── compiler.rs         # Stoffel-Lang compiler wrapper
├── vm.rs               # StoffelVM execution wrapper
├── client.rs           # MPCClient implementation
├── server.rs           # MPCServer implementation
├── session.rs          # MPCNode implementation
├── network_config.rs   # Network configuration types
├── secret_sharing.rs   # Secret sharing utilities
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

3. **MPCNode** (`src/session.rs`)
   - For full participants in collaborative MPC scenarios
   - Combines client and server functionality
   - Provides inputs AND participates in computation
   - Also uses HoneyBadger protocol
   - Should be used explicitly when this behavior is needed

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
