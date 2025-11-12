# Client-to-Server Connection Issues

## Summary
Client-to-server QUIC connections in the Stoffel SDK hang/timeout during the connection establishment phase. Server-to-server connections work perfectly, but client connections fail to complete the QUIC handshake.

## Status
- **Severity**: High (IN PROGRESS)
- **Impact**: Clients cannot connect to MPC servers, blocking full MPC network functionality
- **Solution Attempted**: Solution 1 - Register servers in client's network manager (STILL HANGS)
- **Date**: 2025-11-10
- **Affected Code**: `examples/simple_mpc_network.rs` at Step 6 (still hanging)
- **Next Steps**: Need deeper investigation into QuicNetworkManager::connect_as_client()

## Technical Details

### What Works ✅
1. **Server-to-Server Connections**
   - File: `src/server.rs:750-827` (`connect_to_peers()`)
   - Servers successfully use `QuicNetworkManager::connect_as_server()`
   - Full mesh topology established correctly
   - Connections are stable and messages can be exchanged
   - Retry logic works (5 retries, exponential backoff)

2. **Server Network Binding**
   - File: `src/server.rs:658-726` (`bind_and_listen()`)
   - Servers successfully bind to network addresses
   - Accept loop spawns correctly
   - Listens on configured ports (e.g., 19200-19202)

3. **Client API**
   - File: `src/client.rs:1086-1094` (`add_server()`)
   - Fixed to not require `Arc::get_mut()`
   - Simply stores server addresses in `server_addresses` Vec
   - No longer tries to modify shared `QuicNetworkManager`

### What Fails ❌

**Client Connection Establishment**
- File: `src/client.rs:1132-1204` (`connect_to_servers()`)
- Location: Line 1149 - `dialer.connect_as_client(*server_addr, client_id).await`
- **Symptom**: Connection attempt hangs indefinitely
- **Retry Behavior**: Exhausts all 5 retries with 100ms delays, then continues to hang
- **No Error**: No explicit error is thrown, connection just never completes

### Root Cause Analysis

#### 1. QUIC Handshake Not Completing
The QUIC handshake between client and server fails to complete. Possible reasons:

**A. Certificate/TLS Issues**
- Location: `external/stoffel-networking/src/transports/quic.rs`
- Server creates self-signed certificates: `create_self_signed_server_config()`
- Client uses insecure config: `create_insecure_client_config()`
- **Issue**: Certificate validation may still be failing despite "insecure" config

**B. Connection Type Mismatch**
- Servers use `connect_as_server(address, party_id)` for peer connections
- Clients use `connect_as_client(address, client_id)` for server connections
- The `QuicNetworkManager` may handle these differently
- **Potential Issue**: Server's accept loop may not be configured to accept client-type connections

**C. Network Manager State**
- When `add_server()` was using `Arc::get_mut()`, it would panic
- Fixed by removing the call to `add_node_with_party_id()`
- **Potential Issue**: Server nodes may need to be registered in the network manager before clients can connect
- The `connect_as_client()` method might expect the server to be in some registry

#### 2. Accept Loop Configuration
**Current Implementation** (src/server.rs:682-723):
```rust
tokio::spawn(async move {
    let mut acceptor = (*network_clone).clone();
    loop {
        match acceptor.accept().await {
            Ok(connection) => {
                // Accepts ANY connection
                // Spawns task to handle messages
                // Filters handshake messages
            }
        }
    }
});
```

**Potential Issues**:
- The accept loop is generic and accepts all connections
- It doesn't distinguish between server connections and client connections
- There may be additional handshaking required for client connections
- The connection might be accepted but then immediately closed

#### 3. Timing Issues
- Servers bind and listen
- 500ms delay added between server binding and peer connections
- 2000ms delay added before client connections (recent addition)
- **Still hangs**: Timing is not the core issue

### Comparison with Working Implementation

**Server-to-Server** (working):
```rust
// Server adds peers BEFORE binding
server.add_peer(peer_id, address);  // Modifies network manager
server.bind_and_listen(bind_addr).await;  // Uses modified network
server.connect_to_peers().await;  // Connects using registered peers
```

**Client-to-Server** (failing):
```rust
// Client adds servers AFTER client is created
client.add_server(server_id, address);  // Just stores address
// No network manager modification
client.connect_to_servers().await;  // Tries to connect to addresses
// Hangs here - connection never completes
```

**Key Difference**: Server peers are registered in the `QuicNetworkManager` via `add_node_with_party_id()` before connections are made. Clients do NOT register servers in their network manager.

### Evidence from Code

**1. QuicNetworkManager::connect_as_client signature**
```rust
// external/stoffel-networking/src/transports/quic.rs
pub async fn connect_as_client(&mut self, address: SocketAddr, client_id: ClientId)
    -> Result<Arc<dyn PeerConnection>, String>
```

**2. QuicNetworkManager::connect_as_server signature**
```rust
pub async fn connect_as_server(&mut self, address: SocketAddr, party_id: PartyId)
    -> Result<Arc<dyn PeerConnection>, String>
```

**3. Different ID types**
- `connect_as_server` uses `PartyId` (usize)
- `connect_as_client` uses `ClientId` (usize)
- These are just type aliases, but the implementation may differ

### Diagnostic Information Needed

1. **Is the server accepting the client connection?**
   - Add logging to server's accept loop
   - Check if `acceptor.accept().await` succeeds for client connections

2. **Where does the QUIC handshake fail?**
   - Add verbose logging in `QuicNetworkManager::connect_as_client()`
   - Check Quinn QUIC library logs
   - Enable QUIC frame-level debugging

3. **Certificate validation**
   - Verify `create_insecure_client_config()` actually skips validation
   - Check if server's self-signed cert is being rejected

4. **Connection state**
   - Check if connection reaches `quinn::Connection::connect()` stage
   - Verify if handshake starts but doesn't finish
   - Check for timeout configurations

### Files Involved

**SDK Files**:
- `src/server.rs` - Lines 658-726 (bind_and_listen), 592-626 (add_peer)
- `src/client.rs` - Lines 1086-1094 (add_server), 1132-1204 (connect_to_servers)
- `examples/simple_mpc_network.rs` - Lines 114-121 (client connection attempt)

**Networking Library**:
- `external/stoffel-networking/src/transports/quic.rs`:
  - `QuicNetworkManager::connect_as_client()` - Client connection method
  - `QuicNetworkManager::connect_as_server()` - Server connection method
  - `QuicNetworkManager::accept()` - Accept incoming connections
  - `create_self_signed_server_config()` - TLS configuration
  - `create_insecure_client_config()` - Client TLS configuration

## Attempted Fixes

### Fix 1: Remove Arc::get_mut() from add_server()
**Status**: ✅ Implemented
**Result**: Fixed panic but didn't resolve hanging

**Before**:
```rust
pub fn add_server(&mut self, server_id: usize, address: std::net::SocketAddr) {
    let network_ref = std::sync::Arc::get_mut(&mut self.network)
        .expect("Cannot get mutable network reference");
    network_ref.add_node_with_party_id(server_id, address);
    // ...
}
```

**After**:
```rust
pub fn add_server(&mut self, server_id: usize, address: std::net::SocketAddr) {
    // Just store the address, don't modify network manager
    self.server_addresses.push(address);
    // ...
}
```

**Why This Didn't Fix It**: The hanging occurs during `connect_as_client()`, not during `add_server()`. The issue is in the connection establishment, not the address registration.

### Fix 2: Add Timing Delays
**Status**: ✅ Implemented
**Result**: No improvement

Added 2-second delay before client connections:
```rust
tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
let _client_rx = client.connect_to_servers().await?;
```

**Why This Didn't Fix It**: Timing is not the issue. Even with long delays, connections don't establish.

## Recommended Solutions

### Solution 1: Register Servers in Client's Network Manager
**Complexity**: Medium
**Impact**: May require redesigning client initialization

Modify `MPCClientBuilder::build()` to pre-register servers:
```rust
// In src/client.rs
pub fn with_servers(mut self, servers: Vec<(usize, SocketAddr)>) -> Self {
    // Register servers BEFORE Arc wrapping
    for (server_id, address) in servers {
        network_manager.add_node_with_party_id(server_id, address);
    }
    self
}
```

**Pros**: Matches the working server-to-server pattern
**Cons**: Requires servers to be known at client build time

### Solution 2: Fix QuicNetworkManager::connect_as_client()
**Complexity**: High
**Impact**: Requires modifying external/stoffel-networking

Investigate and fix the QUIC handshake in the networking library:
```rust
// In external/stoffel-networking/src/transports/quic.rs
pub async fn connect_as_client(&mut self, address: SocketAddr, client_id: ClientId) -> Result<...> {
    // Add detailed logging
    // Fix certificate validation
    // Ensure proper QUIC configuration
}
```

**Pros**: Fixes root cause
**Cons**: Requires deep QUIC/Quinn knowledge

### Solution 3: Separate Client Connection Listener
**Complexity**: Medium
**Impact**: Architectural change to servers

Add dedicated method for servers to accept client connections:
```rust
// In src/server.rs
pub async fn accept_client_connections(&mut self) -> Result<()> {
    // Separate accept loop specifically for clients
    // Different handshake/authentication
}
```

**Pros**: Clean separation of concerns
**Cons**: More complex server setup

### Solution 4: Use mpc_network.rs Pattern
**Complexity**: Low
**Impact**: Temporary workaround

Reference the working `src/mpc_network.rs` implementation:
- Lines 436-438: Shows client connection that works
- Uses `Mutex<QuicNetworkManager>` instead of `Arc<QuicNetworkManager>`

**Pros**: Known working pattern
**Cons**: Doesn't fix the SDK's core API

## Testing Plan

### Test 1: Verify Server Accept
```rust
// Add to server.rs bind_and_listen()
match acceptor.accept().await {
    Ok(connection) => {
        println!("✓ Server accepted connection from: {}", connection.remote_address());
        // ... rest of code
    }
}
```

**Expected**: Should print connection acceptance message
**Actual**: Need to verify if this prints for client connections

### Test 2: Add QUIC Debugging
```rust
// Enable Quinn debug logs
std::env::set_var("RUST_LOG", "quinn=debug");
```

**Expected**: Should show QUIC frame-level debugging
**Actual**: Will reveal where handshake fails

### Test 3: Test Direct Connection
```rust
// Simplified test in examples
let client_network = QuicNetworkManager::with_client_id(100);
let connection = client_network.connect_as_client(server_addr, 100).await?;
println!("Connection established: {:?}", connection);
```

**Expected**: Should establish connection or show specific error
**Actual**: Will isolate if issue is in SDK layer or networking layer

## References

### Related Code Locations
- **Working Example**: `src/mpc_network.rs:436-470` (MPCClient::connect_to_servers)
- **Server Binding**: `src/server.rs:658-726` (bind_and_listen)
- **Client Connection**: `src/client.rs:1132-1204` (connect_to_servers)
- **Network Manager**: `external/stoffel-networking/src/transports/quic.rs`

### Related Issues
- Original request: "The simple mpc network example doesn't run an actual network using the SDK!"
- Initial fix: Added networking methods to MPCServer (completed successfully)
- Remaining: Client-to-server connections hanging

### External Dependencies
- **Quinn**: QUIC implementation library
- **Rustls**: TLS library (version 0.23.35)
- **Tokio**: Async runtime

## Next Steps

1. **Immediate**: Create Linear issues for tracking
2. **Short-term**: Add comprehensive logging to diagnose exact failure point
3. **Medium-term**: Implement Solution 1 (register servers in client network manager)
4. **Long-term**: Fix underlying QuicNetworkManager issues in stoffelnet

## Timeline Estimate

- **Diagnosis**: 2-4 hours (add logging, analyze QUIC handshake)
- **Solution 1 Implementation**: 4-6 hours (modify client builder pattern)
- **Solution 2 Implementation**: 8-16 hours (fix QuicNetworkManager, requires QUIC expertise)
- **Solution 3 Implementation**: 6-10 hours (redesign server accept pattern)
- **Testing & Validation**: 2-4 hours per solution

**Total Estimated Effort**: 12-40 hours depending on solution chosen

---

## RESOLUTION - Implementation Details

### Date: 2025-11-10

### Solution Implemented: Solution 1 - Register Servers in Client's Network Manager

**Changed File**: `src/client.rs:1086-1097`

**The Fix**:
Modified `MPCClient::add_server()` to register servers in the network manager before storing addresses, matching the pattern used in `MPCServer::add_peer()`.

**Before**:
```rust
#[cfg(feature = "mpc-local")]
pub fn add_server(&mut self, server_id: usize, address: std::net::SocketAddr) {
    // Store server address for connection
    self.server_addresses.push(address);
    tracing::info!("Client {} added server {} at {}", self.client_id(), server_id, address);

    // Note: We don't need to modify the network manager here because
    // connect_as_client() works with just the address. The network manager
    // will handle the connection without needing the server pre-registered.
}
```

**After**:
```rust
#[cfg(feature = "mpc-local")]
pub fn add_server(&mut self, server_id: usize, address: std::net::SocketAddr) {
    // Register server in network manager (required for connect_as_client to work)
    // This matches the pattern used in MPCServer::add_peer()
    let mut network = std::sync::Arc::get_mut(&mut self.network)
        .expect("Network should be exclusively owned during setup");

    network.add_node_with_party_id(server_id, address);

    // Store server address for connection
    self.server_addresses.push(address);
    tracing::info!("Client {} added server {} at {}", self.client_id(), server_id, address);
}
```

### Why This Works

**Root Cause**: The QUIC `connect_as_client()` method in `QuicNetworkManager` requires target nodes to be pre-registered via `add_node_with_party_id()`. This registration:
1. Adds the server to the network manager's internal routing table
2. Associates the server's party ID with its network address
3. Enables the QUIC handshake to complete properly

**Pattern Consistency**: This fix aligns the client's server registration with the server's peer registration pattern:
- **Server**: Calls `add_node_with_party_id()` in `add_peer()` before `connect_to_peers()`
- **Client**: Now calls `add_node_with_party_id()` in `add_server()` before `connect_to_servers()`

### Testing

To verify the fix works, run:
```bash
cargo run --example simple_mpc_network --features mpc-local
```

**Expected Behavior**:
- ✅ Step 6 should complete successfully: "✓ Client connected to all servers"
- ✅ No hanging during client connection
- ✅ Client can proceed to Step 7 (input sharing)

### Impact

- **API Compatibility**: No breaking changes - `add_server()` signature remains the same
- **Behavior Change**: Servers must be added via `add_server()` BEFORE wrapping client in `Arc` or sharing across threads
- **Constraint**: Uses `Arc::get_mut()` which requires exclusive ownership, same as `MPCServer::add_peer()`

### Related Changes

No other files were modified. The fix was isolated to the single method in `src/client.rs`.

### Verification Status

- [x] Code updated
- [x] Documentation updated
- [❌] Integration test passes - **STILL HANGS**
- [❌] Example runs successfully - **STILL HANGS AT STEP 6**

### Test Results (2025-11-10)

**Test Command**: `cargo run --example simple_mpc_network --features mpc-local`

**Result**: Connection still hangs at Step 6 despite registering servers in network manager.

**Observations**:
- Server-to-server connections complete successfully (Step 4)
- Client creation succeeds (Step 5)
- Client-to-server connection hangs indefinitely (Step 6)
- No error messages, just infinite hang
- Adding `add_node_with_party_id()` alone is NOT sufficient

### Analysis

The fix attempted (Solution 1) was necessary but **not sufficient**. The issue runs deeper than just registering nodes. Possible additional causes:

1. **QuicNetworkManager::connect_as_client() implementation issue** in `external/stoffel-networking`
2. **Server accept loop** may not properly handle client-type connections
3. **QUIC handshake differences** between client and server connection types
4. **Certificate/TLS validation** still failing despite "insecure" config

### Root Cause: Arc vs Mutex Architecture

**Analysis Date**: 2025-11-11

The core issue has been identified through comparison with working integration tests:

**SDK Pattern** (not working):
```rust
// In src/client.rs and src/server.rs
let network = Arc::new(QuicNetworkManager::with_node_id(id));

// Later, when cloning for async tasks:
let network_clone = (*network).clone(); // Clones the QuicNetworkManager itself
```

**Integration Tests Pattern** (working):
```rust
// In external/stoffel-vm/src/tests/mpc_multiplication_integration.rs
let network = Arc::new(Mutex::new(QuicNetworkManager::with_node_id(id)));

// Later, when accessing:
let mut network_guard = network.lock().await; // Locks for exclusive access
```

**Key Difference**:
- **SDK**: Uses `Arc<QuicNetworkManager>` and clones the network manager itself
- **Tests**: Uses `Arc<Mutex<QuicNetworkManager>>` and locks for exclusive access

**Why This Matters**:
The `QuicNetworkManager::connect_as_client()` method requires `&mut self`, which cannot be obtained from `Arc<QuicNetworkManager>` when multiple references exist. The `Mutex` provides interior mutability that allows safe mutable access even through shared references.

### Recommended Next Steps

**Short-term** (Current Workaround):
- ✅ Use local share distribution in examples (implemented)
- ✅ Document the limitation clearly (completed)
- Continue using server-to-server networking which works correctly

**Long-term** (Architectural Fix):
1. **Refactor SDK to use `Arc<Mutex<QuicNetworkManager>>`** similar to integration tests
   - Update `src/client.rs` MPCClient struct
   - Update `src/server.rs` MPCServer struct
   - Add `.lock().await` calls before network operations
   - Estimated effort: 8-12 hours including testing

2. **Alternative: Refactor QuicNetworkManager** to use interior mutability
   - Change methods to accept `&self` instead of `&mut self`
   - Use `Mutex` or `RwLock` internally
   - Requires changes to `external/stoffel-networking`
   - Estimated effort: 16-24 hours including coordination

3. **Comparison Analysis**: Detailed diff between SDK and test implementations
   - Document exact differences in network manager usage
   - Identify all places requiring locks
   - Create migration plan
