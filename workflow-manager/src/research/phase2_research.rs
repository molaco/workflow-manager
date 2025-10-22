//! Phase 2: Research execution with parallel agents

use crate::research::types::{PromptsData, ResearchPrompt, ResearchResult};
use claude_agent_sdk::{query, ClaudeAgentOptions, ContentBlock, Message, SystemPrompt, SystemPromptPreset};
use futures::{stream::FuturesUnordered, StreamExt};
use std::sync::Arc;
use tokio::{fs, sync::Semaphore};
use workflow_manager_sdk::{
    log_agent_complete, log_agent_failed, log_agent_message, log_agent_start,
    log_task_complete, log_task_start,
};

/// Execute all research prompts concurrently with configurable batch size
pub async fn execute_research(
    prompts_data: &PromptsData,
    batch_size: usize,
) -> anyhow::Result<Vec<ResearchResult>> {
    // Create RESULTS directory if it doesn't exist
    fs::create_dir_all("./RESULTS").await?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let total_prompts = prompts_data.prompts.len();
    let sem = Arc::new(Semaphore::new(batch_size));

    println!("{}", "=".repeat(80));
    println!(
        "PHASE 2: Executing {} Research Prompts (concurrency: {})",
        total_prompts, batch_size
    );
    println!("{}", "=".repeat(80));

    // Push all tasks to FuturesUnordered
    let mut tasks = FuturesUnordered::new();
    for (i, prompt) in prompts_data.prompts.iter().enumerate() {
        let prompt = prompt.clone();
        let timestamp = timestamp.clone();
        let sem = sem.clone();
        let result_number = i + 1;
        let prefix = format!("[Suborchestrator {}]: ", result_number);

        tasks.push(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|_| anyhow::anyhow!("Semaphore closed"))?;

            let task_id = format!("research_{}", result_number);
            log_task_start!(
                2,
                &task_id,
                format!("Research task {}/{}", result_number, total_prompts),
                total_prompts
            );

            let result =
                execute_research_prompt(&prompt, result_number, &timestamp, Some(&prefix))
                    .await?;
            log_task_complete!(&task_id, format!("Completed research {}", result_number));
            Ok::<_, anyhow::Error>(result)
        });
    }

    // Collect results as they complete (fail-fast on first error)
    let mut research_results = Vec::new();
    while let Some(result) = tasks.next().await {
        let research_result = result?;
        research_results.push(research_result);
    }

    Ok(research_results)
}

/// Execute a single research prompt with Claude agent
pub async fn execute_research_prompt(
    prompt: &ResearchPrompt,
    result_number: usize,
    timestamp: &str,
    prefix: Option<&str>,
) -> anyhow::Result<ResearchResult> {
    let prefix = prefix.unwrap_or("");
    let task_id = format!("research_{}", result_number);
    let agent_name = format!("Research Agent {}", result_number);

    log_agent_start!(
        &task_id,
        &agent_name,
        format!("Executing: {}", prompt.title)
    );

    println!("\n{}", "-".repeat(80));
    println!("{}EXECUTING: {}", prefix, prompt.title);
    println!("{}", "-".repeat(80));

    let preset = SystemPromptPreset {
        prompt_type: "preset".to_string(),
        preset: "claude_code".to_string(),
        append: Some("IMPORTANT: DO NOT create or write any files. Output all your research findings as yaml only.".to_string()),
    };

    let options = ClaudeAgentOptions::builder()
        .system_prompt(SystemPrompt::Preset(preset))
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    let stream = query(&prompt.query, Some(options)).await?;
    let mut stream = Box::pin(stream);

    let mut response_text = String::new();

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            if !prefix.is_empty() {
                                println!("{}{}", prefix, text);
                            } else {
                                println!("{}", text);
                            }
                            response_text.push_str(text);
                            log_agent_message!(&task_id, &agent_name, text);
                        }
                        ContentBlock::ToolUse { name, .. } => {
                            log_agent_message!(
                                &task_id,
                                &agent_name,
                                format!("🔧 Using tool: {}", name)
                            );
                        }
                        ContentBlock::ToolResult { tool_use_id, .. } => {
                            log_agent_message!(
                                &task_id,
                                &agent_name,
                                format!("✓ Tool result: {}", tool_use_id)
                            );
                        }
                        _ => {}
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

    println!("\n");

    // Write response directly to file (no serialization issues)
    let yaml_content = extract_yaml(&response_text);
    let response_filename = format!(
        "./RESULTS/research_result_{}_{}.yaml",
        result_number, timestamp
    );
    fs::write(&response_filename, &yaml_content)
        .await
        .map_err(|e| {
            log_agent_failed!(
                &task_id,
                &agent_name,
                format!("Failed to write file: {}", e)
            );
            e
        })?;

    println!("{}Response saved to: {}", prefix, response_filename);
    log_agent_complete!(
        &task_id,
        &agent_name,
        format!("Saved to {}", response_filename)
    );

    Ok(ResearchResult {
        title: prompt.title.clone(),
        query: prompt.query.clone(),
        response_file: response_filename,
        focus: prompt.focus.clone(),
    })
}

/// Extract YAML content from markdown code blocks
fn extract_yaml(text: &str) -> String {
    let yaml = if text.contains("```yaml") {
        let yaml_start = text.find("```yaml").unwrap() + 7;
        let yaml_end = text[yaml_start..]
            .rfind("```")
            .map(|pos| pos + yaml_start)
            .unwrap_or(text.len());
        text[yaml_start..yaml_end].trim().to_string()
    } else if text.contains("```") {
        let yaml_start = text.find("```").unwrap() + 3;
        let yaml_end = text[yaml_start..]
            .rfind("```")
            .map(|pos| pos + yaml_start)
            .unwrap_or(text.len());
        text[yaml_start..yaml_end].trim().to_string()
    } else {
        text.trim().to_string()
    };

    // Remove leading document separator (---) if present
    // serde_yaml::from_str doesn't support multi-document YAML
    yaml.trim_start_matches("---").trim().to_string()
}
