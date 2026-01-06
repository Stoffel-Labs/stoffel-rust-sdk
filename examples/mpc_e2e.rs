//! MPC End-to-End Example
//!
//! This example demonstrates the unified `.execute()` API that runs full MPC
//! with QUIC networking. The same API works for both localhost development
//! and production deployment.
//!
//! Run with: cargo run --example mpc_e2e

use stoffel_rust_sdk::prelude::*;

/// Inline Stoffel program: multiply two secret inputs
const STOFFEL_SOURCE: &str = r#"
main main(a: secret int64, b: secret int64) -> secret int64:
    return a * b
"#;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize TLS crypto provider (required for QUIC)
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Set up tracing for visibility into MPC execution
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    println!("=== Stoffel MPC E2E Example ===\n");

    // =========================================================================
    // Example 1: Simple one-liner execution (localhost defaults)
    // =========================================================================
    println!("Example 1: Simple one-liner execution");
    println!("  Using localhost defaults (127.0.0.1:19200-19204)");
    println!("  Inputs: 7 * 6 = 42\n");

    let result = Stoffel::compile(STOFFEL_SOURCE)?
        .parties(5)
        .threshold(1)
        .with_inputs(vec![vec![7], vec![6]])  // Client 1: 7, Client 2: 6
        .execute()
        .await?;

    println!("Result: {:?}\n", result);

    // =========================================================================
    // Example 2: With explicit configuration
    // =========================================================================
    println!("Example 2: With explicit configuration");

    let config = MPCExecutionConfig::new(5, 1)
        .with_instance_id(42)
        .with_preprocessing(5, 12);

    println!("  Parties: {}", config.n_parties);
    println!("  Threshold: {}", config.threshold);
    println!("  Instance ID: {}", config.instance_id);
    println!("  Addresses: {:?}\n", config.addresses);

    // =========================================================================
    // Example 3: Production deployment (commented out - would use real addresses)
    // =========================================================================
    println!("Example 3: Production deployment pattern");
    println!("  (Commented out - demonstrates API only)\n");

    /*
    // Production: just provide different addresses
    let result = Stoffel::compile_file("program.stfl")?
        .parties(5)
        .threshold(1)
        .with_addresses(vec![
            "192.168.1.10:19200",
            "192.168.1.11:19200",
            "192.168.1.12:19200",
            "192.168.1.13:19200",
            "192.168.1.14:19200",
        ])
        .with_inputs(vec![vec![7], vec![6]])
        .execute()
        .await?;
    */

    // =========================================================================
    // Example 4: Local execution (no MPC, for testing program logic)
    // =========================================================================
    println!("Example 4: Local execution (no MPC)");
    println!("  For quick testing of program logic\n");

    let local_result = Stoffel::compile("main main() -> int64:\n  return 42")?
        .execute_local()?;

    println!("Local result: {:?}\n", local_result);

    // =========================================================================
    // Summary
    // =========================================================================
    println!("=== Summary ===");
    println!("The unified .execute() API:");
    println!("  - Uses localhost by default (127.0.0.1:19200+i)");
    println!("  - For production, just add .with_addresses([...])");
    println!("  - Full QUIC networking with HoneyBadger protocol");
    println!("  - Byzantine fault tolerant (n >= 3t+1)\n");

    Ok(())
}
