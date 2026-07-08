# Stoffel Rust SDK Documentation

This directory contains technical documentation and design notes for the Stoffel Rust SDK.

## Documentation Files

### Repository Documentation
- **[../README.md](../README.md)** - Main SDK documentation with quick start guide and examples
- **[../CLAUDE.md](../CLAUDE.md)** - Development guide for Claude Code AI assistant
- **[../examples/README.md](../examples/README.md)** - Example programs and SDK usage patterns

## SDK Architecture

The Stoffel Rust SDK provides three levels of abstraction:

1. **Simple API** (`prelude`) - Recommended for most users
   - High-level API for shipping MPC applications
   - Everything needed for common use cases

2. **Advanced API** (`advanced`) - For custom applications
   - Fine-grained control with proper abstractions
   - ShareManager, NetworkBuilder components

3. **Network Infrastructure** (`network_helpers`) - For production
   - Complete network setup helpers
   - QUIC networking, message processors

## Key Components

### Message Processor Architecture
The SDK implements message processors that route MPC protocol messages between servers:

- **Node Initialization**: Call `initialize_node()` before spawning message processors
- **Message Routing**: `spawn_message_processor()` creates background tasks for protocol message handling
- **State Sharing**: Uses `Arc<Mutex<>>` internally for shared preprocessing material

See `examples/honeybadger_mpc_demo.rs` for the reference implementation.

### MPC Workflow
1. Compile Stoffel program with MPC configuration
2. Initialize MPC nodes
3. Start QUIC network listeners
4. Spawn message processors
5. Connect servers in mesh topology
6. Run preprocessing (requires coordinator service - see STO-245)
7. Create clients with private inputs
8. Prepare servers to receive inputs
9. Connect clients to servers
10. Send secret-shared inputs
11. Execute secure computation

## Related Resources

- [Linear Issue Tracker](https://linear.app/stoffel-labs)
  - [STO-104](https://linear.app/stoffel-labs/issue/STO-104) - Client input access
  - [STO-245](https://linear.app/stoffel-labs/issue/STO-245) - Coordinator service

- External Repositories
  - [Stoffel-Lang](https://github.com/Stoffel-Labs/Stoffel-Lang) - Language compiler
  - [StoffelVM](https://github.com/Stoffel-Labs/StoffelVM) - Virtual machine runtime
  - [MPC Protocols](https://github.com/Stoffel-Labs/mpc-protocols) - HoneyBadger protocol
  - [Stoffel Networking](https://github.com/Stoffel-Labs/stoffel-networking) - QUIC transport

## Contributing

This is an internal Stoffel Labs project. For development guidance, see [CLAUDE.md](../CLAUDE.md).
