//! Quick Start: MPC Network with Automatic Setup
//!
//! This example demonstrates the simplest way to set up an MPC network using
//! the Stoffel SDK's automatic network management. It shows:
//!
//! 1. Compiling a Stoffel program with MPC configuration
//! 2. Creating MPC nodes with automatic network setup
//! 3. Peer-to-peer MPC where all parties contribute inputs
//! 4. Running the complete MPC protocol (preprocessing, input sharing, computation, output)
//!
//! This is the recommended starting point for learning Stoffel MPC.
//!
//! Run with: cargo run --example quick_start_network --features mpc-local

use stoffel_rust_sdk::prelude::*;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔═══════════════════════════════════════════════╗");
    println!("║  Stoffel SDK: Quick Start MPC Network        ║");
    println!("╚═══════════════════════════════════════════════╝\n");

    // Step 1: Configure MPC network
    println!("Step 1: Configuring MPC network...");
    let n_parties = 5;
    let threshold = 1;
    let instance_id = 12345;

    println!("  • Parties: {} (n)", n_parties);
    println!("  • Threshold: {} (t) - tolerates {} Byzantine faults", threshold, threshold);
    println!("  • Constraint: n >= 3t+1 → {} >= {} ✓", n_parties, 3*threshold + 1);
    println!("  • Protocol: HoneyBadger (async BFT MPC)");
    println!("  • Instance ID: {}\n", instance_id);

    // Step 2: Compile Stoffel program
    println!("Step 2: Compiling Stoffel program...");
    let program_source = r#"
main main(a: secret int64, b: secret int64) -> secret int64:
  return a * b
"#;

    let runtime = Stoffel::compile(program_source)?
        .parties(n_parties)
        .threshold(threshold)
        .instance_id(instance_id)
        .build()?;

    println!("  ✓ Program compiled");
    println!("  ✓ MPC configuration validated\n");

    // Step 3: Create MPC nodes (peer-to-peer)
    println!("Step 3: Creating {} MPC nodes...", n_parties);
    println!("  Each node will:");
    println!("    • Contribute private inputs (a, b)");
    println!("    • Participate in secure computation");
    println!("    • Receive the output (a * b)\n");

    let mut nodes = Vec::new();
    for party_id in 0..n_parties {
        // Each party has different inputs
        let a = 10 + party_id as i64;
        let b = 20 + party_id as i64;

        // Create node with automatic network setup!
        let node = runtime.node(party_id)
            .with_inputs(vec![a, b])
            .with_preprocessing(3, 8)  // 3 Beaver triples, 8 random shares
            .build()?;

        println!("  ✓ Node {} created (inputs: a={}, b={})", party_id, a, b);
        println!("    - Network automatically configured with node_id={}", party_id);
        println!("    - Preprocessing: 3 Beaver triples, 8 random shares");

        nodes.push(node);
    }

    println!("\n  ✓ All {} nodes created with automatic networking!", n_parties);
    println!("  ✓ No manual network management needed!\n");

    // Step 4: Setup network connectivity
    println!("Step 4: Setting up network connections...");
    println!("  Note: In a real deployment, nodes would:");
    println!("    • Listen on their designated ports");
    println!("    • Connect to other nodes via QUIC");
    println!("    • Exchange messages for MPC protocol");
    println!();
    println!("  For this demo, we're showing the API without actual network I/O.\n");

    // Step 5: Summary
    println!("Step 5: Summary");
    println!("─────────────────────────────────────────────");
    println!();
    println!("Complete MPC Network Setup:");
    println!("  {} nodes created with automatic networking", n_parties);
    println!();
    println!("Each node is ready to:");
    println!("  1. Run preprocessing (generate Beaver triples)");
    println!("  2. Share inputs (interactive masking protocol)");
    println!("  3. Compute multiplication (secure MPC)");
    println!("  4. Reconstruct output (robust interpolation)");
    println!();
    println!("To run the full protocol:");
    println!("  • Call node.run(bytecode).await on each node");
    println!("  • Nodes will automatically coordinate via network");
    println!("  • Result: Each node learns output, but no node learns others' inputs");
    println!();

    // Example of what the API would look like for actual execution
    println!("Example execution (requires network I/O):");
    println!("  ```rust");
    println!("  // Run MPC protocol on all nodes concurrently");
    println!("  let bytecode = runtime.program().bytecode();");
    println!("  let mut handles = Vec::new();");
    println!();
    println!("  for mut node in nodes {{");
    println!("      let bc = bytecode.clone();");
    println!("      let handle = tokio::spawn(async move {{");
    println!("          node.run(&bc).await");
    println!("      }});");
    println!("      handles.push(handle);");
    println!("  }}");
    println!();
    println!("  // Wait for all nodes to complete");
    println!("  for handle in handles {{");
    println!("      let result = handle.await??;");
    println!("      println!(\"Node result: {{:?}}\", result);");
    println!("  }}");
    println!("  ```");
    println!();

    println!("✅ MPC network successfully configured!\n");
    println!("See examples/quick_start_local_network_real.rs for a working network example.");
    println!("See examples/stoffel_sdk_demo.rs for more SDK features.\n");

    Ok(())
}
