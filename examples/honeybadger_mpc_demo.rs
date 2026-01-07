//! Full E2E HoneyBadger MPC Demo
//!
//! Demonstrates the complete MPC workflow:
//! 1. Start 4 MPC servers with HoneyBadger preprocessing
//! 2. Connect a client with secret inputs
//! 3. Execute secure computation
//! 4. Receive and verify reconstructed output
//!
//! ## Running
//!
//! ```bash
//! cargo run --example honeybadger_mpc_demo
//! ```

use stoffel_rust_sdk::prelude::*;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize crypto provider (required for QUIC)
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install crypto provider");

    // Enable tracing for visibility into MPC protocol
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    tracing_subscriber::fmt::init();

    println!("╔═══════════════════════════════════════════╗");
    println!("║      HoneyBadger MPC Demo                 ║");
    println!("║  Secure Multi-Party Computation Example   ║");
    println!("╚═══════════════════════════════════════════╝\n");

    // ===== Step 1: Define MPC configuration =====
    // Note: TripleGen preprocessing requires n >= 4t + 1 (stricter than basic Byzantine n >= 3t + 1)
    // For threshold=1: need at least 5 parties (5 >= 4*1 + 1)
    let n_parties = 5;
    let threshold = 1;  // Tolerate 1 Byzantine failure
    let base_port = 19200;
    // CRITICAL: All servers MUST share the same instance_id for MPC session coordination
    let instance_id: u64 = 12345;

    println!("MPC Configuration:");
    println!("  Parties (n): {}", n_parties);
    println!("  Threshold (t): {} (tolerates {} Byzantine failures)", threshold, threshold);
    println!("  Instance ID: {} (shared by all servers)", instance_id);
    println!("  TripleGen constraint: n >= 4t + 1 -> {} >= {} ✓", n_parties, 4 * threshold + 1);
    println!();

    // ===== Step 2: Compile Stoffel program =====
    // MPC programs must explicitly load client inputs from ClientStore
    // The program uses ClientStore.take_share(client_index, share_index) to load secret shares
    let source = r#"
main main() -> secret int64:
  var a: secret int64 = ClientStore.take_share(0, 0)
  var b: secret int64 = ClientStore.take_share(0, 1)
  return a + b
    "#;

    println!("Compiling Stoffel program:");
    println!("  Program: secret addition using ClientStore");
    println!("  Source:\n{}", source);

    let runtime = Stoffel::compile(source)?
        .parties(n_parties)
        .threshold(threshold)
        .build()?;

    println!("  Compilation: SUCCESS\n");

    // ===== Step 3: Generate peer addresses =====
    let peer_addrs: Vec<(usize, String)> = (0..n_parties)
        .map(|i| (i, format!("127.0.0.1:{}", base_port + i)))
        .collect();

    // ===== Step 4: Create and start servers =====
    println!("Starting {} MPC servers...", n_parties);
    println!("────────────────────────────────────────────");

    // CRITICAL: Calculate preprocessing start time BEFORE creating any servers
    // All servers will receive the same start time and wait until that absolute moment.
    // This ensures all servers start preprocessing at exactly the same time regardless
    // of when they individually finish initializing.
    //
    // We add 20 seconds to give servers time to:
    // - Start and bind ports
    // - Connect to peers
    // - Create MPC engine
    // - Spawn message processors
    let preprocessing_start_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() + 20;

    println!("  Preprocessing will start at epoch: {} (in ~20 seconds)", preprocessing_start_epoch);

    let mut server_handles = Vec::new();

    // Pre-build all servers before spawning (avoids lifetime issues)
    let mut servers = Vec::new();
    for party_id in 0..n_parties {
        let bind_addr = format!("0.0.0.0:{}", base_port + party_id);

        // Create peers list excluding self as static strings
        let peers: Vec<(usize, &str)> = peer_addrs.iter()
            .filter(|(id, _)| *id != party_id)
            .map(|(id, addr)| (*id, addr.as_str()))
            .collect();

        let program = runtime.program().clone();

        let server = Stoffel::server(party_id)
            .bind(&bind_addr)
            .with_peers(&peers)
            .with_program(program)
            // Use minimal preprocessing values matching StoffelVM test formula:
            // n_triples = 2t + 1 = 3, n_random_shares = 2 + 2*n_triples = 8
            .with_preprocessing(3, 8)  // 3 Beaver triples, 8 random shares
            .with_instance_id(instance_id)  // CRITICAL: Same instance_id for all servers
            .with_preprocessing_start_time(preprocessing_start_epoch)  // CRITICAL: Same start time
            .build()
            .expect(&format!("Failed to build server {}", party_id));

        println!("  Server {} configured on {}", party_id, bind_addr);
        servers.push(server);
    }

    // Spawn server tasks
    for (party_id, server) in servers.into_iter().enumerate() {
        let handle = tokio::spawn(async move {
            println!("  Server {} starting...", party_id);

            if let Err(e) = server.start().await {
                eprintln!("  Server {} failed to start: {}", party_id, e);
                return;
            }

            // Run forever (handles clients in background)
            if let Err(e) = server.run_forever().await {
                eprintln!("  Server {} error: {}", party_id, e);
            }
        });

        server_handles.push(handle);

        // Small delay between starting servers to avoid port conflicts
        sleep(Duration::from_millis(100)).await;
    }

    // Wait for servers to initialize, connect peers, and run preprocessing
    // Timeline:
    //   - Servers take ~5-7 seconds to start and connect to peers
    //   - Servers wait until preprocessing_start_epoch (20 seconds from demo start)
    //   - Preprocessing itself takes ~5-10 seconds
    // Total: ~30-35 seconds, so we wait 40 seconds to be safe
    println!("\nWaiting for servers to complete preprocessing...");
    println!("(This takes ~40 seconds for network mesh, sync, and HoneyBadger triple generation)\n");
    sleep(Duration::from_secs(40)).await;

    // ===== Step 5: Connect client and run computation =====
    println!("────────────────────────────────────────────");
    println!("Client Connecting to MPC Network");
    println!("────────────────────────────────────────────\n");

    let server_addrs: Vec<&str> = peer_addrs.iter()
        .map(|(_, addr)| addr.as_str())
        .collect();

    let client_inputs = vec![42_i64, 100_i64];  // a=42, b=100

    println!("Client Configuration:");
    println!("  Input a: {} (secret)", client_inputs[0]);
    println!("  Input b: {} (secret)", client_inputs[1]);
    println!("  Expected result: a + b = {}\n", client_inputs.iter().sum::<i64>());

    println!("Connecting to servers: {:?}", server_addrs);

    // Build and connect client using the new StoffelClient API
    let client = match StoffelClient::builder()
        .with_servers(&server_addrs)
        .connect()
        .await
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\nConnection failed: {}", e);
            eprintln!("Possible causes:");
            eprintln!("  - Servers not fully started");
            eprintln!("  - Port conflicts (try different base_port)");
            eprintln!("  - Firewall blocking connections");
            return Ok(());
        }
    };

    println!("\nConnected to MPC network:");
    println!("  Number of parties: {}", client.n_parties());
    println!("  Threshold: {}", client.threshold());
    println!("  Client ID: {}", client.client_id());
    println!("  State: {:?}\n", client.state());

    println!("Running secure computation...");
    println!("(Input protocol: MaskShare -> MaskedInput -> Computation -> Output)\n");

    match client.run(&client_inputs).await {
        Ok(result) => {
            println!("────────────────────────────────────────────");
            println!("                   RESULT                   ");
            println!("────────────────────────────────────────────");
            println!();
            println!("  MPC Output: {:?}", result);
            println!();
            println!("  Verification:");
            println!("    {} + {} = {}",
                client_inputs[0],
                client_inputs[1],
                result.get(0).unwrap_or(&0));
            println!("    Expected: {}", client_inputs.iter().sum::<i64>());

            let expected = client_inputs.iter().sum::<i64>();
            if result.get(0) == Some(&expected) {
                println!("\n  ✓ Result matches expected value!");
            } else {
                println!("\n  ✗ Result does not match expected value");
            }
            println!();
        }
        Err(e) => {
            eprintln!("\nComputation error: {}", e);
            eprintln!("This may indicate:");
            eprintln!("  - Preprocessing not complete");
            eprintln!("  - Network connectivity issues");
            eprintln!("  - Protocol message timeout");
        }
    }

    println!("────────────────────────────────────────────");
    println!("Demo complete!");
    println!("────────────────────────────────────────────");

    // Note: In a real application, you would want to gracefully shut down servers
    // For demo purposes, we just let the process exit

    Ok(())
}
