# Claude Agent SDK for Rust - Overview

## Purpose

The Claude Agent SDK for Rust is a complete Rust implementation of the Claude Agent SDK, mirroring the Python Claude Agent SDK with idiomatic Rust patterns and best practices. It provides a type-safe, performant, and memory-safe way to interact with Claude AI agents.

## Repository

- **GitHub**: https://github.com/dhuseby/claude-agent-sdk-rust
- **License**: MIT
- **Language**: Rust 1.75.0+

## Key Features

### Feature Parity
- Full feature parity with Python SDK (v0.1.0)
- Basic `query()` function for simple interactions
- Full message type support
- Hook and permission systems
- Custom tools support via SDK MCP server

### Rust-Specific Benefits
- **Type Safety**: Comprehensive type safety with newtypes
- **Memory Safety**: Ownership patterns and no unsafe code
- **Performance**: Async/await support with tokio
- **Error Handling**: Clear error propagation using thiserror
- **Testing**: Extensive test coverage (70+ tests)

### Security Features
- Environment variable filtering
- Argument validation
- Timeout protection
- Buffer limits
- Secure defaults
- No unsafe code

## Architecture

### Project Structure
```
src/
├── error.rs         # Error types and handling
├── types.rs         # Type definitions and newtypes
├── transport/       # Communication layer
├── message/         # Message parsing and handling
├── query.rs         # Simple query function
└── lib.rs           # Public API exports
```

### Design Principles
- **Type Safety**: Using newtypes to prevent type confusion
- **Memory Safety**: Leveraging Rust's ownership system
- **Clear Error Propagation**: Using Result types throughout
- **Ownership Patterns**: Proper use of borrowing and moving
- **Builder Pattern**: For complex object construction
- **Trait Abstraction**: For extensibility and modularity

## Prerequisites

Before using the SDK, ensure you have:
- Rust 1.75.0 or higher
- Node.js (for Claude Code CLI)
- Claude Code CLI installed

## Quick Links

- [Installation Guide](./installation.md)
- [Getting Started](./getting_started.md)
- [API Reference](./api_reference.md)
- [Examples](./examples.md)
- [Security](./security.md)
