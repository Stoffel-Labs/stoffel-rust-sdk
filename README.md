# Stoffel Rust SDK

The Rust SDK for building Multi-Party Computation (MPC) applications with [Stoffel](https://stoffel.ai).

Stoffel lets you write programs in [StoffelLang](https://github.com/Stoffel-Labs/Stoffel-Lang), compile them to bytecode, and execute them across a distributed MPC network where no single party ever sees the plaintext data.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
stoffel-rust-sdk = { git = "https://github.com/Stoffel-Labs/stoffel-rust-sdk.git", branch = "feature/sdk-v0.1.0-reconcile" }
```

If the repo is private, add to `~/.cargo/config.toml`:

```toml
[net]
git-fetch-with-cli = true
```

**Requirements:** Rust 1.75+

## Quick Start

```rust
use stoffel_rust_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Compile a Stoffel program
    let results = Stoffel::compile("
        main main(a: secret int64, b: secret int64) -> secret int64:
            return a + b
    ")?
    // 2. Configure MPC parameters
    .parties(5)
    .threshold(1)
    .with_inputs(&[("a", 42i64), ("b", 58i64)])
    // 3. Run it
    .execute_local()
    .await?;

    println!("Result: {:?}", results);
    Ok(())
}
```

`execute_local()` spins up a full MPC network on localhost (coordinator + 5 server tasks) for development and testing. For production, deploy separate processes via the [Stoffel CLI](https://github.com/Stoffel-Labs/Stoffel).

## How It Works

```
SDK                          Coordinator                    StoffelVM
 compile() -> bytecode        receives program               MpcRunner executes bytecode
 client -> submit inputs      distributes to servers          HoneyBadger / AVSS engine
 server -> register           manages round state machine     returns output shares
```

The SDK compiles your program, the coordinator distributes it to the MPC network and orchestrates rounds, and the VM executes it using secret-shared arithmetic. No single party ever sees the plaintext inputs.

## Building Locally

```bash
git clone https://github.com/Stoffel-Labs/stoffel-rust-sdk.git
cd stoffel-rust-sdk

cargo build          # Build
cargo test           # Run tests (130 total)
cargo clippy         # Lint
cargo doc --open     # Generate and view API docs
```

## Contributing

We welcome contributions. To get started:

1. Fork the repo and create a feature branch
2. Make your changes with tests
3. Run `cargo fmt && cargo clippy && cargo test`
4. Open a pull request

Please follow the existing code style and keep PRs focused on a single change.

## Security

If you discover a security vulnerability, **do not open a public issue**. Instead, email [security@stoffel.ai](mailto:security@stoffel.ai) with details. We take all reports seriously and will respond promptly.

For the security implications of running `execute_local()` (all MPC parties in one process), see the [shared-memory security analysis](https://hackmd.io/@stoffel-labs/_6iDFAwMSOm-QDua12Wleg).

## License

Apache-2.0
