//! Phase 4: YAML validation and repair
//!
//! Validates and automatically fixes YAML syntax errors in research results.
//!
//! This phase:
//! - Validates all YAML files from Phase 3 using an external Python validator
//! - Identifies files with syntax errors
//! - Uses Claude agents to fix broken YAML files in parallel
//! - Re-validates after each fix iteration
//! - Loops until all files are valid
//!
//! Can run standalone on a directory of YAML files or as part of the full workflow.

use crate::workflow_utils::{execute_agent, extract_yaml, AgentConfig};
use anyhow::{Context, Result};
use claude_agent_sdk::{ClaudeAgentOptions, SystemPrompt, SystemPromptPreset};
use tokio::fs;

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
    _prefix: Option<&str>,
    fixer_number: usize,
) -> Result<()> {
    let task_id = format!("fix_yaml_{}", fixer_number);
    let agent_name = format!("YAML Fixer {}", fixer_number);

    println!("\n{}", "-".repeat(80));
    println!("FIXING: {}", file_path);
    println!("{}", "-".repeat(80));

    // Read the broken YAML
    let broken_yaml = fs::read_to_string(file_path)
        .await
        .with_context(|| format!("Failed to read file for YAML fixing: {}", file_path))?;

    println!("Read {} bytes from file", broken_yaml.len());
    println!(
        "Error: {}",
        error_message.lines().next().unwrap_or("Unknown error")
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

    // Execute agent (handles all stream processing, logging, etc.)
    let config = AgentConfig::new(
        task_id,
        agent_name,
        format!("Fixing YAML: {}", file_path),
        fix_prompt,
        options,
    );

    let response_text = execute_agent(config).await?;

    // Extract and write fixed YAML
    let fixed_yaml = extract_yaml(&response_text);
    fs::write(file_path, &fixed_yaml)
        .await
        .with_context(|| format!("Failed to write fixed YAML to: {}", file_path))?;

    println!("\nFixed YAML written to: {}", file_path);
    println!("Saved {} bytes", fixed_yaml.len());

    Ok(())
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
