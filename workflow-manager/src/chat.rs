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
use crate::app::{AppCommand, TaskRegistry};

/// Initialization result from background task
pub enum InitResult {
    Success(Arc<Mutex<ClaudeSDKClient>>),
    Error(String),
}

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
    /// Channel for receiving initialization result
    init_rx: Option<mpsc::UnboundedReceiver<InitResult>>,
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
    /// Whether to automatically scroll to bottom when new messages arrive
    pub auto_scroll: bool,
    /// Currently active pane for keyboard navigation
    pub active_pane: ActivePane,
    /// Runtime for workflow operations
    runtime: Arc<dyn WorkflowRuntime>,
    /// Workflow history for parameter suggestions
    history: Arc<Mutex<crate::models::WorkflowHistory>>,
    /// Tokio runtime handle for spawning tasks
    tokio_handle: tokio::runtime::Handle,
    /// Initialization state
    pub initialized: bool,
    pub init_error: Option<String>,
    /// Command sender for App communication
    command_tx: mpsc::UnboundedSender<AppCommand>,
    /// Task registry for background task cleanup
    task_registry: TaskRegistry,
    /// History of user's sent messages (for Ctrl+Up/Down cycling)
    message_history: Vec<String>,
    /// Current position when browsing history (None = not browsing)
    history_index: Option<usize>,
    /// Draft message saved when starting to browse history
    history_draft: String,
    /// Database for persistence
    database: Arc<std::sync::Mutex<crate::database::Database>>,
}

impl ChatInterface {
    /// Create a new chat interface and start background initialization
    pub fn new(
        runtime: Arc<dyn WorkflowRuntime>,
        history: Arc<Mutex<crate::models::WorkflowHistory>>,
        command_tx: mpsc::UnboundedSender<AppCommand>,
        task_registry: TaskRegistry,
        tokio_handle: tokio::runtime::Handle,
        database: Arc<std::sync::Mutex<crate::database::Database>>,
    ) -> Self {
        // Load message history from database
        let message_history = database
            .lock()
            .unwrap()
            .get_chat_history(100) // Load last 100 messages
            .unwrap_or_default();

        let mut chat = Self {
            messages: Vec::new(),
            input_buffer: String::new(),
            client: None,
            init_rx: None,
            response_rx: None,
            waiting_for_response: false,
            response_start_time: None,
            spinner_frame: 0,
            scroll_offset: 0,
            message_scroll: 0,
            log_scroll: 0,
            auto_scroll: true, // Start with auto-scroll enabled
            active_pane: ActivePane::ChatMessages,
            runtime: runtime.clone(),
            history: history.clone(),
            tokio_handle: tokio_handle.clone(),
            initialized: false,
            init_error: None,
            command_tx: command_tx.clone(),
            task_registry: task_registry.clone(),
            message_history,
            history_index: None,
            history_draft: String::new(),
            database,
        };

        // Start initialization in background
        chat.start_initialization(runtime, history, command_tx, task_registry, tokio_handle);

        chat
    }

    /// Start initialization in background
    fn start_initialization(
        &mut self,
        runtime: Arc<dyn WorkflowRuntime>,
        history: Arc<Mutex<crate::models::WorkflowHistory>>,
        command_tx: mpsc::UnboundedSender<AppCommand>,
        task_registry: TaskRegistry,
        tokio_handle: tokio::runtime::Handle,
    ) {
        // Create channel for initialization result
        let (tx, rx) = mpsc::unbounded_channel();
        self.init_rx = Some(rx);

        // Spawn initialization task
        tokio_handle.spawn(async move {
            let result = Self::initialize_internal(runtime, history, command_tx, task_registry).await;
            let _ = tx.send(result);
        });
    }

    /// Internal initialization logic (runs in background task)
    async fn initialize_internal(
        runtime: Arc<dyn WorkflowRuntime>,
        history: Arc<Mutex<crate::models::WorkflowHistory>>,
        command_tx: mpsc::UnboundedSender<AppCommand>,
        task_registry: TaskRegistry,
    ) -> InitResult {
        // Create MCP server with workflow tools
        let mcp_server = create_workflow_mcp_server(runtime, history, command_tx, task_registry);

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
        match ClaudeSDKClient::new(options, None).await {
            Ok(client) => InitResult::Success(Arc::new(Mutex::new(client))),
            Err(e) => InitResult::Error(format!("Failed to initialize Claude client: {}", e)),
        }
    }

    /// Poll for initialization completion (non-blocking)
    pub fn poll_initialization(&mut self) {
        if let Some(rx) = &mut self.init_rx {
            match rx.try_recv() {
                Ok(InitResult::Success(client)) => {
                    self.client = Some(client);
                    self.initialized = true;
                    self.init_rx = None;
                }
                Ok(InitResult::Error(error)) => {
                    self.init_error = Some(error);
                    self.initialized = false;
                    self.init_rx = None;
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    // Still initializing
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    self.init_error = Some("Initialization task disconnected".to_string());
                    self.init_rx = None;
                }
            }
        }
    }

    /// Send a message to Claude asynchronously (spawns background task)
    /// Note: User message should be added to history BEFORE calling this
    pub fn send_message_async(&mut self, message: String) {
        if message.trim().is_empty() {
            return;
        }

        // Save to database
        if let Ok(db) = self.database.lock() {
            let _ = db.insert_chat_message(&message);
        }

        // Add to in-memory history (keep last 100)
        self.message_history.push(message.clone());
        if self.message_history.len() > 100 {
            self.message_history.remove(0);
        }

        // Reset history browsing
        self.history_index = None;
        self.history_draft.clear();

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

                    // Note: Actual scrolling happens in render function after layout is known
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

                    // Note: Actual scrolling happens in render function after layout is known
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
                // Disable auto-scroll when user manually scrolls up
                self.auto_scroll = false;
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
        // Re-enable auto-scroll when explicitly scrolling to bottom
        self.auto_scroll = true;
    }

    /// Update spinner animation frame
    pub fn update_spinner(&mut self) {
        self.spinner_frame = (self.spinner_frame + 1) % 8;
    }

    /// Calculate total number of rendered lines for all messages
    /// This accounts for multi-line content, tool calls, spacing, etc.
    pub fn calculate_total_lines(&self) -> u16 {
        let mut total_lines = 0u16;

        for msg in &self.messages {
            // Role line (e.g., "You: " or "Claude: ")
            total_lines = total_lines.saturating_add(1);

            // Message content lines (count newlines + 1)
            let content_lines = msg.content.lines().count().max(1);
            total_lines = total_lines.saturating_add(content_lines as u16);

            // Tool calls display (if any)
            if !msg.tool_calls.is_empty() {
                total_lines = total_lines.saturating_add(1); // Blank line
                total_lines = total_lines.saturating_add(1); // "🔧 X tool(s) used"
            }

            // Blank line after each message
            total_lines = total_lines.saturating_add(1);
        }

        // Add lines for "Thinking..." indicator if waiting
        if self.waiting_for_response {
            total_lines = total_lines.saturating_add(2); // Spinner + help text
        }

        total_lines
    }

    /// Auto-scroll to bottom of messages given viewport height
    pub fn auto_scroll_to_bottom(&mut self, viewport_height: u16) {
        if !self.auto_scroll {
            return;
        }

        let total_lines = self.calculate_total_lines();

        // Scroll position = total_lines - viewport_height (show last page)
        // Saturating sub prevents overflow if content fits in viewport
        self.message_scroll = total_lines.saturating_sub(viewport_height);
    }

    /// Get spinner character for current frame
    pub fn get_spinner_char(&self) -> char {
        const SPINNER: [char; 8] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧'];
        SPINNER[self.spinner_frame]
    }

    /// Get loading indicator for title bar
    pub fn get_loading_indicator(&self) -> &'static str {
        const INDICATORS: [&str; 4] = ["...", ".. ", ".  ", "   "];
        INDICATORS[self.spinner_frame % 4]
    }

    /// Get elapsed time since response started
    pub fn get_elapsed_seconds(&self) -> Option<u64> {
        self.response_start_time.map(|start| start.elapsed().as_secs())
    }

    /// Navigate to previous message in history (Ctrl+Up)
    pub fn history_prev(&mut self) {
        if self.message_history.is_empty() {
            return;
        }

        match self.history_index {
            None => {
                // First time browsing - save current input and go to last message
                self.history_draft = self.input_buffer.clone();
                self.history_index = Some(self.message_history.len() - 1);
                self.input_buffer = self.message_history[self.message_history.len() - 1].clone();
            }
            Some(idx) => {
                // Already browsing - go to older message if possible
                if idx > 0 {
                    self.history_index = Some(idx - 1);
                    self.input_buffer = self.message_history[idx - 1].clone();
                }
            }
        }
    }

    /// Navigate to next message in history (Ctrl+Down)
    pub fn history_next(&mut self) {
        if self.message_history.is_empty() {
            return;
        }

        match self.history_index {
            None => {
                // Not browsing - do nothing
            }
            Some(idx) => {
                if idx < self.message_history.len() - 1 {
                    // Go to newer message
                    self.history_index = Some(idx + 1);
                    self.input_buffer = self.message_history[idx + 1].clone();
                } else {
                    // At newest - restore draft and exit history mode
                    self.input_buffer = self.history_draft.clone();
                    self.history_index = None;
                    self.history_draft.clear();
                }
            }
        }
    }

    /// Exit history browsing mode when user types
    pub fn exit_history_mode(&mut self) {
        if self.history_index.is_some() {
            self.history_index = None;
            self.history_draft.clear();
        }
    }
}
