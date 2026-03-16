//! Utility: Generate .stfb bytecode files from .stfl source files.
//!
//! Run: cargo run --example generate_stfb

use stoffel_rust_sdk::compiler::Compiler;

fn main() {
    let examples = vec![
        ("examples/hello_stoffel/hello.stfl", "examples/hello_stoffel/hello.stfb"),
        ("examples/secret_arithmetic/secret_mul.stfl", "examples/secret_arithmetic/secret_mul.stfb"),
        ("examples/network_builder/salary.stfl", "examples/network_builder/salary.stfb"),
        ("examples/deploy_scaffold/voting.stfl", "examples/deploy_scaffold/voting.stfb"),
    ];

    let compiler = Compiler::new();

    for (src, dst) in &examples {
        let source = std::fs::read_to_string(src)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", src, e));
        match compiler.compile_source(&source) {
            Ok(bytecode) => {
                std::fs::write(dst, &bytecode)
                    .unwrap_or_else(|e| panic!("Failed to write {}: {}", dst, e));
                println!("OK  {} -> {} ({} bytes)", src, dst, bytecode.len());
            }
            Err(e) => {
                eprintln!("ERR {} : {}", src, e);
                std::process::exit(1);
            }
        }
    }
    println!("\nAll .stfb files generated successfully.");
}
