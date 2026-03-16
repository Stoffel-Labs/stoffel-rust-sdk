//! Example 1: Your First Secret Computation
//!
//! Compile a StoffelLang program, inspect it, and load pre-compiled bytecode.
//! Shows the full compile-inspect-configure workflow.
//!
//! Run: cargo run --example hello_stoffel

use stoffel_rust_sdk::prelude::*;

fn main() -> Result<()> {
    let source = include_str!("hello_stoffel/hello.stfl");

    // --- Compile from source and inspect ---
    println!("=== Compiling from source ===");
    let runtime = Stoffel::compile(source)?
        .parties(5)
        .threshold(1)
        .build()?;

    // Inspect functions in the compiled program
    let functions = runtime.program().list_functions()?;
    println!("Functions found:");
    for f in &functions {
        println!("  {} ({} params, {} registers)", f.name, f.parameter_count, f.register_count);
    }

    // Query MPC configuration
    let mpc = runtime.mpc_config().unwrap();
    println!("\nMPC config: {} parties, threshold {}", mpc.parties, mpc.threshold);

    // --- Load pre-compiled bytecode ---
    println!("\n=== Loading pre-compiled bytecode ===");
    let bytecode = include_bytes!("hello_stoffel/hello.stfb");
    let runtime2 = Stoffel::load(bytecode)
        .parties(5)
        .threshold(1)
        .build()?;

    let functions2 = runtime2.program().list_functions()?;
    println!("Functions from bytecode: {}", functions2.len());

    // --- Save bytecode to disk ---
    println!("\n=== Saving bytecode ===");
    let out_path = std::env::temp_dir().join("hello.stfb");
    runtime.program().save(out_path.to_str().unwrap())?;
    println!("Saved to {}", out_path.display());
    let _ = std::fs::remove_file(&out_path);

    println!("\nDone! The program compiles two secret inputs added together.");
    println!("In a real deployment, execute_local() runs the full MPC protocol");
    println!("so no party ever sees the individual secret values.");
    Ok(())
}
