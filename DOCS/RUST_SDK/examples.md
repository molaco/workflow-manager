# Examples

## Basic Examples

### Simple Question and Answer

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

### Multiple Queries

```rust
use claude_agent_sdk::query;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let questions = vec![
        "What is the capital of France?",
        "What is 10 + 15?",
        "Explain Rust ownership",
    ];

    for question in questions {
        println!("\nQuestion: {}", question);
        println!("Answer:");

        let stream = query(question, None).await?;
        let mut stream = Box::pin(stream);

        while let Some(message) = stream.next().await {
            println!("{:?}", message?);
        }
    }

    Ok(())
}
```

## Intermediate Examples

### Collecting Responses

```rust
use claude_agent_sdk::query;
use futures::StreamExt;
use std::collections::Vec;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stream = query("List 5 programming languages", None).await?;
    let mut stream = Box::pin(stream);

    let mut responses = Vec::new();

    while let Some(message) = stream.next().await {
        responses.push(message?);
    }

    println!("Collected {} messages", responses.len());
    for (i, response) in responses.iter().enumerate() {
        println!("Message {}: {:?}", i + 1, response);
    }

    Ok(())
}
```

### Error Handling with Match

```rust
use claude_agent_sdk::{query, error::Error};
use futures::StreamExt;

#[tokio::main]
async fn main() {
    match run_query().await {
        Ok(()) => println!("Query completed successfully"),
        Err(e) => eprintln!("Query failed: {}", e),
    }
}

async fn run_query() -> Result<(), Box<dyn std::error::Error>> {
    let stream = query("Explain async/await", None).await?;
    let mut stream = Box::pin(stream);

    while let Some(result) = stream.next().await {
        match result {
            Ok(message) => {
                println!("✓ {:?}", message);
            }
            Err(e) => {
                eprintln!("✗ Error: {}", e);
                return Err(e.into());
            }
        }
    }

    Ok(())
}
```

### Using Timeouts

```rust
use claude_agent_sdk::query;
use futures::StreamExt;
use tokio::time::{timeout, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let query_future = async {
        let stream = query("Explain quantum computing", None).await?;
        let mut stream = Box::pin(stream);

        while let Some(message) = stream.next().await {
            println!("{:?}", message?);
        }

        Ok::<(), Box<dyn std::error::Error>>(())
    };

    match timeout(Duration::from_secs(30), query_future).await {
        Ok(Ok(())) => {
            println!("Query completed within timeout");
        }
        Ok(Err(e)) => {
            eprintln!("Query error: {}", e);
        }
        Err(_) => {
            eprintln!("Query timed out after 30 seconds");
        }
    }

    Ok(())
}
```

## Advanced Examples

### Concurrent Queries

```rust
use claude_agent_sdk::query;
use futures::StreamExt;
use tokio::task::JoinSet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let questions = vec![
        "What is Rust?",
        "What is Python?",
        "What is JavaScript?",
    ];

    let mut join_set = JoinSet::new();

    for question in questions {
        join_set.spawn(async move {
            let stream = query(question, None).await?;
            let mut stream = Box::pin(stream);

            let mut count = 0;
            while let Some(message) = stream.next().await {
                message?;
                count += 1;
            }

            Ok::<(String, usize), Box<dyn std::error::Error + Send>>(
                (question.to_string(), count)
            )
        });
    }

    while let Some(result) = join_set.join_next().await {
        match result? {
            Ok((question, count)) => {
                println!("{}: {} messages", question, count);
            }
            Err(e) => {
                eprintln!("Task error: {}", e);
            }
        }
    }

    Ok(())
}
```

### Custom Message Processing

```rust
use claude_agent_sdk::query;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stream = query("Write a haiku about programming", None).await?;
    let mut stream = Box::pin(stream);

    let mut message_count = 0;
    let mut total_size = 0;

    while let Some(message) = stream.next().await {
        let msg = message?;

        // Custom processing logic
        let msg_debug = format!("{:?}", msg);
        let size = msg_debug.len();

        message_count += 1;
        total_size += size;

        println!("Message #{}: {} bytes", message_count, size);
        println!("{}", msg_debug);
        println!("---");
    }

    println!("\nSummary:");
    println!("Total messages: {}", message_count);
    println!("Total size: {} bytes", total_size);
    println!("Average size: {} bytes", total_size / message_count.max(1));

    Ok(())
}
```

### Interactive CLI Application

```rust
use claude_agent_sdk::query;
use futures::StreamExt;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Claude Interactive CLI");
    println!("Type 'exit' to quit\n");

    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim();

        if input.eq_ignore_ascii_case("exit") {
            println!("Goodbye!");
            break;
        }

        if input.is_empty() {
            continue;
        }

        match process_query(input).await {
            Ok(()) => println!(),
            Err(e) => eprintln!("Error: {}\n", e),
        }
    }

    Ok(())
}

async fn process_query(prompt: &str) -> Result<(), Box<dyn std::error::Error>> {
    let stream = query(prompt, None).await?;
    let mut stream = Box::pin(stream);

    while let Some(message) = stream.next().await {
        let msg = message?;
        println!("{:?}", msg);
    }

    Ok(())
}
```

### Retry Logic

```rust
use claude_agent_sdk::query;
use futures::StreamExt;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = query_with_retry("What is Rust?", 3).await?;
    println!("Query succeeded with result: {:?}", result);
    Ok(())
}

async fn query_with_retry(
    prompt: &str,
    max_retries: u32,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut attempt = 0;

    loop {
        attempt += 1;
        println!("Attempt {} of {}", attempt, max_retries);

        match run_query(prompt).await {
            Ok(results) => return Ok(results),
            Err(e) => {
                eprintln!("Attempt {} failed: {}", attempt, e);

                if attempt >= max_retries {
                    return Err(e);
                }

                let backoff = Duration::from_secs(2u64.pow(attempt - 1));
                println!("Retrying in {:?}...", backoff);
                sleep(backoff).await;
            }
        }
    }
}

async fn run_query(prompt: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let stream = query(prompt, None).await?;
    let mut stream = Box::pin(stream);

    let mut results = Vec::new();

    while let Some(message) = stream.next().await {
        let msg = format!("{:?}", message?);
        results.push(msg);
    }

    Ok(results)
}
```

### Structured Logging

```rust
use claude_agent_sdk::query;
use futures::StreamExt;
use chrono::Utc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_logged_query("Explain ownership in Rust").await?;
    Ok(())
}

async fn run_logged_query(prompt: &str) -> Result<(), Box<dyn std::error::Error>> {
    log_event("QUERY_START", &format!("prompt: {}", prompt));

    let start = Utc::now();

    match execute_query(prompt).await {
        Ok(count) => {
            let duration = Utc::now() - start;
            log_event(
                "QUERY_SUCCESS",
                &format!("messages: {}, duration: {}ms", count, duration.num_milliseconds()),
            );
        }
        Err(e) => {
            let duration = Utc::now() - start;
            log_event(
                "QUERY_ERROR",
                &format!("error: {}, duration: {}ms", e, duration.num_milliseconds()),
            );
            return Err(e);
        }
    }

    Ok(())
}

async fn execute_query(prompt: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let stream = query(prompt, None).await?;
    let mut stream = Box::pin(stream);

    let mut count = 0;

    while let Some(message) = stream.next().await {
        message?;
        count += 1;
        log_event("MESSAGE_RECEIVED", &format!("count: {}", count));
    }

    Ok(count)
}

fn log_event(level: &str, message: &str) {
    let timestamp = Utc::now().to_rfc3339();
    println!("[{}] {} - {}", timestamp, level, message);
}
```

## Testing Examples

### Unit Test

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[tokio::test]
    async fn test_simple_query() {
        let stream = query("test", None).await;
        assert!(stream.is_ok());

        let mut stream = Box::pin(stream.unwrap());
        let message = stream.next().await;
        assert!(message.is_some());
    }

    #[tokio::test]
    async fn test_empty_prompt() {
        let result = query("", None).await;
        // Should handle empty prompts gracefully
        assert!(result.is_ok() || result.is_err());
    }
}
```

### Integration Test

```rust
#[cfg(test)]
mod integration_tests {
    use claude_agent_sdk::query;
    use futures::StreamExt;

    #[tokio::test]
    async fn test_full_conversation_flow() {
        let prompts = vec![
            "Hello",
            "What is 2+2?",
            "Thank you",
        ];

        for prompt in prompts {
            let stream = query(prompt, None).await;
            assert!(stream.is_ok(), "Query failed for: {}", prompt);

            let mut stream = Box::pin(stream.unwrap());
            let mut message_count = 0;

            while let Some(message) = stream.next().await {
                assert!(message.is_ok(), "Message error for: {}", prompt);
                message_count += 1;
            }

            assert!(message_count > 0, "No messages for: {}", prompt);
        }
    }
}
```
