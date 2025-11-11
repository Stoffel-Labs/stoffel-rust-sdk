# Stoffel SDK Examples

This directory contains examples demonstrating the Stoffel SDK.

## ⭐ **START HERE: Working MPC Example**

### `quick_start_local_network_real.rs` - **Complete Working Example**

**Run:** `cargo run --example quick_start_local_network_real --features mpc-local`

**This is the ONLY fully functional example that actually executes MPC protocols.**

It demonstrates:
- ✅ **Real QUIC networking** with listeners and connections
- ✅ **Actual MPC execution** (all 4 phases)
- ✅ **Message handling** with async tasks
- ✅ **HoneyBadger protocol** in action
- ✅ **Complete infrastructure** setup

**What it does:**
1. Creates 5 HoneyBadger QUIC servers
2. Sets up network listeners and connections
3. Runs preprocessing (Beaver triple generation)
4. Distributes client inputs via secret sharing
5. Executes secure multiplication: 10 × 20
6. Reconstructs and displays the result

**When to use:** Study this example to understand what's required for production MPC deployments.

---

## 📚 SDK API Examples

These examples demonstrate the SDK's high-level API but DO NOT execute the full MPC protocol.

### 1. `stoffel_sdk_demo.rs` - Comprehensive API Tour

**Run:** `cargo run --example stoffel_sdk_demo`

A complete tour of the SDK's capabilities in 6 parts:
1. Program compilation
2. Local VM execution
3. MPC configuration (parties, threshold)
4. Creating all 3 participant types (Server, Client, Node)
5. Secret sharing and distribution
6. Protocol configuration

**Best for:** Learning the complete SDK API systematically.

---

### 2. `quick_start_network.rs` - Quick API Demo

**Run:** `cargo run --example quick_start_network --features mpc-local`

The simplest introduction to the SDK's MPC API.

Shows:
- Compiling Stoffel programs
- Creating MPC runtime with configuration
- Building nodes with the fluent API
- Automatic network manager creation

**Best for:** First-time users wanting a quick overview.

---

### 3. `sdk_api_demo.rs` - Execution Attempt

**Run:** `cargo run --example sdk_api_demo --features mpc-local`

Demonstrates the SDK API and attempts to call `node.run()`.

**Important:** This example will likely fail because it creates network
managers but doesn't set up the required QUIC infrastructure (listeners,
connections, message handlers).

**Purpose:** Shows what the SDK provides and what YOU need to add for
production deployments.

---

## 🔑 Key Understanding

### The SDK Provides:

**High-Level API:**
- ✅ `MPCNode`, `MPCServer`, `MPCClient` types
- ✅ Builder patterns (`runtime.node()`, `runtime.server()`, etc.)
- ✅ Automatic `QuicNetworkManager` creation
- ✅ `node.run()` method for protocol execution

**Network Infrastructure Helpers** (via `network_helpers` module):
- ✅ `setup_honeybadger_quic_network()` - Complete network setup in one call
- ✅ `setup_honeybadger_quic_clients()` - Client setup with connections
- ✅ `HoneyBadgerQuicServer` - Servers with QUIC listeners
- ✅ `HoneyBadgerQuicClient` - Clients with connection management
- ✅ Automatic message handler spawning
- ✅ Full network topology management

**Example Usage:**
```rust
use stoffel_rust_sdk::prelude::*;
use ark_bls12_381::Fr;

// One function call sets up entire network!
let (servers, receivers) = setup_honeybadger_quic_network::<Fr>(
    5, 1, 3, 8, 42, 19200,
    HoneyBadgerQuicConfig::default(),
).await?;
```

See `quick_start_local_network_real.rs` for complete example.

### Example Comparison

| Example | API Demo | Actual Execution | Complexity | Run Time |
|---------|----------|------------------|------------|----------|
| `stoffel_sdk_demo.rs` | ✅ | ❌ | ⭐ Beginner | ~1 sec |
| `quick_start_network.rs` | ✅ | ❌ | ⭐ Beginner | ~1 sec |
| `sdk_api_demo.rs` | ✅ | ⚠️ Attempts | ⭐⭐ Intermediate | ~2 sec |
| **`quick_start_local_network_real.rs`** | ✅ | **✅** | **⭐⭐⭐ Advanced** | **~5 sec** |

---

## 🚀 Learning Path

**Recommended order:**

1. **`stoffel_sdk_demo.rs`** → Learn the SDK API
   ```bash
   cargo run --example stoffel_sdk_demo
   ```

2. **`quick_start_network.rs`** → See automatic networking API
   ```bash
   cargo run --example quick_start_network --features mpc-local
   ```

3. **`quick_start_local_network_real.rs`** → Study the working implementation
   ```bash
   cargo run --example quick_start_local_network_real --features mpc-local
   ```

---

## 🏗️ Building and Running

### Basic Examples (No Network)
```bash
cargo run --example stoffel_sdk_demo
```

### Network Examples (Requires `mpc-local` Feature)
```bash
cargo run --example quick_start_network --features mpc-local
cargo run --example quick_start_local_network_real --features mpc-local
cargo run --example sdk_api_demo --features mpc-local
```

### Build All Examples
```bash
cargo build --examples --features mpc-local
```

---

## 📖 Understanding MPC Execution

### Complete MPC Workflow (from `quick_start_local_network_real.rs`)

1. **Network Setup**
   - Create QUIC network managers
   - Bind listeners to ports
   - Spawn accept() loops
   - Establish peer connections

2. **Preprocessing Phase**
   - Generate Beaver triples
   - Create random shares
   - Distribute preprocessing material

3. **Input Sharing Phase**
   - Client secret-shares inputs
   - Distribute shares to servers
   - Servers store received shares

4. **Computation Phase**
   - Execute Stoffel program on shares
   - Use Beaver triples for multiplication
   - Maintain security throughout

5. **Output Reconstruction**
   - Collect output shares
   - Use robust reconstruction
   - Return final result to client

---

## ❓ FAQ

### Q: Why don't the SDK API examples run actual MPC?

**A:** The API examples (`stoffel_sdk_demo.rs`, `quick_start_network.rs`) are designed
to teach the SDK's API surface without requiring complex network setup. They show you
how to use the builder patterns and create MPC participants.

For actual execution, use the `network_helpers` module with `setup_honeybadger_quic_network()`.

### Q: How do I build a production MPC application?

**A:** Use the SDK's `network_helpers` module! It provides everything you need:

```rust
use stoffel_rust_sdk::prelude::*;
use ark_bls12_381::Fr;

// Setup complete network infrastructure:
let (servers, receivers) = setup_honeybadger_quic_network::<Fr>(
    n_parties, threshold, n_triples, n_random_shares,
    instance_id, base_port, config,
).await?;

// Start servers and connect them
for server in &mut servers {
    server.start().await?;
}
for server in &servers {
    server.connect_to_peers().await?;
}

// Now run your MPC protocol!
```

See `quick_start_local_network_real.rs` for complete working example.

### Q: Do I need to manually set up QUIC listeners and connections?

**A:** No! Use `setup_honeybadger_quic_network()` from the `network_helpers` module.
It handles all the low-level networking automatically:
- QUIC listener binding
- Connection establishment
- Message handler spawning
- Network topology setup

Advanced users can access the network manager via `node.network_mut()` for custom configuration.

---

## 🔍 See Also

- [Main README](../README.md) - SDK overview and installation
- [CLAUDE.md](../CLAUDE.md) - Development notes
- [Stoffel Language](https://github.com/your-org/stoffel-lang) - Language reference

---

## 🤝 Contributing

When adding new examples:
1. Be clear about whether it's an API demo or working execution
2. Add descriptive `//!` doc comments at the top
3. Include "Run with: ..." instruction
4. Update this README
5. Test with `cargo run --example <name>`

---

Made with ❤️ using the Stoffel SDK
