use clap::Parser;
use workflow_manager::task_planner::{cli::Args, run_workflow};
use workflow_manager_sdk::WorkflowDefinition;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Handle workflow metadata flag
    if args.workflow_metadata {
        args.print_metadata();
        return Ok(());
    }

    run_workflow(args).await
}
