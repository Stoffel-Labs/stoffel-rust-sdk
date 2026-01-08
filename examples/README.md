# Stoffel SDK Examples

## Overview

The Stoffel Rust SDK provides a production-ready **MPCaaS (MPC as a Service)** architecture:
- **StoffelClient** - For app developers connecting to an MPC network
- **StoffelServer** - For infrastructure operators running MPC compute nodes

## Available Examples

### honeybadger_mpc_demo.rs

Full end-to-end MPC demonstration:

```bash
cargo run --example honeybadger_mpc_demo
```

This example shows:
- Creating and starting multiple MPC servers
- Client connecting to the server network
- Secure computation with secret inputs
- Output reconstruction and verification
- Complete StoffelServer and StoffelClient API usage

## Quick Start

### As an App Developer (Client)

```rust
use stoffel_rust_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Connect to an existing MPC network
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

### As an Infrastructure Operator (Server)

```rust
use stoffel_rust_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Compile the program
    let program = Stoffel::compile("main main() -> secret int64:\n  return 42")?
        .build()?;

    // Create and start server
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

## MPC Configuration

The SDK uses the HoneyBadger protocol with TripleGen preprocessing which requires:
- **Minimum parties**: 5 (for threshold 1)
- **Constraint**: n >= 4t + 1 (TripleGen requires more parties than basic Byzantine n >= 3t + 1)

Common configurations:
- 5 parties, threshold 1: Local testing (minimum)
- 9 parties, threshold 2: Higher fault tolerance (9 >= 4*2 + 1)

## Running Examples

1. **Initialize submodules** (required):
   ```bash
   git submodule update --init --recursive
   ```

2. **Build examples**:
   ```bash
   cargo build --examples
   ```

3. **Run a specific example**:
   ```bash
   cargo run --example <example_name>
   ```

## See Also

- [Main README](../README.md) - SDK overview
- [CLAUDE.md](../CLAUDE.md) - Development guide
