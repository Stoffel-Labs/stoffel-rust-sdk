//! HoneyBadger MPC Integration Tests
//!
//! These tests verify the complete MPC workflow including:
//! 1. Program compilation with ClientStore
//! 2. Server network setup and preprocessing
//! 3. Client input distribution
//! 4. Secure computation execution
//! 5. Output reconstruction
//!
//! # Running Tests
//!
//! ```bash
//! # Run all integration tests (fast, non-network tests only by default)
//! cargo test --test honeybadger_integration
//!
//! # Run ignored slow tests (requires network setup)
//! cargo test --test honeybadger_integration -- --ignored
//!
//! # Run specific test
//! cargo test --test honeybadger_integration test_compile_clientstore_program
//!
//! # Run with output
//! cargo test --test honeybadger_integration -- --nocapture
//! ```

mod common;

use stoffel_rust_sdk::prelude::*;
use common::*;

// ============================================================================
// Compilation Tests - Critical Path
// ============================================================================

/// Test that ClientStore programs compile successfully
///
/// This is a CRITICAL test - if ClientStore.take_share() doesn't compile,
/// no MPC computation can run.
#[test]
fn test_compile_clientstore_program() {
    let result = Stoffel::compile(TEST_PROGRAM_ADD);
    assert!(
        result.is_ok(),
        "ClientStore program should compile: {:?}",
        result.err()
    );

    let builder = result.unwrap();
    let runtime = builder.parties(5).threshold(1).build();
    assert!(
        runtime.is_ok(),
        "Runtime should build: {:?}",
        runtime.err()
    );

    let program = runtime.unwrap().program().clone();
    assert!(
        !program.bytecode().is_empty(),
        "Bytecode should not be empty"
    );
}

/// Test that compilation works with different MPC configurations
#[test]
fn test_compile_with_mpc_config() {
    let source = TEST_PROGRAM_ADD;

    // Standard config: 5 parties, threshold 1
    let runtime = Stoffel::compile(source)
        .unwrap()
        .parties(5)
        .threshold(1)
        .build();
    assert!(runtime.is_ok(), "Standard config should work");

    // Higher tolerance: 9 parties, threshold 2
    let runtime = Stoffel::compile(source)
        .unwrap()
        .parties(9)
        .threshold(2)
        .build();
    assert!(runtime.is_ok(), "Higher tolerance config should work");
}

/// Test compilation of program with three inputs
#[test]
fn test_compile_three_inputs() {
    let result = compile_test_program(TEST_PROGRAM_THREE_INPUTS);
    assert!(
        result.is_ok(),
        "Three-input program should compile: {:?}",
        result.err()
    );
}

/// Test compilation of multiplication program
#[test]
fn test_compile_multiplication() {
    let result = compile_test_program(TEST_PROGRAM_MULTIPLY);
    assert!(
        result.is_ok(),
        "Multiplication program should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Value Conversion Tests - Regression Protection
// ============================================================================

/// Test that SDK values can be constructed and accessed
#[test]
fn test_sdk_value_construction() {
    use stoffel_rust_sdk::vm::Value;

    let int_val = Value::Int(42);
    assert_eq!(int_val.as_int(), Some(42));

    let bool_val = Value::Bool(true);
    assert_eq!(bool_val.as_bool(), Some(true));

    let unit_val = Value::Unit;
    assert!(unit_val.is_unit());
}

/// Test ShareType enum variants
#[test]
fn test_share_type_variants() {
    use stoffel_rust_sdk::vm::ShareType;

    let secret_int = ShareType::SecretInt { bit_length: 64 };
    match secret_int {
        ShareType::SecretInt { bit_length } => assert_eq!(bit_length, 64),
        _ => panic!("Expected SecretInt"),
    }

    let secret_fp = ShareType::SecretFixedPoint { k: 64, f: 32 };
    match secret_fp {
        ShareType::SecretFixedPoint { k, f } => {
            assert_eq!(k, 64);
            assert_eq!(f, 32);
        }
        _ => panic!("Expected SecretFixedPoint"),
    }
}

// ============================================================================
// Server Builder Tests - Configuration Validation
// ============================================================================

/// Test server builder can be configured with all options
#[test]
fn test_server_builder_full_configuration() {
    let runtime = compile_addition_program().expect("Compile failed");
    let base_port = get_test_port_base();
    let peers = generate_peers_for_party(0, 5, base_port);
    let peers_ref: Vec<(usize, &str)> = peers.iter().map(|(id, addr)| (*id, addr.as_str())).collect();

    let _builder = Stoffel::server(0)
        .bind(&format!("127.0.0.1:{}", base_port))
        .with_peers(&peers_ref)
        .with_program(runtime.program().clone())
        .with_preprocessing(DEFAULT_N_TRIPLES, DEFAULT_N_RANDOM_SHARES)
        .with_instance_id(12345);

    // Builder created successfully - validation happens at build() time
}

/// Test that server state enum has expected variants
#[test]
fn test_server_state_variants() {
    // Just verify the states exist and can be compared
    assert_ne!(ServerState::Initialized, ServerState::Ready);
    assert_ne!(ServerState::Preprocessing, ServerState::Computing);
    assert_eq!(ServerState::Ready, ServerState::Ready);
}

// ============================================================================
// Client Builder Tests - Configuration Validation
// ============================================================================

/// Test client builder configuration
#[test]
fn test_client_builder_configuration() {
    use std::time::Duration;

    let builder = StoffelClient::builder()
        .with_servers(&["localhost:19200", "localhost:19201"])
        .client_id(12345)
        .connection_timeout(Duration::from_secs(30))
        .computation_timeout(Duration::from_secs(120));

    // Builder created successfully - connection happens at connect() time
    let _ = builder;
}

/// Test client state enum variants
#[test]
fn test_client_state_variants() {
    assert_ne!(ClientState::Connected, ClientState::Disconnected);
    assert_ne!(ClientState::Submitting, ClientState::Computing);
    assert_eq!(ClientState::Connected, ClientState::Connected);
}

// ============================================================================
// MPC Configuration Validation Tests
// ============================================================================

/// Test MPC configuration validation helper
#[test]
fn test_mpc_config_validation() {
    // Valid configurations
    assert!(validate_mpc_config(5, 1).is_ok(), "5 parties, t=1 is valid");
    assert!(validate_mpc_config(9, 2).is_ok(), "9 parties, t=2 is valid");

    // Invalid configurations
    assert!(
        validate_mpc_config(4, 1).is_err(),
        "4 parties, t=1 is invalid (needs 5)"
    );
    assert!(
        validate_mpc_config(7, 2).is_err(),
        "7 parties, t=2 is invalid (needs 9)"
    );
}

/// Test peer address generation
#[test]
fn test_peer_address_generation() {
    let base_port = get_test_port_base();
    let all_peers = generate_peer_addresses(5, base_port);

    assert_eq!(all_peers.len(), 5);
    for (i, (id, addr)) in all_peers.iter().enumerate() {
        assert_eq!(*id, i);
        assert!(addr.contains(&format!("{}", base_port + i as u16)));
    }

    // Test excluding self
    let party_0_peers = generate_peers_for_party(0, 5, base_port);
    assert_eq!(party_0_peers.len(), 4);
    assert!(party_0_peers.iter().all(|(id, _)| *id != 0));
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test with minimum valid configuration (5 parties, threshold 1)
#[test]
fn test_minimum_valid_configuration() {
    let result = Stoffel::compile(TEST_PROGRAM_ADD)
        .unwrap()
        .parties(5)
        .threshold(1)
        .build();

    assert!(
        result.is_ok(),
        "Minimum valid config should work: {:?}",
        result.err()
    );
}

/// Test program listing functions
#[test]
fn test_program_list_functions() {
    let runtime = compile_addition_program().expect("Compile failed");
    let program = runtime.program();

    // The program should have at least a main function
    let bytecode = program.bytecode();
    assert!(!bytecode.is_empty(), "Bytecode should exist");

    // Use LoadedProgram to list functions
    use stoffel_rust_sdk::vm::LoadedProgram;
    let loaded = LoadedProgram::from_bytecode(bytecode.to_vec());
    let functions = loaded.list_functions().expect("Should list functions");

    assert!(!functions.is_empty(), "Should have at least one function");
    assert!(
        functions.iter().any(|f| f.name == "main"),
        "Should have main function"
    );
}

// ============================================================================
// E2E Test - Full HoneyBadger Workflow (SLOW - Ignored by default)
// ============================================================================

/// Full end-to-end test: 42 + 100 = 142
///
/// This test sets up a complete MPC network, runs preprocessing,
/// connects a client, and verifies the computation result.
///
/// # Note
///
/// This test is ignored by default because it requires:
/// - Network setup (~5-10 seconds)
/// - Preprocessing (~20-30 seconds)
/// - Full protocol execution (~10-20 seconds)
///
/// Run with: `cargo test --test honeybadger_integration -- --ignored`
#[tokio::test]
#[ignore]
async fn test_honeybadger_mpc_e2e_addition() {
    // Initialize crypto provider
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .ok(); // Ignore if already installed

    let n_parties = DEFAULT_N_PARTIES;
    let threshold = DEFAULT_THRESHOLD;
    let base_port = get_test_port_base();
    let instance_id: u64 = rand::random();

    println!("E2E Test Configuration:");
    println!("  Parties: {}, Threshold: {}", n_parties, threshold);
    println!("  Base port: {}", base_port);
    println!("  Instance ID: {}", instance_id);

    // Compile program
    let runtime = compile_addition_program().expect("Compile failed");
    let program = runtime.program().clone();

    // Calculate preprocessing start time (20 seconds from now)
    let preprocessing_start = calculate_preprocessing_start_time(20);

    // Start servers
    let peer_addrs = generate_peer_addresses(n_parties, base_port);
    let mut server_handles = Vec::new();

    for party_id in 0..n_parties {
        let bind_addr = format!("0.0.0.0:{}", base_port + party_id as u16);
        let peers: Vec<(usize, &str)> = peer_addrs
            .iter()
            .filter(|(id, _)| *id != party_id)
            .map(|(id, addr)| (*id, addr.as_str()))
            .collect();

        let program_clone = program.clone();

        let server = Stoffel::server(party_id)
            .bind(&bind_addr)
            .with_peers(&peers)
            .with_program(program_clone)
            .with_preprocessing(DEFAULT_N_TRIPLES, DEFAULT_N_RANDOM_SHARES)
            .with_instance_id(instance_id)
            .with_preprocessing_start_time(preprocessing_start)
            .build()
            .expect(&format!("Failed to build server {}", party_id));

        let handle = tokio::spawn(async move {
            if let Err(e) = server.start().await {
                eprintln!("Server {} failed to start: {}", party_id, e);
                return;
            }
            if let Err(e) = server.run_forever().await {
                // This is expected when the test ends
                if !format!("{}", e).contains("shutdown") {
                    eprintln!("Server {} error: {}", party_id, e);
                }
            }
        });

        server_handles.push(handle);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    // Wait for preprocessing
    println!("Waiting for preprocessing (~40 seconds)...");
    tokio::time::sleep(std::time::Duration::from_secs(40)).await;

    // Connect client
    let server_addrs: Vec<&str> = peer_addrs.iter().map(|(_, addr)| addr.as_str()).collect();
    let client = StoffelClient::builder()
        .with_servers(&server_addrs)
        .connect()
        .await
        .expect("Client connection failed");

    println!("Client connected: {} parties, threshold {}", client.n_parties(), client.threshold());

    // Run computation
    let inputs = vec![42_i64, 100_i64];
    let expected = inputs.iter().sum::<i64>();

    println!("Running computation: {} + {} = ?", inputs[0], inputs[1]);

    let result = client.run(&inputs).await.expect("Computation failed");

    println!("Result: {:?}", result);

    // Verify
    assert!(
        verify_addition_result(&result, inputs[0], inputs[1]),
        "Expected {}, got {:?}",
        expected,
        result
    );

    println!("SUCCESS: {} + {} = {}", inputs[0], inputs[1], result[0]);
}

/// Test preprocessing completes (basic sanity check for network)
///
/// This test verifies that servers can start, connect, and begin
/// preprocessing without timing out.
#[tokio::test]
#[ignore]
async fn test_preprocessing_completion() {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .ok();

    let n_parties = DEFAULT_N_PARTIES;
    let base_port = get_test_port_base();
    let instance_id: u64 = rand::random();

    let runtime = compile_addition_program().expect("Compile failed");
    let program = runtime.program().clone();
    let preprocessing_start = calculate_preprocessing_start_time(15);

    let peer_addrs = generate_peer_addresses(n_parties, base_port);
    let mut servers = Vec::new();

    for party_id in 0..n_parties {
        let bind_addr = format!("0.0.0.0:{}", base_port + party_id as u16);
        let peers: Vec<(usize, &str)> = peer_addrs
            .iter()
            .filter(|(id, _)| *id != party_id)
            .map(|(id, addr)| (*id, addr.as_str()))
            .collect();

        let server = Stoffel::server(party_id)
            .bind(&bind_addr)
            .with_peers(&peers)
            .with_program(program.clone())
            .with_preprocessing(DEFAULT_N_TRIPLES, DEFAULT_N_RANDOM_SHARES)
            .with_instance_id(instance_id)
            .with_preprocessing_start_time(preprocessing_start)
            .build()
            .expect(&format!("Failed to build server {}", party_id));

        servers.push(server);
    }

    // Start all servers
    for (party_id, server) in servers.iter().enumerate() {
        server.start().await.expect(&format!("Server {} failed to start", party_id));
    }

    // Wait for preprocessing to complete
    let timeout = std::time::Duration::from_secs(60);
    let start = std::time::Instant::now();

    // Check first server reaches Ready state
    while servers[0].state() != ServerState::Ready {
        if start.elapsed() > timeout {
            panic!(
                "Preprocessing timed out after {:?}. State: {:?}",
                timeout,
                servers[0].state()
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    println!("Preprocessing completed successfully in {:?}", start.elapsed());
}
