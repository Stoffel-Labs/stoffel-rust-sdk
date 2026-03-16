//! Example 2: Secret Multiply and Reveal
//!
//! Multiplication on secret types triggers MPC protocol rounds
//! (Beaver triples), unlike addition which is a local share operation.
//! This example shows compilation with different MPC configurations.
//!
//! Run: cargo run --example secret_arithmetic

use stoffel_rust_sdk::prelude::*;

fn main() -> Result<()> {
    let source = include_str!("secret_arithmetic/secret_mul.stfl");

    // --- Compile and inspect MPC config ---
    println!("=== Compiling secret arithmetic program ===");
    let runtime = Stoffel::compile(source)?
        .parties(5)
        .threshold(1)
        .build()?;

    let functions = runtime.program().list_functions()?;
    println!("Functions:");
    for f in &functions {
        println!("  {} ({} params)", f.name, f.parameter_count);
    }

    let mpc = runtime.mpc_config().unwrap();
    println!("\nMPC config: {} parties, threshold {}", mpc.parties, mpc.threshold);
    // HoneyBadger requires: n >= 3t + 1
    // 5 >= 3*1 + 1 = 4 ✓
    println!("Constraint check: {} >= 3*{} + 1 = {} ✓",
        mpc.parties, mpc.threshold, 3 * mpc.threshold + 1);

    // --- Show a higher-tolerance configuration ---
    println!("\n=== Higher tolerance config (7 parties, threshold 2) ===");
    let runtime_ht = Stoffel::compile(source)?
        .parties(7)
        .threshold(2)
        .build()?;

    let mpc_ht = runtime_ht.mpc_config().unwrap();
    println!("MPC config: {} parties, threshold {}", mpc_ht.parties, mpc_ht.threshold);
    // 7 >= 3*2 + 1 = 7 ✓
    println!("Constraint check: {} >= 3*{} + 1 = {} ✓",
        mpc_ht.parties, mpc_ht.threshold, 3 * mpc_ht.threshold + 1);

    // --- Show invalid config is rejected ---
    println!("\n=== Invalid config (3 parties, threshold 1) ===");
    let invalid = Stoffel::compile(source)?
        .parties(3)
        .threshold(1)
        .build();

    match invalid {
        Err(e) => println!("Rejected: {}", e),
        Ok(_) => println!("(unexpectedly accepted)"),
    }

    // --- Bytecode comparison ---
    println!("\n=== Bytecode size ===");
    println!("Source compiled: {} bytes", runtime.program().bytecode().len());
    println!("Pre-compiled:   {} bytes", include_bytes!("secret_arithmetic/secret_mul.stfb").len());

    println!("\nMultiplication on secrets uses Beaver triples for secure computation.");
    println!("Addition on secrets is free (local share operation).");
    Ok(())
}
