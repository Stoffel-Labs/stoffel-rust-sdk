//! Simple MPC Network Example - Network-Based Client-Server Architecture
//!
//! This example demonstrates the Stoffel SDK's network-based MPC architecture
//! running on localhost (all parties on the same machine for testing).
//!
//! It shows:
//! 1. Compiling a Stoffel program
//! 2. Creating MPC servers with network connectivity
//! 3. Binding servers to localhost QUIC addresses (127.0.0.1)
//! 4. Establishing peer-to-peer server connections
//! 5. Creating a network-based MPC client
//! 6. Connecting the client to servers via QUIC
//! 7. Sending secret-shared inputs from client to servers over the network
//!
//! Key Concept: This uses real QUIC networking on localhost.
//! The client generates secret shares of its inputs and distributes them
//! to servers over authenticated QUIC connections.
//!
//! For distributed deployment, simply change the IP addresses from 127.0.0.1
//! to actual machine IPs - no code changes needed!
//!
//! Run with: cargo run --example simple_mpc_network 

use stoffel_rust_sdk::prelude::*;
use tokio;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize rustls crypto provider (required for QUIC)
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    println!("=== Stoffel SDK: MPC Networking Example ===\n");

    // Step 1: Compile Stoffel program
    println!("Step 1: Compiling Stoffel program...");
    let source = r#"
main main() -> int64:
  return 42
    "#;

    let runtime = Stoffel::compile(source)?
        .parties(3)
        .threshold(0)  // Crash fault tolerance
        .build()?;

    println!("✓ Program compiled");
    println!();

    // Step 2: Create MPC servers
    println!("Step 2: Creating MPC servers...");
    let n_parties = 3;
    let n_triples = 3;
    let n_random_shares = 8;

    let base_port = 19200;
    let mut servers = Vec::new();

    // Create servers
    for i in 0..n_parties {
        let mut server = runtime.server(i)
            .with_preprocessing(n_triples, n_random_shares)
            .build()?;

        // Add peer servers to network topology
        for j in 0..n_parties {
            if i != j {
                let peer_addr = format!("127.0.0.1:{}", base_port + j).parse()?;
                server.add_peer(j, peer_addr);
            }
        }

        servers.push(server);
    }

    println!("✓ Created {} MPC servers", servers.len());
    println!();

    // Step 3: Bind servers to network addresses
    println!("Step 3: Starting QUIC network listeners...");
    let mut server_receivers = Vec::new();

    for (i, server) in servers.iter_mut().enumerate() {
        let bind_addr = format!("127.0.0.1:{}", base_port + i).parse()?;
        let rx = server.bind_and_listen(bind_addr).await?;
        server_receivers.push(rx);
        println!("  ✓ Server {} listening on {}", i, bind_addr);
    }

    println!();

    // Step 4: Establish peer-to-peer connections
    println!("Step 4: Connecting servers to each other...");

    // Small delay to ensure all servers are listening
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    for (i, server) in servers.iter().enumerate() {
        let _peer_channels = server.connect_to_peers().await?;
        println!("  ✓ Server {} connected to peers", i);
    }

    println!();

    // Step 5: Create and connect client
    println!("Step 5: Creating MPC client...");

    let mut client = runtime.client(100)
        .with_inputs(vec![10, 20])
        .build()?;

    // Add server addresses
    for i in 0..n_parties {
        let server_addr = format!("127.0.0.1:{}", base_port + i).parse()?;
        client.add_server(i, server_addr);
    }

    println!("✓ Client created with inputs: {:?}", client.inputs());
    println!();

    // Step 6: Client connects to servers and sends secret-shared inputs
    println!("Step 6: Connecting client to servers...");

    // Connect client to all servers via QUIC
    let mut client_rx = client.connect_to_servers().await?;
    println!("✓ Client connected to all servers via QUIC");
    println!();

    println!("Step 7: Sending secret-shared inputs to MPC network...");

    // Client generates shares and sends them over the network
    client.send_inputs().await?;
    println!("✓ Client sent secret-shared inputs to all {} servers", n_parties);
    println!();

    // Small delay to ensure messages are processed
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    println!("=== MPC Network Setup Complete ===");
    println!();
    println!("✓ What's Working:");
    println!("  • Server-to-server QUIC networking (full mesh topology)");
    println!("  • Client-to-server QUIC connections");
    println!("  • {} servers bound to ports {}-{}", n_parties, base_port, base_port + n_parties - 1);
    println!("  • Secret share generation and network distribution");
    println!("  • All {} inputs secret-shared across {} servers", client.inputs().len(), n_parties);
    println!();
    println!("✓ SDK APIs Demonstrated:");
    println!("  • MPCServer: add_peer(), bind_and_listen(), connect_to_peers()");
    println!("  • MPCClient: add_server(), connect_to_servers(), send_inputs()");
    println!("  • Real QUIC networking on localhost (127.0.0.1)");
    println!();
    println!("Next Steps:");
    println!("  • Servers run preprocessing (generate beaver triples)");
    println!("  • Servers execute MPC computation on secret shares");
    println!("  • Client receives and reconstructs output");
    println!();
    println!("For complete MPC execution workflow, see:");
    println!("  external/stoffel-vm/crates/stoffel-vm/src/tests/mpc_multiplication_integration.rs");

    Ok(())
}
