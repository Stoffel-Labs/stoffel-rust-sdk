# Progress - Agent C: Core API & Builder Pattern (RFC-001)

## Assignment
- **RFCs**: RFC-001 (Core API & Builder Pattern)
- **Files owned**: `src/lib.rs`, `src/prelude.rs`, `src/runtime.rs`
- **Goal**: Rewrite Stoffel entry point, builder pattern, StoffelRuntime per RFC-001 spec

## Log

### 2026-03-15 - Implementation Complete

**Status: DONE**

#### Changes Made

1. **Created `src/runtime.rs`**
   - `StoffelRuntime` struct with `program`, `n_parties`, `threshold`, `instance_id`, `network_config`
   - Methods: `program()`, `mpc_config()`, `network_config()`, `execute_local()`, `execute_local_function()`
   - TODO stubs for `client()`, `server()`, `node()` (network agent)

2. **Rewrote `src/lib.rs`**
   - `Stoffel` builder matching RFC-001
   - Entry points: `compile(source)`, `compile_file(path: impl AsRef<Path>)`, `load(bytecode: &[u8])`
   - Config: `parties()`, `threshold()`, `instance_id()`, `with_inputs()`, `optimize()`, `network_config()`, `network_config_file()`
   - Build: `build()` with validation (parties >= 4, n >= 3t+1), `execute_local()`, `execute_local_function()`
   - Removed: `ProtocolType`, `ShareType`, `mpc_types`, old `StoffelRuntime` inline definition
   - Removed module declarations: `client`, `server`, `session`, `stoffel_mpc`, `mpcaas`, `mpc_network`
   - Added module declarations: `types`, `config`, `runtime`, `backend`, `consensus`, `coordinator`, `observability`
   - Internal `resolve_bytecode()` helper eliminates duplication

3. **Rewrote `src/prelude.rs`**
   - Exports: `Stoffel`, `StoffelRuntime`, `Error`, `Result`, `Program`, `Compiler`, `VM`, `Value`, `PartyId`, `ClientId`, `ComputationId`
   - Removed old type exports (`ProtocolType`, `ShareType`, `MPCClient`, `MPCServer`, `MPCNode`, etc.)

4. **Files preserved as-is**: `compiler.rs`, `vm.rs`, `program.rs`, `types.rs`, `error.rs`

#### Design Decisions
- `load()` takes `&[u8]` (more ergonomic than `Vec<u8>`)
- `compile_file()` takes `impl AsRef<Path>` for flexibility
- MPC config uses plain fields (no config module dependency yet)
- Removed generic `F` parameter from builder/runtime
- Private `resolve_bytecode()` avoids duplicating compilation logic
