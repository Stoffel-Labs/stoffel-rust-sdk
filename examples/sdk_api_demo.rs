//! SDK API Demonstration with Execution Attempt
//!
//! This example demonstrates the Stoffel SDK's high-level API for MPC,
//! including an attempt to execute the protocol.
//!
//! **Important**: Full MPC execution requires proper QUIC network setup
//! (listeners, peer connections, message handlers). The SDK provides
//! `MPCNode::run()` but YOU must set up the network infrastructure.
//!
//! For a COMPLETE working example with full network setup, see:
//! `examples/quick_start_local_network_real.rs`
//!
//! Run with: cargo run --example working_mpc_example --features mpc-local

use stoffel_rust_sdk::prelude::*;
use std::time::Duration;
use tokio::time::sleep;
use std::sync::Once;

static INIT: Once = Once::new();

fn init_crypto_provider() {
    INIT.call_once(|| {
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
    });
}

fn setup_tracing() {
    use tracing_subscriber::EnvFilter;
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();
}

#[tokio::main]
async fn main() -> Result<()> {
    init_crypto_provider();
    setup_tracing();

    println!("╔══════════════════════════════════════════════════╗");
    println!("║  Stoffel SDK: API Demo with Execution Attempt   ║");
    println!("║  Target: Secure Multiplication 10 × 20 = 200    ║");
    println!("╚══════════════════════════════════════════════════╝\n");

    println!("📋 What this example demonstrates:");
    println!("   ✓ SDK's high-level API for MPC participants");
    println!("   ✓ Automatic QuicNetworkManager creation");
    println!("   ✓ Builder pattern for MPCNode creation");
    println!("   ✓ Calling node.run() to attempt execution\n");

    println!("⚠️  Network Setup Required:");
    println!("   The SDK creates network managers but does NOT:");
    println!("   • Set up QUIC listeners (listen() calls)");
    println!("   • Establish peer connections (connect() calls)");
    println!("   • Handle incoming messages (accept() loops)");
    println!();
    println!("   For a COMPLETE working example with full network");
    println!("   infrastructure, see: quick_start_local_network_real.rs\n");

    // Step 1: Compile Stoffel program
    println!("Step 1: Compiling Stoffel program...");
    let program_source = r#"
main main(a: secret int64, b: secret int64) -> secret int64:
  return a * b
"#;

    let runtime = Stoffel::compile(program_source)?
        .parties(5)
        .threshold(1)
        .instance_id(42)
        .build()?;

    println!("  ✓ Program compiled");
    println!("  ✓ MPC configuration: 5 parties, threshold=1");
    println!("  ✓ Protocol: HoneyBadger (Byzantine fault-tolerant)\n");

    // Step 2: Create 5 MPC nodes
    println!("Step 2: Creating 5 MPC nodes...");
    let mut nodes = Vec::new();

    for party_id in 0..5 {
        let a = 10 + party_id as i64;
        let b = 20 + party_id as i64;

        let node = runtime.node(party_id)
            .with_inputs(vec![a, b])
            .with_preprocessing(3, 8)
            .build()?;

        println!("  ✓ Node {} created (inputs: a={}, b={})", party_id, a, b);
        nodes.push(node);
    }

    println!("\n  ✓ All {} nodes created with automatic networking\n", nodes.len());

    // Step 3: Get bytecode for execution
    println!("Step 3: Preparing for MPC execution...");
    let bytecode = runtime.program().bytecode();
    println!("  ✓ Bytecode ready ({} bytes)\n", bytecode.len());

    // Step 4: Execute MPC protocol on all nodes concurrently
    println!("Step 4: Running MPC protocol across all nodes...");
    println!("  This will execute all 4 phases:");
    println!("    1. Preprocessing (generate Beaver triples)");
    println!("    2. Input sharing (distribute secret shares)");
    println!("    3. Computation (secure multiplication)");
    println!("    4. Output reconstruction\n");

    let mut handles = Vec::new();
    for (i, mut node) in nodes.into_iter().enumerate() {
        let bc = bytecode.to_vec();
        let handle = tokio::spawn(async move {
            println!("  [Node {}] Starting MPC protocol...", i);

            match node.run(&bc).await {
                Ok(result) => {
                    println!("  [Node {}] ✓ Protocol completed. Result: {:?}", i, result);
                    Ok((i, result))
                }
                Err(e) => {
                    eprintln!("  [Node {}] ✗ Protocol failed: {}", i, e);
                    Err(e)
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all nodes to complete
    println!("  Waiting for all nodes to complete...\n");
    let results = futures::future::join_all(handles).await;

    // Step 5: Display results
    println!("Step 5: Results Summary");
    println!("─────────────────────────────────────────────────");
    println!();

    let mut success_count = 0;
    let mut failure_count = 0;

    for (idx, result) in results.into_iter().enumerate() {
        match result {
            Ok(Ok((node_id, output))) => {
                let a = 10 + node_id as i64;
                let b = 20 + node_id as i64;
                println!("  Node {}: {} × {} = {:?} ✓", node_id, a, b, output);
                success_count += 1;
            }
            Ok(Err(e)) => {
                println!("  Node {}: Error - {} ✗", idx, e);
                failure_count += 1;
            }
            Err(e) => {
                println!("  Node {}: Task error - {:?} ✗", idx, e);
                failure_count += 1;
            }
        }
    }

    println!();
    println!("Execution Summary:");
    println!("  Successful: {}/5", success_count);
    println!("  Failed: {}/5", failure_count);
    println!();

    if success_count == 5 {
        println!("🎉 Unexpected Success!");
        println!();
        println!("All nodes completed! This means the network setup");
        println!("happened to work in your environment.");
        println!();
        println!("Key SDK Features Demonstrated:");
        println!("  ✓ runtime.node(party_id) builder API");
        println!("  ✓ Automatic QuicNetworkManager creation");
        println!("  ✓ node.run(bytecode) execution method");
        println!("  ✓ Concurrent async execution");
    } else {
        println!("⚠️  Expected Result: Execution Failed");
        println!();
        println!("This is EXPECTED because the SDK creates network managers");
        println!("but doesn't set up the required QUIC infrastructure:");
        println!();
        println!("Missing Infrastructure:");
        println!("  ✗ QUIC listeners on designated ports");
        println!("  ✗ Peer-to-peer connections between nodes");
        println!("  ✗ Message handler loops");
        println!("  ✗ Connection acceptance tasks");
        println!();
        println!("What the SDK DOES provide:");
        println!("  ✓ High-level API for creating MPC participants");
        println!("  ✓ QuicNetworkManager instances (but not configured)");
        println!("  ✓ MPCNode::run() method (requires network setup)");
        println!("  ✓ Builder patterns for all participant types");
        println!();
        println!("📚 For a COMPLETE working example:");
        println!("   See: examples/quick_start_local_network_real.rs");
        println!();
        println!("   That example shows:");
        println!("   • How to call listen() on network managers");
        println!("   • How to add peers with add_peer()");
        println!("   • How to connect nodes with connect_to_peers()");
        println!("   • How to spawn message handler tasks");
        println!("   • Complete MPC protocol execution");
    }

    println!();
    Ok(())
}
