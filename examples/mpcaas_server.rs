//! MPC Server Example
//!
//! This demonstrates how an infrastructure operator runs an MPC server node.
//! Unlike the client API, servers require MPC configuration knowledge.
//!
//! # Running a Server Cluster
//!
//! Run multiple instances with different party IDs:
//! ```bash
//! # Terminal 1:
//! cargo run --example mpcaas_server -- --party-id 0 --port 19200
//!
//! # Terminal 2:
//! cargo run --example mpcaas_server -- --party-id 1 --port 19201
//!
//! # Terminal 3:
//! cargo run --example mpcaas_server -- --party-id 2 --port 19202
//! ```
//!
//! Then run the client example:
//! ```bash
//! cargo run --example mpcaas_client
//! ```

use stoffel_rust_sdk::prelude::*;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize rustls crypto provider (required for QUIC)
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args: Vec<String> = env::args().collect();

    let party_id = args.iter()
        .position(|a| a == "--party-id")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);

    let port = args.iter()
        .position(|a| a == "--port")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(19200 + party_id as u16);

    println!("=== MPCaaS Server Example ===\n");
    println!("Party ID: {}", party_id);
    println!("Port: {}", port);
    println!();

    // Compile the MPC program (StoffelLang uses 2-space indentation)
    let source = "main main(a: secret int64, b: secret int64) -> secret int64:\n  return a + b";

    // HoneyBadger requires n >= 3t + 1, so for threshold 1, we need at least 4 parties
    let program = Stoffel::compile(source)?
        .parties(4)
        .threshold(1)
        .build()?;

    println!("Program compiled successfully");
    println!("  - Parties: 4 (HoneyBadger requires n >= 3t+1)");
    println!("  - Threshold: 1 (Byzantine fault tolerance)");
    println!();

    // Define peer addresses (4 parties for threshold 1)
    // Note: Use IP addresses, not hostnames (SocketAddr doesn't resolve DNS)
    let peers: &[(usize, &str)] = &[
        (0, "127.0.0.1:19200"),
        (1, "127.0.0.1:19201"),
        (2, "127.0.0.1:19202"),
        (3, "127.0.0.1:19203"),
    ];

    // Build the server
    let server = Stoffel::server(party_id)
        .bind(&format!("0.0.0.0:{}", port))
        .with_peers(peers)
        .with_program(program.program().clone())
        .with_preprocessing(10, 20)  // 10 triples, 20 random shares
        .build()?;

    println!("Server {} configured:", party_id);
    println!("  - Bind address: 0.0.0.0:{}", port);
    println!("  - Peers: {:?}", peers.iter()
        .filter(|(id, _)| *id != party_id)
        .map(|(id, addr)| format!("party {} at {}", id, addr))
        .collect::<Vec<_>>());
    println!("  - Preprocessing: 10 triples, 20 random shares");
    println!();

    // Start the server
    println!("Starting server {}...", party_id);
    server.start().await?;

    println!("Server {} is ready!", party_id);
    println!("State: {:?}", server.state());
    println!();
    println!("Waiting for client connections...");
    println!("(Press Ctrl+C to stop)\n");

    // Run forever, handling client requests
    // In a real deployment, this would:
    // 1. Accept client connections
    // 2. Receive input shares from clients
    // 3. Coordinate with other servers for computation
    // 4. Send output shares to clients
    server.run_forever().await?;

    Ok(())
}
