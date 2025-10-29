//! CLI argument parsing for task planner

use clap::Parser;
use workflow_manager_sdk::WorkflowDefinition;

#[derive(Parser, Debug, Clone, WorkflowDefinition)]
#[command(
    name = "task-planner",
    about = "Multi-agent task planning orchestrator",
    long_about = "Transforms high-level implementation requirements (IMPL.md) into detailed, validated task specifications using multi-agent workflow."
)]
#[workflow(
    id = "task-planner",
    name = "Task Planner Workflow",
    description = "Multi-phase task planning: Generate overview → Expand with sub-agents → Review & validate"
)]
pub struct Args {
    /// Comma-separated phases to execute (0=overview, 1=expand, 2=review)
    /// Example: --phases 0,1,2 or --phases 0 for just overview
    #[arg(long, default_value = "0,1,2")]
    #[field(
        label = "Phases to Run",
        description = "[PHASES] Select which phases to execute (0-2)",
        type = "phase_selector",
        total_phases = "3"
    )]
    pub phases: String,

    /// Path to IMPL.md file (required for phases 0 and 2)
    #[arg(long)]
    #[field(
        label = "Implementation File",
        description = "[FILE] Path to IMPL.md with implementation requirements",
        type = "file_path",
        required_for_phases = "0,2"
    )]
    pub impl_file: Option<String>,

    /// Path to tasks_overview_template.yaml (required for phase 0)
    #[arg(long)]
    #[field(
        label = "Overview Template",
        description = "[FILE] Path to tasks_overview_template.yaml",
        type = "file_path",
        required_for_phases = "0"
    )]
    pub overview_template: Option<String>,

    /// Path to task_template.yaml (required for phases 1 and 2)
    #[arg(long)]
    #[field(
        label = "Task Template",
        description = "[FILE] Path to task_template.yaml",
        type = "file_path",
        required_for_phases = "1,2"
    )]
    pub task_template: Option<String>,

    /// Path to saved tasks_overview.yaml (for resuming from Phase 1)
    #[arg(long)]
    #[field(
        label = "Overview File",
        description = "[STATE FILE] Resume with existing task overview",
        type = "state_file",
        pattern = "tasks_overview_*.yaml",
        required_for_phases = "1"
    )]
    pub overview_file: Option<String>,

    /// Path to tasks.yaml (required for Phase 2 if Phase 1 didn't run)
    #[arg(long)]
    #[field(
        label = "Tasks File",
        description = "[STATE FILE] Resume with existing detailed tasks",
        type = "state_file",
        pattern = "tasks_*.yaml",
        required_for_phases = "2"
    )]
    pub tasks_file: Option<String>,

    /// Number of tasks to process in parallel
    /// If specified, enables simple fixed-size batching instead of AI dependency analysis
    #[arg(long, default_value = "5")]
    #[field(
        label = "Batch Size",
        description = "[NUMBER] Parallel execution batch size (1-10)",
        type = "number",
        min = "1",
        max = "10"
    )]
    pub batch_size: usize,

    /// Use simple fixed-size batching instead of AI dependency analysis
    #[arg(long)]
    #[field(
        label = "Simple Batching",
        description = "[TOGGLE] Use fixed-size batches instead of AI dependency analysis",
        type = "boolean"
    )]
    pub simple_batching: bool,

    /// Output directory for generated files
    #[arg(long, default_value = "./OUTPUT")]
    #[field(
        label = "Output Directory",
        description = "[TEXT] Directory for generated files",
        type = "file_path"
    )]
    pub output_dir: String,

    /// Directory path to analyze (codebase directory for agents to explore)
    /// Defaults to current directory
    #[arg(long)]
    #[field(
        label = "Codebase Directory",
        description = "[TEXT] Directory to analyze (default: current directory)",
        type = "file_path"
    )]
    pub dir: Option<String>,

    // Hidden metadata flag
    #[arg(long, hide = true)]
    pub workflow_metadata: bool,
}

impl Args {
    /// Parse which phases to run
    pub fn get_phases(&self) -> Vec<usize> {
        self.phases
            .split(',')
            .filter_map(|p| p.trim().parse().ok())
            .collect()
    }

    /// Validate that required arguments are provided for the selected phases
    pub fn validate(&self) -> Result<(), String> {
        let phases = self.get_phases();

        // Phase 0 requires impl_file and overview_template
        if phases.contains(&0) {
            if self.impl_file.is_none() {
                return Err("--impl-file is required when running phase 0".to_string());
            }
            if self.overview_template.is_none() {
                return Err(
                    "--overview-template is required when running phase 0".to_string()
                );
            }
        }

        // Phase 1 requires task_template and either phase 0 ran or overview_file is provided
        if phases.contains(&1) {
            if self.task_template.is_none() {
                return Err("--task-template is required when running phase 1".to_string());
            }
            if !phases.contains(&0) && self.overview_file.is_none() {
                return Err(
                    "Phase 1 requires either phase 0 to run first or --overview-file to be provided"
                        .to_string(),
                );
            }
        }

        // Phase 2 requires impl_file, task_template, and either phase 0+1 ran or files are provided
        if phases.contains(&2) {
            if self.impl_file.is_none() {
                return Err("--impl-file is required when running phase 2".to_string());
            }
            if self.task_template.is_none() {
                return Err("--task-template is required when running phase 2".to_string());
            }
            if !phases.contains(&0) && self.overview_file.is_none() {
                return Err(
                    "Phase 2 requires either phase 0 to run first or --overview-file to be provided"
                        .to_string(),
                );
            }
            if !phases.contains(&1) && self.tasks_file.is_none() {
                return Err(
                    "Phase 2 requires either phase 1 to run first or --tasks-file to be provided"
                        .to_string(),
                );
            }
        }

        Ok(())
    }
}
