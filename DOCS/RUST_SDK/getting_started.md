# Getting Started

## Your First Query

The simplest way to interact with Claude is using the `query()` function:

```rust
use claude_agent_sdk::query;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Send a simple query
    let stream = query("What is 2 + 2?", None).await?;
    let mut stream = Box::pin(stream);

    // Process the response stream
    while let Some(message) = stream.next().await {
        println!("{:?}", message?);
    }

    Ok(())
}
```

## Understanding the Response Stream

The `query()` function returns an async stream of messages. Each message represents a piece of the conversation with Claude:

```rust
use claude_agent_sdk::query;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stream = query("Explain async/await in Rust", None).await?;
    let mut stream = Box::pin(stream);

    while let Some(result) = stream.next().await {
        match result {
            Ok(message) => {
                // Handle the message
                println!("Received: {:?}", message);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    Ok(())
}
```

## Error Handling

The SDK uses Rust's `Result` type for error handling:

```rust
use claude_agent_sdk::{query, error::Error};
use futures::StreamExt;

#[tokio::main]
async fn main() {
    match query("Hello, Claude!", None).await {
        Ok(stream) => {
            let mut stream = Box::pin(stream);
            while let Some(message) = stream.next().await {
                match message {
                    Ok(msg) => println!("{:?}", msg),
                    Err(e) => eprintln!("Stream error: {}", e),
                }
            }
        }
        Err(e) => {
            eprintln!("Query failed: {}", e);
        }
    }
}
```

## Using Async/Await

The SDK is fully asynchronous and uses tokio as the runtime:

```rust
use claude_agent_sdk::query;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // You can use async/await patterns
    let stream = query("What's the weather like?", None).await?;

    // Stream processing is also async
    let mut stream = Box::pin(stream);
    while let Some(message) = stream.next().await {
        let msg = message?;
        // Process message...
    }

    Ok(())
}
```

## Configuration Options

The `query()` function accepts optional configuration:

```rust
use claude_agent_sdk::query;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // With default configuration
    let stream = query("Hello!", None).await?;

    // With custom configuration (when supported)
    // let config = QueryConfig { ... };
    // let stream = query("Hello!", Some(config)).await?;

    Ok(())
}
```

## Best Practices

### 1. Always Handle Errors
```rust
// Good
match query("question", None).await {
    Ok(stream) => { /* handle stream */ },
    Err(e) => { /* handle error */ }
}

// Bad - unwrap() will panic on error
let stream = query("question", None).await.unwrap();
```

### 2. Use Proper Async Runtime
```rust
// Good - using tokio::main
#[tokio::main]
async fn main() { }

// Also good - manual runtime creation
fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async { });
}
```

### 3. Pin Streams Before Iteration
```rust
// Good
let stream = query("question", None).await?;
let mut stream = Box::pin(stream);

// Required for proper stream handling
while let Some(message) = stream.next().await {
    // process message
}
```

### 4. Resource Cleanup
```rust
use futures::StreamExt;

async fn process_query() -> Result<(), Box<dyn std::error::Error>> {
    let stream = query("question", None).await?;
    let mut stream = Box::pin(stream);

    // Stream automatically cleans up when dropped
    while let Some(message) = stream.next().await {
        message?; // Propagate errors
    }

    Ok(())
} // Cleanup happens here automatically
```

## Common Patterns

### Collecting All Responses
```rust
use futures::StreamExt;
use std::collections::Vec;

let stream = query("question", None).await?;
let mut stream = Box::pin(stream);

let mut responses = Vec::new();
while let Some(message) = stream.next().await {
    responses.push(message?);
}
```

### Processing with Timeout
```rust
use tokio::time::{timeout, Duration};

let future = async {
    let stream = query("question", None).await?;
    let mut stream = Box::pin(stream);

    while let Some(message) = stream.next().await {
        println!("{:?}", message?);
    }

    Ok::<(), Box<dyn std::error::Error>>(())
};

match timeout(Duration::from_secs(30), future).await {
    Ok(Ok(())) => println!("Completed successfully"),
    Ok(Err(e)) => eprintln!("Error: {}", e),
    Err(_) => eprintln!("Timeout!"),
}
```

## Next Steps

- [Examples](./examples.md) - More comprehensive examples
- [API Reference](./api_reference.md) - Detailed API documentation
- [Error Handling](./error_handling.md) - Advanced error handling patterns
