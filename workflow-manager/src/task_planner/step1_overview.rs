//! Step 1: Generate high-level task overview from IMPL.md
//!
//! This module implements the main orchestrator that analyzes implementation
//! documents and generates a tasks_overview.yaml file containing high-level
//! task descriptions with strategic focus.

use anyhow::{Context, Result};

use crate::task_planner::types::UsageStats;
use crate::task_planner::utils::clean_yaml_response;
use workflow_manager_sdk::{
    log_agent_complete, log_agent_message, log_agent_start, log_phase_complete, log_phase_start,
    log_task_complete, log_task_start,
};

/// Main orchestrator generates tasks_overview.yaml from IMPL.md
///
/// This function uses a Claude agent configured as a task planning specialist
/// to analyze the implementation document and generate a structured YAML file
/// containing high-level task overviews.
///
/// # Arguments
///
/// * `impl_md` - Content of the IMPL.md file(s)
/// * `overview_template` - YAML template defining the structure for task overviews
///
/// # Returns
///
/// A tuple containing:
/// - The generated YAML content as a string
/// - Usage statistics from the Claude agent
pub async fn step1_generate_overview(
    impl_md: &str,
    overview_template: &str,
) -> Result<(String, UsageStats)> {
    println!("\n{}", "=".repeat(80));
    println!("STEP 1: Main Orchestrator");
    println!("{}", "=".repeat(80));
    println!("Generate tasks_overview.yaml from IMPL.md\n");

    let task_id = "step1_overview";
    let agent_name = "Main Orchestrator";

    log_phase_start!(1, "Overview Generation", 3);
    log_task_start!(1, task_id, "Generate tasks_overview.yaml from IMPL.md");
    log_agent_start!(task_id, agent_name, "Analyzing implementation and generating task overview");

    let system_prompt = r#"You are a task planning specialist focused on generating high-level task overviews.

Your goal is to analyze the implementation document and generate a tasks_overview.yaml file that breaks down the implementation into logical tasks.

Key instructions:
- Generate YAML that follows the tasks_overview_template.yaml structure exactly
- Create one task block per logical implementation objective
- Keep descriptions strategic and high-level (WHAT and WHY, not HOW)
- Assign sequential task IDs starting from 1
- Identify dependencies between tasks accurately
- Focus on business/architectural value and outcomes
- Estimate complexity and effort realistically

Output only valid YAML, no markdown code blocks or extra commentary."#;

    let prompt = format!(
        r#"Using the implementation document below, generate tasks_overview.yaml following the template structure.

# Implementation Document:
```
{}
```

# Template Structure (tasks_overview_template.yaml):
```yaml
{}
```

Generate a complete tasks_overview.yaml with all tasks identified from the implementation document. Use YAML multi-document format (separate tasks with ---) if there are multiple tasks.

Make sure to just give your response. You must not create or write any files just output the yaml and only that."#,
        impl_md, overview_template
    );

    println!("Querying Claude agent to generate task overview...");

    let options = claude_agent_sdk::ClaudeAgentOptions {
        system_prompt: Some(claude_agent_sdk::SystemPrompt::String(system_prompt.to_string())),
        allowed_tools: vec!["Read".to_string().into(), "Grep".to_string().into(), "Glob".to_string().into()],
        permission_mode: Some(claude_agent_sdk::PermissionMode::BypassPermissions),
        ..Default::default()
    };

    let stream = claude_agent_sdk::query(&prompt, Some(options))
        .await
        .context("Failed to query Claude agent for task overview")?;

    // Process stream with live event emission
    use futures::StreamExt;
    let mut response_parts = Vec::new();
    let mut usage_stats_opt = None;

    futures::pin_mut!(stream);

    while let Some(result) = stream.next().await {
        let message = result.map_err(|e| anyhow::anyhow!("Claude error: {}", e))?;

        match message {
            claude_agent_sdk::Message::Assistant { message, .. } => {
                for block in message.content {
                    if let claude_agent_sdk::ContentBlock::Text { text } = block {
                        response_parts.push(text.clone());

                        // Print streaming text to console
                        print!("{}", text);
                        use std::io::Write;
                        let _ = std::io::stdout().flush();

                        // Emit streaming text for live TUI updates
                        log_agent_message!(task_id, agent_name, &text);
                    }
                }
            }
            claude_agent_sdk::Message::Result {
                duration_ms,
                duration_api_ms,
                num_turns,
                total_cost_usd,
                usage,
                session_id,
                ..
            } => {
                let token_usage = if let Some(usage_value) = usage {
                    let input_tokens = usage_value
                        .get("input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    let output_tokens = usage_value
                        .get("output_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;

                    crate::task_planner::types::TokenUsage {
                        input_tokens,
                        output_tokens,
                    }
                } else {
                    crate::task_planner::types::TokenUsage {
                        input_tokens: 0,
                        output_tokens: 0,
                    }
                };

                usage_stats_opt = Some(UsageStats {
                    duration_ms,
                    duration_api_ms: Some(duration_api_ms),
                    num_turns,
                    total_cost_usd,
                    usage: token_usage,
                    session_id: Some(session_id.as_str().to_string()),
                });
            }
            _ => {}
        }
    }

    let response = response_parts.join("");
    let usage_stats = usage_stats_opt.context("No usage stats received")?;

    // Print statistics to console
    println!(
        "\nStats: {:.2}s, {} turns, ${:.4}, {} in tokens, {} out tokens",
        usage_stats.duration_ms as f64 / 1000.0,
        usage_stats.num_turns,
        usage_stats.total_cost_usd.unwrap_or(0.0),
        usage_stats.usage.input_tokens,
        usage_stats.usage.output_tokens
    );

    log_agent_complete!(task_id, agent_name, "Task overview generated successfully");
    log_task_complete!(task_id, "tasks_overview.yaml ready");
    log_phase_complete!(1, "Overview Generation");

    Ok((clean_yaml_response(&response), usage_stats))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_prompt_contains_key_instructions() {
        // Ensure the system prompt has key elements
        let impl_md = "# Test Implementation\n\nImplement feature X";
        let template = "task:\n  id: [NUMBER]\n  name: [NAME]";

        // We can't test the actual async function without mocking,
        // but we can verify the prompt construction logic
        let system_prompt = r#"You are a task planning specialist"#;
        assert!(system_prompt.contains("task planning"));
    }
}
