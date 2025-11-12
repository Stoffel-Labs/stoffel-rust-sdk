# Stoffel SDK Examples

## Current Status

The Stoffel Rust SDK provides:
- ✅ **Stoffel-Lang compilation** - Compile Stoffel programs to bytecode
- ✅ **VM execution** - Run bytecode on the Stoffel VM
- ✅ **Bytecode parsing and execution** - Servers can load and execute compiled bytecode
- ✅ **MPC types and configuration** - Configure MPC parameters (parties, threshold, protocols)
- ✅ **Client-Server Architecture** - Core SDK APIs (client.rs, server.rs, session.rs)

## Examples

### complete_mpc_workflow.rs

**⭐ START HERE** - Demonstrates a complete end-to-end MPC workflow:

```bash
cargo run --example complete_mpc_workflow
```

This example shows:
- ✅ Complete workflow from program compilation to secret share distribution
- ✅ Stoffel program compilation with MPC configuration
- ✅ MPC server creation and QUIC network setup
- ✅ Full mesh peer-to-peer connectivity
- ✅ Client creation with private inputs
- ✅ Secret share generation (robust shares with error correction)
- ✅ Secret share distribution across all servers

**What It Demonstrates:**
- Real end-to-end MPC setup using SDK APIs
- Security properties: No server learns private inputs
- Production-ready networking on localhost (127.0.0.1)
- Clear documentation of what works and what's in progress

### simple_mpc_network.rs

Demonstrates the SDK's network-based client-server architecture running on localhost:

```bash
cargo run --example simple_mpc_network
```

This example shows:
- ✅ Compiling Stoffel programs with the SDK
- ✅ Creating MPC servers with network connectivity
- ✅ Creating network-based MPC clients
- ✅ Establishing QUIC connections between clients and servers
- ✅ Distributing secret-shared inputs over the network

**Key Concepts:**
- **Real QUIC networking on localhost** - All parties run on 127.0.0.1 with different ports
- **Local share generation** - Clients generate secret shares of private inputs
- **Network distribution** - Shares are sent to servers over QUIC connections
- **Production-ready** - For distributed deployment, just change IPs from 127.0.0.1 to actual machine IPs

**Note:** This example demonstrates networking setup. Full MPC execution requires
additional message routing (see StoffelVM integration tests for complete workflow).

### bytecode_execution.rs

Demonstrates bytecode parsing and execution in MPC servers:

```bash
cargo run --example bytecode_execution
```

This example shows:
- ✅ Compiling Stoffel programs to bytecode
- ✅ Extracting bytecode from compiled programs
- ✅ Loading bytecode into server VMs
- ✅ Executing functions from loaded bytecode
- ✅ Integration of StoffelVM with MPCServer

**Key Concepts:**
- **Embedded VMs** - Each server has its own VirtualMachine instance
- **Bytecode loading** - Servers parse `.stfl` bytecode format
- **Function execution** - VMs execute named functions from bytecode
- **Transparent execution** - Same code works for clear and secret-shared values

### mpc_computation.rs

Shows the SDK's compilation and configuration API without networking:

```bash
cargo run --example mpc_computation
```

## For Complete MPC Network Execution

The SDK provides the core client-server architecture (client.rs, server.rs, session.rs) as the primary developer-facing APIs. However, full end-to-end MPC network execution with QUIC connections requires integration work that is currently demonstrated in StoffelVM's integration tests.

**StoffelVM Integration Tests** (Reference Implementation):
```
external/stoffel-vm/crates/stoffel-vm/src/tests/mpc_multiplication_integration.rs
```

**To run the complete MPC workflow:**
```bash
cd external/stoffel-vm
cargo test --package stoffel-vm --lib tests::mpc_multiplication_integration -- --nocapture --test-threads=1
```

This test demonstrates:
- ✅ QUIC network setup with `setup_honeybadger_quic_network()`
- ✅ Server startup and peer connections
- ✅ Preprocessing (Beaver triple generation)
- ✅ Input sharing from clients
- ✅ Secure multiplication
- ✅ Output reconstruction

**Note:** Future SDK versions will integrate this networking functionality directly into the MPCServer and MPCClient APIs for a more seamless developer experience.

## SDK Roadmap

### Currently Available

The SDK provides high-level APIs for:

```rust
use stoffel_rust_sdk::prelude::*;

// 1. Compile Stoffel programs
let runtime = Stoffel::compile(source)?
    .parties(5)
    .threshold(1)
    .build()?;

// 2. Test locally
let result = runtime.program().execute_local()?;

// 3. Configure MPC participants
let server = runtime.server(0).build()?;
let client = runtime.client(100).with_inputs(vec![10, 20]).build()?;
```

### Planned: Network Infrastructure

The SDK will provide its own MPC network infrastructure:

```rust
// Future SDK API (not yet implemented)
use stoffel_rust_sdk::mpc_network::*;

let network = MPCNetwork::builder()
    .parties(5)
    .threshold(1)
    .build()
    .await?;

network.start().await?;
let result = network.execute(program, inputs).await?;
```

This will wrap StoffelVM's networking components (`QuicNetworkManager`, `HoneyBadgerMpcEngine`) into a cohesive, easy-to-use API.

## For Application Developers

**Current best practice:**

1. Use the SDK for compilation and VM execution
2. Reference StoffelVM's integration tests for MPC networking patterns
3. Build your own network layer using StoffelVM's exported components:
   - `stoffel_vm::net::QuicNetworkManager`
   - `stoffel_vm::net::hb_engine::HoneyBadgerMpcEngine`
   - `stoffelmpc_mpc::honeybadger::HoneyBadgerMPCNode`

**Example structure:**
```rust
use stoffel_rust_sdk::prelude::*;
use stoffel_vm::net::{QuicNetworkManager, hb_engine::HoneyBadgerMpcEngine};

// Use SDK for compilation
let program = Stoffel::compile(source)?.build()?;

// Build your own network using StoffelVM components
let network = QuicNetworkManager::new();
// ... (see StoffelVM tests for complete setup)
```

## Contributing

The SDK is under active development. The main gap is **MPC network infrastructure**.

To contribute:
1. Study `external/stoffel-vm/crates/stoffel-vm/src/tests/mpc_multiplication_integration.rs`
2. Design a high-level API that wraps these components
3. Implement network setup helpers in `src/mpc_network.rs` (when created)
4. Add examples that use the SDK's infrastructure (not tests)

## See Also

- [Main README](../README.md) - SDK overview
- [CLAUDE.md](../CLAUDE.md) - Development guide
- [StoffelVM Repository](https://github.com/Stoffel-Labs/StoffelVM) - VM and networking implementation
- [StoffelVM Tests](../external/stoffel-vm/crates/stoffel-vm/src/tests/) - Reference MPC implementation

---

**Note:** This SDK is under active development. The MPC networking layer is the next major milestone.
