//! Agent execution utilities with stream handling and sub-agent detection

use anyhow::Result;
use claude_agent_sdk::{query, ClaudeAgentOptions, ContentBlock, Message};
use futures::{Stream, StreamExt};
use std::collections::HashMap;
use workflow_manager_sdk::{log_agent_complete, log_agent_failed, log_agent_message, log_agent_start};

/// Configuration for agent execution
pub struct AgentConfig {
    /// Task ID this agent belongs to
    pub task_id: String,
    /// Agent name (for logging)
    pub agent_name: String,
    /// Description of what this agent is doing
    pub description: String,
    /// Prompt to send to the agent
    pub prompt: String,
    /// Claude agent options (system prompt, tools, sub-agents, etc.)
    pub options: ClaudeAgentOptions,
}

impl AgentConfig {
    /// Create a new agent configuration
    pub fn new(
        task_id: impl Into<String>,
        agent_name: impl Into<String>,
        description: impl Into<String>,
        prompt: impl Into<String>,
        options: ClaudeAgentOptions,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            agent_name: agent_name.into(),
            description: description.into(),
            prompt: prompt.into(),
            options,
        }
    }
}

/// Execute a sub-orchestrator agent with automatic stream handling
///
/// Handles:
/// - Agent start/complete/failed logging
/// - Stream processing with TUI logging
/// - Sub-agent delegation detection
/// - Text, tool use, and tool result logging
///
/// Returns the full response text collected from all Text blocks.
///
/// # Example
/// ```rust
/// let config = AgentConfig {
///     task_id: "research_1".to_string(),
///     agent_name: "Research Agent".to_string(),
///     description: "Researching authentication".to_string(),
///     prompt: "How does authentication work?".to_string(),
///     options: ClaudeAgentOptions::builder()
///         .system_prompt("You are a researcher...")
///         .build(),
/// };
///
/// let response = execute_agent(config).await?;
/// ```
pub async fn execute_agent(config: AgentConfig) -> Result<String> {
    log_agent_start!(&config.task_id, &config.agent_name, &config.description);

    eprintln!("[DEBUG] execute_agent called: task_id={}, agent_name={}", config.task_id, config.agent_name);

    // Query Claude
    eprintln!("[DEBUG] Calling query() with prompt length: {}", config.prompt.len());
    let stream = query(&config.prompt, Some(config.options))
        .await
        .map_err(|e| {
            eprintln!("[DEBUG] query() failed: {}", e);
            log_agent_failed!(&config.task_id, &config.agent_name, e.to_string());
            e
        })?;

    eprintln!("[DEBUG] query() succeeded, starting stream handling");

    // Handle stream
    match handle_stream(stream, &config.task_id, &config.agent_name).await {
        Ok(response) => {
            eprintln!("[DEBUG] Stream handling complete, response length: {}", response.len());
            log_agent_complete!(&config.task_id, &config.agent_name, "Completed");
            Ok(response)
        }
        Err(e) => {
            eprintln!("[DEBUG] Stream handling failed: {}", e);
            log_agent_failed!(&config.task_id, &config.agent_name, e.to_string());
            Err(e)
        }
    }
}

/// Tracks active sub-agent delegations
struct DelegationTracker {
    active: HashMap<String, String>,
}

impl DelegationTracker {
    fn new() -> Self {
        Self {
            active: HashMap::new(),
        }
    }

    fn start_delegation(&mut self, tool_use_id: String, subagent_name: String) {
        self.active.insert(tool_use_id, subagent_name);
    }

    fn complete_delegation(&mut self, tool_use_id: &str) -> Option<String> {
        self.active.remove(tool_use_id)
    }
}

/// Extract subagent name from Task tool input
///
/// Checks multiple fields for subagent identification:
/// 1. `subagent_type` field (explicit)
/// 2. `prompt`, `message`, or `description` fields starting with `@` (implicit)
fn extract_subagent_name(input: &serde_json::Value) -> Option<String> {
    // Try explicit subagent_type field
    if let Some(subagent_type) = input.get("subagent_type") {
        if let Some(name) = subagent_type.as_str() {
            return Some(name.to_string());
        }
    }

    // Try extracting from prompt/message/description with @prefix
    for field in ["prompt", "message", "description"] {
        if let Some(text) = input.get(field).and_then(|v| v.as_str()) {
            // Look for @prefix at start or after whitespace
            for word in text.split_whitespace() {
                if let Some(agent_name) = word.strip_prefix('@') {
                    // Return just the agent name (everything up to next special char)
                    let name = agent_name
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                        .collect::<String>();
                    if !name.is_empty() {
                        return Some(name);
                    }
                }
            }
        }
    }

    None
}

/// Extract detailed information from tool use for better logging
///
/// Returns a formatted string with tool-specific details extracted from input JSON
fn extract_tool_details(tool_name: &str, input: &serde_json::Value) -> String {
    match tool_name {
        "Read" => {
            if let Some(file_path) = input.get("file_path").and_then(|v| v.as_str()) {
                format!("📖 Reading: {}", file_path)
            } else {
                format!("📖 Reading file")
            }
        }

        "Write" => {
            if let Some(file_path) = input.get("file_path").and_then(|v| v.as_str()) {
                format!("✍️  Writing: {}", file_path)
            } else {
                format!("✍️  Writing file")
            }
        }

        "Edit" => {
            if let Some(file_path) = input.get("file_path").and_then(|v| v.as_str()) {
                format!("✏️  Editing: {}", file_path)
            } else {
                format!("✏️  Editing file")
            }
        }

        "Bash" => {
            if let Some(command) = input.get("command").and_then(|v| v.as_str()) {
                // Truncate long commands
                let display_cmd = if command.len() > 80 {
                    format!("{}...", &command[..77])
                } else {
                    command.to_string()
                };
                format!("⚡ Running: {}", display_cmd)
            } else {
                format!("⚡ Running bash command")
            }
        }

        "Grep" => {
            let pattern = input
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let path = input
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            format!("🔍 Searching: \"{}\" in {}", pattern, path)
        }

        "Glob" => {
            if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
                format!("📂 Finding: {}", pattern)
            } else {
                format!("📂 Finding files")
            }
        }

        "WebSearch" => {
            if let Some(query) = input.get("query").and_then(|v| v.as_str()) {
                let display_query = if query.len() > 60 {
                    format!("{}...", &query[..57])
                } else {
                    query.to_string()
                };
                format!("🌐 Searching: {}", display_query)
            } else {
                format!("🌐 Web search")
            }
        }

        "WebFetch" => {
            if let Some(url) = input.get("url").and_then(|v| v.as_str()) {
                format!("🌐 Fetching: {}", url)
            } else {
                format!("🌐 Fetching URL")
            }
        }

        "Task" => {
            // This is handled separately for sub-agent detection
            if let Some(subagent) = extract_subagent_name(input) {
                // Get first part of prompt/message for context
                let context = ["prompt", "message", "description"]
                    .iter()
                    .find_map(|field| input.get(field).and_then(|v| v.as_str()))
                    .and_then(|text| {
                        // Get first 60 chars after removing @agent-name
                        let without_mention = text.replace(&format!("@{}", subagent), "").trim().to_string();
                        if without_mention.is_empty() {
                            None
                        } else {
                            Some(if without_mention.len() > 60 {
                                format!("{}...", &without_mention[..57])
                            } else {
                                without_mention
                            })
                        }
                    });

                if let Some(ctx) = context {
                    format!("🤝 Delegating to @{}: {}", subagent, ctx)
                } else {
                    format!("🤝 Delegating to @{}", subagent)
                }
            } else {
                format!("🔧 Using tool: Task")
            }
        }

        // Generic handling for other tools
        _ => format!("🔧 Using tool: {}", tool_name),
    }
}

/// Handle agent stream - logs everything to TUI and stdout, collects text
///
/// Features:
/// - Logs all text content to TUI and prints to stdout
/// - Detects and logs sub-agent delegations (Task tool with @agent)
/// - Logs tool usage
/// - Tracks tool results and matches them to delegations
/// - Returns full response text
async fn handle_stream(
    stream: impl Stream<Item = claude_agent_sdk::error::Result<Message>>,
    task_id: &str,
    agent_name: &str,
) -> Result<String> {
    let mut response_text = String::new();
    let mut stream = Box::pin(stream);
    let mut delegations = DelegationTracker::new();
    let mut message_count = 0;

    eprintln!("[DEBUG] handle_stream starting for task_id={}, agent_name={}", task_id, agent_name);

    while let Some(message) = stream.next().await {
        message_count += 1;
        eprintln!("[DEBUG] Received message #{}", message_count);

        match message? {
            Message::Assistant { message, .. } => {
                eprintln!("[DEBUG] Assistant message with {} content blocks", message.content.len());
                for (i, block) in message.content.iter().enumerate() {
                    match block {
                        ContentBlock::Text { text } => {
                            eprintln!("[DEBUG] Block {}: Text ({} chars)", i, text.len());
                            // Print to stdout
                            println!("{}", text);
                            // Log to TUI
                            log_agent_message!(task_id, agent_name, text);
                            // Collect for return
                            response_text.push_str(text);
                        }

                        ContentBlock::ToolUse { id, name, input } => {
                            eprintln!("[DEBUG] Block {}: ToolUse - name={}, id={}", i, name, id);
                            // Extract detailed tool information
                            let tool_details = extract_tool_details(name, input);
                            log_agent_message!(task_id, agent_name, &tool_details);

                            // Track sub-agent delegations
                            if name == "Task" {
                                if let Some(subagent_name) = extract_subagent_name(input) {
                                    eprintln!("[DEBUG] Detected sub-agent delegation: @{}", subagent_name);
                                    delegations.start_delegation(id.clone(), subagent_name);
                                }
                            }
                        }

                        ContentBlock::ToolResult { tool_use_id, .. } => {
                            eprintln!("[DEBUG] Block {}: ToolResult - tool_use_id={}", i, tool_use_id);
                            // Check if this was a sub-agent delegation
                            if let Some(subagent_name) = delegations.complete_delegation(tool_use_id)
                            {
                                eprintln!("[DEBUG] Sub-agent completed: @{}", subagent_name);
                                log_agent_message!(
                                    task_id,
                                    agent_name,
                                    format!("✓ Sub-agent @{} completed", subagent_name)
                                );
                            } else {
                                log_agent_message!(
                                    task_id,
                                    agent_name,
                                    format!("✓ Tool result: {}", tool_use_id)
                                );
                            }
                        }

                        _ => {
                            eprintln!("[DEBUG] Block {}: Other block type", i);
                        }
                    }
                }
            }
            Message::Result { .. } => {
                eprintln!("[DEBUG] Received Result message, ending stream");
                break;
            }
            _ => {
                eprintln!("[DEBUG] Received other message type");
            }
        }
    }

    eprintln!("[DEBUG] handle_stream complete: {} messages processed, {} chars collected", message_count, response_text.len());

    Ok(response_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_subagent_name_explicit() {
        let input = serde_json::json!({
            "subagent_type": "file-condenser"
        });

        assert_eq!(
            extract_subagent_name(&input),
            Some("file-condenser".to_string())
        );
    }

    #[test]
    fn test_extract_subagent_name_from_prompt() {
        let input = serde_json::json!({
            "prompt": "@file-condenser please process this file"
        });

        assert_eq!(
            extract_subagent_name(&input),
            Some("file-condenser".to_string())
        );
    }

    #[test]
    fn test_extract_subagent_name_from_message() {
        let input = serde_json::json!({
            "message": "Using @reviewer to validate"
        });

        assert_eq!(
            extract_subagent_name(&input),
            Some("reviewer".to_string())
        );
    }

    #[test]
    fn test_extract_subagent_name_none() {
        let input = serde_json::json!({
            "prompt": "No subagent here"
        });

        assert_eq!(extract_subagent_name(&input), None);
    }

    #[test]
    fn test_delegation_tracker() {
        let mut tracker = DelegationTracker::new();

        tracker.start_delegation("tool1".to_string(), "agent1".to_string());
        tracker.start_delegation("tool2".to_string(), "agent2".to_string());

        assert_eq!(tracker.complete_delegation("tool1"), Some("agent1".to_string()));
        assert_eq!(tracker.complete_delegation("tool2"), Some("agent2".to_string()));
        assert_eq!(tracker.complete_delegation("tool3"), None);
    }

    #[test]
    fn test_extract_tool_details_read() {
        let input = serde_json::json!({
            "file_path": "/path/to/file.rs"
        });

        assert_eq!(
            extract_tool_details("Read", &input),
            "📖 Reading: /path/to/file.rs"
        );
    }

    #[test]
    fn test_extract_tool_details_write() {
        let input = serde_json::json!({
            "file_path": "/path/to/output.yaml",
            "content": "some content"
        });

        assert_eq!(
            extract_tool_details("Write", &input),
            "✍️  Writing: /path/to/output.yaml"
        );
    }

    #[test]
    fn test_extract_tool_details_bash() {
        let input = serde_json::json!({
            "command": "find . -name '*.rs' | wc -l"
        });

        assert_eq!(
            extract_tool_details("Bash", &input),
            "⚡ Running: find . -name '*.rs' | wc -l"
        );
    }

    #[test]
    fn test_extract_tool_details_bash_long_command() {
        let long_cmd = "a".repeat(100);
        let input = serde_json::json!({
            "command": long_cmd
        });

        let result = extract_tool_details("Bash", &input);
        assert!(result.starts_with("⚡ Running: "));
        assert!(result.ends_with("..."));
        assert!(result.len() < 100);
    }

    #[test]
    fn test_extract_tool_details_grep() {
        let input = serde_json::json!({
            "pattern": "async fn",
            "path": "./src"
        });

        assert_eq!(
            extract_tool_details("Grep", &input),
            "🔍 Searching: \"async fn\" in ./src"
        );
    }

    #[test]
    fn test_extract_tool_details_glob() {
        let input = serde_json::json!({
            "pattern": "**/*.rs"
        });

        assert_eq!(
            extract_tool_details("Glob", &input),
            "📂 Finding: **/*.rs"
        );
    }

    #[test]
    fn test_extract_tool_details_task_with_subagent() {
        let input = serde_json::json!({
            "subagent_type": "file-condenser",
            "prompt": "Condense this large file"
        });

        assert_eq!(
            extract_tool_details("Task", &input),
            "🤝 Delegating to @file-condenser: Condense this large file"
        );
    }

    #[test]
    fn test_extract_tool_details_unknown_tool() {
        let input = serde_json::json!({});

        assert_eq!(
            extract_tool_details("UnknownTool", &input),
            "🔧 Using tool: UnknownTool"
        );
    }
}
