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
/// This function is used internally by `VM::run_bytecode()` and MPC server implementations
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
    /// Secret-shared value (share type, share data bytes)
    /// Used in MPC computations where the result is still in shared form
    Share(ShareType, Vec<u8>),
}

/// Type of a secret share
#[derive(Debug, Clone, PartialEq)]
pub enum ShareType {
    /// Secret integer with specified bit length
    SecretInt { bit_length: usize },
    /// Secret fixed-point with precision (k = total bits, f = fractional bits)
    SecretFixedPoint { k: usize, f: usize },
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
pub(crate) fn convert_vm_value_to_sdk_value(vm_value: stoffel_vm_types::core_types::Value) -> Value {
    use stoffel_vm_types::core_types::Value as VMValue;
    use stoffel_vm_types::core_types::ShareType as VMShareType;

    match vm_value {
        VMValue::I64(i) => Value::Int(i),
        VMValue::Float(f) => {
            // Float is now an F64 wrapper - extract the inner value
            Value::Float(f.value())
        },
        VMValue::Bool(b) => Value::Bool(b),
        VMValue::String(s) => Value::String(s),
        VMValue::Object(_id) => Value::Object(HashMap::new()), // TODO: Extract object data
        VMValue::Array(_id) => Value::Array(vec![]), // TODO: Extract array data
        VMValue::Unit => Value::Unit,
        VMValue::Share(st, data) => {
            // Convert VM ShareType to SDK ShareType
            let sdk_st = match st {
                VMShareType::SecretInt { bit_length } => ShareType::SecretInt { bit_length },
                VMShareType::SecretFixedPoint { precision } => ShareType::SecretFixedPoint {
                    k: precision.k(),
                    f: precision.f(),
                },
            };
            Value::Share(sdk_st, data)
        },
        _ => Value::Unit, // For other types, default to Unit
    }
}

/// Convert SDK Value to StoffelVM Value
fn convert_sdk_value_to_vm_value(sdk_value: Value) -> stoffel_vm_types::core_types::Value {
    use stoffel_vm_types::core_types::Value as VMValue;
    use stoffel_vm_types::core_types::ShareType as VMShareType;
    use stoffel_vm_types::core_types::F64;
    use stoffelmpc_mpc::common::types::fixed::FixedPointPrecision;

    match sdk_value {
        Value::Int(i) => VMValue::I64(i),
        Value::Float(f) => {
            // Float is now an F64 wrapper
            VMValue::Float(F64::new(f))
        },
        Value::Bool(b) => VMValue::Bool(b),
        Value::String(s) => VMValue::String(s),
        Value::Unit => VMValue::Unit,
        // For complex types, we'd need to serialize them properly
        Value::Object(_) => VMValue::Unit, // TODO: Implement object conversion
        Value::Array(_) => VMValue::Unit, // TODO: Implement array conversion
        Value::Share(st, data) => {
            // Convert SDK ShareType to VM ShareType
            let vm_st = match st {
                ShareType::SecretInt { bit_length } => VMShareType::SecretInt { bit_length },
                ShareType::SecretFixedPoint { k, f } => VMShareType::SecretFixedPoint {
                    precision: FixedPointPrecision::new(k, f),
                },
            };
            VMValue::Share(vm_st, data)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stoffel_vm_types::core_types::Value as VMValue;
    use stoffel_vm_types::core_types::ShareType as VMShareType;
    use stoffel_vm_types::core_types::F64;
    use stoffelmpc_mpc::common::types::fixed::FixedPointPrecision;

    // =========================================================================
    // Value Conversion Tests
    // =========================================================================

    /// Test converting VM I64 to SDK Int
    #[test]
    fn test_convert_vm_int_to_sdk() {
        let vm_value = VMValue::I64(42);
        let sdk_value = convert_vm_value_to_sdk_value(vm_value);

        assert_eq!(sdk_value, Value::Int(42));
        assert_eq!(sdk_value.as_int(), Some(42));
    }

    /// Test converting VM Float to SDK Float
    #[test]
    fn test_convert_vm_float_to_sdk() {
        let vm_value = VMValue::Float(F64::new(3.14));
        let sdk_value = convert_vm_value_to_sdk_value(vm_value);

        match sdk_value {
            Value::Float(f) => assert!((f - 3.14).abs() < 0.001),
            _ => panic!("Expected Float value"),
        }
    }

    /// Test converting VM Bool to SDK Bool
    #[test]
    fn test_convert_vm_bool_to_sdk() {
        let vm_true = VMValue::Bool(true);
        let vm_false = VMValue::Bool(false);

        assert_eq!(convert_vm_value_to_sdk_value(vm_true), Value::Bool(true));
        assert_eq!(convert_vm_value_to_sdk_value(vm_false), Value::Bool(false));
    }

    /// Test converting VM String to SDK String
    #[test]
    fn test_convert_vm_string_to_sdk() {
        let vm_value = VMValue::String("hello".to_string());
        let sdk_value = convert_vm_value_to_sdk_value(vm_value);

        assert_eq!(sdk_value, Value::String("hello".to_string()));
        assert_eq!(sdk_value.as_string(), Some("hello"));
    }

    /// Test converting VM Unit to SDK Unit
    #[test]
    fn test_convert_vm_unit_to_sdk() {
        let vm_value = VMValue::Unit;
        let sdk_value = convert_vm_value_to_sdk_value(vm_value);

        assert_eq!(sdk_value, Value::Unit);
        assert!(sdk_value.is_unit());
    }

    /// Test converting SDK Int to VM I64
    #[test]
    fn test_convert_sdk_int_to_vm() {
        let sdk_value = Value::Int(100);
        let vm_value = convert_sdk_value_to_vm_value(sdk_value);

        match vm_value {
            VMValue::I64(i) => assert_eq!(i, 100),
            _ => panic!("Expected I64 value"),
        }
    }

    /// Test converting SDK Float to VM Float
    #[test]
    fn test_convert_sdk_float_to_vm() {
        let sdk_value = Value::Float(2.71828);
        let vm_value = convert_sdk_value_to_vm_value(sdk_value);

        match vm_value {
            VMValue::Float(f) => assert!((f.value() - 2.71828).abs() < 0.00001),
            _ => panic!("Expected Float value"),
        }
    }

    // =========================================================================
    // Share Type Tests
    // =========================================================================

    /// Test ShareType::SecretInt enum conversion
    #[test]
    fn test_share_type_secret_int() {
        let share_type = ShareType::SecretInt { bit_length: 64 };

        match share_type {
            ShareType::SecretInt { bit_length } => assert_eq!(bit_length, 64),
            _ => panic!("Expected SecretInt"),
        }
    }

    /// Test ShareType::SecretFixedPoint enum conversion
    #[test]
    fn test_share_type_secret_fixed_point() {
        let share_type = ShareType::SecretFixedPoint { k: 64, f: 32 };

        match share_type {
            ShareType::SecretFixedPoint { k, f } => {
                assert_eq!(k, 64);
                assert_eq!(f, 32);
            }
            _ => panic!("Expected SecretFixedPoint"),
        }
    }

    /// Test VM Share value round-trip conversion
    #[test]
    fn test_share_roundtrip() {
        // Create a VM Share with SecretInt type
        let share_data = vec![0x01, 0x02, 0x03, 0x04];
        let vm_share = VMValue::Share(
            VMShareType::SecretInt { bit_length: 64 },
            share_data.clone()
        );

        // Convert VM -> SDK
        let sdk_value = convert_vm_value_to_sdk_value(vm_share);

        // Verify SDK value
        match &sdk_value {
            Value::Share(st, data) => {
                match st {
                    ShareType::SecretInt { bit_length } => assert_eq!(*bit_length, 64),
                    _ => panic!("Expected SecretInt"),
                }
                assert_eq!(data, &share_data);
            }
            _ => panic!("Expected Share value"),
        }

        // Convert SDK -> VM
        let vm_back = convert_sdk_value_to_vm_value(sdk_value);

        // Verify round-trip
        match vm_back {
            VMValue::Share(st, data) => {
                match st {
                    VMShareType::SecretInt { bit_length } => assert_eq!(bit_length, 64),
                    _ => panic!("Expected SecretInt"),
                }
                assert_eq!(data, share_data);
            }
            _ => panic!("Expected Share value after round-trip"),
        }
    }

    /// Test FixedPoint share round-trip conversion
    #[test]
    fn test_fixed_point_share_roundtrip() {
        let share_data = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let vm_share = VMValue::Share(
            VMShareType::SecretFixedPoint {
                precision: FixedPointPrecision::new(64, 32)
            },
            share_data.clone()
        );

        // Convert VM -> SDK
        let sdk_value = convert_vm_value_to_sdk_value(vm_share);

        match &sdk_value {
            Value::Share(ShareType::SecretFixedPoint { k, f }, data) => {
                assert_eq!(*k, 64);
                assert_eq!(*f, 32);
                assert_eq!(data, &share_data);
            }
            _ => panic!("Expected SecretFixedPoint Share"),
        }
    }

    // =========================================================================
    // Value Accessor Tests
    // =========================================================================

    /// Test Value::as_int() accessor
    #[test]
    fn test_value_as_int() {
        assert_eq!(Value::Int(42).as_int(), Some(42));
        assert_eq!(Value::Float(3.14).as_int(), None);
        assert_eq!(Value::String("hello".to_string()).as_int(), None);
    }

    /// Test Value::as_float() accessor
    #[test]
    fn test_value_as_float() {
        assert_eq!(Value::Float(3.14).as_float(), Some(3.14));
        assert_eq!(Value::Int(42).as_float(), None);
    }

    /// Test Value::as_bool() accessor
    #[test]
    fn test_value_as_bool() {
        assert_eq!(Value::Bool(true).as_bool(), Some(true));
        assert_eq!(Value::Bool(false).as_bool(), Some(false));
        assert_eq!(Value::Int(1).as_bool(), None);
    }

    /// Test Value::as_string() accessor
    #[test]
    fn test_value_as_string() {
        assert_eq!(Value::String("test".to_string()).as_string(), Some("test"));
        assert_eq!(Value::Int(42).as_string(), None);
    }

    /// Test Value::is_unit() method
    #[test]
    fn test_value_is_unit() {
        assert!(Value::Unit.is_unit());
        assert!(!Value::Int(0).is_unit());
        assert!(!Value::Bool(false).is_unit());
    }

    // =========================================================================
    // VM Integration Tests
    // =========================================================================

    /// Test VM can execute a simple program
    #[test]
    fn test_vm_execute_simple() {
        let source = "main main() -> int64:\n  return 42\n";

        // Compile the program
        let compiler = crate::compiler::Compiler::new();
        let bytecode = compiler.compile_source(source).expect("Compilation failed");

        // Execute with VM
        let vm = VM::new();
        let result = vm.run_bytecode(&bytecode, "main");

        assert!(result.is_ok(), "VM execution failed: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Int(42));
    }

    /// Test LoadedProgram::from_bytecode constructor
    #[test]
    fn test_loaded_program_from_bytecode() {
        let source = "main main() -> int64:\n  return 100\n";

        let compiler = crate::compiler::Compiler::new();
        let bytecode = compiler.compile_source(source).expect("Compilation failed");

        let loaded = LoadedProgram::from_bytecode(bytecode);
        let result = loaded.execute("main");

        assert!(result.is_ok(), "Execution failed: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Int(100));
    }

    /// Test LoadedProgram::list_functions
    #[test]
    fn test_loaded_program_list_functions() {
        let source = "main main() -> int64:\n  return 1\n";

        let compiler = crate::compiler::Compiler::new();
        let bytecode = compiler.compile_source(source).expect("Compilation failed");

        let loaded = LoadedProgram::from_bytecode(bytecode);
        let functions = loaded.list_functions().expect("Failed to list functions");

        assert!(!functions.is_empty(), "Should have at least one function");
        assert!(
            functions.iter().any(|f| f.name == "main"),
            "Should contain 'main' function"
        );
    }
}
