//! Phase 3: Research execution with parallel agents
//!
//! Executes research prompts concurrently using multiple Claude agents.
//!
//! This phase:
//! - Takes research prompts from Phase 2
//! - Spawns concurrent agents (configurable batch size)
//! - Each agent executes one research prompt with full tool access
//! - Results are saved as individual YAML files in `RESULTS/`
//! - A summary file `research_results_<timestamp>.yaml` tracks all results
//!
//! Supports configurable concurrency for efficient parallel execution.

use crate::research::types::{PromptsData, ResearchPrompt, ResearchResult};
use crate::workflow_utils::{execute_agent, execute_batch, execute_task, AgentConfig};
use anyhow::Context;
use claude_agent_sdk::{ClaudeAgentOptions, SystemPrompt, SystemPromptPreset};
use tokio::fs;

/// Execute all research prompts concurrently with configurable batch size
pub async fn execute_research(
    prompts_data: &PromptsData,
    batch_size: usize,
) -> anyhow::Result<Vec<ResearchResult>> {
    // Create RESULTS directory if it doesn't exist
    fs::create_dir_all("./RESULTS")
        .await
        .with_context(|| "Failed to create ./RESULTS directory")?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();

    println!("{}", "=".repeat(80));
    println!(
        "PHASE 2: Executing {} Research Prompts (concurrency: {})",
        prompts_data.prompts.len(),
        batch_size
    );
    println!("{}", "=".repeat(80));

    // Use execute_batch to handle parallel execution with concurrency control
    let results = execute_batch(
        2, // phase
        prompts_data.prompts.clone(),
        batch_size,
        move |prompt, ctx| {
            let timestamp = timestamp.clone();
            async move {
                // Execute task with automatic logging
                execute_task(
                    format!("research_{}", ctx.task_number),
                    format!("Research task {}/{}", ctx.task_number, ctx.total_tasks),
                    ctx,
                    || async {
                        let result =
                            execute_research_prompt(&prompt, ctx.task_number, &timestamp).await?;
                        let summary = format!("Saved to {}", result.response_file);
                        Ok((result, summary))
                    },
                )
                .await
            }
        },
    )
    .await?;

    Ok(results)
}

/// Execute a single research prompt with Claude agent
pub async fn execute_research_prompt(
    prompt: &ResearchPrompt,
    result_number: usize,
    timestamp: &str,
) -> anyhow::Result<ResearchResult> {
    let task_id = format!("research_{}", result_number);
    let agent_name = format!("Research Agent {}", result_number);

    println!("\n{}", "-".repeat(80));
    println!("EXECUTING: {}", prompt.title);
    println!("{}", "-".repeat(80));

    // Configure agent with system prompt
    let preset = SystemPromptPreset {
        prompt_type: "preset".to_string(),
        preset: "claude_code".to_string(),
        append: Some(
            "IMPORTANT: DO NOT create or write any files. Output all your research findings as yaml only."
                .to_string(),
        ),
    };

    let options = ClaudeAgentOptions::builder()
        .system_prompt(SystemPrompt::Preset(preset))
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    // Execute agent (handles all stream processing, logging, etc.)
    let config = AgentConfig::new(
        task_id.clone(),
        agent_name,
        format!("Executing: {}", prompt.title),
        prompt.query.clone(),
        options,
    );

    let response_text = execute_agent(config).await?;

    println!("\n");

    // Extract YAML and save to file
    let yaml_content = crate::workflow_utils::extract_yaml(&response_text);
    let response_filename = format!(
        "./RESULTS/research_result_{}_{}.yaml",
        result_number, timestamp
    );
    fs::write(&response_filename, &yaml_content)
        .await
        .with_context(|| format!("Failed to write research result file: {}", response_filename))?;

    println!("Response saved to: {}", response_filename);

    Ok(ResearchResult {
        title: prompt.title.clone(),
        query: prompt.query.clone(),
        response_file: response_filename,
        focus: prompt.focus.clone(),
    })
}
