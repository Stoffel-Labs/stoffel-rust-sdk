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
