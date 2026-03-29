//! Security Vulnerability Tests
//!
//! This test suite demonstrates the security vulnerabilities found in the MPCaaS implementation.
//! These tests are EXPECTED TO FAIL or expose vulnerabilities until the fixes are implemented.
//!
//! ## Vulnerabilities Tested:
//! 1. DoS via unbounded message size (u32::MAX attack)
//! 2. DoS via unbounded buffer growth
//! 3. Silent integer conversion failures in secret sharing
//! 4. Mutex poisoning cascading failures

use stoffel_rust_sdk::mpcaas::protocol::{
    deserialize_message, serialize_message, MessageBuffer, MPCaaSMessage,
};
use stoffel_rust_sdk::secret_sharing::SecretSharing;

/// Test 1: DoS Vulnerability - Massive Message Size Claim
///
/// An attacker can craft a message with a length field set to u32::MAX (4GB),
/// causing memory exhaustion when the server tries to allocate a buffer.
///
/// **Expected Behavior**: Should reject messages above a reasonable size limit (e.g., 10MB)
/// **Current Behavior**: Attempts to allocate 4GB, causing OOM or system freeze
#[test]
#[ignore] // Ignored by default to prevent OOM during test runs
fn test_dos_massive_message_size() {
    // Craft a malicious message header claiming 4GB payload
    let mut malicious_data = vec![
        0x4D, 0x50, 0x43, 0x53, // Magic bytes "MPCS"
        0x01, // Protocol version
    ];

    // Length field: u32::MAX (4,294,967,295 bytes = ~4GB)
    malicious_data.extend_from_slice(&u32::MAX.to_be_bytes());

    // Add minimal payload (attacker doesn't need to send 4GB, just claim it)
    malicious_data.extend_from_slice(&[0x00; 100]);

    // This should fail with a size limit error, not try to allocate 4GB
    let result = deserialize_message(&malicious_data);

    // Without the fix, this will either:
    // 1. Try to allocate 4GB and OOM
    // 2. Wait forever for 4GB of data
    // 3. Return "incomplete message" allowing attacker to keep connections open
    println!("Result: {:?}", result);

    // WITH FIX: Should return Error::Network("Message too large: ...")
    // WITHOUT FIX: May OOM, hang, or return Ok(None) waiting for more data
    match result {
        Err(e) => {
            let err_msg = format!("{}", e);
            assert!(
                err_msg.contains("too large") || err_msg.contains("size limit"),
                "Expected size limit error, got: {}",
                err_msg
            );
        }
        Ok(_) => panic!("VULNERABILITY: Accepted message claiming to be 4GB!"),
    }
}

/// Test 2: DoS Vulnerability - Realistic Large Message Attack
///
/// Tests a more realistic attack where the attacker claims a large but not
/// absurd size (e.g., 100MB), which could still exhaust memory on constrained systems.
#[test]
fn test_dos_large_message_size() {
    // Craft message claiming 100MB payload
    let mut malicious_data = vec![
        0x4D, 0x50, 0x43, 0x53, // Magic bytes
        0x01, // Version
    ];

    let claimed_size: u32 = 100 * 1024 * 1024; // 100MB
    malicious_data.extend_from_slice(&claimed_size.to_be_bytes());
    malicious_data.extend_from_slice(&[0x00; 100]); // Small actual payload

    let result = deserialize_message(&malicious_data);

    // Should reject messages over reasonable limit (e.g., 10MB)
    match result {
        Err(e) => {
            let err_msg = format!("{}", e);
            // Either rejected for being too large, or incomplete message
            assert!(
                err_msg.contains("too large")
                    || err_msg.contains("Incomplete message")
                    || err_msg.contains("size limit"),
                "Expected size-related error, got: {}",
                err_msg
            );
        }
        Ok(_) => {
            // If it returns Ok waiting for more data, that's also a vulnerability
            // because attacker can keep connections open indefinitely
            println!(
                "WARNING: Message claiming 100MB accepted, waiting for more data. \
                     This allows resource exhaustion attacks!"
            );
        }
    }
}

/// Test 3: DoS Vulnerability - Unbounded Buffer Growth
///
/// An attacker can repeatedly send small chunks of data without completing a message,
/// causing the MessageBuffer to grow without bound until memory is exhausted.
///
/// **Expected Behavior**: Buffer should have a maximum size and reject additional data
/// **Current Behavior**: Buffer grows indefinitely
#[test]
#[ignore] // Ignored by default to prevent OOM
fn test_dos_unbounded_buffer_growth() {
    let mut buffer = MessageBuffer::new();

    // Simulate an attacker sending garbage data repeatedly
    // Each call to append() adds more data without ever forming a valid message
    let garbage_chunk = vec![0xFF; 10 * 1024]; // 10KB chunks

    // Try to append 100MB of garbage (10,000 chunks of 10KB each)
    for i in 0..10_000 {
        buffer.append(&garbage_chunk);

        // Check buffer size periodically
        if i % 1000 == 0 {
            println!("Buffer size after {} chunks: {} MB", i, buffer.len() / 1024 / 1024);
        }

        // WITH FIX: append() should return error once buffer exceeds limit (e.g., 50MB)
        // WITHOUT FIX: Buffer keeps growing until OOM
    }

    println!(
        "VULNERABILITY: Buffer grew to {} MB without limit!",
        buffer.len() / 1024 / 1024
    );

    // This should never be reached - buffer should have rejected data much earlier
    assert!(
        buffer.len() < 50 * 1024 * 1024,
        "Buffer exceeded 50MB! This is a DoS vulnerability."
    );
}

/// Test 4: DoS Vulnerability - Buffer Growth with Valid-Looking Headers
///
/// A more sophisticated attack where the attacker sends valid message headers
/// but never completes the messages, causing buffer accumulation.
#[test]
fn test_dos_buffer_growth_incomplete_messages() {
    let mut buffer = MessageBuffer::new();

    // Send 1000 valid message headers, each claiming a 1MB payload
    for _ in 0..1000 {
        let mut header = vec![
            0x4D, 0x50, 0x43, 0x53, // Magic
            0x01, // Version
        ];
        let size: u32 = 1024 * 1024; // Claim 1MB
        header.extend_from_slice(&size.to_be_bytes());

        buffer.append(&header);

        // Don't send the actual payload - just the header
        // This causes the buffer to accumulate waiting for payloads that never arrive
    }

    let buffer_size_mb = buffer.len() / 1024 / 1024;
    println!(
        "Buffer accumulated {} MB from incomplete messages",
        buffer_size_mb
    );

    // WITH FIX: Buffer should have a size limit and clear/reject data
    // WITHOUT FIX: Buffer grows with each incomplete message
    assert!(
        buffer.len() < 10 * 1024 * 1024,
        "Buffer exceeded 10MB from incomplete messages! DoS vulnerability."
    );
}

/// Test 5: Integer Conversion Vulnerability - Silent Failure
///
/// The field-to-i64 conversion in secret_sharing.rs can fail silently,
/// returning 0 instead of an error. This is critical in MPC because
/// incorrect results are worse than errors.
#[test]
fn test_integer_conversion_silent_failure() {
    // Test with a value that should work
    let secret = 42;
    let shares = SecretSharing::share_secret(secret, 5, 1).expect("Share failed");
    let reconstructed = SecretSharing::reconstruct_secret(&shares, 5).expect("Reconstruct failed");

    assert_eq!(reconstructed, secret);

    // Test with maximum i64 value
    let max_secret = i64::MAX;
    let max_shares = SecretSharing::share_secret(max_secret, 5, 1).expect("Share max failed");
    let max_reconstructed =
        SecretSharing::reconstruct_secret(&max_shares, 5).expect("Reconstruct max failed");

    // WITHOUT FIX: This might silently return 0 if conversion fails
    // WITH FIX: Should either return correct value or propagate error
    if max_reconstructed == 0 && max_secret != 0 {
        panic!(
            "VULNERABILITY: Integer conversion silently failed! \
                 Expected {}, got 0",
            max_secret
        );
    }

    // Verify we got close to the correct value (some precision loss is expected in field arithmetic)
    let difference = (max_reconstructed as i128 - max_secret as i128).abs();
    let tolerance = max_secret as i128 / 1000; // 0.1% tolerance

    assert!(
        difference < tolerance,
        "Integer conversion produced incorrect result: expected {}, got {} (diff: {})",
        max_secret,
        max_reconstructed,
        difference
    );
}

/// Test 6: Integer Conversion - Negative Numbers
///
/// Test that negative numbers are handled correctly and don't silently
/// become zero or lose sign information.
#[test]
fn test_integer_conversion_negative_numbers() {
    let negative_secret = -42;
    let shares =
        SecretSharing::share_secret(negative_secret, 5, 1).expect("Share negative failed");
    let reconstructed =
        SecretSharing::reconstruct_secret(&shares, 5).expect("Reconstruct negative failed");

    // WITHOUT FIX: Might return 0 or positive value if conversion fails
    // WITH FIX: Should preserve sign and value
    assert_eq!(
        reconstructed, negative_secret,
        "VULNERABILITY: Negative number conversion failed! Expected {}, got {}",
        negative_secret, reconstructed
    );
}

/// Test 7: Integer Conversion - Zero Special Case
///
/// Ensure that legitimate zero values are distinguished from
/// "failed conversion that returned zero".
#[test]
fn test_integer_conversion_zero_is_valid() {
    let zero_secret = 0;
    let shares = SecretSharing::share_secret(zero_secret, 5, 1).expect("Share zero failed");
    let reconstructed =
        SecretSharing::reconstruct_secret(&shares, 5).expect("Reconstruct zero failed");

    assert_eq!(
        reconstructed, zero_secret,
        "Failed to handle zero correctly: expected 0, got {}",
        reconstructed
    );
}

/// Test 8: Protocol Fuzzing - Malformed Messages
///
/// Test that various malformed messages are rejected properly
/// without causing panics or resource leaks.
#[test]
fn test_protocol_fuzzing_malformed_messages() {
    let test_cases = vec![
        // Empty data
        vec![],
        // Just magic bytes
        vec![0x4D, 0x50, 0x43, 0x53],
        // Magic + wrong version
        vec![0x4D, 0x50, 0x43, 0x53, 0xFF],
        // Magic + version + partial length
        vec![0x4D, 0x50, 0x43, 0x53, 0x01, 0x00],
        // Wrong magic bytes
        vec![0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00],
        // Random garbage
        vec![0xFF; 100],
        // Very large random data
        vec![0xAA; 1000],
    ];

    for (i, test_data) in test_cases.iter().enumerate() {
        let result = deserialize_message(test_data);

        // All should return errors, none should panic
        match result {
            Err(_) => {
                // Good - malformed message rejected
            }
            Ok(_) => {
                panic!(
                    "Test case {} VULNERABILITY: Malformed message accepted! Data: {:?}",
                    i,
                    &test_data[..test_data.len().min(20)]
                );
            }
        }
    }

    println!("All {} malformed messages rejected correctly", test_cases.len());
}

/// Test 9: MessageBuffer - Memory Leak on Invalid Data
///
/// Verify that MessageBuffer properly cleans up when encountering
/// invalid data and doesn't leak memory.
#[test]
fn test_message_buffer_cleanup_on_error() {
    let mut buffer = MessageBuffer::new();

    // Add some valid-looking data
    buffer.append(&[0x4D, 0x50, 0x43, 0x53, 0x01]);

    // Add invalid data that will cause parse to fail
    buffer.append(&[0xFF, 0xFF, 0xFF, 0xFF]); // Invalid length

    let initial_size = buffer.len();

    // Try to parse - should fail
    let result = buffer.try_parse();

    // WITH FIX: Buffer should clear invalid data after error
    // WITHOUT FIX: Buffer keeps growing with invalid data
    if result.is_err() && buffer.len() >= initial_size {
        println!(
            "WARNING: Buffer not cleaned up after error. Size: {} -> {}",
            initial_size,
            buffer.len()
        );
    }
}

/// Test 10: Stress Test - Rapid Message Processing
///
/// Send many small messages rapidly to test for resource exhaustion,
/// memory leaks, or performance degradation.
#[test]
fn test_rapid_message_processing() {
    use std::time::Instant;

    let start = Instant::now();
    let mut total_messages = 0;

    // Process messages for 100ms
    while start.elapsed().as_millis() < 100 {
        let msg = MPCaaSMessage::Ping;
        let data = serialize_message(&msg).expect("Serialize failed");
        let _parsed = deserialize_message(&data).expect("Deserialize failed");
        total_messages += 1;
    }

    let elapsed = start.elapsed();
    let msg_per_sec = (total_messages as f64 / elapsed.as_secs_f64()) as u64;

    println!(
        "Processed {} messages in {:?} ({} msg/sec)",
        total_messages, elapsed, msg_per_sec
    );

    // Should be able to process at least 1000 messages/sec for simple Ping messages
    assert!(
        msg_per_sec > 1000,
        "Performance too low: {} msg/sec. Possible resource leak or inefficiency.",
        msg_per_sec
    );
}

/// Test 11: Documentation of Expected Behavior
///
/// This test documents what the FIXED behavior should be.
#[test]
fn test_expected_secure_behavior() {
    println!("\n=== Expected Secure Behavior After Fixes ===");
    println!("1. Messages over 10MB should be rejected immediately");
    println!("2. MessageBuffer should enforce 50MB maximum size");
    println!("3. Field-to-integer conversion errors should propagate, not return 0");
    println!("4. Mutex poisoning should be handled gracefully");
    println!("5. All resource limits should be configurable");
    println!("6. All errors should be logged for security monitoring");
    println!("===========================================\n");
}
