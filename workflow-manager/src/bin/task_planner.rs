//! Task Planner Binary
//!
//! Multi-agent task planning orchestrator that transforms high-level
//! implementation requirements into detailed, executable task specifications.
//!
//! ## Usage
//!
//! ```bash
//! # Run complete workflow (all steps)
//! task-planner --impl IMPL.md \
//!              --tasks-overview-template templates/tasks_overview_template.yaml \
//!              --task-template templates/task_template.yaml
//!
//! # Run individual steps
//! task-planner --step 1 --impl IMPL.md --tasks-overview-template templates/overview.yaml
//! task-planner --step 2 --task-template templates/task.yaml
//! task-planner --step 3 --task-template templates/task.yaml
//!
//! # Use simple fixed-size batching
//! task-planner --batch-size 3 --task-template templates/task.yaml
//!
//! # Enable debug output
//! task-planner --debug --task-template templates/task.yaml
//! ```

use anyhow::Result;
use clap::Parser;
use workflow_manager::task_planner::{
    cli::Args,
    workflow::{run_task_planning_workflow, WorkflowConfig},
};
use workflow_manager_sdk::WorkflowDefinition;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let args = Args::parse();

    // Handle --workflow-metadata flag
    if args.workflow_metadata {
        args.print_metadata();
        return Ok(());
    }

    // Validate arguments
    args.validate()?;

    // Convert args to workflow config
    let config = WorkflowConfig::from(args);

    // Run the workflow
    run_task_planning_workflow(config).await?;

    Ok(())
}
