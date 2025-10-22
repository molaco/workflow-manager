//! Phase 1: Research prompt generation
//!
//! Generates targeted research prompts based on a research objective and codebase analysis.
//!
//! This phase:
//! - Takes a research objective (user's question)
//! - Combines it with the codebase analysis from Phase 0
//! - Uses custom system prompts and output style templates
//! - Generates a set of focused research prompts as YAML
//!
//! The result is saved to `OUTPUT/research_prompts_<timestamp>.yaml` for use in Phase 2.

use crate::research::types::{CodebaseAnalysis, PromptsData, ResearchPrompt};
use claude_agent_sdk::{query, ClaudeAgentOptions, ContentBlock, Message};
use futures::StreamExt;
use workflow_manager_sdk::{
    log_agent_complete, log_agent_failed, log_agent_message, log_agent_start,
};

/// Generate research prompts based on objective and codebase analysis
pub async fn generate_prompts(
    objective: &str,
    codebase_analysis: &CodebaseAnalysis,
    prompt_writer: &str,
    output_style: &str,
) -> anyhow::Result<PromptsData> {
    let task_id = "generate";
    let agent_name = "Suborchestrator Agent";

    log_agent_start!(task_id, agent_name, "Generating research prompts");

    // Build system prompt with codebase analysis
    let analysis_yaml = serde_yaml::to_string(codebase_analysis)?;
    let system_prompt = format!(
        "{}\n\n# Codebase Analysis\n{}\n\n# Output Style\n{}",
        prompt_writer, analysis_yaml, output_style
    );

    let options = ClaudeAgentOptions::builder()
        .system_prompt(system_prompt)
        .allowed_tools(vec![
            "Read".to_string(),
            "Glob".to_string(),
            "Grep".to_string(),
        ])
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    let query_text = format!("Generate research prompts for: {}", objective);
    let stream = query(&query_text, Some(options)).await?;
    let mut stream = Box::pin(stream);

    let mut response_text = String::new();

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    if let ContentBlock::Text { text } = block {
                        // Collect raw text without printing
                        response_text.push_str(text);
                        // Emit structured agent message for TUI
                        log_agent_message!(task_id, agent_name, text);
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

    let yaml_content = extract_yaml(&response_text);

    // Parse YAML flexibly first (like Python's yaml.safe_load)
    let prompts_yaml: serde_yaml::Value = serde_yaml::from_str(&yaml_content).map_err(|e| {
        let error_msg = format!("Failed to parse prompts YAML: {}", e);
        log_agent_failed!(task_id, agent_name, &error_msg);

        // If duplicate key error, provide helpful context
        if error_msg.contains("duplicate") {
            eprintln!("\n❌ YAML PARSING ERROR: Duplicate keys detected in prompts");
            eprintln!("The prompts YAML contains duplicate keys which is invalid.");
            eprintln!("\nGenerated YAML preview (first 500 chars):");
            eprintln!("{}", &yaml_content.chars().take(500).collect::<String>());
        }

        anyhow::anyhow!("{}", error_msg)
    })?;

    // Extract fields with safe defaults
    let objective = prompts_yaml["objective"]
        .as_str()
        .unwrap_or("Unknown objective")
        .to_string();

    let prompts_array = prompts_yaml["prompts"]
        .as_sequence()
        .cloned()
        .unwrap_or_default();

    let prompts: Vec<ResearchPrompt> = prompts_array
        .iter()
        .filter_map(|p| {
            Some(ResearchPrompt {
                title: p["title"].as_str()?.to_string(),
                query: p["query"].as_str()?.to_string(),
                focus: p["focus"]
                    .as_sequence()
                    .map(|seq| {
                        seq.iter()
                            .filter_map(|f| f.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect();

    let prompts_data = PromptsData { objective, prompts };

    log_agent_complete!(
        task_id,
        agent_name,
        format!("Generated {} prompts", prompts_data.prompts.len())
    );
    Ok(prompts_data)
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
