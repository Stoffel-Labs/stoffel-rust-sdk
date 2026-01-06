//! Simple MPC Client Example
//!
//! This demonstrates how an app developer integrates MPC using the MPCaaS API.
//! The developer only needs to know: connect to servers, submit inputs, get output.
//!
//! # One-Liner Usage
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::prelude::*;
//!
//! let result = run(
//!     &["localhost:19200", "localhost:19201", "localhost:19202", "localhost:19203"],
//!     &[42, 100]  // My inputs (as many as the program requires)
//! ).await?;
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

    // Example 1: Connect to servers and run computation
    println!("Connecting to MPC network...");

    match connect(servers).await {
        Ok(mpc) => {
            println!("Connected! (n={}, t={}, client_id={})",
                mpc.n_parties(), mpc.threshold(), mpc.client_id());
            println!("\nSubmitting inputs and waiting for result...");

            match mpc.run(inputs).await {
                Ok(result) => {
                    println!("\n=== Computation Complete ===");
                    println!("Result: {:?}", result);
                }
                Err(e) => {
                    println!("\nComputation error: {}", e);
                    println!("(This is expected if servers don't send ComputationComplete yet)");
                }
            }
        }
        Err(e) => {
            println!("Connection failed: {}", e);
            println!("\nMake sure the servers are running:");
            println!("  cargo run --example mpcaas_server -- --party-id 0 --port 19200");
            println!("  cargo run --example mpcaas_server -- --party-id 1 --port 19201");
            println!("  cargo run --example mpcaas_server -- --party-id 2 --port 19202");
            println!("  cargo run --example mpcaas_server -- --party-id 3 --port 19203");
        }
    }

    println!("\n=== Example Complete ===");

    Ok(())
}
