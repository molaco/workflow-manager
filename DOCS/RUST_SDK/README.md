# Claude Agent SDK for Rust - Documentation

Complete documentation for the [claude-agent-sdk-rust](https://github.com/dhuseby/claude-agent-sdk-rust) library.

## Documentation Index

### Getting Started
- [Overview](./overview.md) - Introduction to the SDK, features, and architecture
- [Installation](./installation.md) - How to install and set up the SDK
- [Getting Started](./getting_started.md) - Your first steps with the SDK

### Reference
- [API Reference](./api_reference.md) - Complete API documentation
- [Examples](./examples.md) - Practical code examples for common use cases
- [Security](./security.md) - Security features and best practices

## Quick Links

- **Repository**: https://github.com/dhuseby/claude-agent-sdk-rust
- **License**: MIT
- **Version**: 0.1.0
- **Rust Version**: 1.75.0+

## Quick Start

```rust
use claude_agent_sdk::query;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stream = query("What is 2 + 2?", None).await?;
    let mut stream = Box::pin(stream);

    while let Some(message) = stream.next().await {
        println!("{:?}", message?);
    }

    Ok(())
}
```

## Key Features

- ✅ Full feature parity with Python SDK (v0.1.0)
- ✅ Type-safe implementation with newtypes
- ✅ Comprehensive error handling
- ✅ Async/await support with tokio
- ✅ Subprocess CLI integration
- ✅ Hook and permission systems
- ✅ Custom tools support via SDK MCP server
- ✅ Extensive test coverage (70+ tests)
- ✅ Memory safe (no unsafe code)
- ✅ Security-first design

## Prerequisites

- Rust 1.75.0 or higher
- Node.js (for Claude Code CLI)
- Claude Code CLI installed and configured

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
claude-agent-sdk = "0.1.0"
tokio = { version = "1", features = ["full"] }
futures = "0.3"
```

## Documentation Structure

```
DOCS/RUST_SDK/
├── README.md              # This file - documentation index
├── overview.md            # SDK overview and architecture
├── installation.md        # Installation and setup guide
├── getting_started.md     # Tutorial for beginners
├── api_reference.md       # Complete API documentation
├── examples.md            # Code examples and patterns
└── security.md           # Security features and best practices
```

## Support

For issues, questions, or contributions:
- GitHub Issues: https://github.com/dhuseby/claude-agent-sdk-rust/issues
- Repository: https://github.com/dhuseby/claude-agent-sdk-rust

## Contributing

Contributions are welcome! Please:
1. Read the documentation thoroughly
2. Check existing issues and PRs
3. Follow Rust best practices
4. Include tests for new features
5. Update documentation as needed

## License

MIT License - See the repository for full license text.
