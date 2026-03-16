//! Example 4: Deploy a Private Voting System
//!
//! Compiles a voting program and scaffolds production deployment artifacts
//! (Docker configs, server binaries, coordinator setup).
//!
//! Run: cargo run --example deploy_scaffold

use stoffel_rust_sdk::prelude::*;

fn main() -> Result<()> {
    let source = include_str!("deploy_scaffold/voting.stfl");

    // --- Compile the voting program ---
    println!("=== Compiling voting program ===");
    let bytecode = Compiler::new().compile_source(source)?;
    println!("Compiled to {} bytes", bytecode.len());

    // --- Build network and scaffold ---
    let output_dir = std::env::temp_dir().join("stoffel_voting_deploy");

    // Clean up any previous run
    let _ = std::fs::remove_dir_all(&output_dir);

    println!("\n=== Scaffolding deployment to {} ===", output_dir.display());
    StoffelNetwork::builder()
        .program(bytecode)
        .parties(5)
        .threshold(1)
        .build()?
        .scaffold(&output_dir)?;

    // --- List generated files ---
    println!("\nGenerated files:");
    for entry in walkdir(&output_dir, "") {
        println!("  {}", entry);
    }

    // --- Print deployment instructions ---
    println!("\n=== Deployment Instructions ===");
    println!("1. cd {}", output_dir.display());
    println!("2. docker-compose up");
    println!("3. Clients submit votes through the coordinator's input masking protocol");
    println!("4. Only the aggregate tally is revealed — individual votes stay secret");

    // Clean up
    let _ = std::fs::remove_dir_all(&output_dir);
    println!("\n(Cleaned up temp directory)");
    Ok(())
}

/// Simple recursive directory listing
fn walkdir(dir: &std::path::Path, prefix: &str) -> Vec<String> {
    let mut entries = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(dir) {
        let mut items: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
        items.sort_by_key(|e| e.file_name());
        for entry in items {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = format!("{}{}", prefix, name);
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                entries.push(format!("{}/", path));
                entries.extend(walkdir(&entry.path(), &format!("{}  ", prefix)));
            } else {
                entries.push(path);
            }
        }
    }
    entries
}
