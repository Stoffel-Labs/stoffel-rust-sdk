//! Example 3: Private Salary Calculation with StoffelNetwork
//!
//! Uses StoffelNetwork::builder() for fine-grained control over the MPC
//! network configuration. Shows the MPC-safe pattern: branch on public
//! values, modify secret values. Runs the full MPC protocol on localhost.
//!
//! Run: cargo run --example network_builder

use stoffel_rust_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let source = include_str!("network_builder/salary.stfl");

    // --- Explicit compilation step ---
    println!("=== Compiling with Compiler ===");
    let bytecode = Compiler::new().compile_source(source)?;
    println!("Compiled to {} bytes of bytecode", bytecode.len());

    // --- Build network with StoffelNetwork::builder() ---
    println!("\n=== Building MPC network ===");
    let network = StoffelNetwork::builder()
        .program(bytecode.clone())
        .parties(5)
        .threshold(1)
        .with_preprocessing(2000, 1000) // more triples for multiplication-heavy programs
        .build()?;

    // Inspect the network config
    let config = network.config();
    println!("Network: {} parties, threshold {}",
        config.network.parties, config.network.threshold);
    println!("Servers configured: {}", config.server.len());
    println!("Bytecode size: {} bytes", network.bytecode().len());

    // --- Also show loading from pre-compiled .stfb ---
    println!("\n=== Loading from pre-compiled bytecode ===");
    let stfb_bytes = include_bytes!("network_builder/salary.stfb");
    let network2 = StoffelNetwork::builder()
        .program(stfb_bytes.to_vec())
        .parties(5)
        .threshold(1)
        .build()?;

    println!("Loaded network with {} bytes of bytecode", network2.bytecode().len());

    // --- Run full MPC on localhost via StoffelNetwork ---
    println!("\n=== Running full MPC on localhost ===");
    let results = StoffelNetwork::builder()
        .program(bytecode.clone())
        .parties(5)
        .threshold(1)
        .with_preprocessing(2000, 1000)
        .build()?
        .execute_local()
        .await?;
    println!("MPC result: {:?}", results);
    // Expected: 5000 + 1000 (senior) + 500 (top performer) = 6500

    println!("\nStoffelNetwork::builder() gives you fine-grained control:");
    println!("  - Custom preprocessing (triples, random shares)");
    println!("  - Load from source or pre-compiled bytecode");
    println!("  - Full MPC execution on localhost with execute_local()");
    println!("  - .scaffold() to generate Docker deployment (see deploy_scaffold example)");
    Ok(())
}
