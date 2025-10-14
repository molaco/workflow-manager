use clap::Parser;
use workflow_manager_sdk::WorkflowDefinition;
use anyhow::Result;

#[derive(Parser, Debug, Clone, WorkflowDefinition)]
#[workflow(
    id = "test_workflow",
    name = "Test Workflow",
    description = "A simple test workflow to verify the macro system works"
)]
struct Args {
    /// Input file
    #[arg(short, long, required_unless_present = "workflow_metadata")]
    #[field(
        label = "Input File",
        description = "[FILE PATH] Input file (e.g., 'data.txt')",
        type = "file_path"
    )]
    input: Option<String>,

    /// Batch size
    #[arg(short, long, default_value = "2")]
    #[field(
        label = "Batch Size",
        description = "[NUMBER] Tasks to run in parallel (1-5)",
        type = "number",
        min = "1",
        max = "5"
    )]
    batch_size: usize,

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

    println!("🚀 Test Workflow Running");
    println!("   Input: {}", args.input.as_deref().unwrap_or("none"));
    println!("   Batch Size: {}", args.batch_size);

    Ok(())
}
