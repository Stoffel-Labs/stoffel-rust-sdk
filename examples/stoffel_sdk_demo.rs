//! Comprehensive Stoffel SDK Demo
//!
//! This example demonstrates the complete Stoffel SDK workflow:
//!
//! 1. **Program Compilation** - Compiling Stoffel programs to bytecode
//! 2. **Local Execution** - Testing programs locally on the VM
//! 3. **MPC Configuration** - Setting up MPC infrastructure (parties, threshold)
//! 4. **MPC Participants** - Creating servers, clients, and nodes
//! 5. **Secret Sharing** - Distributing inputs across MPC network
//! 6. **Network Configuration** - Manual and TOML-based network setup
//! 7. **Protocol Configuration** - Understanding HoneyBadger protocol
//!
//! Run with: cargo run --example stoffel_sdk_demo

use stoffel_rust_sdk::prelude::*;

fn main() -> Result<()> {
    println!("╔══════════════════════════════════════════╗");
    println!("║   Stoffel SDK: Comprehensive Demo       ║");
    println!("╚══════════════════════════════════════════╝\n");

    part1_program_compilation()?;
    part2_local_execution()?;
    part3_mpc_configuration()?;
    part4_mpc_participants()?;
    part5_protocol_configuration()?;
    part6_network_configuration()?;

    println!("\n✅ All demos completed successfully!\n");
    Ok(())
}

// ============================================================================
// PART 1: Program Compilation
// ============================================================================

fn part1_program_compilation() -> Result<()> {
    println!("┌────────────────────────────────────────┐");
    println!("│  PART 1: Program Compilation           │");
    println!("└────────────────────────────────────────┘\n");

    // Simple Stoffel program
    let source = r#"
main main(a: int64, b: int64) -> int64:
  var c = a + b
  return c * 2
"#;

    println!("Compiling Stoffel program:");
    println!("{}", source);

    // Compile without MPC configuration (for testing)
    let runtime = Stoffel::compile(source)?.build()?;

    println!("✓ Program compiled successfully");
    println!("✓ Bytecode generated for VM execution\n");

    Ok(())
}

// ============================================================================
// PART 2: Local Execution
// ============================================================================

fn part2_local_execution() -> Result<()> {
    println!("┌────────────────────────────────────────┐");
    println!("│  PART 2: Local VM Execution            │");
    println!("└────────────────────────────────────────┘\n");

    let source = r#"
main main() -> int64:
  return 42
"#;

    let runtime = Stoffel::compile(source)?.build()?;

    println!("Executing program locally on VM...");
    let result = runtime.program().execute_local()?;

    println!("✓ Execution completed");
    println!("✓ Result: {:?}\n", result);

    Ok(())
}

// ============================================================================
// PART 3: MPC Configuration
// ============================================================================

fn part3_mpc_configuration() -> Result<()> {
    println!("┌────────────────────────────────────────┐");
    println!("│  PART 3: MPC Configuration             │");
    println!("└────────────────────────────────────────┘\n");

    let source = r#"
main main(a: secret int64, b: secret int64) -> secret int64:
  return a * b
"#;

    let n_parties = 5;
    let threshold = 1;
    let instance_id = 12345;

    println!("Configuring MPC infrastructure:");
    println!("  • Parties: {} (n)", n_parties);
    println!("  • Threshold: {} (t)", threshold);
    println!("  • Byzantine constraint: n >= 3t+1 → {} >= {}", n_parties, 3*threshold + 1);
    println!("  • Instance ID: {}", instance_id);

    let runtime = Stoffel::compile(source)?
        .parties(n_parties)
        .threshold(threshold)
        .instance_id(instance_id)
        .build()?;

    println!("\n✓ MPC configuration validated");
    println!("✓ Runtime ready for MPC participant creation\n");

    Ok(())
}

// ============================================================================
// PART 4: MPC Participants (Servers, Clients, Nodes)
// ============================================================================

fn part4_mpc_participants() -> Result<()> {
    println!("┌────────────────────────────────────────┐");
    println!("│  PART 4: MPC Participants              │");
    println!("└────────────────────────────────────────┘\n");

    let source = r#"
main main(a: secret int64, b: secret int64) -> secret int64:
  return a * b
"#;

    let runtime = Stoffel::compile(source)?
        .parties(5)
        .threshold(1)
        .instance_id(12345)
        .build()?;

    // 4.1 MPC Server Network (MPC as a Service)
    println!("4.1 Creating 5-Party MPC Server Network:");
    println!("  Architecture: MPC as a Service");
    println!("  • Servers perform computation");
    println!("  • Clients provide inputs and receive outputs\n");

    let mut servers = Vec::new();
    for party_id in 0..5 {
        let server = runtime.server(party_id)
            .with_preprocessing(3, 8)  // 3 triples, 8 random shares
            .build()?;

        println!("  ✓ Server {} created (network automatically configured)", party_id);
        servers.push(server);
    }

    println!("\n  ✓ Complete 5-party MPC network ready");
    println!("  ✓ All servers have automatic network management\n");

    // 4.2 MPC Client
    println!("4.2 Creating MPC Client:");
    let client = runtime.client(100)
        .with_inputs(vec![10, 20])
        .build()?;

    println!("  ✓ Client created (client_id=100)");
    println!("  ✓ Network automatically configured");
    println!("  ✓ Inputs: [10, 20]");
    println!("  • Role: Provides inputs to the 5 servers, receives outputs\n");

    // 4.3 Secret Sharing and Distribution
    println!("4.3 Secret Sharing and Distribution:");
    let shares = client.generate_input_shares_robust()?;

    println!("  ✓ Generated {} shares for each input", shares.len());
    println!("  ✓ Sharing scheme: Reed-Solomon erasure coding");
    println!("  ✓ Byzantine fault tolerance: up to t={} corrupted shares", 1);
    println!("  ✓ Reconstruction requires: 2t+1={} shares", 2*1 + 1);

    println!("\n  Distributing shares to servers:");
    // In a real deployment, client would send shares to each server
    // For this demo, we show the distribution pattern
    for (server_idx, server) in servers.iter_mut().enumerate() {
        // shares[party_id] contains all input shares for that party
        let server_shares = &shares[server_idx];

        println!("    → Server {} receives {} shares (one per input)", server_idx, server_shares.len());

        // Server stores the shares it received
        server.receive_input_shares(server_shares)?;
    }

    println!("\n  ✓ All servers received their shares");
    println!("  ✓ Each server has partial information (cannot learn inputs alone)\n");

    // 4.3b Output Reconstruction Demo
    println!("4.3b Output Reconstruction:");
    println!("  After MPC computation completes:");
    println!("  • Each server computes on their shares");
    println!("  • Servers send output shares back to client");
    println!("  • Client reconstructs the final result\n");

    println!("  Example reconstruction process:");
    println!("    1. Servers perform secure computation → output shares");
    println!("    2. Client collects output shares from servers");
    println!("    3. Client runs robust reconstruction");
    println!("    4. Client obtains final result: f(inputs)");
    println!("    5. Servers never learn client's private inputs ✓\n");

    // 4.4 MPC Node (Peer-to-Peer)
    println!("4.4 Creating MPC Nodes (Peer-to-Peer):");
    println!("  Architecture: Peer-to-Peer MPC");
    println!("  • All parties provide inputs AND compute\n");

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

    println!("\n  ✓ Complete 5-party peer-to-peer network ready");
    println!("  ✓ Each node contributes inputs AND participates in computation\n");

    Ok(())
}

// ============================================================================
// PART 5: Protocol Configuration (HoneyBadger)
// ============================================================================

fn part5_protocol_configuration() -> Result<()> {
    println!("┌────────────────────────────────────────┐");
    println!("│  PART 5: Protocol Configuration        │");
    println!("└────────────────────────────────────────┘\n");

    let source = r#"
main main(a: secret int64, b: secret int64) -> secret int64:
  return a * b
"#;

    println!("HoneyBadger Protocol (Default):");
    println!("  • Byzantine Fault Tolerant (BFT)");
    println!("  • Asynchronous (no timing assumptions)");
    println!("  • Optimal resilience: n >= 3t+1");
    println!("  • Guaranteed output for honest parties");
    println!("  • Uses Beaver triples for multiplication");
    println!("  • Interactive masking for input privacy\n");

    let runtime = Stoffel::compile(source)?
        .parties(5)
        .threshold(1)
        .build()?;

    println!("✓ Runtime uses HoneyBadger protocol");
    println!("✓ Secret sharing: RobustShare (Reed-Solomon)");
    println!("✓ Reliable broadcast: AVID (Asynchronous Verifiable Information Dispersal)\n");

    Ok(())
}

// ============================================================================
// PART 6: Network Configuration
// ============================================================================

fn part6_network_configuration() -> Result<()> {
    println!("┌────────────────────────────────────────┐");
    println!("│  PART 6: Network Configuration         │");
    println!("└────────────────────────────────────────┘\n");

    println!("Automatic Network Setup:");
    println!("  When you create MPC participants via the runtime,");
    println!("  the network is configured automatically:\n");

    let source = r#"
main main(a: secret int64) -> secret int64:
  return a * 2
"#;

    let runtime = Stoffel::compile(source)?
        .parties(5)
        .threshold(1)
        .build()?;

    let _server = runtime.server(0).build()?;
    println!("  let server = runtime.server(0).build()?;");
    println!("  ✓ Network created automatically with node_id=0");
    println!("  ✓ QUIC transport configured");
    println!("  ✓ Ready to connect to other parties");
    println!("  ✓ No manual network management needed!\n");

    let _client = runtime.client(100).with_inputs(vec![42]).build()?;
    println!("  let client = runtime.client(100).with_inputs(vec![42]).build()?;");
    println!("  ✓ Client network created automatically with node_id=100");
    println!("  ✓ Ready to connect to MPC servers\n");

    let _node = runtime.node(0).with_inputs(vec![10, 20]).build()?;
    println!("  let node = runtime.node(0).with_inputs(vec![10, 20]).build()?;");
    println!("  ✓ Node network created automatically with node_id=0");
    println!("  ✓ Ready for peer-to-peer MPC\n");

    println!("Advanced Network Configuration:");
    println!("  For production deployments, you can use TOML configuration:");
    println!(r#"
  # stoffel.toml
  [network]
  party_id = 0
  bind_address = "0.0.0.0:19200"
  bootstrap_address = "bootstrap.example.com:9000"
  min_parties = 5

  [mpc]
  n_parties = 5
  threshold = 1
  instance_id = 12345
"#);

    println!("  Load with: NetworkConfig::from_file(\"stoffel.toml\")?;\n");

    Ok(())
}
