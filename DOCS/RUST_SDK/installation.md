# Installation Guide

## Prerequisites

### System Requirements
- **Rust**: 1.75.0 or higher
- **Node.js**: Required for Claude Code CLI
- **Claude Code CLI**: Must be installed and configured

### Installing Rust
If you don't have Rust installed:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Verify installation:
```bash
rustc --version
cargo --version
```

### Installing Claude Code CLI
Follow the instructions at the Claude Code documentation to install and configure the CLI.

## Adding the SDK to Your Project

### Using Cargo.toml
Add the following to your `Cargo.toml` file:

```toml
[dependencies]
claude-agent-sdk = "0.1.0"
tokio = { version = "1", features = ["full"] }
futures = "0.3"
```

### From Source
If you want to use the latest development version:

```toml
[dependencies]
claude-agent-sdk = { git = "https://github.com/dhuseby/claude-agent-sdk-rust" }
tokio = { version = "1", features = ["full"] }
futures = "0.3"
```

### Installing from Crates.io
```bash
cargo add claude-agent-sdk
cargo add tokio --features full
cargo add futures
```

## Verification

Create a simple test file to verify installation:

```rust
use claude_agent_sdk::query;

#[tokio::main]
async fn main() {
    println!("Claude Agent SDK installed successfully!");
}
```

Build the project:
```bash
cargo build
```

If the build succeeds, the SDK is properly installed and ready to use.

## Next Steps

- [Getting Started Guide](./getting_started.md) - Learn basic usage
- [Examples](./examples.md) - See practical examples
- [API Reference](./api_reference.md) - Explore the full API
