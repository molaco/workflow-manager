use clap::Parser;
use workflow_manager_sdk::{WorkflowDefinition, log_phase_start, log_phase_complete, log_task_start, log_task_complete};
use anyhow::Result;
use claude_agent_sdk::{query, ClaudeAgentOptions, Message};
use futures::StreamExt;

#[derive(Parser, Debug, Clone, WorkflowDefinition)]
#[workflow(
    id = "simple_query",
    name = "Simple Query Workflow",
    description = "Ask Claude a question using the Agent SDK"
)]
struct Args {
    /// Question to ask Claude
    #[arg(short, long, required_unless_present = "workflow_metadata")]
    #[field(
        label = "Question",
        description = "[TEXT] Question to ask Claude (e.g., 'What is 2 + 2?')",
        type = "text"
    )]
    question: Option<String>,

    /// System prompt (optional)
    #[arg(short, long)]
    #[field(
        label = "System Prompt",
        description = "[TEXT] Optional system prompt to guide Claude's responses",
        type = "text"
    )]
    system_prompt: Option<String>,

    /// Maximum conversation turns
    #[arg(short = 't', long, default_value = "5")]
    #[field(
        label = "Max Turns",
        description = "[NUMBER] Maximum conversation turns (1-10)",
        type = "number",
        min = "1",
        max = "10"
    )]
    max_turns: usize,

    // Hidden metadata flag
    #[arg(long, hide = true)]
    workflow_metadata: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Handle metadata request from TUI
    if args.workflow_metadata {
        args.print_metadata();
        return Ok(());
    }

    println!("🚀 Simple Query Workflow Started");
    println!("========================================\n");

    // Emit phase start
    log_phase_start!(0, "Query Execution", 1);

    let question = args.question.as_deref().unwrap_or("What is 2 + 2?");

    // Build options
    let mut options_builder = ClaudeAgentOptions::builder()
        .max_turns(args.max_turns as u32);

    if let Some(system_prompt) = &args.system_prompt {
        options_builder = options_builder.system_prompt(system_prompt.clone());
    }

    let options = options_builder.build();

    println!("Question: {}\n", question);
    if let Some(prompt) = &args.system_prompt {
        println!("System Prompt: {}\n", prompt);
    }
    println!("Response:\n");

    // Emit task start
    log_task_start!(0, "query_task", format!("Asking: {}", question));

    // Send query
    let stream = query(question, Some(options)).await?;
    let mut stream = Box::pin(stream);

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    if let claude_agent_sdk::ContentBlock::Text { text } = block {
                        println!("{}", text);
                    }
                }
            }
            Message::Result {
                total_cost_usd,
                num_turns,
                is_error,
                ..
            } => {
                println!("\n========================================");
                if is_error {
                    println!("❌ Query failed");
                    log_task_complete!("query_task", "Failed");
                    log_phase_complete!(0, "Query Execution");
                } else {
                    println!("✅ Completed in {} turns", num_turns);
                    log_task_complete!("query_task", format!("Completed in {} turns", num_turns));
                    log_phase_complete!(0, "Query Execution");
                }
                if let Some(cost) = total_cost_usd {
                    println!("💰 Cost: ${:.4}", cost);
                }
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
