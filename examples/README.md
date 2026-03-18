# Stoffel Rust SDK Examples

A learning path from "hello world" to "deploy to production". Each example builds on the previous one, introducing new SDK concepts and MPC patterns. Every example runs the **full HoneyBadger MPC protocol** on localhost — no simulations or placeholders.

## Prerequisites

```bash
cargo build
```

## Examples

| # | Example | Concept | Command |
|---|---------|---------|---------|
| 1 | [`hello_stoffel`](hello_stoffel/) | Compile, inspect, load bytecode, and run MPC on localhost | `cargo run --example hello_stoffel` |
| 2 | [`secret_arithmetic`](secret_arithmetic/) | MPC config validation, Beaver triple multiplication | `cargo run --example secret_arithmetic` |
| 3 | [`network_builder`](network_builder/) | `StoffelNetwork::builder()` with end-to-end MPC execution | `cargo run --example network_builder` |
| 4 | [`deploy_scaffold`](deploy_scaffold/) | Client input injection + Docker deployment scaffolding | `cargo run --example deploy_scaffold` |

## Learning Path

**Example 1** shows the basics: compile StoffelLang source, inspect the resulting functions, load pre-compiled bytecode, and run the full MPC protocol with `execute_local()`. **Example 2** dives into MPC configuration and runs a secret multiplication that triggers the real Beaver triple protocol across 5 parties. **Example 3** introduces `StoffelNetwork::builder()` with custom preprocessing and end-to-end MPC execution. **Example 4** demonstrates client input injection for a private voting system, then scaffolds a complete Docker deployment.

## Utility

| Tool | Command |
|------|---------|
| Regenerate `.stfb` files | `cargo run --example generate_stfb` |
