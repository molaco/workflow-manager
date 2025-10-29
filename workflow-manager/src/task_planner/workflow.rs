//! Main workflow orchestrator for task planner

use crate::task_planner::cli::Args;
use crate::task_planner::{phase0_overview, phase1_expand, phase2_review};
use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::fs;
use workflow_manager_sdk::{
    log_phase_complete, log_phase_start, log_state_file, log_task_complete, log_task_start,
};

/// Main workflow function that orchestrates all phases
pub async fn run_workflow(args: Args) -> Result<()> {
    // Validate arguments
    args.validate()
        .map_err(|e| anyhow::anyhow!("Invalid arguments: {}", e))?;

    let phases = args.get_phases();
    let output_dir = PathBuf::from(&args.output_dir);

    // Get codebase directory (for agent exploration)
    let codebase_path = if let Some(dir_str) = &args.dir {
        PathBuf::from(dir_str)
    } else {
        std::env::current_dir()?
    };

    // Change to codebase directory BEFORE any phase execution
    // This ensures logging infrastructure stays consistent throughout execution
    std::env::set_current_dir(&codebase_path)
        .with_context(|| format!("Failed to change to codebase directory: {}", codebase_path.display()))?;
    println!("📁 Working directory: {}", codebase_path.display());

    // Create output directory (now relative to codebase_path)
    fs::create_dir_all(&output_dir)
        .await
        .with_context(|| format!("Failed to create output directory: {}", output_dir.display()))?;

    println!("\n{}", "=".repeat(80));
    println!("TASK PLANNER WORKFLOW");
    println!("{}", "=".repeat(80));
    println!("Phases to execute: {:?}", phases);
    println!("Output directory: {}", output_dir.display());
    println!("{}", "=".repeat(80));

    // Track file paths
    let mut tasks_overview_yaml = String::new();
    let mut tasks_yaml = String::new();
    let mut task_files: Vec<PathBuf> = Vec::new();

    // Phase 0: Generate overview
    if phases.contains(&0) {
        log_phase_start!(0, "Generate Task Overview", 3);
        log_task_start!(
            0,
            "generate_overview",
            "Generating task overview from IMPL.md"
        );

        let impl_file = args.impl_file.as_ref().unwrap();
        let impl_md = fs::read_to_string(impl_file)
            .await
            .with_context(|| format!("Failed to read IMPL file: {}", impl_file))?;

        let overview_template_path = args.overview_template.as_ref().unwrap();
        let overview_template = fs::read_to_string(overview_template_path)
            .await
            .with_context(|| {
                format!(
                    "Failed to read overview template: {}",
                    overview_template_path
                )
            })?;

        tasks_overview_yaml =
            phase0_overview::generate_overview(&impl_md, &overview_template).await?;

        // Save to file
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let overview_path = output_dir.join(format!("tasks_overview_{}.yaml", timestamp));
        fs::write(&overview_path, &tasks_overview_yaml)
            .await
            .with_context(|| format!("Failed to write overview file: {}", overview_path.display()))?;

        println!("✓ Saved: {}", overview_path.display());

        log_task_complete!("generate_overview", format!("Saved to {}", overview_path.display()));
        log_state_file!(0, overview_path.display().to_string(), "Task overview");
        log_phase_complete!(0, "Task overview generated");
    } else if let Some(overview_file) = &args.overview_file {
        // Load existing overview
        tasks_overview_yaml = fs::read_to_string(overview_file)
            .await
            .with_context(|| format!("Failed to read overview file: {}", overview_file))?;
        println!("Loaded overview from: {}", overview_file);
    }

    // Phase 1: Expand tasks
    if phases.contains(&1) {
        log_phase_start!(1, "Expand Tasks", 3);

        let task_template_path = args.task_template.as_ref().unwrap();
        let task_template = fs::read_to_string(task_template_path)
            .await
            .with_context(|| {
                format!("Failed to read task template: {}", task_template_path)
            })?;

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();

        task_files = phase1_expand::expand_tasks(
            &tasks_overview_yaml,
            &task_template,
            args.simple_batching,
            args.batch_size,
            &output_dir,
            &timestamp,
        )
        .await?;

        println!("\n✓ Saved {} task files:", task_files.len());
        for path in &task_files {
            println!("  - {}", path.display());
        }

        log_phase_complete!(1, format!("Expanded {} tasks", task_files.len()));
    } else if let Some(tasks_path) = &args.tasks_file {
        // Load existing tasks - check if it's a directory or file
        let metadata = fs::metadata(tasks_path)
            .await
            .with_context(|| format!("Failed to read path: {}", tasks_path))?;

        if metadata.is_dir() {
            // Load all task_*.yaml files from directory
            let mut entries = fs::read_dir(tasks_path)
                .await
                .with_context(|| format!("Failed to read directory: {}", tasks_path))?;

            let mut task_yamls = Vec::new();

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if let Some(name) = path.file_name() {
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with("task_") && name_str.ends_with(".yaml") {
                        let content = fs::read_to_string(&path)
                            .await
                            .with_context(|| format!("Failed to read task file: {}", path.display()))?;
                        task_yamls.push(content);
                        task_files.push(path);
                    }
                }
            }

            tasks_yaml = task_yamls.join("\n---\n");
            println!("Loaded {} task files from directory: {}", task_files.len(), tasks_path);
        } else {
            // Load single file (backwards compatibility)
            tasks_yaml = fs::read_to_string(tasks_path)
                .await
                .with_context(|| format!("Failed to read tasks file: {}", tasks_path))?;
            println!("Loaded tasks from file: {}", tasks_path);
        }
    }

    // Phase 2: Review tasks
    if phases.contains(&2) {
        log_phase_start!(2, "Review Tasks", 3);

        let impl_file = args.impl_file.as_ref().unwrap();
        let impl_md = fs::read_to_string(impl_file)
            .await
            .with_context(|| format!("Failed to read IMPL file: {}", impl_file))?;

        let task_template_path = args.task_template.as_ref().unwrap();
        let task_template = fs::read_to_string(task_template_path)
            .await
            .with_context(|| {
                format!("Failed to read task template: {}", task_template_path)
            })?;

        phase2_review::review_tasks(
            &tasks_overview_yaml,
            &tasks_yaml,
            &impl_md,
            &task_template,
            args.batch_size,
        )
        .await?;

        log_phase_complete!(2, "Review complete");
    }

    println!("\n{}", "=".repeat(80));
    println!("✓ WORKFLOW COMPLETE");
    println!("{}", "=".repeat(80));

    Ok(())
}
