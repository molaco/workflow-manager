use claude_agent_sdk::types::{ClaudeAgentOptions, ContentBlock, McpServerConfig, McpServers, Message, PermissionMode, SdkMcpServerMarker, ToolName};
use claude_agent_sdk::ClaudeSDKClient;
use std::collections::HashMap;
use std::sync::Arc;
use workflow_manager_sdk::WorkflowRuntime;

use crate::mcp_tools::create_workflow_mcp_server;

/// A chat message in the conversation
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChatRole {
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub input: String,
    pub output: String,
}

/// Chat interface state
pub struct ChatInterface {
    /// Message history
    pub messages: Vec<ChatMessage>,
    /// Current input buffer
    pub input_buffer: String,
    /// Claude SDK client
    client: Option<ClaudeSDKClient>,
    /// Whether we're waiting for a response
    pub waiting_for_response: bool,
    /// Scroll position for message history
    pub scroll_offset: usize,
    /// Runtime for workflow operations
    runtime: Arc<dyn WorkflowRuntime>,
    /// Initialization state
    pub initialized: bool,
    pub init_error: Option<String>,
}

impl ChatInterface {
    /// Create a new chat interface
    pub fn new(runtime: Arc<dyn WorkflowRuntime>) -> Self {
        Self {
            messages: vec![
                ChatMessage {
                    role: ChatRole::Assistant,
                    content: "Hello! I'm Claude. I can help you manage and execute workflows.\n\nTry asking me to:\n• List available workflows\n• Execute a workflow\n• Check workflow status\n• Get workflow logs".to_string(),
                    tool_calls: Vec::new(),
                }
            ],
            input_buffer: String::new(),
            client: None,
            waiting_for_response: false,
            scroll_offset: 0,
            runtime,
            initialized: false,
            init_error: None,
        }
    }

    /// Initialize Claude SDK client with MCP tools
    pub async fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.initialized {
            return Ok(());
        }

        // Create MCP server with workflow tools
        let mcp_server = create_workflow_mcp_server(self.runtime.clone());

        // Register MCP server
        let mut mcp_servers = HashMap::new();
        mcp_servers.insert(
            "workflow_manager".to_string(),
            McpServerConfig::Sdk(SdkMcpServerMarker {
                name: "workflow_manager".to_string(),
                instance: Arc::new(mcp_server),
            }),
        );

        // Create options with SDK MCP server
        let options = ClaudeAgentOptions {
            mcp_servers: McpServers::Dict(mcp_servers),
            allowed_tools: vec![
                ToolName::new("mcp__workflow_manager__list_workflows"),
                ToolName::new("mcp__workflow_manager__execute_workflow"),
                ToolName::new("mcp__workflow_manager__get_workflow_logs"),
                ToolName::new("mcp__workflow_manager__get_workflow_status"),
                ToolName::new("mcp__workflow_manager__cancel_workflow"),
            ],
            max_turns: Some(10),
            permission_mode: Some(PermissionMode::BypassPermissions),
            ..Default::default()
        };

        // Create client
        let client = ClaudeSDKClient::new(options, None).await?;
        self.client = Some(client);
        self.initialized = true;

        Ok(())
    }

    /// Send a message to Claude
    pub async fn send_message(&mut self, message: String) -> Result<(), Box<dyn std::error::Error>> {
        if message.trim().is_empty() {
            return Ok(());
        }

        // Add user message to history
        self.messages.push(ChatMessage {
            role: ChatRole::User,
            content: message.clone(),
            tool_calls: Vec::new(),
        });

        self.waiting_for_response = true;

        // Send to Claude
        if let Some(client) = &mut self.client {
            client.send_message(message).await?;

            // Collect response
            let mut assistant_content = String::new();
            let mut tool_calls = Vec::new();

            while let Some(msg) = client.next_message().await {
                match msg {
                    Ok(Message::Assistant { message, .. }) => {
                        for block in &message.content {
                            match block {
                                ContentBlock::Text { text } => {
                                    assistant_content.push_str(text);
                                    assistant_content.push('\n');
                                }
                                ContentBlock::ToolUse { name, input, .. } => {
                                    let input_str = serde_json::to_string_pretty(input).unwrap_or_default();
                                    tool_calls.push(ToolCall {
                                        name: name.clone(),
                                        input: input_str,
                                        output: String::new(), // Will be filled by tool result
                                    });
                                }
                                ContentBlock::ToolResult { content, .. } => {
                                    // ToolResult blocks are internal - Claude processes them
                                    // We just record that a tool was called
                                    // The actual result will be in Claude's text response

                                    let result_text = if let Some(content_value) = content {
                                        match content_value {
                                            claude_agent_sdk::types::ContentValue::String(text) => {
                                                // Truncate long results for display
                                                if text.len() > 200 {
                                                    format!("{}... [truncated]", &text[..200])
                                                } else {
                                                    text.clone()
                                                }
                                            }
                                            claude_agent_sdk::types::ContentValue::Blocks(_) => {
                                                "[Structured data returned]".to_string()
                                            }
                                        }
                                    } else {
                                        String::new()
                                    };

                                    // Update the last tool call with result
                                    if let Some(last_tool) = tool_calls.last_mut() {
                                        last_tool.output = result_text;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok(Message::Result { is_error, .. }) => {
                        if is_error {
                            assistant_content.push_str("\n[Error occurred during conversation]");
                        }
                        break;
                    }
                    Err(e) => {
                        assistant_content.push_str(&format!("\n[Error: {}]", e));
                        break;
                    }
                    _ => {}
                }
            }

            // Add assistant response to history
            self.messages.push(ChatMessage {
                role: ChatRole::Assistant,
                content: assistant_content.trim().to_string(),
                tool_calls,
            });
        }

        self.waiting_for_response = false;
        Ok(())
    }

    /// Clear input buffer
    pub fn clear_input(&mut self) {
        self.input_buffer.clear();
    }

    /// Scroll up in message history
    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    /// Scroll down in message history
    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    /// Reset scroll to bottom
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }
}
