//! Workflow orchestration for the research agent
//!
//! This module contains the main workflow orchestration logic that manages the execution
//! flow across all phases, state tracking, and error handling.
//!
//! The primary entry point is [`run_research_workflow`], which executes the complete
//! multi-phase workflow based on the provided [`WorkflowConfig`].

use anyhow::{Context, Result};
use chrono::Local;
use futures::stream::{FuturesUnordered, StreamExt};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{fs, sync::Semaphore};

use workflow_manager_sdk::{
    log_phase_complete, log_phase_start, log_state_file, log_task_complete, log_task_failed,
    log_task_start,
};

use crate::research::{
    phase0_analyze::analyze_codebase,
    phase1_prompts::generate_prompts,
    phase2_research::execute_research,
    phase3_validate::{execute_fix_yaml, find_yaml_files, validate_yaml_file},
    phase4_synthesize::synthesize_documentation,
    types::{CodebaseAnalysis, PromptsData, ResearchResult},
};

/// Configuration for the research workflow
///
/// This struct contains all configuration options for running the research workflow.
/// Most fields are optional to support resuming from intermediate phases.
///
/// # Examples
///
/// ```no_run
/// use workflow_manager::research::WorkflowConfig;
///
/// // Full workflow with all phases
/// let config = WorkflowConfig {
///     objective: Some("Analyze the API layer".to_string()),
///     phases: vec![0, 1, 2, 3, 4],
///     batch_size: 2,
///     dir: Some(".".to_string()),
///     system_prompt: Some("prompts/writer.md".to_string()),
///     append: Some("prompts/style.md".to_string()),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct WorkflowConfig {
    /// Research objective/question (required for Phase 1)
    pub objective: Option<String>,
    /// Which phases to execute (0-4)
    pub phases: Vec<u32>,
    /// Number of concurrent agents for Phase 2 and Phase 3
    pub batch_size: usize,
    /// Directory to analyze (for Phase 0)
    pub dir: Option<String>,
    /// Path to saved codebase analysis (for resuming from Phase 1)
    pub analysis_file: Option<String>,
    /// Path to saved prompts (for resuming from Phase 2)
    pub prompts_file: Option<String>,
    /// Path to saved results (for resuming from Phase 3 or 4)
    pub results_file: Option<String>,
    /// Directory containing YAML files to validate (for Phase 3)
    pub results_dir: Option<String>,
    /// Output path for final documentation (Phase 4)
    pub output: Option<String>,
    /// System prompt for prompt generation (required for Phase 1)
    pub system_prompt: Option<String>,
    /// Output style template (required for Phase 1)
    pub append: Option<String>,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            objective: None,
            phases: vec![0, 1, 2, 3, 4],
            batch_size: 1,
            dir: None,
            analysis_file: None,
            prompts_file: None,
            results_file: None,
            results_dir: None,
            output: None,
            system_prompt: None,
            append: None,
        }
    }
}

/// Load file content or return literal string
async fn load_prompt_file(file_path: &str) -> Result<String> {
    let path = Path::new(file_path);
    if path.exists() && path.is_file() {
        fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read prompt file: {}", file_path))
    } else {
        Ok(file_path.to_string())
    }
}

/// Run the complete research workflow with the given configuration
///
/// This is the main entry point for executing the research workflow. It orchestrates
/// all phases according to the provided configuration, handles state persistence,
/// and manages error recovery.
///
/// # Arguments
///
/// * `config` - Configuration specifying which phases to run and their parameters
///
/// # Phases Executed
///
/// Based on `config.phases`, the following phases may be executed:
///
/// - **Phase 0**: Analyze codebase structure and save to `OUTPUT/codebase_analysis_*.yaml`
/// - **Phase 1**: Generate research prompts and save to `OUTPUT/research_prompts_*.yaml`
/// - **Phase 2**: Execute research in parallel and save to `RESULTS/research_result_*.yaml`
/// - **Phase 3**: Validate and fix YAML files iteratively until all are valid
/// - **Phase 4**: Synthesize documentation and save to output path
///
/// # Resumability
///
/// The workflow can be resumed from any phase by providing saved state files:
/// - Use `analysis_file` to skip Phase 0
/// - Use `prompts_file` to skip Phases 0-1
/// - Use `results_file` to skip Phases 0-2
///
/// # Errors
///
/// Returns an error if:
/// - Required parameters for a phase are missing
/// - A phase execution fails
/// - File I/O operations fail
///
/// # Examples
///
/// ```no_run
/// use workflow_manager::research::{run_research_workflow, WorkflowConfig};
///
/// # async fn example() -> anyhow::Result<()> {
/// let config = WorkflowConfig {
///     objective: Some("How does authentication work?".to_string()),
///     phases: vec![0, 1, 2, 3, 4],
///     batch_size: 2,
///     dir: Some(".".to_string()),
///     system_prompt: Some("prompts/writer.md".to_string()),
///     append: Some("prompts/style.md".to_string()),
///     ..Default::default()
/// };
///
/// run_research_workflow(config).await?;
/// # Ok(())
/// # }
/// ```
pub async fn run_research_workflow(config: WorkflowConfig) -> Result<()> {
    // Validate required arguments based on phases
    if config.phases.contains(&1) {
        if config.objective.is_none() {
            anyhow::bail!("--input is required when running phase 1");
        }
        if config.system_prompt.is_none() {
            anyhow::bail!("--system-prompt is required when running phase 1");
        }
        if config.append.is_none() {
            anyhow::bail!("--append is required when running phase 1");
        }
    }

    // Change working directory to target directory if specified
    if let Some(dir) = &config.dir {
        let target_dir = PathBuf::from(dir)
            .canonicalize()
            .map_err(|e| anyhow::anyhow!("Invalid directory path '{}': {}", dir, e))?;
        std::env::set_current_dir(&target_dir).map_err(|e| {
            anyhow::anyhow!(
                "Failed to change directory to '{}': {}",
                target_dir.display(),
                e
            )
        })?;
        println!("📁 Working directory: {}", target_dir.display());
        println!();
    }

    // Create directory structure for workflow artifacts
    fs::create_dir_all("./RESULTS")
        .await
        .with_context(|| "Failed to create ./RESULTS directory")?;
    fs::create_dir_all("./OUTPUT")
        .await
        .with_context(|| "Failed to create ./OUTPUT directory")?;

    let mut codebase_analysis: Option<CodebaseAnalysis> = None;
    let mut prompts_data: Option<PromptsData> = None;
    let mut research_results: Vec<ResearchResult> = Vec::new();
    let mut results_file_path: Option<PathBuf> = None;

    // Phase 0: Analyze codebase
    if config.phases.contains(&0) {
        log_phase_start!(0, "Analyze Codebase", 5);
        log_task_start!(
            0,
            "analyze",
            "Analyzing codebase structure and dependencies"
        );

        let codebase_path = config
            .dir
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap());

        let analysis = analyze_codebase(&codebase_path).await?;

        // Save analysis to file
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let analysis_path = PathBuf::from(format!("./OUTPUT/codebase_analysis_{}.yaml", timestamp));
        let analysis_yaml = serde_yaml::to_string(&analysis)?;
        fs::write(&analysis_path, &analysis_yaml)
            .await
            .with_context(|| format!("Failed to write analysis file: {}", analysis_path.display()))?;
        println!("[Phase 0] Analysis saved to: {}", analysis_path.display());

        log_task_complete!("analyze", format!("Saved to {}", analysis_path.display()));
        log_state_file!(0, analysis_path.display().to_string(), "Codebase analysis");
        log_phase_complete!(0, "Analyze Codebase");

        codebase_analysis = Some(analysis);
    } else if let Some(analysis_file) = &config.analysis_file {
        let content = fs::read_to_string(analysis_file)
            .await
            .with_context(|| format!("Failed to read analysis file: {}", analysis_file))?;
        codebase_analysis = Some(
            serde_yaml::from_str(&content)
                .with_context(|| format!("Failed to parse analysis YAML from: {}", analysis_file))?,
        );
        println!("[Phase 0] Loaded analysis from: {}", analysis_file);
    }

    // Phase 1: Generate prompts
    if config.phases.contains(&1) {
        log_phase_start!(1, "Generate Prompts", 5);
        log_task_start!(1, "generate", "Generating research prompts from objective");

        let analysis = codebase_analysis.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Phase 0 must run before Phase 1, or provide --analysis-file")
        })?;

        let prompt_writer = load_prompt_file(config.system_prompt.as_ref().unwrap()).await?;
        let output_style = load_prompt_file(config.append.as_ref().unwrap()).await?;

        let prompts = generate_prompts(
            config.objective.as_ref().unwrap(),
            analysis,
            &prompt_writer,
            &output_style,
        )
        .await?;

        // Save prompts to file
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let prompts_path = PathBuf::from(format!("./OUTPUT/research_prompts_{}.yaml", timestamp));
        let prompts_yaml = serde_yaml::to_string(&prompts)?;
        fs::write(&prompts_path, &prompts_yaml)
            .await
            .with_context(|| format!("Failed to write prompts file: {}", prompts_path.display()))?;
        println!("[Phase 1] Prompts saved to: {}", prompts_path.display());
        println!("Generated {} research prompts", prompts.prompts.len());

        log_task_complete!(
            "generate",
            format!("Generated {} prompts", prompts.prompts.len())
        );
        log_state_file!(
            1,
            prompts_path.display().to_string(),
            "Research prompts for Phase 2"
        );
        log_phase_complete!(1, "Generate Prompts");

        prompts_data = Some(prompts);
    } else if let Some(prompts_file) = &config.prompts_file {
        let content = fs::read_to_string(prompts_file)
            .await
            .with_context(|| format!("Failed to read prompts file: {}", prompts_file))?;
        prompts_data = Some(
            serde_yaml::from_str(&content)
                .with_context(|| format!("Failed to parse prompts YAML from: {}", prompts_file))?,
        );
        println!("[Phase 1] Loaded prompts from: {}", prompts_file);
    }

    // Phase 2: Execute research prompts concurrently
    if config.phases.contains(&2) {
        log_phase_start!(2, "Execute Research", 5);

        let prompts = prompts_data.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Phase 1 must run before Phase 2, or provide --prompts-file")
        })?;

        research_results = execute_research(prompts, config.batch_size).await?;

        // Save research results to file
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let results_path = PathBuf::from(format!("./RESULTS/research_results_{}.yaml", timestamp));
        let results_yaml = serde_yaml::to_string(&research_results)?;
        fs::write(&results_path, &results_yaml)
            .await
            .with_context(|| format!("Failed to write results file: {}", results_path.display()))?;
        println!("\n[Phase 2] Results saved to: {}", results_path.display());

        log_state_file!(
            2,
            results_path.display().to_string(),
            "Research results for Phase 3 validation"
        );
        log_phase_complete!(2, "Execute Research");

        results_file_path = Some(results_path);
    } else if let Some(results_file) = &config.results_file {
        let content = fs::read_to_string(results_file)
            .await
            .with_context(|| format!("Failed to read results file: {}", results_file))?;
        research_results = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse results YAML from: {}", results_file))?;
        println!("[Phase 2] Loaded results from: {}", results_file);
        results_file_path = Some(PathBuf::from(results_file));
    }

    // Phase 3: Validate and fix YAML files
    if config.phases.contains(&3) {
        log_phase_start!(3, "Validate YAML", 5);
        log_task_start!(3, "validate_initial", "Initial YAML validation scan");

        println!("\n{}", "=".repeat(80));
        println!("PHASE 3: Validating YAML Results");
        println!("{}", "=".repeat(80));

        // Determine which files to validate
        let result_files: Vec<String> = if let Some(results_dir) = &config.results_dir {
            // Use directory path to find all YAML files
            println!("Scanning directory for YAML files: {}", results_dir);
            let files = find_yaml_files(results_dir).await?;
            println!("Found {} YAML files", files.len());
            files
        } else if !research_results.is_empty() {
            // Use results from Phase 2
            research_results
                .iter()
                .map(|r| r.response_file.clone())
                .collect()
        } else {
            anyhow::bail!("No YAML files to validate. Run Phase 2 first, provide --results-file, or specify --results-dir");
        };

        let mut validation_tasks = FuturesUnordered::new();
        for file in &result_files {
            let file = file.clone();
            validation_tasks.push(async move { validate_yaml_file(&file).await });
        }

        let mut files_with_errors = Vec::new();
        while let Some(result) = validation_tasks.next().await {
            let (file, is_valid, error) = result?;
            if !is_valid {
                files_with_errors.push((file, error));
            }
        }

        log_task_complete!(
            "validate_initial",
            format!("Found {} files with errors", files_with_errors.len())
        );

        // Loop to fix and re-validate until all are valid
        let mut fix_iteration = 0;
        loop {
            if files_with_errors.is_empty() {
                println!("\n✓ All files validated successfully!");
                break;
            }

            fix_iteration += 1;
            let task_id = format!("fix_iteration_{}", fix_iteration);
            log_task_start!(
                3,
                &task_id,
                format!(
                    "Fixing {} YAML files (iteration {})",
                    files_with_errors.len(),
                    fix_iteration
                )
            );

            println!(
                "\n⚠ Found {} files with errors. Fixing...",
                files_with_errors.len()
            );

            let current_batch = std::mem::take(&mut files_with_errors);

            // Fix all broken files in parallel
            let sem = Arc::new(Semaphore::new(config.batch_size));
            let mut fix_tasks = FuturesUnordered::new();

            for (i, (file, error)) in current_batch.iter().enumerate() {
                let file = file.clone();
                let error = error.clone();
                let sem = sem.clone();
                let fixer_number = i + 1;
                let prefix = format!("[YAML Fixer {}]: ", fixer_number);

                fix_tasks.push(async move {
                    let _permit = sem
                        .acquire()
                        .await
                        .map_err(|_| anyhow::anyhow!("Semaphore closed"))?;

                    let fix_task_id = format!("fix_yaml_{}", fixer_number);
                    log_task_start!(
                        3,
                        &fix_task_id,
                        format!("Fixing YAML file {}", fixer_number)
                    );

                    let result = execute_fix_yaml(&file, &error, Some(&prefix), fixer_number).await;

                    if result.is_ok() {
                        log_task_complete!(&fix_task_id, format!("Fixed {}", file));
                    } else if let Err(ref e) = result {
                        log_task_failed!(&fix_task_id, format!("Failed to fix: {}", e));
                    }

                    result
                });
            }

            // Wait for all fixes to complete (fail-fast on error)
            while let Some(result) = fix_tasks.next().await {
                result?;
            }

            // Re-validate the files we just fixed and repopulate files_with_errors
            for (file, _) in current_batch {
                let (path, is_valid, error_msg) = validate_yaml_file(&file).await?;
                if !is_valid {
                    files_with_errors.push((path, error_msg));
                }
            }

            log_task_complete!(
                &task_id,
                format!("{} files still invalid", files_with_errors.len())
            );
        }

        log_phase_complete!(3, "Validate YAML");
    }

    // Phase 4: Synthesize documentation
    if config.phases.contains(&4) {
        log_phase_start!(4, "Synthesize Docs", 5);

        let results_file = results_file_path.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "No research results file available. Run Phase 2 first or provide --results-file"
            )
        })?;

        let output_path = if let Some(output) = &config.output {
            PathBuf::from(output)
        } else {
            let timestamp = Local::now().format("%Y%m%d_%H%M%S");
            PathBuf::from(format!("./OUTPUT/research_output_{}.md", timestamp))
        };

        synthesize_documentation(results_file, &output_path).await?;

        log_state_file!(
            4,
            output_path.display().to_string(),
            "Final synthesized documentation"
        );
        log_phase_complete!(4, "Synthesize Docs");

        println!("\n{}", "=".repeat(80));
        println!(
            "Research complete! Documentation saved to: {}",
            output_path.display()
        );
        println!("{}", "=".repeat(80));
    } else {
        println!("\n{}", "=".repeat(80));
        println!("Selected phases completed!");
        println!("{}", "=".repeat(80));
    }

    Ok(())
}
