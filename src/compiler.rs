//! High-level API for compiling Stoffel source code
//!
//! This module provides a friendly interface to the Stoffel-Lang compiler,
//! abstracting away low-level details while providing flexible configuration.
//!
//! # Examples
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::compiler::Compiler;
//!
//! # fn main() -> stoffel_rust_sdk::Result<()> {
//! // Compile from source string
//! let bytecode = Compiler::new()
//!     .optimize(true)
//!     .compile_source("fn main() { println(\"Hello!\"); }")?;
//!
//! // Compile from file
//! let bytecode = Compiler::new()
//!     .optimize(true)
//!     .compile_file("program.stfl")?;
//!
//! // Get intermediate representation
//! let ir = Compiler::new()
//!     .print_ir(true)
//!     .compile_source("fn add(a: i32, b: i32) -> i32 { return a + b; }")?;
//! # Ok(())
//! # }
//! ```

use crate::{Error, Result};

/// High-level compiler for Stoffel source code
pub struct Compiler {
    optimize: bool,
    optimization_level: OptimizationLevel,
    print_ir: bool,
}

/// Optimization levels for the compiler
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationLevel {
    /// No optimization
    None,
    /// Basic optimization
    O1,
    /// Moderate optimization
    O2,
    /// Aggressive optimization
    O3,
}

impl Compiler {
    /// Create a new compiler with default settings
    pub fn new() -> Self {
        Self {
            optimize: false,
            optimization_level: OptimizationLevel::None,
            print_ir: false,
        }
    }

    /// Enable or disable optimization
    pub fn optimize(mut self, enable: bool) -> Self {
        self.optimize = enable;
        if enable && self.optimization_level == OptimizationLevel::None {
            self.optimization_level = OptimizationLevel::O2;
        }
        self
    }

    /// Set the optimization level
    pub fn optimization_level(mut self, level: OptimizationLevel) -> Self {
        self.optimization_level = level;
        self.optimize = level != OptimizationLevel::None;
        self
    }

    /// Print intermediate representation
    pub fn print_ir(mut self, enable: bool) -> Self {
        self.print_ir = enable;
        self
    }

    /// Compile source code from a string
    pub fn compile_source(&self, _source: &str) -> Result<Vec<u8>> {
        // Use stoffellang compiler
        let mut options = stoffellang::CompilerOptions::default();
        options.optimize = self.optimize;
        options.optimization_level = match self.optimization_level {
            OptimizationLevel::None => 0,
            OptimizationLevel::O1 => 1,
            OptimizationLevel::O2 => 2,
            OptimizationLevel::O3 => 3,
        };
        options.print_ir = self.print_ir;

        let compiled = stoffellang::compile(_source, "source.stfl", &options)
            .map_err(|errors| {
                let error_msg = errors
                    .iter()
                    .map(|e| format!("{}", e))
                    .collect::<Vec<_>>()
                    .join("\n");
                Error::CompilationError(error_msg)
            })?;

        // Convert compiled program to binary format
        let binary = stoffellang::convert_to_binary(&compiled);

        // Serialize to bytes
        let mut buffer = Vec::new();
        binary.serialize(&mut buffer)
            .map_err(|e| Error::CompilationError(format!("Failed to serialize binary: {:?}", e)))?;

        Ok(buffer)
    }

    /// Compile source code from a file
    pub fn compile_file(&self, path: &str) -> Result<Vec<u8>> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| Error::IoError(e))?;
        self.compile_source(&source)
    }

    /// Compile and return both bytecode and IR (if enabled)
    pub fn compile_with_ir(&self, _source: &str) -> Result<CompilationOutput> {
        // Compile with IR printing enabled
        let mut options = stoffellang::CompilerOptions::default();
        options.optimize = self.optimize;
        options.optimization_level = match self.optimization_level {
            OptimizationLevel::None => 0,
            OptimizationLevel::O1 => 1,
            OptimizationLevel::O2 => 2,
            OptimizationLevel::O3 => 3,
        };
        options.print_ir = true; // Always enable for this method

        let compiled = stoffellang::compile(_source, "source.stfl", &options)
            .map_err(|errors| {
                let error_msg = errors
                    .iter()
                    .map(|e| format!("{}", e))
                    .collect::<Vec<_>>()
                    .join("\n");
                Error::CompilationError(error_msg)
            })?;

        let binary = stoffellang::convert_to_binary(&compiled);

        // Serialize to bytes
        let mut bytecode = Vec::new();
        binary.serialize(&mut bytecode)
            .map_err(|e| Error::CompilationError(format!("Failed to serialize binary: {:?}", e)))?;

        // Note: The IR is printed to stdout by stoffellang, not captured
        // In the future, we could modify stoffellang to return the IR
        Ok(CompilationOutput {
            bytecode,
            ir: Some("IR printed to stdout (see console)".to_string()),
        })
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Output from compilation including bytecode and optional IR
pub struct CompilationOutput {
    /// The compiled bytecode
    pub bytecode: Vec<u8>,
    /// Intermediate representation (if requested)
    pub ir: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that ClientStore.take_share() compiles correctly
    #[test]
    fn test_compile_clientstore_take_share() {
        let source = r#"
main main() -> secret int64:
  var a: secret int64 = ClientStore.take_share(0, 0)
  return a
        "#;

        let compiler = Compiler::new();
        let result = compiler.compile_source(source);

        assert!(result.is_ok(), "ClientStore.take_share() should compile: {:?}", result.err());

        let bytecode = result.unwrap();
        assert!(!bytecode.is_empty(), "Bytecode should not be empty");
    }

    /// Test compiling a program with multiple ClientStore inputs
    #[test]
    fn test_compile_clientstore_multiple_inputs() {
        let source = r#"
main main() -> secret int64:
  var a: secret int64 = ClientStore.take_share(0, 0)
  var b: secret int64 = ClientStore.take_share(0, 1)
  var c: secret int64 = ClientStore.take_share(0, 2)
  return a + b + c
        "#;

        let compiler = Compiler::new();
        let result = compiler.compile_source(source);

        assert!(result.is_ok(), "Multiple ClientStore inputs should compile: {:?}", result.err());
    }

    /// Test compiling secret integer addition
    #[test]
    fn test_compile_secret_addition() {
        let source = r#"
main main() -> secret int64:
  var a: secret int64 = ClientStore.take_share(0, 0)
  var b: secret int64 = ClientStore.take_share(0, 1)
  return a + b
        "#;

        let compiler = Compiler::new();
        let result = compiler.compile_source(source);

        assert!(result.is_ok(), "Secret addition should compile: {:?}", result.err());
    }

    /// Test that a simple non-MPC program compiles
    #[test]
    fn test_compile_simple_program() {
        let source = "main main() -> int64:\n  return 42\n";

        let compiler = Compiler::new();
        let result = compiler.compile_source(source);

        assert!(result.is_ok(), "Simple program should compile: {:?}", result.err());
    }

    /// Test compiler optimization levels
    #[test]
    fn test_compile_with_optimization() {
        let source = "main main() -> int64:\n  return 42\n";

        let compiler = Compiler::new()
            .optimization_level(OptimizationLevel::O2);

        let result = compiler.compile_source(source);
        assert!(result.is_ok(), "Optimized compilation should succeed");
    }

    /// Test compiler default settings
    #[test]
    fn test_compiler_default() {
        let compiler = Compiler::default();
        let source = "main main() -> int64:\n  return 1\n";

        let result = compiler.compile_source(source);
        assert!(result.is_ok(), "Default compiler should work");
    }

    /// Test that invalid syntax produces a CompilationError
    #[test]
    fn test_compile_invalid_syntax() {
        let source = "this is not valid stoffel code!!!";

        let compiler = Compiler::new();
        let result = compiler.compile_source(source);

        assert!(result.is_err(), "Invalid syntax should produce an error");
        match result.err() {
            Some(Error::CompilationError(_)) => { /* expected */ }
            other => panic!("Expected CompilationError, got {:?}", other),
        }
    }

    /// Test enable optimization flag
    #[test]
    fn test_compiler_optimize_flag() {
        let compiler = Compiler::new().optimize(true);
        let source = "main main() -> int64:\n  return 42\n";

        let result = compiler.compile_source(source);
        assert!(result.is_ok(), "Optimize flag should not break compilation");
    }
}
