//! Example of using custom agents with Claude Agent SDK
//!
//! This example demonstrates how to define and use custom agents with specific
//! tools, prompts, and models.
//!
//! Usage:
//! cargo run --example agents

use claude_agent_sdk::{query, ClaudeAgentOptions, AgentDefinition, Message, ContentBlock};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    code_reviewer_example().await?;
    multiple_agents_example().await?;
    Ok(())
}

async fn code_reviewer_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Code Reviewer Agent Example ===\n");

    let options = ClaudeAgentOptions::builder()
        .add_agent("code-reviewer", AgentDefinition {
            description: "Reviews code for best practices and potential issues".to_string(),
            prompt: "You are a code reviewer. Analyze code for bugs, performance issues, \
                     security vulnerabilities, and adherence to best practices. \
                     Provide constructive feedback.".to_string(),
            tools: Some(vec!["Read".to_string(), "Grep".to_string()]),
            model: Some("sonnet".to_string()),
        })
        .build();

    let stream = query(
        "Use the code-reviewer agent to review the code in src/types.rs",
        Some(options)
    ).await?;

    let mut stream = Box::pin(stream);

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    if let ContentBlock::Text { text } = block {
                        println!("Claude: {}", text);
                    }
                }
            }
            Message::Result { total_cost_usd, .. } => {
                if let Some(cost) = total_cost_usd {
                    println!("\nCost: ${:.4}", cost);
                }
            }
            _ => {}
        }
    }

    println!();
    Ok(())
}

async fn multiple_agents_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Multiple Agents Example ===\n");

    let options = ClaudeAgentOptions::builder()
        .add_agent("analyzer", AgentDefinition {
            description: "Analyzes code structure and patterns".to_string(),
            prompt: "You are a code analyzer. Examine code structure, patterns, and architecture.".to_string(),
            tools: Some(vec!["Read".to_string(), "Grep".to_string(), "Glob".to_string()]),
            model: None,
        })
        .add_agent("tester", AgentDefinition {
            description: "Creates and runs tests".to_string(),
            prompt: "You are a testing expert. Write comprehensive tests and ensure code quality.".to_string(),
            tools: Some(vec!["Read".to_string(), "Write".to_string(), "Bash".to_string()]),
            model: Some("sonnet".to_string()),
        })
        .build();

    let stream = query(
        "Use the analyzer agent to find all Rust files in the examples/ directory",
        Some(options)
    ).await?;

    let mut stream = Box::pin(stream);

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    if let ContentBlock::Text { text } = block {
                        println!("Claude: {}", text);
                    }
                }
            }
            Message::Result { total_cost_usd, .. } => {
                if let Some(cost) = total_cost_usd {
                    println!("\nCost: ${:.4}", cost);
                }
            }
            _ => {}
        }
    }

    println!();
    Ok(())
}
