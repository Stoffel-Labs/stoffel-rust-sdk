//! High-level API for the StoffelVM runtime
//!
//! This module provides a friendly interface to execute Stoffel bytecode
//! on the StoffelVM, with support for custom functions, FFI, and debugging.
//!
//! # Examples
//!
//! ```rust,no_run
//! use stoffel_rust_sdk::vm::{VM, Value};
//!
//! # fn main() -> stoffel_rust_sdk::Result<()> {
//! // Create a VM and execute bytecode
//! let vm = VM::new();
//! let result = vm.load_bytecode("program.stfb")?
//!     .execute("main")?;
//!
//! // Execute with arguments
//! let result = vm.load_bytecode("program.stfb")?
//!     .execute_with_args("add", vec![Value::Int(5), Value::Int(10)])?;
//!
//! println!("Result: {:?}", result);
//! # Ok(())
//! # }
//! ```

use crate::{Error, Result};
use std::collections::HashMap;
use stoffel_vm::core_vm::VirtualMachine;

// Re-export the raw VirtualMachine for advanced use cases
pub use stoffel_vm::core_vm::VirtualMachine as RawVirtualMachine;

/// Load bytecode into a VirtualMachine instance
///
/// This is a shared utility function that deserializes Stoffel bytecode
/// and registers all functions into the provided VM instance.
///
/// # Arguments
/// * `vm` - Mutable reference to the VirtualMachine to load functions into
/// * `bytecode` - Compiled Stoffel bytecode bytes (.stfl format)
///
/// # Returns
/// * `Ok(())` - Bytecode loaded successfully
/// * `Err(_)` - Failed to parse or register bytecode
///
/// # Note
/// This function is used internally by both `VM::run_bytecode()` and `MPCServer::load_bytecode()`
/// to avoid code duplication while allowing different VM lifecycle management.
pub(crate) fn load_bytecode_into_vm(vm: &mut VirtualMachine, bytecode: &[u8]) -> Result<()> {
    use std::io::Cursor;
    use stoffel_vm_types::compiled_binary::CompiledBinary;

    // Deserialize bytecode into CompiledBinary
    let mut cursor = Cursor::new(bytecode);
    let compiled_binary = CompiledBinary::deserialize(&mut cursor)
        .map_err(|e| Error::RuntimeError(format!("Failed to deserialize bytecode: {:?}", e)))?;

    // Convert to VM functions and register them
    let vm_functions = compiled_binary.to_vm_functions();
    for func in vm_functions {
        vm.register_function(func);
    }

    Ok(())
}

/// The Stoffel Virtual Machine runtime
pub struct VM {
    inner: VirtualMachine,
    debug: bool,
}

impl VM {
    /// Create a new VM instance
    pub fn new() -> Self {
        Self {
            inner: VirtualMachine::new(),
            debug: false,
        }
    }

    /// Create a new VM with debug mode enabled
    pub fn with_debug() -> Self {
        Self {
            inner: VirtualMachine::new(),
            debug: true,
        }
    }

    /// Load bytecode from a file
    pub fn load_bytecode(&self, path: &str) -> Result<LoadedProgram> {
        let bytecode = std::fs::read(path)
            .map_err(|e| Error::IoError(e))?;
        Ok(LoadedProgram {
            bytecode,
            debug: self.debug,
        })
    }

    /// Run bytecode directly
    pub fn run_bytecode(&self, bytecode: &[u8], entry_function: &str) -> Result<Value> {
        // Create a new VM instance (we need mutable access)
        let mut vm = VirtualMachine::new();

        // Load bytecode into the VM
        load_bytecode_into_vm(&mut vm, bytecode)?;

        // Execute the entry function
        vm.execute(entry_function)
            .map(|v| convert_vm_value_to_sdk_value(v))
            .map_err(|e| Error::RuntimeError(format!("Execution failed: {}", e)))
    }

    /// Register a custom Rust function for FFI
    pub fn register_function<F>(&mut self, _name: &str, _function: F) -> Result<()>
    where
        F: Fn(Vec<Value>) -> Result<Value> + 'static + Send + Sync,
    {
        // TODO: Wrap the SDK function and register with VM's FFI
        // This requires converting between SDK Values and VM Values
        Err(Error::RuntimeError(
            "FFI registration not yet fully implemented".to_string(),
        ))
    }
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

/// A loaded program ready for execution
pub struct LoadedProgram {
    bytecode: Vec<u8>,
    debug: bool,
}

impl LoadedProgram {
    /// Create a LoadedProgram from bytecode
    pub fn from_bytecode(bytecode: Vec<u8>) -> Self {
        Self {
            bytecode,
            debug: false,
        }
    }
}

impl LoadedProgram {
    /// Execute a function by name
    pub fn execute(&self, function_name: &str) -> Result<Value> {
        self.execute_with_args(function_name, vec![])
    }

    /// Execute a function with arguments
    pub fn execute_with_args(&self, function_name: &str, args: Vec<Value>) -> Result<Value> {
        // Deserialize bytecode
        use std::io::Cursor;
        use stoffel_vm_types::compiled_binary::CompiledBinary;

        let mut cursor = Cursor::new(&self.bytecode);
        let compiled_binary = CompiledBinary::deserialize(&mut cursor)
            .map_err(|e| Error::RuntimeError(format!("Failed to deserialize bytecode: {:?}", e)))?;

        // Convert to VM functions
        let vm_functions = compiled_binary.to_vm_functions();

        // Create VM instance
        let mut vm = VirtualMachine::new();

        // Register all functions
        for func in vm_functions {
            vm.register_function(func);
        }

        // Convert SDK args to VM args
        let vm_args: Vec<stoffel_vm_types::core_types::Value> = args
            .into_iter()
            .map(convert_sdk_value_to_vm_value)
            .collect();

        // Execute with args
        vm.execute_with_args(function_name, &vm_args)
            .map(|v| convert_vm_value_to_sdk_value(v))
            .map_err(|e| Error::RuntimeError(format!("Execution failed: {}", e)))
    }

    /// List all functions in the loaded program
    pub fn list_functions(&self) -> Result<Vec<FunctionInfo>> {
        // Deserialize bytecode
        use std::io::Cursor;
        use stoffel_vm_types::compiled_binary::CompiledBinary;

        let mut cursor = Cursor::new(&self.bytecode);
        let compiled_binary = CompiledBinary::deserialize(&mut cursor)
            .map_err(|e| Error::RuntimeError(format!("Failed to deserialize bytecode: {:?}", e)))?;

        // Extract function info
        let functions = compiled_binary.functions
            .iter()
            .map(|f| FunctionInfo {
                name: f.name.clone(),
                parameter_count: f.parameters.len(),
                register_count: f.register_count,
            })
            .collect();

        Ok(functions)
    }
}

/// Information about a function in the bytecode
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub parameter_count: usize,
    pub register_count: usize,
}

/// Runtime values in the StoffelVM
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Integer value
    Int(i64),
    /// Float value
    Float(f64),
    /// Boolean value
    Bool(bool),
    /// String value
    String(String),
    /// Object (key-value map)
    Object(HashMap<String, Value>),
    /// Array of values
    Array(Vec<Value>),
    /// Unit/null value
    Unit,
}

impl Value {
    /// Try to extract an integer
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Try to extract a float
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Try to extract a boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to extract a string
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// Check if value is unit
    pub fn is_unit(&self) -> bool {
        matches!(self, Value::Unit)
    }
}

/// Convert StoffelVM Value to SDK Value
fn convert_vm_value_to_sdk_value(vm_value: stoffel_vm_types::core_types::Value) -> Value {
    use stoffel_vm_types::core_types::Value as VMValue;

    match vm_value {
        VMValue::I64(i) => Value::Int(i),
        VMValue::Float(f) => {
            // F64 implements From<F64> for f64
            Value::Float(f64::from(f))
        },
        VMValue::Bool(b) => Value::Bool(b),
        VMValue::String(s) => Value::String(s),
        VMValue::Object(id) => Value::Object(HashMap::new()), // TODO: Extract object data
        VMValue::Array(id) => Value::Array(vec![]), // TODO: Extract array data
        VMValue::Unit => Value::Unit,
        _ => Value::Unit, // For other types, default to Unit
    }
}

/// Convert SDK Value to StoffelVM Value
fn convert_sdk_value_to_vm_value(sdk_value: Value) -> stoffel_vm_types::core_types::Value {
    use stoffel_vm_types::core_types::Value as VMValue;

    match sdk_value {
        Value::Int(i) => VMValue::I64(i),
        Value::Float(f) => {
            // F64 implements From<f64> for F64
            VMValue::Float(stoffel_vm_types::core_types::F64::from(f))
        },
        Value::Bool(b) => VMValue::Bool(b),
        Value::String(s) => VMValue::String(s),
        Value::Unit => VMValue::Unit,
        // For complex types, we'd need to serialize them properly
        Value::Object(_) => VMValue::Unit, // TODO: Implement object conversion
        Value::Array(_) => VMValue::Unit, // TODO: Implement array conversion
    }
}
