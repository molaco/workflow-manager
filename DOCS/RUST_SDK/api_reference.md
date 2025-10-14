# API Reference

## Core Functions

### `query()`

The primary function for interacting with Claude.

```rust
pub async fn query(
    prompt: &str,
    config: Option<QueryConfig>
) -> Result<impl Stream<Item = Result<Message>>>
```

**Parameters:**
- `prompt: &str` - The question or prompt to send to Claude
- `config: Option<QueryConfig>` - Optional configuration for the query

**Returns:**
- `Result<impl Stream<Item = Result<Message>>>` - An async stream of messages

**Example:**
```rust
use claude_agent_sdk::query;
use futures::StreamExt;

let stream = query("What is 2 + 2?", None).await?;
let mut stream = Box::pin(stream);

while let Some(message) = stream.next().await {
    println!("{:?}", message?);
}
```

## Error Types

The SDK uses a custom error type for comprehensive error handling.

```rust
pub enum Error {
    Transport(TransportError),
    Message(MessageError),
    Timeout,
    InvalidArgument(String),
    // ... other variants
}
```

### Error Variants

#### `Error::Transport`
Errors related to communication with the Claude CLI process.

```rust
match query("prompt", None).await {
    Err(Error::Transport(e)) => {
        eprintln!("Transport error: {}", e);
    }
    _ => {}
}
```

#### `Error::Message`
Errors related to message parsing or handling.

```rust
match query("prompt", None).await {
    Err(Error::Message(e)) => {
        eprintln!("Message error: {}", e);
    }
    _ => {}
}
```

#### `Error::Timeout`
Operation exceeded configured timeout.

```rust
match query("prompt", None).await {
    Err(Error::Timeout) => {
        eprintln!("Operation timed out");
    }
    _ => {}
}
```

#### `Error::InvalidArgument`
Invalid argument provided to a function.

```rust
match query("", None).await {
    Err(Error::InvalidArgument(msg)) => {
        eprintln!("Invalid argument: {}", msg);
    }
    _ => {}
}
```

## Type System

### Message Types

The SDK provides comprehensive message type support:

```rust
pub enum Message {
    Text(String),
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    // ... other variants
}
```

### Newtypes

The SDK uses newtypes for type safety:

```rust
pub struct ToolId(String);
pub struct MessageId(String);
pub struct SessionId(String);
```

These prevent accidental mixing of different ID types:

```rust
// This won't compile - type safety!
let tool_id: ToolId = ToolId("tool-123".to_string());
let message_id: MessageId = tool_id; // Error: mismatched types
```

## Async Support

### Runtime Requirements

The SDK requires an async runtime (tokio):

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Your async code here
    Ok(())
}
```

### Stream Processing

Messages are delivered as an async stream:

```rust
use futures::StreamExt;

let stream = query("prompt", None).await?;
let mut stream = Box::pin(stream);

while let Some(result) = stream.next().await {
    match result {
        Ok(message) => {
            // Process message
        }
        Err(e) => {
            // Handle error
        }
    }
}
```

## Configuration

### QueryConfig

(Note: Configuration structure may vary based on SDK version)

```rust
pub struct QueryConfig {
    pub timeout: Option<Duration>,
    pub max_tokens: Option<usize>,
    // ... other fields
}
```

## Traits and Interfaces

### Stream Trait

The SDK returns streams implementing the `Stream` trait from `futures`:

```rust
use futures::Stream;

pub fn query(...) -> impl Stream<Item = Result<Message, Error>>
```

## Module Organization

### `error` Module

Contains all error types and error handling utilities:

```rust
use claude_agent_sdk::error::Error;
```

### `types` Module

Contains type definitions and newtypes:

```rust
use claude_agent_sdk::types::{Message, ToolId, MessageId};
```

### `transport` Module

Handles communication with the Claude CLI (typically internal):

```rust
// Internal module - not usually accessed directly
```

### `message` Module

Message parsing and handling (typically internal):

```rust
// Internal module - not usually accessed directly
```

## Security Considerations

### Environment Variable Filtering

The SDK automatically filters sensitive environment variables:

```rust
// Sensitive variables are never exposed
// - API keys
// - Tokens
// - Passwords
// - Secrets
```

### Argument Validation

All inputs are validated before processing:

```rust
// Invalid arguments return Error::InvalidArgument
query("", None).await // Returns error for empty prompt
```

### Timeout Protection

Operations have configurable timeouts:

```rust
// Prevents indefinite hanging
// Default timeout applies if not configured
```

## Testing Support

The SDK includes comprehensive testing utilities:

```rust
#[cfg(test)]
mod tests {
    use claude_agent_sdk::query;

    #[tokio::test]
    async fn test_simple_query() {
        // Your tests here
    }
}
```

## Thread Safety

All public types are thread-safe and can be safely shared across threads:

```rust
use std::sync::Arc;

let shared_data = Arc::new(/* ... */);

tokio::spawn(async move {
    // Safe to use in different tasks
});
```

## Performance Considerations

### Async by Default
- All I/O operations are non-blocking
- Efficient use of system resources
- Can handle multiple concurrent queries

### Zero-Copy Where Possible
- Minimizes memory allocations
- Uses references and borrowing effectively
- Efficient string handling

### Buffer Management
- Configurable buffer limits
- Automatic cleanup of resources
- Prevention of memory leaks

## Version Compatibility

Current SDK version: **0.1.0**

- Full feature parity with Python SDK v0.1.0
- Rust 1.75.0+ required
- Tokio 1.x compatibility
