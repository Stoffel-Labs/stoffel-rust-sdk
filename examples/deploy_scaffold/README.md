# Example 4: Deploy a Private Voting System

Compile a voting program and scaffold production deployment artifacts — Docker configs, server binaries, and coordinator setup.

## What This Shows

- **Client input injection** via `with_client_inputs()` — secret-share and inject votes into each party's VM
- **Full MPC execution** of the voting protocol with injected client inputs on localhost
- **`StoffelNetwork::scaffold()`** generates a complete deployment directory
- **Generated artifacts**: `docker-compose.yml`, Dockerfiles, TOML configs, Rust source for coordinator/server/client

## Key Concepts

**`ClientStore.take_share(client_id, share_idx)`** — In production, clients submit secret-shared inputs through the coordinator's input masking protocol. `take_share()` reads a client's secret share during MPC computation. No party (including the coordinator) ever sees the plaintext vote.

**What `scaffold()` generates:**
```
deployment/
  docker-compose.yml         # Orchestrates all containers
  Dockerfile.{coordinator,server,client}
  config/
    coordinator.toml         # Coordinator bind address, party count
    server.toml              # PARTY_ID from env, preprocessing config
    client.toml              # Coordinator address, program path
  coordinator/src/main.rs    # Coordinator binary
  server/src/main.rs         # Server binary (one per party)
  client/src/main.rs         # Client binary
  program.stfb               # Compiled bytecode
```

**Deployment:**
```bash
cd deployment/
docker-compose up
# Coordinator starts, N servers connect, clients submit votes
# Only the aggregate tally is revealed
```

## Run

```bash
cargo run --example deploy_scaffold
```

## Files

- `voting.stfl` — Private vote tally with `ClientStore.take_share()`
- `voting.stfb` — Pre-compiled bytecode
