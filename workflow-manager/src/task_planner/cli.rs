//! CLI argument definitions for the task planner workflow.
//!
//! This module defines command-line arguments using clap, with WorkflowDefinition
//! derive macro for integration with the workflow manager system.

use anyhow::Result;
use clap::Parser;
use workflow_manager_sdk::WorkflowDefinition;

/// Multi-agent task planning orchestrator
///
/// This CLI tool implements a 3-step workflow for transforming high-level
/// implementation requirements into detailed, executable task specifications:
///
/// - Step 1: Generate high-level task overview from IMPL.md
/// - Step 2: Expand tasks into detailed specifications
/// - Step 3: Review and validate expanded tasks
#[derive(Parser, WorkflowDefinition, Debug, Clone)]
#[workflow(
    id = "task-planner",
    name = "Task Planner",
    description = "Multi-agent task planning orchestrator"
)]
#[command(name = "task-planner")]
#[command(about = "Multi-agent task planning orchestrator")]
#[command(version)]
pub struct Args {
    /// Which step to run (1=overview, 2=expand, 3=review, all=complete workflow)
    #[arg(long, value_name = "STEP", default_value = "all")]
    pub step: String,

    /// Path(s) to implementation file(s) - can specify multiple files
    ///
    /// If multiple files are provided, they will be combined with separators.
    /// If not specified, attempts to auto-detect IMPL.md in project root or DOCS/.
    #[arg(long = "impl", value_name = "PATH")]
    pub impl_files: Option<Vec<String>>,

    /// Path to tasks_overview.yaml
    ///
    /// Used as input for steps 2 and 3, or as output for step 1.
    /// Defaults to ./tasks_overview.yaml
    #[arg(long, value_name = "PATH")]
    pub tasks_overview: Option<String>,

    /// Path to tasks.yaml
    ///
    /// Used as input for step 3, or as output for step 2.
    /// Defaults to ./tasks.yaml
    #[arg(long, value_name = "PATH")]
    pub tasks: Option<String>,

    /// Stream tasks to file immediately (reduces memory usage)
    ///
    /// When enabled, task specifications are written to the output file
    /// as they are generated, rather than accumulating in memory.
    #[arg(long)]
    pub stream: bool,

    /// Enable debug output
    ///
    /// Prints detailed debug information including batch plans, YAML content,
    /// and intermediate agent outputs.
    #[arg(long)]
    pub debug: bool,

    /// Use simple fixed-size batching with specified size
    ///
    /// If specified, uses fixed batch sizes instead of AI-based dependency analysis.
    /// For example, --batch-size 5 will group tasks into batches of 5.
    #[arg(long, value_name = "SIZE")]
    pub batch_size: Option<usize>,

    /// Path to tasks_overview_template.yaml
    ///
    /// Required for step 1. Defines the structure for task overview YAML.
    #[arg(long, value_name = "PATH")]
    pub tasks_overview_template: Option<String>,

    /// Path to task_template.yaml
    ///
    /// Required for steps 2 and 3. Defines the structure for detailed task YAML.
    #[arg(long, value_name = "PATH")]
    pub task_template: Option<String>,

    /// Print workflow metadata and exit
    ///
    /// Outputs JSON metadata about this workflow for integration with
    /// the workflow manager TUI.
    #[arg(long)]
    pub workflow_metadata: bool,
}

impl Args {
    /// Validate arguments for step 1 (generate overview)
    pub fn validate_step1(&self) -> Result<()> {
        if self.tasks_overview_template.is_none() {
            anyhow::bail!(
                "Step 1 requires --tasks-overview-template to define the output structure"
            );
        }
        Ok(())
    }

    /// Validate arguments for step 2 or 3 (expand/review)
    pub fn validate_step2_or_3(&self) -> Result<()> {
        if self.task_template.is_none() {
            anyhow::bail!(
                "Steps 2 and 3 require --task-template to define the task specification structure"
            );
        }
        Ok(())
    }

    /// Validate arguments for the selected step
    pub fn validate(&self) -> Result<()> {
        match self.step.as_str() {
            "1" => self.validate_step1(),
            "2" | "3" => self.validate_step2_or_3(),
            "all" => {
                self.validate_step1()?;
                self.validate_step2_or_3()?;
                Ok(())
            }
            _ => anyhow::bail!("Invalid step: '{}'. Must be 1, 2, 3, or 'all'", self.step),
        }
    }

    /// Get the default batch size for task expansion
    pub fn get_batch_size(&self) -> Option<usize> {
        self.batch_size.or(Some(5))
    }

    /// Determine if AI-based execution planning should be used
    pub fn use_ai_execution_planning(&self) -> bool {
        self.batch_size.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_step1() {
        let mut args = Args {
            step: "1".to_string(),
            impl_files: None,
            tasks_overview: None,
            tasks: None,
            stream: false,
            debug: false,
            batch_size: None,
            tasks_overview_template: None,
            task_template: None,
            workflow_metadata: false,
        };

        // Should fail without template
        assert!(args.validate_step1().is_err());

        // Should pass with template
        args.tasks_overview_template = Some("template.yaml".to_string());
        assert!(args.validate_step1().is_ok());
    }

    #[test]
    fn test_validate_step2() {
        let mut args = Args {
            step: "2".to_string(),
            impl_files: None,
            tasks_overview: None,
            tasks: None,
            stream: false,
            debug: false,
            batch_size: None,
            tasks_overview_template: None,
            task_template: None,
            workflow_metadata: false,
        };

        // Should fail without template
        assert!(args.validate_step2_or_3().is_err());

        // Should pass with template
        args.task_template = Some("template.yaml".to_string());
        assert!(args.validate_step2_or_3().is_ok());
    }

    #[test]
    fn test_use_ai_execution_planning() {
        let args = Args {
            step: "all".to_string(),
            impl_files: None,
            tasks_overview: None,
            tasks: None,
            stream: false,
            debug: false,
            batch_size: None,
            tasks_overview_template: Some("t1.yaml".to_string()),
            task_template: Some("t2.yaml".to_string()),
            workflow_metadata: false,
        };

        assert!(args.use_ai_execution_planning());

        let args_with_batch = Args {
            batch_size: Some(3),
            ..args
        };

        assert!(!args_with_batch.use_ai_execution_planning());
    }
}
