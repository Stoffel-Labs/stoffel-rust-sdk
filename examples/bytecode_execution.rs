//! Bytecode Execution Example
//!
//! This example demonstrates how the Stoffel SDK can parse and execute
//! compiled Stoffel bytecode on MPC servers.
//!
//! **Note**: This example shows execution on a single server with clear values.
//! For MPC programs operating on secret shares, ALL servers must execute
//! simultaneously. See external/stoffel-vm/crates/stoffel-vm/src/tests/vm_mesh_integration.rs
//! for collaborative MPC execution patterns.
//!
//! It shows:
//! 1. Compiling a Stoffel program to bytecode
//! 2. Creating an MPC server
//! 3. Loading bytecode into the server's VM
//! 4. Executing functions from the bytecode
//!
//! Run with: cargo run --example bytecode_execution 

use stoffel_rust_sdk::prelude::*;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize rustls crypto provider (required for QUIC)
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    println!("=== Stoffel SDK: Bytecode Execution Example ===\n");

    // Step 1: Compile Stoffel program
    println!("Step 1: Compiling Stoffel program...");
    let source = r#"
main main() -> int64:
  return 42
    "#;

    let runtime = Stoffel::compile(source)?
        .parties(3)
        .threshold(0)
        .build()?;

    println!("✓ Program compiled successfully");
    println!();

    // Step 2: Get bytecode
    println!("Step 2: Extracting bytecode...");
    let bytecode = runtime.program().bytecode();
    println!("✓ Bytecode extracted: {} bytes", bytecode.len());
    println!();

    // Step 3: Create MPC server
    println!("Step 3: Creating MPC server...");
    let mut server = runtime.server(0)
        .with_preprocessing(0, 0)  // No preprocessing needed for this simple example
        .build()?;

    println!("✓ Server created (party {})", server.party_id());
    println!();

    // Step 4: Load bytecode into server
    println!("Step 4: Loading bytecode into server...");
    server.load_bytecode(bytecode)?;
    println!("✓ Bytecode loaded into server's VM");
    println!();

    // Step 5: Execute the main function
    println!("Step 5: Executing 'main' function...");
    let result = server.execute_function("main")?;
    println!("✓ Execution completed");
    println!();

    // Step 6: Display result
    println!("Step 6: Result:");
    println!("  {:?}", result);
    println!();

    println!("=== Bytecode Execution Demonstration Complete ===");
    println!();
    println!("✓ What This Demonstrates:");
    println!("  • Bytecode compilation from Stoffel source");
    println!("  • Bytecode loading into server VM");
    println!("  • Function execution from bytecode");
    println!("  • Integration of StoffelVM with MPC servers");
    println!();
    println!("✓ Key Features:");
    println!("  • Servers have embedded VMs for bytecode execution");
    println!("  • Bytecode is parsed using CompiledBinary format");
    println!("  • VMs can execute both clear and secret-shared computations");
    println!("  • Clean API: load_bytecode() + execute_function()");
    println!();
    println!("⚠ Important Note:");
    println!("  • This example executes on a SINGLE server with clear values");
    println!("  • MPC programs on secret shares require ALL servers to execute simultaneously");
    println!("  • See vm_mesh_integration.rs test for collaborative MPC execution");
    println!();

    Ok(())
}
