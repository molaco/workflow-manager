use anyhow::Result;
use clap::Parser;
use workflow_manager_sdk::WorkflowDefinition;

#[derive(Parser, Debug, Clone, WorkflowDefinition)]
#[workflow(
    id = "simple_echo",
    name = "Simple Echo Workflow",
    description = "A simple workflow that echoes a message with a repeat count"
)]
struct Args {
    /// Message to echo
    #[arg(short, long, required_unless_present = "workflow_metadata")]
    #[field(
        label = "Message",
        description = "[TEXT] Message to echo (e.g., 'Hello World')",
        type = "text"
    )]
    message: Option<String>,

    /// Number of times to repeat
    #[arg(short = 'n', long, default_value = "3")]
    #[field(
        label = "Repeat Count",
        description = "[NUMBER] How many times to repeat (1-10)",
        type = "number",
        min = "1",
        max = "10"
    )]
    repeat: usize,

    /// Optional file path to echo
    #[arg(short = 'f', long)]
    #[field(
        label = "File Path",
        description = "[FILE PATH] Optional file to read and echo (e.g., 'README.md')",
        type = "file_path"
    )]
    file_path: Option<String>,

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

    println!("🚀 Simple Echo Workflow Started");
    println!("========================================");

    // Check if we should echo a file
    if let Some(file_path) = &args.file_path {
        println!("Reading file: {}", file_path);
        match tokio::fs::read_to_string(file_path).await {
            Ok(contents) => {
                println!("\n--- File Contents ---");
                for i in 1..=args.repeat {
                    println!("[{}]\n{}", i, contents);
                    if i < args.repeat {
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    }
                }
                println!("--- End File Contents ---\n");
            }
            Err(e) => {
                eprintln!("❌ Error reading file: {}", e);
                return Err(e.into());
            }
        }
    } else {
        // Echo message
        let message = args.message.as_deref().unwrap_or("Hello from workflow!");

        for i in 1..=args.repeat {
            println!("[{}] {}", i, message);
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }

    println!("========================================");
    println!("✅ Workflow completed successfully!");

    Ok(())
}
