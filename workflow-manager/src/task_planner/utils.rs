//! Utility functions for task planning workflow.
//!
//! Provides helper functions for:
//! - Loading templates and implementation files
//! - Parsing and cleaning YAML content
//! - Extracting responses from Claude agents
//! - File I/O operations

use anyhow::{Context, Result};
use futures::StreamExt;
use std::path::Path;

use crate::task_planner::types::{DetailedTask, TaskOverview, TokenUsage, UsageStats};

// ============================================================================
// File Loading Utilities
// ============================================================================

/// Load a YAML template from the given path
pub fn load_template(template_path: &Path) -> Result<String> {
    std::fs::read_to_string(template_path)
        .with_context(|| format!("Failed to load template: {}", template_path.display()))
}

/// Load IMPL.md from project root or DOCS/ directory
pub fn load_impl_md(project_root: &Path) -> Result<String> {
    let possible_paths = vec![
        project_root.join("IMPL.md"),
        project_root.join("DOCS").join("IMPL.md"),
    ];

    for path in possible_paths {
        if path.exists() {
            return std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()));
        }
    }

    anyhow::bail!("IMPL.md not found in project root or DOCS/")
}

/// Load multiple implementation files and combine them
pub fn load_impl_files(paths: &[String]) -> Result<String> {
    let mut parts = Vec::new();

    for path_str in paths {
        let path = Path::new(path_str);
        if !path.exists() {
            anyhow::bail!("Implementation file not found: {}", path.display());
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        if paths.len() > 1 {
            // Add filename separator for multiple files
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            parts.push(format!("# Source: {}\n\n{}", filename, content));
        } else {
            parts.push(content);
        }
    }

    Ok(parts.join("\n\n---\n\n"))
}

// ============================================================================
// File Saving Utilities
// ============================================================================

/// Save YAML data to file with logging
pub fn save_yaml(data: &str, output_path: &Path) -> Result<()> {
    std::fs::write(output_path, data)
        .with_context(|| format!("Failed to write to {}", output_path.display()))?;

    println!("✓ Saved: {}", output_path.display());
    Ok(())
}

// ============================================================================
// YAML Processing Utilities
// ============================================================================

/// Clean YAML response by removing markdown code blocks if present
pub fn clean_yaml_response(response: &str) -> String {
    if let Some(start) = response.find("```yaml") {
        // Find the closing ```
        if let Some(end_start) = response[start + 7..].find("```") {
            return response[start + 7..start + 7 + end_start]
                .trim()
                .to_string();
        }
    } else if let Some(start) = response.find("```") {
        // Generic code block
        if let Some(end_start) = response[start + 3..].find("```") {
            return response[start + 3..start + 3 + end_start]
                .trim()
                .to_string();
        }
    }
    response.to_string()
}

// ============================================================================
// YAML Parsing Utilities
// ============================================================================

/// Parse tasks_overview.yaml and extract task list
pub fn parse_tasks_overview(yaml_content: &str) -> Result<Vec<TaskOverview>> {
    use serde::Deserialize;

    // Parse multi-document YAML using Deserializer
    let mut tasks = Vec::new();

    for document in serde_yaml::Deserializer::from_str(yaml_content) {
        let value = serde_yaml::Value::deserialize(document)
            .context("Failed to parse YAML document")?;

        // Check if this document has a "task" field
        if let Some(mapping) = value.as_mapping() {
            let task_key = serde_yaml::Value::String("task".to_string());
            if mapping.contains_key(&task_key) {
                let task: TaskOverview = serde_yaml::from_value(value)
                    .context("Failed to deserialize TaskOverview")?;
                tasks.push(task);
            }
        }
    }

    // If no tasks found, try parsing as a single document
    if tasks.is_empty() {
        let task: TaskOverview = serde_yaml::from_str(yaml_content)
            .context("Failed to parse as single TaskOverview")?;
        tasks.push(task);
    }

    Ok(tasks)
}

/// Parse detailed tasks from tasks.yaml
pub fn parse_detailed_tasks(yaml_content: &str) -> Result<Vec<DetailedTask>> {
    use serde::Deserialize;

    // Parse multi-document YAML using Deserializer
    let mut tasks = Vec::new();

    for document in serde_yaml::Deserializer::from_str(yaml_content) {
        let value = serde_yaml::Value::deserialize(document)
            .context("Failed to parse YAML document")?;

        // Check if this document has a "task" field
        if let Some(mapping) = value.as_mapping() {
            let task_key = serde_yaml::Value::String("task".to_string());
            if mapping.contains_key(&task_key) {
                let task: DetailedTask = serde_yaml::from_value(value)
                    .context("Failed to deserialize DetailedTask")?;
                tasks.push(task);
            }
        }
    }

    // If no tasks found, try parsing as a single document
    if tasks.is_empty() {
        let task: DetailedTask = serde_yaml::from_str(yaml_content)
            .context("Failed to parse as single DetailedTask")?;
        tasks.push(task);
    }

    Ok(tasks)
}

// ============================================================================
// Agent Response Extraction
// ============================================================================

/// Extract text and usage stats from agent response stream
///
/// This function consumes a message stream from the Claude agent SDK and extracts:
/// - Text content from assistant messages
/// - Usage statistics from result messages
pub async fn extract_text_and_stats(
    stream: impl futures::Stream<Item = Result<claude_agent_sdk::Message>>,
) -> Result<(String, UsageStats)> {
    let mut response_parts = Vec::new();
    let mut usage_stats = None;

    futures::pin_mut!(stream);

    while let Some(result) = stream.next().await {
        let message = result.context("Failed to receive message from stream")?;

        match message {
            claude_agent_sdk::Message::Assistant { message, .. } => {
                // Extract text from content blocks
                for block in message.content {
                    if let claude_agent_sdk::ContentBlock::Text { text } = block {
                        response_parts.push(text);
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
                // Parse usage from JSON value
                let token_usage = if let Some(usage_value) = usage {
                    let input_tokens = usage_value
                        .get("input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    let output_tokens = usage_value
                        .get("output_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;

                    TokenUsage {
                        input_tokens,
                        output_tokens,
                    }
                } else {
                    TokenUsage {
                        input_tokens: 0,
                        output_tokens: 0,
                    }
                };

                usage_stats = Some(UsageStats {
                    duration_ms,
                    duration_api_ms: Some(duration_api_ms),
                    num_turns,
                    total_cost_usd,
                    usage: token_usage,
                    session_id: Some(session_id.as_str().to_string()),
                });
            }
            _ => {
                // Ignore other message types (User, System, StreamEvent)
            }
        }
    }

    // Verify we received at least one assistant message
    if response_parts.is_empty() {
        anyhow::bail!("No assistant message found - stream may have been interrupted or aborted");
    }

    let text = response_parts.join("\n");
    let stats = usage_stats.ok_or_else(|| anyhow::anyhow!("No usage stats received"))?;

    Ok((text, stats))
}

/// Extract text from a query response stream (convenience wrapper)
///
/// This is a simplified version that only returns the text content
pub async fn extract_text_from_query(
    stream: impl futures::Stream<Item = Result<claude_agent_sdk::Message>>,
) -> Result<String> {
    let (text, _) = extract_text_and_stats(stream).await?;
    Ok(text)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_yaml_response_with_yaml_fence() {
        let response = r#"Here is the YAML:
```yaml
task:
  id: 1
  name: Test
```
That's it!"#;

        let cleaned = clean_yaml_response(response);
        assert_eq!(cleaned, "task:\n  id: 1\n  name: Test");
    }

    #[test]
    fn test_clean_yaml_response_with_generic_fence() {
        let response = r#"```
task:
  id: 1
```"#;

        let cleaned = clean_yaml_response(response);
        assert_eq!(cleaned, "task:\n  id: 1");
    }

    #[test]
    fn test_clean_yaml_response_without_fence() {
        let response = "task:\n  id: 1\n  name: Test";
        let cleaned = clean_yaml_response(response);
        assert_eq!(cleaned, response);
    }

    #[test]
    fn test_load_impl_files_single() {
        use std::io::Write;
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_impl.md");

        // Create test file
        let mut file = std::fs::File::create(&test_file).unwrap();
        writeln!(file, "# Test Implementation").unwrap();

        // Load single file
        let paths = vec![test_file.to_str().unwrap().to_string()];
        let result = load_impl_files(&paths).unwrap();

        assert!(result.contains("# Test Implementation"));
        assert!(!result.contains("Source:"));

        // Cleanup
        std::fs::remove_file(test_file).ok();
    }

    #[test]
    fn test_load_impl_files_multiple() {
        use std::io::Write;
        let temp_dir = std::env::temp_dir();
        let test_file1 = temp_dir.join("test_impl1.md");
        let test_file2 = temp_dir.join("test_impl2.md");

        // Create test files
        let mut file1 = std::fs::File::create(&test_file1).unwrap();
        writeln!(file1, "# Implementation 1").unwrap();

        let mut file2 = std::fs::File::create(&test_file2).unwrap();
        writeln!(file2, "# Implementation 2").unwrap();

        // Load multiple files
        let paths = vec![
            test_file1.to_str().unwrap().to_string(),
            test_file2.to_str().unwrap().to_string(),
        ];
        let result = load_impl_files(&paths).unwrap();

        assert!(result.contains("# Source: test_impl1.md"));
        assert!(result.contains("# Implementation 1"));
        assert!(result.contains("# Source: test_impl2.md"));
        assert!(result.contains("# Implementation 2"));

        // Cleanup
        std::fs::remove_file(test_file1).ok();
        std::fs::remove_file(test_file2).ok();
    }
}
