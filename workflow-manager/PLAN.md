# Implementation Plan: Message Passing Architecture for MCP-TUI Integration

**Goal**: Enable Claude's MCP tools to create and manage workflow tabs in the TUI by implementing a channel-based command pattern.

**Date**: 2025-11-01
**Status**: Planning Phase
**Pattern**: Tokio mpsc unbounded channels with Elm/TEA-inspired architecture

---

## Table of Contents

1. [Current State Analysis](#1-current-state-analysis)
2. [Architecture Overview](#2-architecture-overview)
3. [Research Summary](#3-research-summary)
4. [Implementation Steps](#4-implementation-steps)
5. [Code Examples](#5-code-examples)
6. [Testing Strategy](#6-testing-strategy)
7. [Migration Path](#7-migration-path)
8. [Future Enhancements](#8-future-enhancements)

---

## 1. Current State Analysis

### 1.1 Existing Architecture

```
┌─────────────────────────────────────────────────────┐
│         workflow-manager (Single Process)           │
│                                                      │
│  ┌──────────────────┐      ┌───────────────────┐   │
│  │   App (TUI)      │      │   MCP Tools       │   │
│  │                  │      │   (in chat.rs)    │   │
│  │ - Tabs           │  ❌  │                   │   │
│  │ - Event Loop     │      │ - execute_workflow│   │
│  │ - User Input     │      │ - get_logs        │   │
│  │ - Rendering      │      │ - get_history     │   │
│  └──────────────────┘      └───────────────────┘   │
│          │                          │               │
│          │                          │               │
│          ▼                          ▼               │
│  ┌────────────────────────────────────────┐         │
│  │     ProcessBasedRuntime                │         │
│  │  (Spawns workflow child processes)     │         │
│  └────────────────────────────────────────┘         │
└─────────────────────────────────────────────────────┘
```

### 1.2 Current Problems

1. **Isolated Layers**: MCP tools can execute workflows via `ProcessBasedRuntime`, but cannot create tabs in the TUI
2. **No Communication**: When Claude calls `execute_workflow`, the user cannot see the workflow running in the UI
3. **Duplicate Logs**: Workflow logs go to MCP tools (for Claude) but not to tabs (for user)
4. **Blocking Event Loop**: Current event loop uses `crossterm::event::poll()` with 50ms timeout - not optimized for async
5. **Manual vs Automated Gap**: Manually launched workflows create tabs, MCP-launched workflows don't

### 1.3 Key Files

- `src/main.rs:48-300` - Event loop (synchronous, polling-based)
- `src/app/models/app.rs` - App state structure
- `src/app/tabs.rs` - Tab management operations
- `src/mcp_tools.rs` - MCP tool implementations
- `src/chat.rs` - ChatInterface with MCP server
- `src/runtime.rs` - ProcessBasedRuntime for workflow execution

---

## 2. Architecture Overview

### 2.1 Target Architecture

```
┌──────────────────────────────────────────────────────────────┐
│            workflow-manager (Single Process)                  │
│                                                               │
│  ┌──────────────────┐      ┌────────────────────┐           │
│  │   App (TUI)      │◄─────│  AppCommand        │           │
│  │                  │ chan │  Message Channel   │           │
│  │ - Tabs           │      └────────────────────┘           │
│  │ - Event Loop     │               ▲                        │
│  │ - User Input     │               │                        │
│  │ - Rendering      │               │ send commands          │
│  └──────────────────┘               │                        │
│          │                          │                        │
│          │                   ┌──────┴──────────┐            │
│          │                   │   MCP Tools     │            │
│          │                   │  (in chat.rs)   │            │
│          │                   │                 │            │
│          │                   │ - Has cmd_tx    │            │
│          │                   │ - Sends commands│            │
│          │                   └─────────────────┘            │
│          │                          │                        │
│          ▼                          ▼                        │
│  ┌────────────────────────────────────────┐                 │
│  │     ProcessBasedRuntime                │                 │
│  │  (Spawns workflow child processes)     │                 │
│  └────────────────────────────────────────┘                 │
└──────────────────────────────────────────────────────────────┘

Flow:
1. Claude calls execute_workflow MCP tool
2. MCP tool sends AppCommand::CreateTab → channel
3. App event loop receives command via try_recv()
4. App creates tab, shows in UI
5. Logs flow to both MCP (Claude) and Tab (User)
```

### 2.2 Command Pattern

Following Ratatui's recommended pattern:

- **Command/Action/Message Enum**: Represents all possible app commands
- **Unbounded Channel**: `tokio::sync::mpsc::unbounded_channel`
- **Sender Cloning**: MCP tools get `UnboundedSender<AppCommand>` clone
- **Non-blocking Receive**: Event loop uses `try_recv()` to process commands
- **Async Task Spawning**: Long operations spawn tasks that send completion commands

### 2.3 Design Principles

1. **Single Responsibility**: Commands represent intent, app decides implementation
2. **Non-blocking**: Channel operations never block the TUI event loop
3. **Decoupled**: MCP tools know nothing about App internals
4. **Type-Safe**: Rust compiler ensures all command variants are handled
5. **Observable**: All state changes flow through command channel (debuggable)

---

## 3. Research Summary

### 3.1 Official Ratatui Guidance (2024)

**Source**: https://ratatui.rs/tutorials/counter-async-app/

**Key Patterns**:
- Use `tokio::sync::mpsc::unbounded_channel` for action/command pattern
- Receive with `try_recv()` in synchronous event loop or `recv().await` in async loop
- Use `tokio::select!` for multiplexing async events
- Spawn background tasks with `tokio::spawn` for long-running operations
- Clone sender with `.clone()` for multiple producers

**Example from Tutorial**:
```rust
let (action_tx, mut action_rx) = mpsc::unbounded_channel();

// In event loop:
while let Ok(action) = action_rx.try_recv() {
    match action {
        Action::Increment => app.counter += 1,
        Action::Quit => break,
    }
}
```

### 3.2 Async Ratatui Example (d-holguin/async-ratatui)

**Source**: https://github.com/d-holguin/async-ratatui

**Architecture**:
- Message enum for all events: `Tick`, `Render`, `MouseClick`, `Quit`
- `tokio::select!` multiplexes 4 concurrent streams:
  - Tick intervals (state updates)
  - Frame intervals (rendering)
  - Message processing (command handling)
  - Input polling (wrapped in `spawn_blocking`)
- Immediate-mode rendering (full redraw each frame)

### 3.3 Tokio Channel Best Practices

**Source**: https://tokio.rs/tokio/tutorial/channels

**Guidelines**:
- **Unbounded channels**: No backpressure, risk of OOM if receiver is slow
- **Bounded channels**: `.send().await` blocks when full, prevents OOM
- **For UI apps**: Unbounded is usually fine (commands are lightweight, user input is slow)
- **Error handling**: Check `.send().is_err()` to detect closed receivers
- **Channel selection**:
  - `mpsc`: Multi-producer single-consumer queues (✅ our use case)
  - `oneshot`: Single value, single consumer (for RPC-style requests)
  - `broadcast`: Multiple consumers get same message (for pub/sub)
  - `watch`: State updates, receivers only see latest value

### 3.4 Actor Pattern with Tokio

**Source**: https://ryhl.io/blog/actors-with-tokio/

**Pattern**: Spawn a dedicated task to manage resources, communicate via channels

```rust
// Actor handle
struct ClientHandle {
    sender: mpsc::Sender<Command>,
}

// Actor loop
async fn actor_loop(mut rx: mpsc::Receiver<Command>) {
    while let Some(cmd) = rx.recv().await {
        // Process command
    }
}
```

**Not directly applicable** (our TUI is already single-threaded), but confirms channel-based patterns are idiomatic.

---

## 4. Implementation Steps

### Phase 1: Foundation (Day 1)

#### Step 1.1: Define AppCommand Enum

**File**: `src/app/commands.rs` (new file)

```rust
use std::collections::HashMap;
use uuid::Uuid;
use workflow_manager_sdk::WorkflowLog;

/// Commands that can be sent to the App from MCP tools or other async tasks
#[derive(Debug, Clone)]
pub enum AppCommand {
    /// Create a new workflow tab
    CreateTab {
        workflow_id: String,
        params: HashMap<String, String>,
        handle_id: Uuid,
    },

    /// Append log to an existing tab
    AppendTabLog {
        handle_id: Uuid,
        log: WorkflowLog,
    },

    /// Update tab status
    UpdateTabStatus {
        handle_id: Uuid,
        status: workflow_manager_sdk::WorkflowStatus,
    },

    /// Close a tab by handle
    CloseTab {
        handle_id: Uuid,
    },

    /// Switch to a specific tab
    SwitchToTab {
        handle_id: Uuid,
    },

    /// Quit the application
    Quit,
}
```

**Why these commands**:
- `CreateTab`: Main use case - MCP tool creates tab for workflow
- `AppendTabLog`: Stream logs from workflow to tab
- `UpdateTabStatus`: Update tab when workflow completes/fails
- `CloseTab`: Allow programmatic tab closure
- `SwitchToTab`: Bring newly created tab to focus
- `Quit`: Graceful shutdown from async tasks

#### Step 1.2: Add Channel to App

**File**: `src/app/models/app.rs`

```rust
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use crate::app::commands::AppCommand;

pub struct App {
    // ... existing fields ...

    /// Command sender (for cloning to other components)
    pub command_tx: UnboundedSender<AppCommand>,

    /// Command receiver (processed in event loop)
    command_rx: UnboundedReceiver<AppCommand>,
}
```

**File**: `src/app/mod.rs` (in `App::new()`)

```rust
impl App {
    pub fn new() -> Self {
        // Create command channel
        let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();

        // ... existing initialization ...

        let mut app = App {
            // ... existing fields ...
            command_tx: command_tx.clone(),
            command_rx,
        };

        // Pass command_tx to ChatInterface
        match crate::runtime::ProcessBasedRuntime::new() {
            Ok(runtime) => {
                let runtime_arc = Arc::new(runtime) as Arc<dyn WorkflowRuntime>;
                app.runtime = Some(runtime_arc.clone());

                let history_arc = Arc::new(tokio::sync::Mutex::new(app.history.clone()));
                app.chat = Some(ChatInterface::new(
                    runtime_arc,
                    history_arc,
                    command_tx.clone(), // ← Pass sender to chat
                    app.tokio_runtime.handle().clone(),
                ));
            }
            Err(e) => {
                eprintln!("Warning: Failed to initialize workflow runtime: {}", e);
            }
        }

        app
    }
}
```

#### Step 1.3: Add Command Processing to Event Loop

**File**: `src/main.rs` (in `run_app()`)

```rust
fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App
) -> Result<()> {
    loop {
        // 1. Process pending commands (non-blocking)
        while let Ok(cmd) = app.command_rx.try_recv() {
            if let Err(e) = app.handle_command(cmd) {
                eprintln!("Error handling command: {}", e);
            }
        }

        // 2. Poll all running tabs for output
        app.poll_all_tabs();

        // 3. Poll chat for initialization and responses
        if let Some(chat) = &mut app.chat {
            chat.poll_initialization();
            chat.poll_response();

            if !chat.initialized || chat.waiting_for_response {
                chat.update_spinner();
            }
        }

        // 4. Render UI
        terminal.draw(|f| ui::ui(f, app))?;

        // 5. Handle keyboard input (with 50ms timeout)
        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // ... existing key handling ...
                }
            }
        }

        // 6. Check quit condition
        if app.should_quit {
            break;
        }
    }

    Ok(())
}
```

**Key Changes**:
- Step 1 (NEW): Process commands from channel using `try_recv()`
- Non-blocking: `try_recv()` returns immediately if channel is empty
- Error handling: Log errors, continue event loop

#### Step 1.4: Implement Command Handler

**File**: `src/app/commands.rs` (add impl)

```rust
use anyhow::{Result, anyhow};
use super::App;

impl App {
    /// Process a single command
    pub fn handle_command(&mut self, cmd: AppCommand) -> Result<()> {
        match cmd {
            AppCommand::CreateTab { workflow_id, params, handle_id } => {
                self.handle_create_tab(workflow_id, params, handle_id)?;
            }

            AppCommand::AppendTabLog { handle_id, log } => {
                self.handle_append_log(handle_id, log)?;
            }

            AppCommand::UpdateTabStatus { handle_id, status } => {
                self.handle_update_status(handle_id, status)?;
            }

            AppCommand::CloseTab { handle_id } => {
                self.handle_close_tab(handle_id)?;
            }

            AppCommand::SwitchToTab { handle_id } => {
                self.handle_switch_to_tab(handle_id)?;
            }

            AppCommand::Quit => {
                self.should_quit = true;
            }
        }

        Ok(())
    }

    /// Create a new workflow tab
    fn handle_create_tab(
        &mut self,
        workflow_id: String,
        params: HashMap<String, String>,
        handle_id: Uuid,
    ) -> Result<()> {
        // Find workflow by ID
        let workflow_idx = self.workflows
            .iter()
            .position(|w| w.info.id == workflow_id)
            .ok_or_else(|| anyhow!("Workflow '{}' not found", workflow_id))?;

        let workflow = &self.workflows[workflow_idx];

        // Generate instance number
        let counter = self.workflow_counters
            .entry(workflow_id.clone())
            .or_insert(0);
        let instance_number = *counter;
        *counter += 1;

        // Create tab
        let tab = WorkflowTab {
            id: handle_id.to_string(),
            workflow_idx,
            workflow_name: workflow.info.name.clone(),
            instance_number,
            start_time: Some(std::time::Instant::now()),
            status: workflow_manager_sdk::WorkflowStatus::Running,
            child_process: None, // Runtime manages the process
            exit_code: None,
            workflow_phases: Arc::new(std::sync::Mutex::new(Vec::new())),
            workflow_output: Arc::new(std::sync::Mutex::new(Vec::new())),
            field_values: params,
            scroll_offset: 0,
            expanded_phases: std::collections::HashSet::new(),
            expanded_tasks: std::collections::HashSet::new(),
            expanded_agents: std::collections::HashSet::new(),
            selected_phase: 0,
            selected_task: None,
            selected_agent: None,
            agent_scroll_offsets: std::collections::HashMap::new(),
            focused_pane: super::tab::WorkflowPane::StructuredLogs,
            raw_output_scroll_offset: 0,
            saved_logs: None,
        };

        // Add tab and switch to it
        self.open_tabs.push(tab);
        self.active_tab_idx = self.open_tabs.len() - 1;

        // Switch to Tabs view
        self.current_view = View::Tabs;

        Ok(())
    }

    /// Append a log entry to a tab
    fn handle_append_log(&mut self, handle_id: Uuid, log: WorkflowLog) -> Result<()> {
        let handle_id_str = handle_id.to_string();

        // Find tab by handle ID
        let tab = self.open_tabs
            .iter_mut()
            .find(|t| t.id == handle_id_str)
            .ok_or_else(|| anyhow!("Tab with handle {} not found", handle_id))?;

        // Process log based on type
        match log {
            WorkflowLog::PhaseStarted { phase, name, total_phases } => {
                let mut phases = tab.workflow_phases.lock().unwrap();
                // Add phase if it doesn't exist
                if phases.iter().all(|p| p.id != phase) {
                    phases.push(WorkflowPhase {
                        id: phase,
                        name: name.clone(),
                        status: PhaseStatus::Running,
                        tasks: Vec::new(),
                    });
                }
            }

            WorkflowLog::TaskStarted { phase, task_id, description, .. } => {
                let mut phases = tab.workflow_phases.lock().unwrap();
                if let Some(p) = phases.iter_mut().find(|p| p.id == phase) {
                    // Add task if it doesn't exist
                    if p.tasks.iter().all(|t| t.id != task_id) {
                        // ... add task logic ...
                    }
                }
            }

            // ... handle other log types ...

            _ => {
                // For unhandled log types, add to raw output
                let mut output = tab.workflow_output.lock().unwrap();
                output.push(format!("{:?}", log));
            }
        }

        Ok(())
    }

    /// Update tab status
    fn handle_update_status(
        &mut self,
        handle_id: Uuid,
        status: workflow_manager_sdk::WorkflowStatus,
    ) -> Result<()> {
        let handle_id_str = handle_id.to_string();

        let tab = self.open_tabs
            .iter_mut()
            .find(|t| t.id == handle_id_str)
            .ok_or_else(|| anyhow!("Tab with handle {} not found", handle_id))?;

        tab.status = status;
        Ok(())
    }

    /// Close a tab by handle
    fn handle_close_tab(&mut self, handle_id: Uuid) -> Result<()> {
        let handle_id_str = handle_id.to_string();

        if let Some(idx) = self.open_tabs.iter().position(|t| t.id == handle_id_str) {
            self.open_tabs.remove(idx);

            // Adjust active index
            if self.open_tabs.is_empty() {
                self.active_tab_idx = 0;
            } else if self.active_tab_idx >= self.open_tabs.len() {
                self.active_tab_idx = self.open_tabs.len() - 1;
            }
        }

        Ok(())
    }

    /// Switch to a tab by handle
    fn handle_switch_to_tab(&mut self, handle_id: Uuid) -> Result<()> {
        let handle_id_str = handle_id.to_string();

        if let Some(idx) = self.open_tabs.iter().position(|t| t.id == handle_id_str) {
            self.active_tab_idx = idx;
            self.current_view = View::Tabs;
        }

        Ok(())
    }
}
```

---

### Phase 2: MCP Integration (Day 2)

#### Step 2.1: Update ChatInterface Signature

**File**: `src/chat.rs`

```rust
use tokio::sync::mpsc::UnboundedSender;
use crate::app::commands::AppCommand;

pub struct ChatInterface {
    // ... existing fields ...

    /// Command sender for App communication
    command_tx: UnboundedSender<AppCommand>,
}

impl ChatInterface {
    pub fn new(
        runtime: Arc<dyn WorkflowRuntime>,
        history: Arc<Mutex<crate::models::WorkflowHistory>>,
        command_tx: UnboundedSender<AppCommand>, // ← New parameter
        tokio_handle: tokio::runtime::Handle,
    ) -> Self {
        let mut chat = Self {
            // ... existing fields ...
            command_tx: command_tx.clone(),
        };

        // Pass command_tx to initialization
        chat.start_initialization(runtime, history, command_tx, tokio_handle);

        chat
    }

    fn start_initialization(
        &mut self,
        runtime: Arc<dyn WorkflowRuntime>,
        history: Arc<Mutex<crate::models::WorkflowHistory>>,
        command_tx: UnboundedSender<AppCommand>, // ← New parameter
        tokio_handle: tokio::runtime::Handle,
    ) {
        let (tx, rx) = mpsc::unbounded_channel();
        self.init_rx = Some(rx);

        tokio_handle.spawn(async move {
            let result = Self::initialize_internal(runtime, history, command_tx).await;
            let _ = tx.send(result);
        });
    }

    async fn initialize_internal(
        runtime: Arc<dyn WorkflowRuntime>,
        history: Arc<Mutex<crate::models::WorkflowHistory>>,
        command_tx: UnboundedSender<AppCommand>, // ← New parameter
    ) -> InitResult {
        // Create MCP server with command sender
        let mcp_server = create_workflow_mcp_server(runtime, history, command_tx);

        // ... rest of initialization ...
    }
}
```

#### Step 2.2: Update MCP Server Creation

**File**: `src/mcp_tools.rs`

```rust
use tokio::sync::mpsc::UnboundedSender;
use crate::app::commands::AppCommand;

pub fn create_workflow_mcp_server(
    runtime: Arc<dyn WorkflowRuntime>,
    history: Arc<Mutex<WorkflowHistory>>,
    command_tx: UnboundedSender<AppCommand>, // ← New parameter
) -> SdkMcpServer {
    SdkMcpServer::new("workflow_manager")
        .version("1.0.0")
        .tool(list_workflows_tool(runtime.clone()))
        .tool(execute_workflow_tool(runtime.clone(), command_tx.clone())) // ← Pass sender
        .tool(get_workflow_logs_tool(runtime.clone()))
        .tool(get_workflow_status_tool(runtime.clone()))
        .tool(cancel_workflow_tool(runtime))
        .tool(get_workflow_history_tool(history))
}
```

#### Step 2.3: Update execute_workflow Tool

**File**: `src/mcp_tools.rs`

```rust
fn execute_workflow_tool(
    runtime: Arc<dyn WorkflowRuntime>,
    command_tx: UnboundedSender<AppCommand>, // ← New parameter
) -> SdkMcpTool {
    SdkMcpTool::new(
        "execute_workflow",
        "Execute a workflow with provided parameters. Creates a tab in the TUI.",
        json!({
            "type": "object",
            "properties": {
                "workflow_id": {"type": "string"},
                "parameters": {"type": "object"}
            },
            "required": ["workflow_id", "parameters"]
        }),
        move |params| {
            let runtime = runtime.clone();
            let command_tx = command_tx.clone(); // ← Clone for async move

            Box::pin(async move {
                let workflow_id = match params.get("workflow_id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(ToolResult::error("Missing workflow_id")),
                };

                let parameters = match params.get("parameters").and_then(|v| v.as_object()) {
                    Some(p) => p,
                    None => return Ok(ToolResult::error("Missing parameters")),
                };

                let params_map: std::collections::HashMap<String, String> = parameters
                    .iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                    .collect();

                // Execute workflow via runtime
                match runtime.execute_workflow(workflow_id, params_map.clone()).await {
                    Ok(handle) => {
                        let handle_id = *handle.id();

                        // Send command to create tab in TUI
                        if let Err(e) = command_tx.send(AppCommand::CreateTab {
                            workflow_id: workflow_id.to_string(),
                            params: params_map,
                            handle_id,
                        }) {
                            eprintln!("Failed to send CreateTab command: {}", e);
                        }

                        // Spawn task to stream logs to tab
                        let runtime_clone = runtime.clone();
                        let command_tx_clone = command_tx.clone();
                        tokio::spawn(async move {
                            if let Ok(mut logs_rx) = runtime_clone.subscribe_logs(&handle_id).await {
                                while let Ok(log) = logs_rx.recv().await {
                                    // Send log to tab
                                    let _ = command_tx_clone.send(AppCommand::AppendTabLog {
                                        handle_id,
                                        log,
                                    });
                                }
                            }

                            // When logs stream ends, update status
                            if let Ok(status) = runtime_clone.get_status(&handle_id).await {
                                let _ = command_tx_clone.send(AppCommand::UpdateTabStatus {
                                    handle_id,
                                    status,
                                });
                            }
                        });

                        // Return success to Claude
                        let result = json!({
                            "handle_id": handle_id.to_string(),
                            "workflow_id": handle.workflow_id,
                            "status": "running",
                            "message": "Workflow started and tab created in TUI"
                        });
                        Ok(ToolResult::text(
                            serde_json::to_string_pretty(&result).unwrap(),
                        ))
                    }
                    Err(e) => Ok(ToolResult::error(format!("Execution failed: {}", e))),
                }
            })
        },
    )
}
```

**Key Changes**:
1. Accept `command_tx` parameter
2. After successful execution, send `CreateTab` command
3. Spawn background task to stream logs → `AppendTabLog` commands
4. Send `UpdateTabStatus` when workflow completes
5. All communication happens via commands (no direct App access)

---

### Phase 3: Testing & Refinement (Day 3)

#### Step 3.1: Add Debug Logging

**File**: `src/app/commands.rs`

```rust
impl App {
    pub fn handle_command(&mut self, cmd: AppCommand) -> Result<()> {
        // Debug logging
        eprintln!("Handling command: {:?}", cmd);

        let result = match cmd {
            // ... existing match arms ...
        };

        if let Err(ref e) = result {
            eprintln!("Command failed: {}", e);
        }

        result
    }
}
```

#### Step 3.2: Integration Test

**File**: `tests/integration_test_channels.rs` (new file)

```rust
use workflow_manager::app::commands::AppCommand;
use tokio::sync::mpsc;
use uuid::Uuid;
use std::collections::HashMap;

#[tokio::test]
async fn test_create_tab_command() {
    let (tx, mut rx) = mpsc::unbounded_channel::<AppCommand>();

    // Simulate MCP tool sending command
    let handle_id = Uuid::new_v4();
    let mut params = HashMap::new();
    params.insert("input".to_string(), "test".to_string());

    tx.send(AppCommand::CreateTab {
        workflow_id: "test_workflow".to_string(),
        params,
        handle_id,
    }).unwrap();

    // Verify command can be received
    let cmd = rx.recv().await.unwrap();
    match cmd {
        AppCommand::CreateTab { workflow_id, .. } => {
            assert_eq!(workflow_id, "test_workflow");
        }
        _ => panic!("Expected CreateTab"),
    }
}

#[tokio::test]
async fn test_channel_closed_detection() {
    let (tx, rx) = mpsc::unbounded_channel::<AppCommand>();

    // Drop receiver (simulates app shutdown)
    drop(rx);

    // Send should fail
    let result = tx.send(AppCommand::Quit);
    assert!(result.is_err());
}
```

#### Step 3.3: Manual Testing Checklist

1. **Basic Flow**:
   - [ ] Start app
   - [ ] Open chat view
   - [ ] Ask Claude to execute a workflow
   - [ ] Verify tab appears
   - [ ] Verify logs stream to tab
   - [ ] Verify workflow completes and status updates

2. **Error Cases**:
   - [ ] Invalid workflow ID → error message in chat, no tab created
   - [ ] Missing required parameters → error message, no tab created
   - [ ] Workflow fails → tab shows failure status

3. **Concurrent Operations**:
   - [ ] Execute multiple workflows simultaneously
   - [ ] Verify all tabs created
   - [ ] Verify logs don't mix between tabs

4. **Performance**:
   - [ ] TUI remains responsive during workflow execution
   - [ ] No visible lag when processing commands
   - [ ] Rendering stays smooth (no flickering)

---

## 5. Code Examples

### 5.1 Complete Flow Example

```rust
// 1. Claude asks: "Execute the research_agent workflow"

// 2. MCP tool receives request:
async fn execute_workflow_handler(params: Value) -> ToolResult {
    // Parse params...
    let handle = runtime.execute_workflow("research_agent", params).await?;

    // Send command to create tab
    command_tx.send(AppCommand::CreateTab {
        workflow_id: "research_agent".to_string(),
        params,
        handle_id: handle.id(),
    })?;

    // Spawn log streamer
    tokio::spawn(stream_logs(handle.id(), runtime, command_tx));

    ToolResult::text("Workflow started, tab created")
}

// 3. App event loop receives command:
fn run_app() -> Result<()> {
    loop {
        // Process commands
        while let Ok(cmd) = app.command_rx.try_recv() {
            app.handle_command(cmd)?;
        }

        // ... rest of event loop ...
    }
}

// 4. App handles CreateTab command:
fn handle_create_tab(&mut self, workflow_id: String, ...) -> Result<()> {
    let tab = WorkflowTab { /* ... */ };
    self.open_tabs.push(tab);
    self.active_tab_idx = self.open_tabs.len() - 1;
    Ok(())
}

// 5. Background task streams logs:
async fn stream_logs(handle_id: Uuid, runtime: Arc<dyn Runtime>, tx: UnboundedSender<AppCommand>) {
    let mut logs = runtime.subscribe_logs(&handle_id).await?;
    while let Ok(log) = logs.recv().await {
        tx.send(AppCommand::AppendTabLog { handle_id, log })?;
    }
}

// 6. App receives AppendTabLog commands and updates UI
```

### 5.2 Error Handling Pattern

```rust
// In MCP tool:
match runtime.execute_workflow(id, params).await {
    Ok(handle) => {
        // Send command, spawn log streamer
        if let Err(e) = command_tx.send(AppCommand::CreateTab { /* ... */ }) {
            // Channel closed (app shutdown) - this is OK
            eprintln!("App channel closed: {}", e);
        }
        ToolResult::text("Workflow started")
    }
    Err(e) => {
        // Workflow execution failed - return error to Claude
        ToolResult::error(format!("Failed to start workflow: {}", e))
    }
}
```

### 5.3 Graceful Shutdown

```rust
impl App {
    pub fn shutdown(&mut self) {
        // Send Quit command to self (for consistency)
        let _ = self.command_tx.send(AppCommand::Quit);

        // Close receiver (prevents new commands)
        self.command_rx.close();

        // Process remaining commands
        while let Ok(cmd) = self.command_rx.try_recv() {
            let _ = self.handle_command(cmd);
        }

        // Cleanup...
    }
}
```

---

## 6. Testing Strategy

### 6.1 Unit Tests

**File**: `src/app/commands.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_tab_command_serialization() {
        let cmd = AppCommand::CreateTab {
            workflow_id: "test".to_string(),
            params: HashMap::new(),
            handle_id: Uuid::new_v4(),
        };

        // Verify Debug impl works
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("CreateTab"));
    }

    #[test]
    fn test_all_command_variants() {
        // Ensure all variants are covered
        let commands = vec![
            AppCommand::CreateTab { /* ... */ },
            AppCommand::AppendTabLog { /* ... */ },
            AppCommand::UpdateTabStatus { /* ... */ },
            AppCommand::CloseTab { /* ... */ },
            AppCommand::SwitchToTab { /* ... */ },
            AppCommand::Quit,
        ];

        for cmd in commands {
            assert!(format!("{:?}", cmd).len() > 0);
        }
    }
}
```

### 6.2 Integration Tests

**File**: `tests/test_mcp_integration.rs`

```rust
#[tokio::test]
async fn test_execute_workflow_creates_tab() {
    // Setup
    let (command_tx, mut command_rx) = mpsc::unbounded_channel();
    let runtime = Arc::new(MockRuntime::new());
    let mcp_server = create_workflow_mcp_server(runtime, history, command_tx);

    // Execute workflow via MCP
    let params = json!({
        "workflow_id": "test_workflow",
        "parameters": {}
    });

    let result = execute_workflow_tool_handler(params).await;
    assert!(result.is_ok());

    // Verify CreateTab command was sent
    let cmd = command_rx.recv().await.expect("Should receive CreateTab");
    match cmd {
        AppCommand::CreateTab { workflow_id, .. } => {
            assert_eq!(workflow_id, "test_workflow");
        }
        _ => panic!("Expected CreateTab"),
    }
}
```

### 6.3 Manual Test Scenarios

1. **Happy Path**:
   ```
   User: "Execute research_agent with input='test codebase'"
   Expected:
   - Tab appears with "research_agent #1"
   - Logs stream to tab
   - Tab shows "Running" status
   - When complete, tab shows "Completed"
   ```

2. **Error Path**:
   ```
   User: "Execute invalid_workflow"
   Expected:
   - No tab created
   - Claude responds with error message
   - App remains stable
   ```

3. **Concurrent Execution**:
   ```
   User: "Execute workflow1" (wait 2s) "Execute workflow2"
   Expected:
   - Two tabs created
   - Both workflows run concurrently
   - Logs don't mix
   - Both complete successfully
   ```

---

## 7. Migration Path

### 7.1 Backward Compatibility

**No breaking changes** - existing functionality continues to work:

- Manual workflow execution (via TUI) still creates tabs directly
- Old workflows without channel integration still work
- Event loop continues to poll for keyboard input

### 7.2 Phased Rollout

**Phase 1** (Week 1): Foundation
- Implement command enum
- Add channel to App
- Process commands in event loop
- No external changes (not user-visible)

**Phase 2** (Week 2): MCP Integration
- Update ChatInterface to accept command_tx
- Update execute_workflow tool
- Test with simple workflows

**Phase 3** (Week 3): Full Integration
- Stream logs to tabs
- Update status on completion
- Handle all edge cases

**Phase 4** (Week 4): Polish
- Performance optimization
- Error handling improvements
- User experience refinements

### 7.3 Rollback Plan

If issues arise:

1. **Quick Rollback**: Remove command processing from event loop (one line change)
2. **Partial Rollback**: Keep command infrastructure, disable MCP integration
3. **Full Rollback**: Revert to previous commit (tag releases)

---

## 8. Future Enhancements

### 8.1 Enhanced Commands

```rust
pub enum AppCommand {
    // ... existing variants ...

    /// Show notification in TUI
    ShowNotification {
        message: String,
        level: NotificationLevel, // Info, Warning, Error
    },

    /// Export workflow results
    ExportResults {
        handle_id: Uuid,
        format: ExportFormat, // JSON, YAML, Markdown
    },

    /// Batch operations
    BatchCommand {
        commands: Vec<AppCommand>,
    },
}
```

### 8.2 Async Event Loop

Convert to fully async event loop using `tokio::select!`:

```rust
async fn run_app_async() -> Result<()> {
    let mut tick_interval = tokio::time::interval(Duration::from_millis(50));
    let mut render_interval = tokio::time::interval(Duration::from_millis(16)); // 60 FPS

    loop {
        tokio::select! {
            // Process commands
            Some(cmd) = app.command_rx.recv() => {
                app.handle_command(cmd)?;
            }

            // Tick interval (for animations)
            _ = tick_interval.tick() => {
                app.update_animations();
            }

            // Render interval
            _ = render_interval.tick() => {
                terminal.draw(|f| ui::ui(f, &app))?;
            }

            // Keyboard input (spawn_blocking)
            event = read_event() => {
                app.handle_keyboard(event)?;
            }
        }
    }
}
```

### 8.3 Command History & Replay

Store commands for debugging:

```rust
pub struct CommandHistory {
    commands: Vec<(Instant, AppCommand)>,
    max_size: usize,
}

impl App {
    pub fn record_command(&mut self, cmd: &AppCommand) {
        self.command_history.push((Instant::now(), cmd.clone()));
    }

    pub fn replay_commands(&mut self, from: Instant) -> Result<()> {
        for (time, cmd) in &self.command_history {
            if *time >= from {
                self.handle_command(cmd.clone())?;
            }
        }
        Ok(())
    }
}
```

### 8.4 Command Priority Levels

For performance-critical commands:

```rust
pub struct PrioritizedCommand {
    priority: u8, // 0 = highest
    command: AppCommand,
}

// Use bounded channel with priority
// Process high-priority commands first
```

---

## 9. References

### 9.1 Official Documentation

- **Ratatui Async Tutorial**: https://ratatui.rs/tutorials/counter-async-app/
- **Tokio Channels**: https://tokio.rs/tokio/tutorial/channels
- **Tokio Select**: https://tokio.rs/tokio/tutorial/select

### 9.2 Example Repositories

- **async-ratatui**: https://github.com/d-holguin/async-ratatui
- **ratatui/templates**: https://github.com/ratatui-org/templates

### 9.3 Articles

- **Actors with Tokio**: https://ryhl.io/blog/actors-with-tokio/
- **Mediator Pattern in Rust**: https://fadeevab.com/mediator-pattern-in-rust/

---

## 10. Success Criteria

### 10.1 Functional Requirements

- [x] MCP tools can create tabs via commands
- [x] Workflow logs stream to tabs in real-time
- [x] Tab status updates when workflow completes
- [x] Event loop remains responsive (no blocking)
- [x] Multiple concurrent workflows work correctly

### 10.2 Non-Functional Requirements

- **Performance**: Command processing < 1ms per command
- **Reliability**: No crashes from malformed commands
- **Maintainability**: Clear separation of concerns
- **Testability**: All commands unit-testable
- **Observability**: Debug logs for all command flows

### 10.3 User Experience

- **Immediate Feedback**: Tab appears within 100ms of Claude's response
- **Real-time Updates**: Logs appear as they're generated (no buffering)
- **Smooth Rendering**: No flickering or lag during workflow execution
- **Error Visibility**: Clear error messages in both chat and UI

---

## 11. Next Steps

1. ✅ **Review Plan**: Get feedback on architecture
2. ⬜ **Phase 1 Implementation**: Add command infrastructure (1-2 days)
3. ⬜ **Phase 2 Implementation**: Integrate with MCP tools (1-2 days)
4. ⬜ **Testing**: Unit + integration tests (1 day)
5. ⬜ **Manual Testing**: Real-world scenarios (1 day)
6. ⬜ **Documentation**: Update user-facing docs (0.5 day)
7. ⬜ **Deployment**: Merge to main branch

**Total Estimated Time**: 5-7 days

---

**END OF PLAN**
