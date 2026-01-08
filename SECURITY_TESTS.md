# Security Vulnerability Test Cases

This document describes test cases that demonstrate the security vulnerabilities found in the MPCaaS implementation.

## How to Run

```bash
# Run all security tests (some are ignored by default to prevent OOM)
cargo test --test security_vulnerabilities

# Run specific tests
cargo test --test security_vulnerabilities test_dos_large_message_size -- --nocapture
cargo test --test security_vulnerabilities test_integer_conversion -- --nocapture

# Run dangerous tests (may cause OOM - use with caution)
cargo test --test security_vulnerabilities -- --ignored --nocapture
```

## Test Cases Overview

### 1. DoS via Unbounded Message Size (CRITICAL)

**Test**: `test_dos_massive_message_size` (ignored by default)
**File**: `tests/security_vulnerabilities.rs`

**Vulnerability**: An attacker can craft a message with the length field set to `u32::MAX` (4GB), causing:
- Memory exhaustion (OOM)
- System freeze
- Service unavailability

**Attack Scenario**:
```
Attacker -> Server: [Magic: "MPCS"][Version: 1][Length: 0xFFFFFFFF][Payload: 100 bytes]
Server: Attempts to allocate 4GB buffer -> OOM/Crash
```

**Current Behavior**: 
- Protocol reads length as u32 (up to 4GB)
- No validation of message size before allocation
- Results in OOM or indefinite wait for data

**Expected Behavior**: 
- Reject messages over reasonable limit (e.g., 10MB)
- Return error: `Network("Message too large: 4294967295 bytes")`

**Fix**:
```rust
const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB
if length > MAX_MESSAGE_SIZE {
    return Err(Error::Network(format!("Message too large: {} bytes", length)));
}
```

---

### 2. DoS via Unbounded Buffer Growth (CRITICAL)

**Test**: `test_dos_unbounded_buffer_growth` (ignored by default)
**File**: `tests/security_vulnerabilities.rs`

**Vulnerability**: `MessageBuffer::append()` has no size limit, allowing attackers to:
- Send repeated small chunks without completing messages
- Exhaust server memory
- Keep connections alive indefinitely

**Attack Scenario**:
```
Loop forever:
    Attacker -> Server: [10KB garbage data]
    Server: Appends to buffer (no limit)
    
After 10,000 iterations: 100MB consumed
After 100,000 iterations: 1GB consumed -> OOM
```

**Current Behavior**:
```rust
pub fn append(&mut self, data: &[u8]) {
    self.buffer.extend_from_slice(data);  // No limit!
}
```

**Expected Behavior**:
- Buffer enforces maximum size (e.g., 50MB)
- Rejects additional data once limit reached
- Clears buffer on protocol errors

**Fix**:
```rust
const MAX_BUFFER_SIZE: usize = 50 * 1024 * 1024; // 50MB

pub fn append(&mut self, data: &[u8]) -> Result<()> {
    if self.buffer.len() + data.len() > MAX_BUFFER_SIZE {
        self.buffer.clear(); // Drop malicious data
        return Err(Error::Network("Buffer overflow protection triggered".into()));
    }
    self.buffer.extend_from_slice(data);
    Ok(())
}
```

---

### 3. Buffer Growth with Incomplete Messages

**Test**: `test_dos_buffer_growth_incomplete_messages`
**File**: `tests/security_vulnerabilities.rs`

**Vulnerability**: Attacker sends valid message headers but never completes payloads.

**Attack Scenario**:
```
For i in 1..1000:
    Attacker -> Server: [Magic][Version][Length: 1MB]
    // Don't send the 1MB payload
    // Buffer accumulates 9 bytes * 1000 = 9KB of headers
    // But waiting for 1000MB of payload data
```

**Current Behavior**: Buffer accumulates headers indefinitely

**Expected Behavior**: 
- Timeout incomplete messages
- Clear buffer after reasonable wait time
- Enforce maximum number of incomplete messages

---

### 4. Integer Conversion Silent Failure (HIGH)

**Test**: `test_integer_conversion_silent_failure`
**File**: `tests/security_vulnerabilities.rs`

**Vulnerability**: Field-to-integer conversion can fail silently, returning 0 instead of error.

**Location**: `src/secret_sharing.rs:111-124`

**Problem Code**:
```rust
let secret_value = secret_bytes.parse::<i64>()
    .unwrap_or_else(|_| {
        // VULNERABILITY: Silent failure returns 0!
        0
    });
```

**Attack Impact**:
- MPC computation produces incorrect result (0) instead of failing
- Security breach: Attacker can manipulate results
- Data corruption: Users receive wrong answers without knowing

**Test Scenario**:
```rust
// Share i64::MAX
let shares = SecretSharing::share_secret(i64::MAX, 5, 1);

// Reconstruct
let result = SecretSharing::reconstruct_secret(&shares, 5);

// WITHOUT FIX: Returns 0 (silent failure)
// WITH FIX: Returns i64::MAX or propagates error
assert_ne!(result, 0, "VULNERABILITY: Returned 0 instead of correct value!");
```

**Expected Behavior**:
- Conversion errors should propagate, not be swallowed
- Failed conversions should cause computation to fail loudly
- Better: Use proper field-to-integer conversion that can't fail

**Fix**:
```rust
// Option 1: Proper error propagation
let secret_value = field_to_i64(&secret_field)
    .map_err(|e| Error::Other(format!("Field conversion failed: {}", e)))?;

// Option 2: Use BigInt or proper field arithmetic
let secret_value = secret_field.into_bigint().to_i64()
    .ok_or_else(|| Error::Other("Field value out of i64 range".into()))?;
```

---

### 5. Negative Number Conversion

**Test**: `test_integer_conversion_negative_numbers`
**File**: `tests/security_vulnerabilities.rs`

**Vulnerability**: Negative numbers might lose sign information during field conversion.

**Test Scenario**:
```rust
let secret = -42;
let shares = SecretSharing::share_secret(secret, 5, 1);
let result = SecretSharing::reconstruct_secret(&shares, 5);

// Verify sign preserved
assert_eq!(result, -42, "Lost sign information!");
```

---

### 6. Mutex Poisoning Cascade (MEDIUM)

**Test**: Cannot easily test without causing panics, but documented here.

**Vulnerability**: Using `.lock().unwrap()` on mutexes causes cascading failures.

**Locations**:
- `src/mpcaas/server.rs:445` - `*self.state.lock().unwrap()`
- `src/mpcaas/handle.rs:98` - `self.cached_result.lock().unwrap()`

**Problem**:
```rust
pub fn state(&self) -> ServerState {
    *self.state.lock().unwrap()  // Panics if poisoned!
}
```

**Attack Scenario**:
1. Thread A acquires lock on `self.state`
2. Thread A panics while holding lock
3. Mutex becomes "poisoned"
4. Thread B calls `state()` -> unwrap() on poisoned mutex -> panic!
5. Cascading failures throughout the system

**Expected Behavior**:
- Handle poison error gracefully
- Log the poisoning event
- Either recover or return proper error

**Fix Options**:
```rust
// Option 1: Clear error message
pub fn state(&self) -> ServerState {
    *self.state.lock().expect("State mutex poisoned - server may be in inconsistent state")
}

// Option 2: Recover from poison
pub fn state(&self) -> ServerState {
    match self.state.lock() {
        Ok(guard) => *guard,
        Err(poisoned) => {
            tracing::error!("State mutex was poisoned, recovering");
            *poisoned.into_inner() // Use the data despite poisoning
        }
    }
}

// Option 3: Return Result
pub fn state(&self) -> Result<ServerState> {
    self.state.lock()
        .map(|guard| *guard)
        .map_err(|_| Error::Other("State mutex poisoned".into()))
}
```

---

### 7. Protocol Fuzzing

**Test**: `test_protocol_fuzzing_malformed_messages`
**File**: `tests/security_vulnerabilities.rs`

**Purpose**: Ensure protocol handles malformed messages without panicking.

**Test Cases**:
- Empty data
- Partial headers
- Wrong magic bytes
- Invalid version numbers  
- Random garbage
- Truncated messages

**Expected**: All should return errors, none should panic.

---

### 8. Performance Degradation

**Test**: `test_rapid_message_processing`
**File**: `tests/security_vulnerabilities.rs`

**Purpose**: Detect performance issues or memory leaks during sustained load.

**Metrics**:
- Messages processed per second
- Memory usage over time
- Response latency

**Expected**: Should maintain >1000 msg/sec for simple Ping messages.

---

## Running the Full Test Suite

1. **Safe tests** (won't cause OOM):
```bash
cargo test --test security_vulnerabilities
```

2. **Dangerous tests** (may cause OOM):
```bash
# Only run on systems with sufficient memory
cargo test --test security_vulnerabilities -- --ignored --nocapture
```

3. **Individual tests**:
```bash
cargo test --test security_vulnerabilities test_dos_large_message_size -- --nocapture
cargo test --test security_vulnerabilities test_integer_conversion_silent_failure -- --nocapture
cargo test --test security_vulnerabilities test_protocol_fuzzing -- --nocapture
```

## Expected Test Results (Before Fixes)

Most tests should **FAIL** or demonstrate vulnerabilities:

- ✅ `test_protocol_fuzzing_malformed_messages` - Should pass (basic validation works)
- ❌ `test_dos_large_message_size` - May accept 100MB message or wait indefinitely
- ❌ `test_dos_buffer_growth_incomplete_messages` - Buffer grows without limit
- ⚠️ `test_integer_conversion_silent_failure` - May silently return 0
- ⚠️ `test_integer_conversion_negative_numbers` - May lose sign information

## Expected Test Results (After Fixes)

All tests should **PASS**:

- ✅ All DoS tests reject oversized messages
- ✅ Buffer enforces size limits
- ✅ Integer conversions propagate errors
- ✅ Protocol handles all malformed input gracefully
- ✅ Performance remains acceptable under load

## Integration with CI/CD

Add to `.github/workflows/security.yml`:

```yaml
name: Security Tests

on: [push, pull_request]

jobs:
  security:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
      - name: Run security tests
        run: |
          cargo test --test security_vulnerabilities
          # Don't run --ignored tests in CI to prevent OOM
```

## References

- Original security review: PR comment #3724107708
- Protocol implementation: `src/mpcaas/protocol.rs`
- Secret sharing: `src/secret_sharing.rs`
- Server implementation: `src/mpcaas/server.rs`
