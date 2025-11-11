# Stoffel Network Configuration Examples

This directory contains example configuration files for deploying Stoffel MPC programs across a network.

## Configuration Format

Each party in the MPC network needs a configuration file that specifies:

### Network Settings
- `party_id`: Unique identifier for this party (0 to n_parties-1)
- `bind_address`: Address to bind for incoming connections
- `bootstrap_address`: Bootnode address for party discovery
- `min_parties`: Minimum parties required before starting

### MPC Settings
- `n_parties`: Total number of MPC parties
- `threshold`: Fault tolerance threshold (n >= 3t + 1)
- `instance_id`: Optional unique ID for this computation

## Example Files

- `party0.toml` - Configuration for party 0
- `party1.toml` - Configuration for party 1

## Usage

### Loading Configuration in Code

```rust
use stoffel_rust_sdk::prelude::*;

// Load from file
let program = Stoffel::compile(source)?
    .network_config_file("examples/configs/party0.toml")?
    .build()?;

// Create node from program
let node = program.node(0).build()?;
```

### Creating Configuration Programmatically

```rust
use stoffel_rust_sdk::prelude::*;

let config = NetworkConfigBuilder::new()
    .party_id(0)
    .bind_address("127.0.0.1:9001")
    .bootstrap_address("127.0.0.1:9000")
    .n_parties(5)
    .threshold(1)
    .build()?;

// Save for later use
config.save("my_party.toml")?;
```

## Deployment Workflow

1. **Start Bootnode**: Run a bootnode process on the `bootstrap_address`
   ```bash
   stoffel-run --bootnode --bind 127.0.0.1:9000
   ```

2. **Each Party**: Load their config and start
   ```bash
   # Party 0
   cargo run --example my_mpc_program -- --config party0.toml

   # Party 1
   cargo run --example my_mpc_program -- --config party1.toml
   ```

3. **Clients**: Connect and submit inputs
   ```rust
   let client = program.client(100)
       .with_inputs(vec![42, 10])
       .build()?;
   ```

## Configuration Validation

The SDK automatically validates:
- MPC parameters (n >= 3t + 1)
- Party ID is valid (< n_parties)
- Addresses are well-formed

Any validation errors will be reported when loading the config.
