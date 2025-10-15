use claude_agent_sdk::types::{
    ClaudeAgentOptions, ContentBlock, McpServerConfig, McpServers, Message, PermissionMode,
    SdkMcpServerMarker, ToolName,
};
use claude_agent_sdk::ClaudeSDKClient;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};
use workflow_manager_sdk::WorkflowRuntime;

use crate::mcp_tools::create_workflow_mcp_server;

/// Response from background chat task
#[derive(Debug)]
pub enum ChatResponse {
    Success {
        content: String,
        tool_calls: Vec<ToolCall>,
    },
    Error(String),
}

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

/// Active pane in chat view
#[derive(Debug, Clone, PartialEq)]
pub enum ActivePane {
    ChatMessages,
    Logs,
}

/// Chat interface state
pub struct ChatInterface {
    /// Message history
    pub messages: Vec<ChatMessage>,
    /// Current input buffer
    pub input_buffer: String,
    /// Claude SDK client (wrapped for async access)
    client: Option<Arc<Mutex<ClaudeSDKClient>>>,
    /// Channel for receiving responses from background task
    pub response_rx: Option<mpsc::UnboundedReceiver<ChatResponse>>,
    /// Whether we're waiting for a response
    pub waiting_for_response: bool,
    /// When we started waiting for response (for timing display)
    pub response_start_time: Option<Instant>,
    /// Current spinner frame (for animation)
    pub spinner_frame: usize,
    /// Scroll position for message history (deprecated, use message_scroll)
    pub scroll_offset: usize,
    /// Scroll position for chat messages pane
    pub message_scroll: u16,
    /// Scroll position for logs pane
    pub log_scroll: u16,
    /// Currently active pane for keyboard navigation
    pub active_pane: ActivePane,
    /// Runtime for workflow operations
    runtime: Arc<dyn WorkflowRuntime>,
    /// Tokio runtime handle for spawning tasks
    tokio_handle: tokio::runtime::Handle,
    /// Initialization state
    pub initialized: bool,
    pub init_error: Option<String>,
}

impl ChatInterface {
    /// Create a new chat interface
    pub fn new(runtime: Arc<dyn WorkflowRuntime>, tokio_handle: tokio::runtime::Handle) -> Self {
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
            response_rx: None,
            waiting_for_response: false,
            response_start_time: None,
            spinner_frame: 0,
            scroll_offset: 0,
            message_scroll: 0,
            log_scroll: 0,
            active_pane: ActivePane::ChatMessages,
            runtime,
            tokio_handle,
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

        // Create client and wrap in Arc<Mutex<>>
        let client = ClaudeSDKClient::new(options, None).await?;
        self.client = Some(Arc::new(Mutex::new(client)));
        self.initialized = true;

        Ok(())
    }

    /// Send a message to Claude asynchronously (spawns background task)
    /// Note: User message should be added to history BEFORE calling this
    pub fn send_message_async(&mut self, message: String) {
        if message.trim().is_empty() {
            return;
        }

        self.waiting_for_response = true;
        self.response_start_time = Some(Instant::now());

        // Create channel for receiving response
        let (tx, rx) = mpsc::unbounded_channel();
        self.response_rx = Some(rx);

        // Clone client Arc for background task
        if let Some(client) = self.client.clone() {
            self.tokio_handle.spawn(async move {
                // Send message and collect response
                let result = Self::send_message_internal(client, message).await;

                // Send result back via channel
                let _ = tx.send(result);
            });
        }
    }

    /// Internal method to send message and collect response (runs in background task)
    async fn send_message_internal(
        client: Arc<Mutex<ClaudeSDKClient>>,
        message: String,
    ) -> ChatResponse {
        // Lock client and send message
        let send_result = {
            let mut client_guard = client.lock().await;
            client_guard.send_message(message).await
        };

        if let Err(e) = send_result {
            return ChatResponse::Error(format!("Failed to send message: {}", e));
        }

        // Collect response
        let mut assistant_content = String::new();
        let mut tool_calls = Vec::new();

        loop {
            let msg_result = {
                let mut client_guard = client.lock().await;
                client_guard.next_message().await
            };

            match msg_result {
                Some(Ok(Message::Assistant { message, .. })) => {
                    for block in &message.content {
                        match block {
                            ContentBlock::Text { text } => {
                                assistant_content.push_str(text);
                                assistant_content.push('\n');
                            }
                            ContentBlock::ToolUse { name, input, .. } => {
                                let input_str =
                                    serde_json::to_string_pretty(input).unwrap_or_default();
                                tool_calls.push(ToolCall {
                                    name: name.clone(),
                                    input: input_str,
                                    output: String::new(),
                                });
                            }
                            ContentBlock::ToolResult { content, .. } => {
                                let result_text = if let Some(content_value) = content {
                                    match content_value {
                                        claude_agent_sdk::types::ContentValue::String(text) => {
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

                                if let Some(last_tool) = tool_calls.last_mut() {
                                    last_tool.output = result_text;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Some(Ok(Message::Result { is_error, .. })) => {
                    if is_error {
                        assistant_content.push_str("\n[Error occurred during conversation]");
                    }
                    break;
                }
                Some(Err(e)) => {
                    return ChatResponse::Error(format!("Error during conversation: {}", e));
                }
                None => break,
                _ => {}
            }
        }

        ChatResponse::Success {
            content: assistant_content.trim().to_string(),
            tool_calls,
        }
    }

    /// Poll for response from background task (non-blocking)
    pub fn poll_response(&mut self) {
        if let Some(rx) = &mut self.response_rx {
            // Non-blocking receive
            match rx.try_recv() {
                Ok(response) => {
                    // Response received
                    self.waiting_for_response = false;
                    self.response_start_time = None;
                    self.response_rx = None;

                    // Add to message history
                    match response {
                        ChatResponse::Success { content, tool_calls } => {
                            self.messages.push(ChatMessage {
                                role: ChatRole::Assistant,
                                content,
                                tool_calls,
                            });
                        }
                        ChatResponse::Error(error) => {
                            self.messages.push(ChatMessage {
                                role: ChatRole::Assistant,
                                content: format!("Error: {}", error),
                                tool_calls: Vec::new(),
                            });
                        }
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    // No response yet, keep waiting
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Channel closed unexpectedly
                    self.waiting_for_response = false;
                    self.response_start_time = None;
                    self.response_rx = None;
                    self.messages.push(ChatMessage {
                        role: ChatRole::Assistant,
                        content: "Error: Response channel disconnected".to_string(),
                        tool_calls: Vec::new(),
                    });
                }
            }
        }
    }

    /// Clear input buffer
    pub fn clear_input(&mut self) {
        self.input_buffer.clear();
    }

    /// Scroll up in active pane
    pub fn scroll_up(&mut self) {
        match self.active_pane {
            ActivePane::ChatMessages => {
                self.message_scroll = self.message_scroll.saturating_sub(1);
            }
            ActivePane::Logs => {
                self.log_scroll = self.log_scroll.saturating_sub(1);
            }
        }
        // Keep old scroll_offset for backwards compat
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    /// Scroll down in active pane
    pub fn scroll_down(&mut self) {
        match self.active_pane {
            ActivePane::ChatMessages => {
                self.message_scroll = self.message_scroll.saturating_add(1);
            }
            ActivePane::Logs => {
                self.log_scroll = self.log_scroll.saturating_add(1);
            }
        }
        // Keep old scroll_offset for backwards compat
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    /// Switch to next pane (cycle through)
    pub fn next_pane(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::ChatMessages => ActivePane::Logs,
            ActivePane::Logs => ActivePane::ChatMessages,
        };
    }

    /// Reset scroll to bottom
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.message_scroll = 0;
        self.log_scroll = 0;
    }

    /// Update spinner animation frame
    pub fn update_spinner(&mut self) {
        self.spinner_frame = (self.spinner_frame + 1) % 8;
    }

    /// Get spinner character for current frame
    pub fn get_spinner_char(&self) -> char {
        const SPINNER: [char; 8] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧'];
        SPINNER[self.spinner_frame]
    }

    /// Get elapsed time since response started
    pub fn get_elapsed_seconds(&self) -> Option<u64> {
        self.response_start_time.map(|start| start.elapsed().as_secs())
    }
}
