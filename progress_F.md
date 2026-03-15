# Progress - Agent F: Client API & On-Chain Coordination (RFC-002, RFC-007)

## Assignment
- **RFCs**: RFC-002 (StoffelClient API), RFC-007 (On-Chain Coordination)
- **Files owned**: `src/client.rs` (rewrite), `src/coordinator/mod.rs`, `src/coordinator/onchain.rs`, `src/coordinator/offchain.rs`
- **Goal**: Implement StoffelClient, ClientBuilder, ComputationHandle, Round state machine, OnChainCoordinator, OffChainCoordinator

## Log

<!-- Agent F appends entries below this line -->

### 2026-03-15 - Initial implementation complete

**RFC-002 (StoffelClient API) -- `src/client.rs`**
- `ClientState` enum: Disconnected, Connecting, Connected, AwaitingConsensus, Ready, Computing
- `ClientBuilder`: fluent builder with `.server()`, `.servers()`, `.client_id()`, `.timeout()`, `.retry()`, `.connect()`
- `StoffelClient`: connected client with `.run()`, `.run_function()`, `.submit()`, `.disconnect()`
- `ComputationStatus` enum: Pending, InputsSubmitted, Computing, Completed, Failed, Cancelled
- `ComputationHandle`: async handle with `.status()`, `.computation_id()`, `.await_result()`, `.cancel()`
- All async methods are real `async fn` (tokio); stubs return `Err(Error::Computation("not yet implemented"))`
- Unit tests for builder defaults, server addition, connect validation, submit stub, disconnect

**RFC-007 (On-Chain Coordination) -- `src/coordinator/`**
- `coordinator/mod.rs`: `Round` enum (7 states matching Solidity contract), `MaskIndex`, `CoordinatorEvent`
  - `Round::next()` for state machine transitions
  - `Round::is_client_round()` / `Round::is_server_round()` helpers
- `coordinator/offchain.rs`: `OffChainCoordinator` with thread-safe (Arc<Mutex>) state
  - `.current_round()`, `.advance_round()`, `.set_round()`, `.reserve_input_mask()`
  - Tests: round progression, set override, mask index monotonicity, thread safety
- `coordinator/onchain.rs`: `OnChainCoordinator` placeholder
  - `.new(contract_address, computation_id)`, `.current_round()`, `.await_round()`
  - Stub implementations returning not-yet-implemented errors

**Note**: `lib.rs` does not yet declare `pub mod client;` -- the team lead will add it during integration.
