//! Phase 3: YAML validation and repair
//!
//! Validates and automatically fixes YAML syntax errors in research results.
//!
//! This phase:
//! - Validates all YAML files from Phase 2 using an external Python validator
//! - Identifies files with syntax errors
//! - Uses Claude agents to fix broken YAML files in parallel
//! - Re-validates after each fix iteration
//! - Loops until all files are valid
//!
//! Can run standalone on a directory of YAML files or as part of the full workflow.

use anyhow::Result;
use claude_agent_sdk::{query, ClaudeAgentOptions, ContentBlock, Message, SystemPrompt, SystemPromptPreset};
use futures::StreamExt;
use tokio::fs;
use workflow_manager_sdk::{log_agent_complete, log_agent_failed, log_agent_message, log_agent_start};

/// Validate YAML file using check_yaml.py script
pub async fn validate_yaml_file(file_path: &str) -> Result<(String, bool, String)> {
    use tokio::process::Command;

    let output = Command::new("uv")
        .args([
            "run",
            "/home/molaco/Documents/japanese/SCRIPTS/check_yaml.py",
            file_path,
        ])
        .output()
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined_output = format!("{}{}", stdout, stderr);

    let is_valid = output.status.success()
        && !combined_output.contains("❌")
        && !combined_output.contains("Error");

    Ok((file_path.to_string(), is_valid, combined_output))
}

/// Fix invalid YAML file by querying Claude
pub async fn execute_fix_yaml(
    file_path: &str,
    error_message: &str,
    prefix: Option<&str>,
    fixer_number: usize,
) -> Result<()> {
    let prefix = prefix.unwrap_or("");
    let task_id = format!("fix_yaml_{}", fixer_number);
    let agent_name = format!("YAML Fixer {}", fixer_number);

    log_agent_start!(&task_id, &agent_name, format!("Fixing: {}", file_path));

    println!("\n{}", "-".repeat(80));
    println!("{}FIXING: {}", prefix, file_path);
    println!("{}", "-".repeat(80));

    // Read the broken YAML
    let broken_yaml = fs::read_to_string(file_path).await.map_err(|e| {
        log_agent_failed!(&task_id, &agent_name, format!("Failed to read file: {}", e));
        e
    })?;

    log_agent_message!(
        &task_id,
        &agent_name,
        format!("Read {} bytes from file", broken_yaml.len())
    );
    log_agent_message!(
        &task_id,
        &agent_name,
        format!(
            "Error: {}",
            error_message.lines().next().unwrap_or("Unknown error")
        )
    );

    let fix_prompt = format!(
        r#"The following YAML file has validation errors. Please fix it and output ONLY the corrected YAML.

File: {}

Validation Error:
{}

Current Content:
```yaml
{}
```

Output the fixed YAML wrapped in ```yaml code blocks."#,
        file_path, error_message, broken_yaml
    );

    let preset = SystemPromptPreset {
        prompt_type: "preset".to_string(),
        preset: "claude_code".to_string(),
        append: Some("You are a YAML expert. Fix the YAML syntax errors and output ONLY valid YAML. Do not add explanations.".to_string()),
    };

    let options = ClaudeAgentOptions::builder()
        .system_prompt(SystemPrompt::Preset(preset))
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    let stream = query(&fix_prompt, Some(options)).await?;
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

    // Extract and write fixed YAML
    let fixed_yaml = extract_yaml(&response_text);
    fs::write(file_path, &fixed_yaml).await.map_err(|e| {
        log_agent_failed!(
            &task_id,
            &agent_name,
            format!("Failed to write fixed YAML: {}", e)
        );
        e
    })?;

    println!("\n{}Fixed YAML written to: {}", prefix, file_path);
    log_agent_complete!(
        &task_id,
        &agent_name,
        format!("Fixed and saved {} bytes", fixed_yaml.len())
    );

    Ok(())
}

/// Extract YAML from markdown code blocks and clean document separators
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

/// Find all YAML files in a directory
pub async fn find_yaml_files(dir_path: &str) -> Result<Vec<String>> {
    let mut yaml_files = Vec::new();
    let mut entries = fs::read_dir(dir_path).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "yaml" || ext == "yml" {
                    if let Some(path_str) = path.to_str() {
                        yaml_files.push(path_str.to_string());
                    }
                }
            }
        }
    }

    yaml_files.sort();
    Ok(yaml_files)
}
