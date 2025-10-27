//! Main workflow orchestration for the task planning system.
//!
//! This module coordinates the execution of all 3 steps:
//! 1. Generate task overview from IMPL.md
//! 2. Expand tasks into detailed specifications
//! 3. Review and validate expanded tasks

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::task_planner::{
    cli::Args,
    step1_overview::step1_generate_overview,
    step2_expand::step2_expand_all_tasks,
    step3_review::{step3_main_orchestrator_report, step3_review_tasks},
    utils::{load_impl_files, load_impl_md, load_template, save_yaml},
};
use workflow_manager_sdk::{log_phase_complete, log_state_file};

/// Workflow configuration derived from CLI arguments
#[derive(Debug, Clone)]
pub struct WorkflowConfig {
    /// Which step to execute (1, 2, 3, or "all")
    pub step: String,

    /// Path(s) to implementation file(s)
    pub impl_files: Option<Vec<String>>,

    /// Path to tasks_overview.yaml
    pub tasks_overview_path: PathBuf,

    /// Path to tasks.yaml
    pub tasks_path: PathBuf,

    /// Path to review_report.txt
    pub review_report_path: PathBuf,

    /// Stream tasks to file immediately
    pub stream: bool,

    /// Enable debug output
    pub debug: bool,

    /// Use AI-based execution planning
    pub use_ai_planning: bool,

    /// Batch size for task processing
    pub batch_size: usize,

    /// Path to tasks_overview_template.yaml
    pub tasks_overview_template: Option<String>,

    /// Path to task_template.yaml
    pub task_template: Option<String>,

    /// Project root directory
    pub project_root: PathBuf,
}

impl From<Args> for WorkflowConfig {
    fn from(args: Args) -> Self {
        // Use specified directory or current directory
        let project_root = args
            .dir
            .as_ref()
            .map(|d| PathBuf::from(d))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        // Determine paths based on arguments or defaults
        let tasks_overview_path = args
            .tasks_overview
            .clone()
            .map(PathBuf::from)
            .unwrap_or_else(|| project_root.join("tasks_overview.yaml"));

        let tasks_path = args
            .tasks
            .clone()
            .map(PathBuf::from)
            .unwrap_or_else(|| project_root.join("tasks.yaml"));

        let review_report_path = project_root.join("review_report.txt");

        WorkflowConfig {
            step: args.step.clone(),
            impl_files: args.impl_files.clone(),
            tasks_overview_path,
            tasks_path,
            review_report_path,
            stream: args.stream,
            debug: args.debug,
            use_ai_planning: args.use_ai_execution_planning(),
            batch_size: args.get_batch_size().unwrap_or(5),
            tasks_overview_template: args.tasks_overview_template.clone(),
            task_template: args.task_template.clone(),
            project_root,
        }
    }
}

/// Run the complete task planning workflow
///
/// This function orchestrates the execution of steps 1, 2, and/or 3 based on
/// the configuration. It handles file loading, step execution, and state
/// management between steps.
pub async fn run_task_planning_workflow(config: WorkflowConfig) -> Result<()> {
    // Load IMPL.md if needed (step 1 or 3 or all)
    let impl_md = if config.step == "1" || config.step == "3" || config.step == "all" {
        Some(load_implementation_files(&config)?)
    } else {
        None
    };

    // Load templates based on step
    let overview_template = if config.step == "1" || config.step == "all" {
        Some(load_overview_template(&config)?)
    } else {
        None
    };

    let task_template = if config.step == "2" || config.step == "3" || config.step == "all" {
        Some(load_task_template(&config)?)
    } else {
        None
    };

    // Execute workflow steps
    if config.step == "1" || config.step == "all" {
        execute_step1(&config, impl_md.as_ref().unwrap(), overview_template.as_ref().unwrap())
            .await?;

        println!("\n✓ Phase 1 complete");
        log_phase_complete!(1, "Overview Generation");

        if config.step == "1" {
            return Ok(());
        }
    }

    // Load tasks_overview.yaml if not generated in step 1
    let tasks_overview_yaml = if config.step == "1" {
        String::new() // Not needed for step 1 only
    } else if config.step == "all" {
        // Already generated, read it back
        std::fs::read_to_string(&config.tasks_overview_path)
            .context("Failed to read tasks_overview.yaml after step 1")?
    } else {
        // Load existing file
        if !config.tasks_overview_path.exists() {
            anyhow::bail!(
                "tasks_overview.yaml not found at {}. Run step 1 first or specify with --tasks-overview.",
                config.tasks_overview_path.display()
            );
        }
        println!(
            "Loading tasks_overview.yaml from {}",
            config.tasks_overview_path.display()
        );
        std::fs::read_to_string(&config.tasks_overview_path)
            .context("Failed to read tasks_overview.yaml")?
    };

    if config.step == "2" || config.step == "all" {
        execute_step2(
            &config,
            &tasks_overview_yaml,
            task_template.as_ref().unwrap(),
        )
        .await?;

        println!("\n✓ Phase 2 complete");
        log_phase_complete!(2, "Task Expansion");

        if config.step == "2" {
            return Ok(());
        }
    }

    // Load tasks.yaml if not generated in step 2
    let tasks_yaml = if config.step == "2" {
        String::new() // Not needed for step 2 only
    } else if config.step == "all" {
        // Already generated, read it back
        std::fs::read_to_string(&config.tasks_path).context("Failed to read tasks.yaml after step 2")?
    } else {
        // Load existing file
        if !config.tasks_path.exists() {
            anyhow::bail!(
                "tasks.yaml not found at {}. Run step 2 first or specify with --tasks.",
                config.tasks_path.display()
            );
        }
        println!("Loading tasks.yaml from {}", config.tasks_path.display());
        std::fs::read_to_string(&config.tasks_path).context("Failed to read tasks.yaml")?
    };

    if config.step == "3" || config.step == "all" {
        execute_step3(
            &config,
            &tasks_overview_yaml,
            &tasks_yaml,
            impl_md.as_ref().unwrap(),
            task_template.as_ref().unwrap(),
        )
        .await?;

        println!("\n✓ Phase 3 complete");
        log_phase_complete!(3, "Review & Validation");
    }

    Ok(())
}

/// Load implementation files
fn load_implementation_files(config: &WorkflowConfig) -> Result<String> {
    if let Some(ref impl_files) = config.impl_files {
        println!("Loading {} implementation file(s)...", impl_files.len());
        load_impl_files(impl_files)
    } else {
        println!("Auto-detecting IMPL.md...");
        load_impl_md(&config.project_root)
    }
}

/// Load overview template
fn load_overview_template(config: &WorkflowConfig) -> Result<String> {
    let template_path = config
        .tasks_overview_template
        .as_ref()
        .context("tasks_overview_template is required")?;

    println!("Loading tasks_overview_template from {}", template_path);
    load_template(Path::new(template_path))
}

/// Load task template
fn load_task_template(config: &WorkflowConfig) -> Result<String> {
    let template_path = config
        .task_template
        .as_ref()
        .context("task_template is required")?;

    println!("Loading task_template from {}", template_path);
    load_template(Path::new(template_path))
}

/// Execute Step 1: Generate task overview
async fn execute_step1(
    config: &WorkflowConfig,
    impl_md: &str,
    overview_template: &str,
) -> Result<()> {
    let (tasks_overview_yaml, _usage_stats) =
        step1_generate_overview(impl_md, overview_template).await?;

    save_yaml(&tasks_overview_yaml, &config.tasks_overview_path)?;

    Ok(())
}

/// Execute Step 2: Expand tasks
async fn execute_step2(
    config: &WorkflowConfig,
    tasks_overview_yaml: &str,
    task_template: &str,
) -> Result<()> {
    let tasks_yaml = step2_expand_all_tasks(
        tasks_overview_yaml,
        task_template,
        &config.project_root,
        config.stream,
        config.debug,
        config.use_ai_planning,
        config.batch_size,
    )
    .await?;

    // Only save if we actually generated tasks and not streaming
    if !config.stream && !tasks_yaml.trim().is_empty() {
        save_yaml(&tasks_yaml, &config.tasks_path)?;
    } else if config.stream {
        // Streaming mode already saved the file
        println!("✓ Saved: {}", config.tasks_path.display());
        log_state_file!(2, config.tasks_path.display().to_string(), "Expanded tasks (streamed)");
    } else {
        anyhow::bail!("No tasks generated");
    }

    Ok(())
}

/// Execute Step 3: Review tasks
async fn execute_step3(
    config: &WorkflowConfig,
    tasks_overview_yaml: &str,
    tasks_yaml: &str,
    impl_md: &str,
    task_template: &str,
) -> Result<()> {
    let review_results = step3_review_tasks(
        tasks_overview_yaml,
        tasks_yaml,
        impl_md,
        task_template,
        config.batch_size,
        config.debug,
    )
    .await?;

    step3_main_orchestrator_report(&review_results, &config.review_report_path).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_config_from_args() {
        let args = Args {
            step: "all".to_string(),
            dir: None,
            impl_files: Some(vec!["IMPL.md".to_string()]),
            tasks_overview: None,
            tasks: None,
            stream: false,
            debug: true,
            batch_size: Some(3),
            tasks_overview_template: Some("templates/overview.yaml".to_string()),
            task_template: Some("templates/task.yaml".to_string()),
            workflow_metadata: false,
        };

        let config = WorkflowConfig::from(args);

        assert_eq!(config.step, "all");
        assert_eq!(config.debug, true);
        assert_eq!(config.batch_size, 3);
        assert_eq!(config.use_ai_planning, false); // batch_size is set, so no AI planning
    }

    #[test]
    fn test_workflow_config_default_paths() {
        let args = Args {
            step: "2".to_string(),
            dir: None,
            impl_files: None,
            tasks_overview: None,
            tasks: None,
            stream: false,
            debug: false,
            batch_size: None,
            tasks_overview_template: None,
            task_template: Some("task.yaml".to_string()),
            workflow_metadata: false,
        };

        let config = WorkflowConfig::from(args);

        assert!(config.tasks_overview_path.ends_with("tasks_overview.yaml"));
        assert!(config.tasks_path.ends_with("tasks.yaml"));
        assert_eq!(config.use_ai_planning, true); // No batch_size, so use AI
    }
}
