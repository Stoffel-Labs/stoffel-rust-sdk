//! Simple MPC Client Example
//!
//! This demonstrates how an app developer integrates MPC using the MPCaaS API.
//! The developer only needs to know: connect to servers, submit inputs, get output.
//!
//! # One-Liner Usage
//!
//! ```rust,no_run
//! let result = stoffel::run(
//!     &["localhost:19200", "localhost:19201", "localhost:19202"],
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
    tracing_subscriber::fmt::init();

    println!("=== MPCaaS Client Example ===\n");

    // Example 1: One-liner (simplest API)
    println!("Example 1: One-liner API");
    println!("-------------------------");

    // In production, this would connect to actual servers and run the computation
    // For now, we demonstrate the API surface
    // Note: HoneyBadger requires n >= 3t+1, so 4 servers for threshold 1
    let servers = &["127.0.0.1:19200", "127.0.0.1:19201", "127.0.0.1:19202", "127.0.0.1:19203"];
    let inputs = &[42_i64, 100_i64];

    println!("Servers: {:?}", servers);
    println!("Inputs: {:?}", inputs);

    // This is how simple the API is:
    // let result = stoffel::run(servers, inputs).await?;
    // println!("Result: {:?}", result);

    println!("\n(Server connection not implemented yet - showing API surface)\n");

    // Example 2: Connection reuse (for multiple computations)
    println!("Example 2: Connection Reuse");
    println!("----------------------------");

    // Connect once, run multiple computations
    // let mpc = stoffel::connect(servers).await?;
    //
    // // First computation
    // let result1 = mpc.run(&[10, 20]).await?;
    // println!("Computation 1: {:?}", result1);
    //
    // // Second computation
    // let result2 = mpc.run(&[30, 40]).await?;
    // println!("Computation 2: {:?}", result2);

    println!("(Connection reuse API demonstrated but not connected)\n");

    // Example 3: Async/Non-blocking (for UI apps)
    println!("Example 3: Async/Non-blocking");
    println!("------------------------------");

    // Submit without waiting, do other work, then get result
    // let mpc = stoffel::connect(servers).await?;
    // let handle = mpc.submit(&[42, 100]).await?;
    //
    // // Do other work while computation runs...
    // println!("Computation submitted, doing other work...");
    //
    // // Option A: Wait for result
    // let result = handle.await_result().await?;
    //
    // // Option B: Poll without blocking
    // loop {
    //     if let Some(result) = handle.try_result() {
    //         println!("Done: {:?}", result?);
    //         break;
    //     }
    //     tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    // }

    println!("(Async API demonstrated but not connected)\n");

    // Example 4: Variable number of inputs
    println!("Example 4: Variable Inputs");
    println!("--------------------------");
    println!("The number of inputs depends on the MPC program:");
    println!("  - Single input:  stoffel::run(servers, &[42]).await?");
    println!("  - Two inputs:    stoffel::run(servers, &[bid, max_price]).await?");
    println!("  - Many inputs:   stoffel::run(servers, &[100, 200, 300, 400]).await?");

    println!("\n=== Example Complete ===");

    Ok(())
}
