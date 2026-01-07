//! Simple MPC Client Example
//!
//! This demonstrates how an app developer integrates MPC using the MPCaaS API.
//! The developer only needs to know: connect to servers, submit inputs, get output.
//!
//! # Usage
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let client = StoffelClient::builder()
//!         .with_servers(&["localhost:19200", "localhost:19201", "localhost:19202", "localhost:19203"])
//!         .connect()
//!         .await?;
//!
//!     let result = client.run(&[42, 100]).await?;
//!     println!("Result: {:?}", result);
//!     Ok(())
//! }
//! ```
//!
//! # Running
//!
//! First, start the MPC servers (see mpcaas_server.rs example):
//! ```bash
//! # In separate terminals:
//! cargo run --example mpcaas_server -- --party-id 0 --port 19200
//! cargo run --example mpcaas_server -- --party-id 1 --port 19201
//! cargo run --example mpcaas_server -- --party-id 2 --port 19202
//! cargo run --example mpcaas_server -- --party-id 3 --port 19203
//! ```
//!
//! Then run this client:
//! ```bash
//! cargo run --example mpcaas_client
//! ```

use stoffel_rust_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize rustls crypto provider (required for QUIC)
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("stoffel_rust_sdk=info".parse().unwrap())
            .add_directive("stoffelnet=info".parse().unwrap()))
        .init();

    println!("=== MPCaaS Client Example ===\n");

    // HoneyBadger requires n >= 3t+1, so 4 servers for threshold 1
    let servers = &["127.0.0.1:19200", "127.0.0.1:19201", "127.0.0.1:19202", "127.0.0.1:19203"];
    let inputs = &[42_i64, 100_i64];

    println!("Servers: {:?}", servers);
    println!("Inputs: {:?}\n", inputs);

    // Connect to servers using the StoffelClient builder
    println!("Connecting to MPC network...");

    let client = match StoffelClient::builder()
        .with_servers(servers)
        .connect()
        .await
    {
        Ok(c) => c,
        Err(e) => {
            println!("Connection failed: {}", e);
            println!("\nMake sure the servers are running:");
            println!("  cargo run --example mpcaas_server -- --party-id 0 --port 19200");
            println!("  cargo run --example mpcaas_server -- --party-id 1 --port 19201");
            println!("  cargo run --example mpcaas_server -- --party-id 2 --port 19202");
            println!("  cargo run --example mpcaas_server -- --party-id 3 --port 19203");
            return Ok(());
        }
    };

    println!("Connected! (n={}, t={}, client_id={})",
        client.n_parties(), client.threshold(), client.client_id());
    println!("State: {:?}", client.state());
    println!("\nSubmitting inputs and waiting for result...");

    match client.run(inputs).await {
        Ok(result) => {
            println!("\n=== Computation Complete ===");
            println!("Result: {:?}", result);
        }
        Err(e) => {
            println!("\nComputation error: {}", e);
            println!("(This is expected if servers don't send ComputationComplete yet)");
        }
    }

    println!("\n=== Example Complete ===");

    Ok(())
}
