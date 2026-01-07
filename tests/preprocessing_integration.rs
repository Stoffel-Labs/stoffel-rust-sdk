//! Integration test for MPC preprocessing with message processing.
//!
//! This test verifies that preprocessing completes successfully when
//! message processors are properly spawned. It exercises the full
//! MPC network setup flow including:
//! - Server creation with builder pattern
//! - Peer connection establishment
//! - Message processor spawning
//! - HoneyBadger preprocessing protocol execution
//!
//! The test uses a 4-party configuration which is the minimum for
//! HoneyBadger (n >= 3t + 1, where t = 1 threshold).

use stoffel_rust_sdk::prelude::*;
use std::time::Duration;
use std::sync::Arc;

/// Simple test program for MPC execution
const TEST_PROGRAM: &str = "main main() -> int64:\n  return 42\n";

/// Test that preprocessing completes successfully for a 4-party network.
///
/// This test creates 4 MPC servers, each connecting to the others,
/// and verifies that preprocessing completes within a reasonable timeout.
///
/// The test documents the expected behavior after the message processing fix:
/// - Servers should spawn message processors before preprocessing
/// - Messages sent during preprocessing should be received and processed
/// - All servers should reach the Ready state
#[tokio::test]
#[ignore] // Run with: cargo test --test preprocessing_integration -- --ignored
async fn test_preprocessing_completes_with_message_processing() {
    // Setup: 4 servers with explicit peers (minimum for HoneyBadger with t=1)
    let n_parties = 4;
    let base_port = 29200; // Use different port range for tests

    // Create program
    let program = Stoffel::compile(TEST_PROGRAM)
        .expect("Compile failed")
        .build()
        .expect("Build failed");

    // Share the program across all servers
    let program_arc = Arc::new(program.program().clone());

    // Build server handles
    let mut server_handles = Vec::new();

    for party_id in 0..n_parties {
        let port = base_port + party_id as u16;
        let bind_addr = format!("127.0.0.1:{}", port);

        // Build peer list (all other parties)
        let peers: Vec<(usize, String)> = (0..n_parties)
            .filter(|&p| p != party_id)
            .map(|p| (p, format!("127.0.0.1:{}", base_port + p as u16)))
            .collect();

        let program_clone = Arc::clone(&program_arc);

        let handle = tokio::spawn(async move {
            // Build peer list with proper lifetime
            let peers_ref: Vec<(usize, &str)> = peers.iter()
                .map(|(id, addr)| (*id, addr.as_str()))
                .collect();

            // Build server using the SDK's builder pattern
            let server_result = Stoffel::server(party_id)
                .bind(&bind_addr)
                .with_peers(&peers_ref)
                .with_program((*program_clone).clone())
                .with_preprocessing(5, 10) // Small amount for test
                .build();

            let server = match server_result {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Server {} build failed: {:?}", party_id, e);
                    return Err(format!("Server {} build failed: {:?}", party_id, e));
                }
            };

            // Start the server
            if let Err(e) = server.start().await {
                eprintln!("Server {} start failed: {:?}", party_id, e);
                return Err(format!("Server {} start failed: {:?}", party_id, e));
            }

            // Wait for preprocessing to complete (timeout after 60 seconds)
            let start = std::time::Instant::now();
            let timeout = Duration::from_secs(60);

            while server.state() != ServerState::Ready {
                if start.elapsed() > timeout {
                    return Err(format!(
                        "Server {} preprocessing timed out after {:?}. State: {:?}",
                        party_id,
                        timeout,
                        server.state()
                    ));
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            println!("Server {} reached Ready state after {:?}", party_id, start.elapsed());
            Ok(party_id)
        });

        server_handles.push(handle);
    }

    // Wait for all servers with a global timeout
    let global_timeout = Duration::from_secs(120);
    let results = tokio::time::timeout(
        global_timeout,
        async {
            let mut results = Vec::new();
            for handle in server_handles {
                results.push(handle.await);
            }
            results
        }
    ).await
    .expect("Test timed out");

    // Process results
    let mut success_count = 0;
    let mut failures = Vec::new();

    for r in results {
        match r {
            Ok(Ok(party_id)) => {
                success_count += 1;
                println!("Server {} completed successfully", party_id);
            }
            Ok(Err(e)) => {
                failures.push(e);
            }
            Err(e) => {
                failures.push(format!("Task panicked: {:?}", e));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "Test failed: {}/{} servers did not complete preprocessing.\nFailures:\n{}",
            failures.len(),
            n_parties,
            failures.join("\n")
        );
    }

    assert_eq!(
        success_count, n_parties,
        "All {} servers should complete preprocessing", n_parties
    );
    println!("All {} servers completed preprocessing successfully!", n_parties);
}

/// Test that preprocessing fails gracefully when message processors are missing.
/// This test is kept as documentation of the original bug behavior.
///
/// Before the fix:
/// - Servers would start preprocessing
/// - Messages would be sent but never received
/// - Preprocessing would timeout
///
/// This test is ignored by default and should only be run to verify
/// the bug still exists (e.g., after reverting the fix).
#[tokio::test]
#[ignore]
async fn test_preprocessing_timeout_without_message_processing() {
    // This test documents the original bug behavior.
    // If you need to verify the bug still exists without the fix,
    // revert the changes to spawn_message_processors and run this test.
    //
    // Expected behavior without fix:
    // - Servers connect to each other
    // - Preprocessing starts
    // - Messages are sent but never processed
    // - Timeout after ~30 seconds with "Preprocessing timeout" error

    println!("This test documents the original bug.");
    println!("Run test_preprocessing_completes_with_message_processing instead.");
}

/// Quick sanity test that the SDK can compile a simple program
#[tokio::test]
async fn test_can_compile_program() {
    let result = Stoffel::compile(TEST_PROGRAM);
    assert!(result.is_ok(), "Should be able to compile test program");

    let builder = result.unwrap();
    let runtime = builder.build();
    assert!(runtime.is_ok(), "Should be able to build runtime");

    let program = runtime.unwrap().program().clone();
    assert!(!program.bytecode().is_empty(), "Bytecode should not be empty");
}

/// Test that server builder can be configured
#[tokio::test]
async fn test_server_builder_configuration() {
    let program = Stoffel::compile(TEST_PROGRAM)
        .expect("Compile failed")
        .build()
        .expect("Build failed");

    // Test that we can create and configure a server builder
    // The builder pattern allows chaining configuration
    let _server_builder = Stoffel::server(0)
        .bind("127.0.0.1:30000")
        .with_program(program.program().clone())
        .with_preprocessing(10, 20);

    // Builder can be created without errors
    // Full validation happens at build() time
}
