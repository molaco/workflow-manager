//! Phase 2: Research prompt generation
//!
//! Generates targeted research prompts based on a research objective and codebase analysis.
//!
//! This phase:
//! - Takes a research objective (user's question)
//! - Combines it with the validated codebase analysis from Phase 1
//! - Uses custom system prompts and output style templates
//! - Generates a set of focused research prompts as YAML
//!
//! The result is saved to `OUTPUT/research_prompts_<timestamp>.yaml` for use in Phase 3.

use crate::research::types::{CodebaseAnalysis, PromptsData, ResearchPrompt};
use crate::workflow_utils::{execute_agent, extract_yaml, AgentConfig};
use anyhow::Context;
use claude_agent_sdk::ClaudeAgentOptions;

/// Generate research prompts based on objective and codebase analysis
pub async fn generate_prompts(
    objective: &str,
    codebase_analysis: &CodebaseAnalysis,
    prompt_writer: &str,
    output_style: &str,
) -> anyhow::Result<PromptsData> {
    let task_id = "generate";
    let agent_name = "Prompt Generator";

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

    // Execute agent (handles all stream processing, logging, etc.)
    let config = AgentConfig::new(
        task_id,
        agent_name,
        "Generating research prompts",
        format!("Generate research prompts for: {}", objective),
        options,
    );

    let response_text = execute_agent(config).await?;
    let yaml_content = extract_yaml(&response_text);

    // Parse YAML flexibly first (like Python's yaml.safe_load)
    let prompts_yaml: serde_yaml::Value =
        serde_yaml::from_str(&yaml_content).with_context(|| {
            let error_msg = format!("Failed to parse prompts YAML");

            // If duplicate key error, provide helpful context
            if yaml_content.contains("duplicate") {
                eprintln!("\n❌ YAML PARSING ERROR: Duplicate keys detected in prompts");
                eprintln!("The prompts YAML contains duplicate keys which is invalid.");
                eprintln!("\nGenerated YAML preview (first 500 chars):");
                eprintln!("{}", &yaml_content.chars().take(500).collect::<String>());
            }

            error_msg
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

    println!("Generated {} research prompts", prompts_data.prompts.len());

    Ok(prompts_data)
}
