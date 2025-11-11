# Stoffel-Lang Example Programs

This directory contains simple Stoffel-Lang programs used in the SDK examples.

## multiply.stfl

A program that demonstrates **secure multiplication over secret shares** - the core functionality of MPC.

```stoffel
main main() -> secret int64:
  var x: secret int64 = 6
  var y: secret int64 = 7
  var result: secret int64 = x * y
  return result
```

**Expected Result:** 42 (computed securely over secret-shared values)

**Features:**
- **Secret-shared multiplication** - Variables are marked with `secret` type
- MPC computation on encrypted data
- Demonstrates how Stoffel-Lang handles secure computation
- Values remain encrypted throughout the computation
- Only the final result is revealed after reconstruction

## Stoffel-Lang Syntax Notes

- **Indentation:** Exactly 2 spaces per indentation level (required)
- **Main entry point:** Use `main main() -> type:` (note: "main" appears twice)
- **Function definitions:** Use `def` keyword followed by `:` (e.g., `def add(x: int64) -> int64:`)
- **Variable declarations:** Use `var` keyword (not `let`)
- **Type names:** Use full names (e.g., `int64`, `float`, `string`, not `i64`, `f64`)
- **No semicolons:** Statements don't require semicolons
- **Comments:** Use `#` for single-line comments
- **Secret types:** Use `secret` keyword before type to mark values as secret-shared
  - Example: `secret int64` declares a secret-shared 64-bit integer
  - Operations on secret types are performed using secure MPC protocols
  - Secret multiplication uses beaver triples for efficient computation
- **Secret functions:** Use `secret def` to mark entire functions as secret

## Usage

These programs are used by the SDK examples:
- `quick_start.rs` - Uses `multiply.stfl` to demonstrate compilation and execution
- `complete_workflow.rs` - Demonstrates the full workflow with inline Stoffel code

## Current Limitations

### Hardcoded Values vs Client Inputs

Currently, the program uses **hardcoded secret values** (6 and 7) rather than accepting inputs from MPC clients. While the SDK examples show creating MPC clients with inputs like `vec![42, 100, 200]`, these client inputs cannot yet be accessed within Stoffel programs.

**Status:** Support for using client inputs in Stoffel programs is tracked in [Linear issue STO-104](https://linear.app/stoffel-labs/issue/STO-104).

**Future workflow (once implemented):**
```stoffel
main main() -> secret int64:
  # Access client 0's first input
  var x: secret int64 = client_input(0, 0)
  # Access client 1's first input
  var y: secret int64 = client_input(1, 0)
  var result: secret int64 = x * y
  return result
```

### Compilation and Execution Status

✅ **Compilation:** The Stoffel-Lang compiler successfully compiles the multiplication program to bytecode (178 bytes).

⚠️ **Execution:** StoffelVM execution of secret-shared operations requires MPC engine configuration. When you run the examples, you'll see:
- ✓ Compilation successful (generates bytecode)
- ✗ Execution failed: MPC engine not configured

**Why this happens:**
- The bytecode is valid and ready to execute
- Secret operations (multiplication over `secret int64`) require:
  1. QUIC network manager for party communication
  2. HoneyBadger MPC engine with preprocessing
  3. Multiple parties coordinating over the network

**How to enable full execution:**
See the "Advanced: MPC Engine Configuration" section in the main README.md, or refer to the `stoffel-run` binary in the StoffelVM repository for a complete working example of distributed MPC execution.
