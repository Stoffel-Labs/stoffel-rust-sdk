# Stoffel Rust SDK Examples

A learning path from "hello world" to "deploy to production". Each example builds on the previous one, introducing new SDK concepts and MPC patterns.

## Prerequisites

```bash
git submodule update --init --recursive
cargo build
```

## Examples

| # | Example | Concept | Command |
|---|---------|---------|---------|
| 1 | [`hello_stoffel`](hello_stoffel/) | Compile, inspect, and load a program with secret types | `cargo run --example hello_stoffel` |
| 2 | [`secret_arithmetic`](secret_arithmetic/) | MPC config validation, secret multiply vs. add costs | `cargo run --example secret_arithmetic` |
| 3 | [`network_builder`](network_builder/) | `StoffelNetwork::builder()` for fine-grained control | `cargo run --example network_builder` |
| 4 | [`deploy_scaffold`](deploy_scaffold/) | Generate Docker deployment artifacts | `cargo run --example deploy_scaffold` |

## Learning Path

**Example 1** shows the basics: compile StoffelLang source, inspect the resulting functions, and load pre-compiled bytecode. **Example 2** dives into MPC configuration — how parties and thresholds work, and what happens when you violate the `n >= 3t + 1` constraint. **Example 3** introduces `StoffelNetwork::builder()` for full control over preprocessing, inputs, and network topology. **Example 4** takes a program to production by scaffolding a complete Docker deployment.

## Utility

| Tool | Command |
|------|---------|
| Regenerate `.stfb` files | `cargo run --example generate_stfb` |
