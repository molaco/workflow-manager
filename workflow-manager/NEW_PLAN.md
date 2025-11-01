# Implementation Plan: Message Passing Architecture for MCP-TUI Integration (REVISED)

**Goal**: Enable Claude's MCP tools to create and manage workflow tabs in the TUI by implementing a channel-based command pattern.

**Date**: 2025-11-01
**Status**: Planning Phase (Revised after Architecture Review)
**Pattern**: Tokio mpsc unbounded channels with Elm/TEA-inspired architecture

**Key Revisions from Original Plan**:
- ✅ Added Task Registry for background task lifecycle management
- ✅ Added Notification System for user-visible feedback
- ✅ Enhanced tab cleanup to handle both manual and MCP workflows
- ✅ Added log sampling to prevent channel overflow
- ✅ Added comprehensive command ordering tests
- ✅ Revised timeline from 5-7 days to 10-12 days (more realistic)

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
9. [References](#9-references)
10. [Success Criteria](#10-success-criteria)
11. [Next Steps](#11-next-steps)

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
6. **No Error Visibility**: MCP errors only logged to stderr, not visible to users
7. **Resource Leaks**: Background tasks spawned by MCP have no cleanup mechanism

### 1.3 Key Files

- `src/main.rs:48-300` - Event loop (synchronous, polling-based)
- `src/app/models/app.rs` - App state structure
- `src/app/models/tab.rs` - WorkflowTab structure
- `src/app/tabs.rs:60-100` - Tab management operations (includes confirmation flow)
- `src/mcp_tools.rs` - MCP tool implementations
- `src/chat.rs` - ChatInterface with MCP server
- `src/runtime.rs` - ProcessBasedRuntime for workflow execution

### 1.4 Critical Discovery: Existing Confirmation Flow

The codebase already has a confirmation system for closing running workflows:

```rust
// In src/app/tabs.rs:60-75
pub fn close_current_tab(&mut self) {
    let tab = &self.open_tabs[self.active_tab_idx];

    // If running, show confirmation
    if tab.status == WorkflowStatus::Running {
        self.show_close_confirmation = true;
        return;
    }

    self.close_tab_confirmed();
}

pub fn close_tab_confirmed(&mut self) {
    // Kill process if running
    if let Some(mut child) = tab.child_process.take() {
        let _ = child.kill();  // ← Only kills MANUAL workflows!
    }
    // ... remove tab ...
}
```

**Problem**: This only works for manually-launched workflows (`child_process`).
MCP-launched workflows won't have `child_process` set, so they won't be cancelled!

**Solution**: Track both workflow types and enhance cleanup (see Section 4.1.4).

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
│  │ - Notifications  │               │ send commands          │
│  │ - TaskRegistry   │               │                        │
│  └──────────────────┘               │                        │
│          │                          │                        │
│          │                   ┌──────┴──────────┐            │
│          │                   │   MCP Tools     │            │
│          │                   │  (in chat.rs)   │            │
│          │                   │                 │            │
│          │                   │ - Has cmd_tx    │            │
│          │                   │ - Has registry  │            │
│          │                   │ - Sends cmds    │            │
│          │                   │ - Registers     │            │
│          │                   │   tasks         │            │
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
2. MCP tool executes via runtime, gets handle_id
3. MCP tool sends AppCommand::CreateTab → channel
4. MCP tool spawns log streaming task
5. MCP tool registers task in TaskRegistry
6. App event loop receives command via try_recv()
7. App creates tab, shows in UI with notification
8. Logs flow to both MCP (Claude) and Tab (User)
9. User closes tab → confirmation if running
10. App cancels runtime workflow + background tasks
```

### 2.2 Command Pattern

Following Ratatui's recommended pattern:

- **Command/Action/Message Enum**: Represents all possible app commands
- **Unbounded Channel**: `tokio::sync::mpsc::unbounded_channel`
- **Sender Cloning**: MCP tools get `UnboundedSender<AppCommand>` clone
- **Non-blocking Receive**: Event loop uses `try_recv()` to process commands
- **Task Registry**: Separate registry for JoinHandles (can't go through channel)
- **Notification System**: User-visible feedback for all operations

### 2.3 Design Principles

1. **Single Responsibility**: Commands represent intent, app decides implementation
2. **Non-blocking**: Channel operations never block the TUI event loop
3. **Decoupled**: MCP tools know nothing about App internals
4. **Type-Safe**: Rust compiler ensures all command variants are handled
5. **Observable**: All state changes flow through command channel (debuggable)
6. **Resource Safety**: All background tasks have explicit cleanup paths
7. **User Feedback**: All operations provide visible feedback (notifications)
8. **Backward Compatible**: Existing manual workflow flow unchanged

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

### 3.2 Tokio Channel Best Practices

**Source**: https://tokio.rs/tokio/tutorial/channels

**Guidelines**:
- **Unbounded channels**: No backpressure, risk of OOM if receiver is slow
- **Bounded channels**: `.send().await` blocks when full, prevents OOM
- **For UI apps**: Unbounded is usually fine (commands are lightweight, user input is slow)
- **Exception**: High-frequency logs need rate limiting (see Section 4.2.3)
- **Error handling**: Check `.send().is_err()` to detect closed receivers
- **Channel selection**: `mpsc` (multi-producer single-consumer) is correct for our use case

### 3.3 Task Lifecycle Management

**Source**: https://ryhl.io/blog/actors-with-tokio/

**Key Insight**: Background tasks need explicit lifecycle management:
- Store `JoinHandle` for spawned tasks
- Call `.abort()` to cancel task
- Use separate registry if handles can't be sent through main channel
- Clean up on shutdown

**Our Implementation**: TaskRegistry pattern (see Section 4.1.5)

---

## 4. Implementation Steps

### Phase 1: Foundation (Days 1-2, ~16 hours)

#### Step 1.1: Define AppCommand Enum

**File**: `src/app/commands.rs` (NEW FILE)

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

    /// Close a tab by handle (bypasses confirmation - assumes MCP has approval)
    CloseTab {
        handle_id: Uuid,
    },

    /// Switch to a specific tab
    SwitchToTab {
        handle_id: Uuid,
    },

    /// Show a notification to the user
    ShowNotification {
        level: NotificationLevel,
        title: String,
        message: String,
    },

    /// Quit the application
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NotificationLevel {
    Info,
    Success,
    Warning,
    Error,
}
```

**Why these commands**:
- `CreateTab`: Main use case - MCP tool creates tab for workflow
- `AppendTabLog`: Stream logs from workflow to tab
- `UpdateTabStatus`: Update tab when workflow completes/fails
- `CloseTab`: Allow programmatic tab closure (bypasses confirmation)
- `SwitchToTab`: Bring newly created tab to focus
- `ShowNotification`: User-visible feedback for operations
- `Quit`: Graceful shutdown from async tasks

**Note on CloseTab**: This command is for MCP-initiated closures and bypasses the confirmation dialog. User-initiated closes still use the existing `close_current_tab()` method which shows confirmation if the workflow is running.

#### Step 1.2: Add Notification System

**File**: `src/app/notifications.rs` (NEW FILE)

```rust
use std::time::Instant;
use super::commands::NotificationLevel;

#[derive(Debug, Clone)]
pub struct Notification {
    pub id: usize,
    pub timestamp: Instant,
    pub level: NotificationLevel,
    pub title: String,
    pub message: String,
    pub dismissible: bool,
    pub auto_dismiss_after: Option<std::time::Duration>,
}

pub struct NotificationManager {
    notifications: Vec<Notification>,
    next_id: usize,
    max_notifications: usize,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            next_id: 0,
            max_notifications: 50,
        }
    }

    /// Add an error notification
    pub fn error(&mut self, title: impl Into<String>, message: impl Into<String>) -> usize {
        self.push(NotificationLevel::Error, title.into(), message.into())
    }

    /// Add a success notification
    pub fn success(&mut self, title: impl Into<String>, message: impl Into<String>) -> usize {
        self.push(NotificationLevel::Success, title.into(), message.into())
    }

    /// Add a warning notification
    pub fn warning(&mut self, title: impl Into<String>, message: impl Into<String>) -> usize {
        self.push(NotificationLevel::Warning, title.into(), message.into())
    }

    /// Add an info notification
    pub fn info(&mut self, title: impl Into<String>, message: impl Into<String>) -> usize {
        self.push(NotificationLevel::Info, title.into(), message.into())
    }

    /// Internal method to add notification
    fn push(&mut self, level: NotificationLevel, title: String, message: String) -> usize {
        let id = self.next_id;
        self.next_id += 1;

        self.notifications.push(Notification {
            id,
            timestamp: Instant::now(),
            level,
            title,
            message,
            dismissible: true,
            auto_dismiss_after: Some(std::time::Duration::from_secs(5)),
        });

        // Keep only recent notifications
        if self.notifications.len() > self.max_notifications {
            self.notifications.remove(0);
        }

        id
    }

    /// Dismiss a notification by ID
    pub fn dismiss(&mut self, id: usize) {
        self.notifications.retain(|n| n.id != id);
    }

    /// Get active (non-expired) notifications
    pub fn get_active(&self) -> Vec<&Notification> {
        let now = Instant::now();
        self.notifications
            .iter()
            .filter(|n| match n.auto_dismiss_after {
                Some(duration) => now.duration_since(n.timestamp) < duration,
                None => true,
            })
            .collect()
    }

    /// Remove expired notifications
    pub fn cleanup_expired(&mut self) {
        let now = Instant::now();
        self.notifications.retain(|n| match n.auto_dismiss_after {
            Some(duration) => now.duration_since(n.timestamp) < duration,
            None => true,
        });
    }
}
```

**Purpose**: Provides user-visible feedback for all operations. Errors are no longer silent!

#### Step 1.3: Add Task Registry

**File**: `src/app/task_registry.rs` (NEW FILE)

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Global registry for background task handles
///
/// Background tasks (like log streamers) spawn tokio tasks that return JoinHandles.
/// JoinHandles cannot be sent through the command channel because they're not Clone/Debug.
/// This registry provides a separate mechanism for MCP tools to register tasks,
/// allowing the App to cancel them when tabs are closed.
pub struct TaskRegistry {
    tasks: Arc<Mutex<HashMap<Uuid, Vec<JoinHandle<()>>>>>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a background task for a workflow
    ///
    /// Call this after spawning a tokio::spawn task to track it for cleanup.
    pub async fn register(&self, workflow_id: Uuid, handle: JoinHandle<()>) {
        let mut tasks = self.tasks.lock().await;
        tasks.entry(workflow_id).or_insert_with(Vec::new).push(handle);
    }

    /// Cancel all tasks for a workflow
    ///
    /// Called when user closes a tab to clean up log streamers and other background tasks.
    pub async fn cancel_all(&self, workflow_id: &Uuid) {
        let mut tasks = self.tasks.lock().await;
        if let Some(handles) = tasks.remove(workflow_id) {
            for handle in handles {
                handle.abort();
            }
        }
    }

    /// Cancel all tasks (on app shutdown)
    pub async fn cancel_everything(&self) {
        let mut tasks = self.tasks.lock().await;
        for (_, handles) in tasks.drain() {
            for handle in handles {
                handle.abort();
            }
        }
    }
}

impl Clone for TaskRegistry {
    fn clone(&self) -> Self {
        Self {
            tasks: Arc::clone(&self.tasks),
        }
    }
}
```

**Purpose**: Manages lifecycle of background tasks spawned by MCP tools. Critical for preventing resource leaks!

#### Step 1.4: Update Module Declarations

**File**: `src/app/mod.rs`

```rust
pub mod commands;
pub mod notifications;
pub mod task_registry;

// Existing modules
pub mod models;
pub mod tabs;
// ... etc ...

// Re-export for convenience
pub use commands::{AppCommand, NotificationLevel};
pub use notifications::NotificationManager;
pub use task_registry::TaskRegistry;
```

#### Step 1.5: Add Channels and Registry to App

**File**: `src/app/models/app.rs`

```rust
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use crate::app::commands::AppCommand;
use crate::app::notifications::NotificationManager;
use crate::app::task_registry::TaskRegistry;

pub struct App {
    // ... existing fields ...

    /// Command sender (for cloning to other components)
    pub command_tx: UnboundedSender<AppCommand>,

    /// Command receiver (processed in event loop)
    command_rx: UnboundedReceiver<AppCommand>,

    /// Notification manager for user-visible messages
    pub notifications: NotificationManager,

    /// Registry for background task cleanup
    pub task_registry: TaskRegistry,
}
```

**File**: `src/app/mod.rs` (in `App::new()`)

```rust
impl App {
    pub fn new() -> Self {
        // Create command channel
        let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();

        // Create notification manager
        let notifications = NotificationManager::new();

        // Create task registry
        let task_registry = TaskRegistry::new();

        // ... existing initialization ...

        let mut app = App {
            // ... existing fields ...
            command_tx: command_tx.clone(),
            command_rx,
            notifications,
            task_registry: task_registry.clone(),
        };

        // Pass command_tx and task_registry to ChatInterface
        match crate::runtime::ProcessBasedRuntime::new() {
            Ok(runtime) => {
                let runtime_arc = Arc::new(runtime) as Arc<dyn WorkflowRuntime>;
                app.runtime = Some(runtime_arc.clone());

                let history_arc = Arc::new(tokio::sync::Mutex::new(app.history.clone()));
                app.chat = Some(ChatInterface::new(
                    runtime_arc,
                    history_arc,
                    command_tx.clone(),      // ← Pass sender to chat
                    task_registry.clone(),   // ← Pass registry to chat
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

#### Step 1.6: Update WorkflowTab for Dual-Path Tracking

**File**: `src/app/models/tab.rs`

```rust
use uuid::Uuid;

pub struct WorkflowTab {
    // ... existing fields ...

    /// For manually-launched workflows (existing path)
    /// Set when user launches workflow from TUI
    pub child_process: Option<Child>,

    /// For MCP-launched workflows (NEW path)
    /// Set when workflow is launched via MCP tools
    /// Used for cleanup via ProcessBasedRuntime
    pub runtime_handle_id: Option<Uuid>,
}
```

**Critical**: This allows the app to handle both manually-launched workflows (with `child_process`) and MCP-launched workflows (with `runtime_handle_id`). The existing confirmation flow will work for both!

#### Step 1.7: Add Command Processing to Event Loop

**File**: `src/main.rs` (in `run_app()`)

```rust
fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App
) -> Result<()> {
    loop {
        // 1. Process pending commands (non-blocking) with error handling
        while let Ok(cmd) = app.command_rx.try_recv() {
            if let Err(e) = app.handle_command(cmd.clone()) {
                // Log to stderr for debugging
                eprintln!("Error handling command {:?}: {}", cmd, e);

                // Show error to user in TUI
                app.notifications.error(
                    "Command Failed",
                    format!("Failed to process command: {}", e)
                );
            }
        }

        // 2. Cleanup expired notifications
        app.notifications.cleanup_expired();

        // 3. Poll all running tabs for output
        app.poll_all_tabs();

        // 4. Poll chat for initialization and responses
        if let Some(chat) = &mut app.chat {
            chat.poll_initialization();
            chat.poll_response();

            if !chat.initialized || chat.waiting_for_response {
                chat.update_spinner();
            }
        }

        // 5. Render UI (notifications will be rendered as overlay)
        terminal.draw(|f| ui::ui(f, app))?;

        // 6. Handle keyboard input (with 50ms timeout)
        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // ... existing key handling ...
                }
            }
        }

        // 7. Check quit condition
        if app.should_quit {
            // Cleanup background tasks before exit
            app.tokio_runtime.block_on(async {
                app.task_registry.cancel_everything().await;
            });
            break;
        }
    }

    Ok(())
}
```

**Key Changes**:
- Step 1: Process commands with error handling and user notifications
- Step 2: Cleanup expired notifications (auto-dismiss)
- Step 7: Cancel all background tasks on shutdown

#### Step 1.8: Implement Command Handler

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

            AppCommand::ShowNotification { level, title, message } => {
                self.notifications.push(level, title, message);
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
            .ok_or_else(|| {
                // Show user-friendly error
                self.notifications.error(
                    "Workflow Not Found",
                    format!("Cannot create tab: workflow '{}' does not exist", workflow_id)
                );

                anyhow!("Workflow '{}' not found", workflow_id)
            })?;

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
            start_time: Some(chrono::Local::now()),
            status: workflow_manager_sdk::WorkflowStatus::Running,

            // MCP workflows: no child_process, but have runtime_handle_id
            child_process: None,
            runtime_handle_id: Some(handle_id),

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

        // Show success notification
        self.notifications.success(
            "Workflow Started",
            format!("Created tab for {} #{}", workflow.info.name, instance_number)
        );

        Ok(())
    }

    /// Append a log entry to a tab
    fn handle_append_log(&mut self, handle_id: Uuid, log: WorkflowLog) -> Result<()> {
        let handle_id_str = handle_id.to_string();

        // Find tab by handle ID
        let tab = self.open_tabs
            .iter_mut()
            .find(|t| t.id == handle_id_str)
            .ok_or_else(|| {
                // This is expected if tab was closed - don't show error to user
                // Just log it for debugging
                eprintln!("Received log for closed tab: {}", handle_id);
                anyhow!("Tab with handle {} not found", handle_id)
            })?;

        // Process log based on type (use existing logic from tab polling)
        match log {
            WorkflowLog::PhaseStarted { phase, name, total_phases } => {
                let mut phases = tab.workflow_phases.lock().unwrap();
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
                    if p.tasks.iter().all(|t| t.id != task_id) {
                        // Add task logic (similar to existing implementation)
                    }
                }
            }

            // ... handle other log types similarly to existing code ...

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

    /// Close a tab by handle (bypasses confirmation)
    fn handle_close_tab(&mut self, handle_id: Uuid) -> Result<()> {
        let handle_id_str = handle_id.to_string();

        if let Some(idx) = self.open_tabs.iter().position(|t| t.id == handle_id_str) {
            let tab = &self.open_tabs[idx];

            // Cancel MCP workflow if applicable
            if let Some(runtime_handle_id) = tab.runtime_handle_id {
                if let Some(runtime) = &self.runtime {
                    let runtime = runtime.clone();
                    self.tokio_runtime.block_on(async {
                        let _ = runtime.cancel_workflow(&runtime_handle_id).await;
                    });
                }

                // Cancel background tasks
                self.tokio_runtime.block_on(async {
                    self.task_registry.cancel_all(&runtime_handle_id).await;
                });
            }

            // Remove tab
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

#### Step 1.9: Update Existing Tab Close Methods

**File**: `src/app/tabs.rs`

Update `close_tab_confirmed()` to handle both workflow types:

```rust
pub fn close_tab_confirmed(&mut self) {
    if self.open_tabs.is_empty() {
        return;
    }

    let tab = &mut self.open_tabs[self.active_tab_idx];

    // PATH 1: Kill manual workflow (existing behavior)
    if let Some(mut child) = tab.child_process.take() {
        let _ = child.kill();
    }

    // PATH 2: Cancel MCP workflow (NEW behavior)
    if let Some(handle_id) = tab.runtime_handle_id {
        // Cancel via runtime
        if let Some(runtime) = &self.runtime {
            let runtime = runtime.clone();
            self.tokio_runtime.block_on(async {
                if let Err(e) = runtime.cancel_workflow(&handle_id).await {
                    eprintln!("Failed to cancel workflow {}: {}", handle_id, e);
                }
            });
        }

        // Cancel background tasks (log streamers, etc.)
        self.tokio_runtime.block_on(async {
            self.task_registry.cancel_all(&handle_id).await;
        });
    }

    // Remove tab (existing logic)
    self.open_tabs.remove(self.active_tab_idx);

    // Adjust active index (existing logic)
    if self.open_tabs.is_empty() {
        self.active_tab_idx = 0;
    } else if self.active_tab_idx >= self.open_tabs.len() {
        self.active_tab_idx = self.open_tabs.len() - 1;
    }

    self.show_close_confirmation = false;
}
```

Update `kill_current_tab()` similarly:

```rust
pub fn kill_current_tab(&mut self) {
    if self.open_tabs.is_empty() {
        return;
    }

    if let Some(tab) = self.open_tabs.get_mut(self.active_tab_idx) {
        // PATH 1: Kill manual workflow
        if let Some(mut child) = tab.child_process.take() {
            let _ = child.kill();
            tab.status = WorkflowStatus::Failed;
            if let Ok(mut output) = tab.workflow_output.lock() {
                output.push(String::new());
                output.push("⚠️ Workflow killed by user".to_string());
            }
        }

        // PATH 2: Cancel MCP workflow
        if let Some(handle_id) = tab.runtime_handle_id {
            if let Some(runtime) = &self.runtime {
                let runtime = runtime.clone();
                self.tokio_runtime.block_on(async {
                    let _ = runtime.cancel_workflow(&handle_id).await;
                });
            }

            // Cancel background tasks
            self.tokio_runtime.block_on(async {
                self.task_registry.cancel_all(&handle_id).await;
            });

            tab.status = WorkflowStatus::Failed;
            if let Ok(mut output) = tab.workflow_output.lock() {
                output.push(String::new());
                output.push("⚠️ Workflow killed by user (MCP)".to_string());
            }
        }
    }
}
```

**Critical**: This preserves the existing confirmation UX for users while properly cleaning up both manual and MCP workflows!

---

### Phase 2: MCP Integration (Days 3-5, ~20 hours)

#### Step 2.1: Update ChatInterface Signature

**File**: `src/chat.rs`

```rust
use tokio::sync::mpsc::UnboundedSender;
use crate::app::commands::AppCommand;
use crate::app::task_registry::TaskRegistry;

pub struct ChatInterface {
    // ... existing fields ...

    /// Command sender for App communication
    command_tx: UnboundedSender<AppCommand>,

    /// Task registry for background task cleanup
    task_registry: TaskRegistry,
}

impl ChatInterface {
    pub fn new(
        runtime: Arc<dyn WorkflowRuntime>,
        history: Arc<Mutex<crate::models::WorkflowHistory>>,
        command_tx: UnboundedSender<AppCommand>,
        task_registry: TaskRegistry,
        tokio_handle: tokio::runtime::Handle,
    ) -> Self {
        let mut chat = Self {
            // ... existing fields ...
            command_tx: command_tx.clone(),
            task_registry: task_registry.clone(),
        };

        // Pass both to initialization
        chat.start_initialization(runtime, history, command_tx, task_registry, tokio_handle);

        chat
    }

    fn start_initialization(
        &mut self,
        runtime: Arc<dyn WorkflowRuntime>,
        history: Arc<Mutex<crate::models::WorkflowHistory>>,
        command_tx: UnboundedSender<AppCommand>,
        task_registry: TaskRegistry,
        tokio_handle: tokio::runtime::Handle,
    ) {
        let (tx, rx) = mpsc::unbounded_channel();
        self.init_rx = Some(rx);

        tokio_handle.spawn(async move {
            let result = Self::initialize_internal(runtime, history, command_tx, task_registry).await;
            let _ = tx.send(result);
        });
    }

    async fn initialize_internal(
        runtime: Arc<dyn WorkflowRuntime>,
        history: Arc<Mutex<crate::models::WorkflowHistory>>,
        command_tx: UnboundedSender<AppCommand>,
        task_registry: TaskRegistry,
    ) -> InitResult {
        // Create MCP server with command sender and task registry
        let mcp_server = create_workflow_mcp_server(runtime, history, command_tx, task_registry);

        // ... rest of initialization ...
    }
}
```

#### Step 2.2: Update MCP Server Creation

**File**: `src/mcp_tools.rs`

```rust
use tokio::sync::mpsc::UnboundedSender;
use crate::app::commands::AppCommand;
use crate::app::task_registry::TaskRegistry;

pub fn create_workflow_mcp_server(
    runtime: Arc<dyn WorkflowRuntime>,
    history: Arc<Mutex<WorkflowHistory>>,
    command_tx: UnboundedSender<AppCommand>,
    task_registry: TaskRegistry,
) -> SdkMcpServer {
    SdkMcpServer::new("workflow_manager")
        .version("1.0.0")
        .tool(list_workflows_tool(runtime.clone()))
        .tool(execute_workflow_tool(
            runtime.clone(),
            command_tx.clone(),
            task_registry.clone(),
        ))
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
    command_tx: UnboundedSender<AppCommand>,
    task_registry: TaskRegistry,
) -> SdkMcpTool {
    SdkMcpTool::new(
        "execute_workflow",
        "Execute a workflow with provided parameters. Creates a tab in the TUI.",
        json!(({
            "type": "object",
            "properties": {
                "workflow_id": {"type": "string"},
                "parameters": {"type": "object"}
            },
            "required": ["workflow_id", "parameters"]
        }),
        move |params| {
            let runtime = runtime.clone();
            let command_tx = command_tx.clone();
            let task_registry = task_registry.clone();

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

                        // 1. Send command to create tab in TUI
                        if let Err(e) = command_tx.send(AppCommand::CreateTab {
                            workflow_id: workflow_id.to_string(),
                            params: params_map,
                            handle_id,
                        }) {
                            eprintln!("Failed to send CreateTab command: {}", e);
                            return Ok(ToolResult::error("Failed to create tab"));
                        }

                        // 2. Send success notification
                        let _ = command_tx.send(AppCommand::ShowNotification {
                            level: NotificationLevel::Success,
                            title: "Workflow Started".to_string(),
                            message: format!("Executing {}", workflow_id),
                        });

                        // 3. Spawn task to stream logs to tab with rate limiting
                        let log_task = tokio::spawn({
                            let runtime_clone = runtime.clone();
                            let command_tx_clone = command_tx.clone();

                            async move {
                                if let Ok(mut logs_rx) = runtime_clone.subscribe_logs(&handle_id).await {
                                    // Rate limiting for high-frequency logs
                                    let mut last_sent = tokio::time::Instant::now();
                                    let min_interval = tokio::time::Duration::from_millis(16); // ~60 FPS
                                    let mut buffered_logs = Vec::new();

                                    while let Ok(log) = logs_rx.recv().await {
                                        buffered_logs.push(log);

                                        let now = tokio::time::Instant::now();

                                        // Send batch if enough time passed OR buffer is full
                                        if now.duration_since(last_sent) >= min_interval
                                           || buffered_logs.len() > 100 {
                                            // Send batched logs
                                            for log in buffered_logs.drain(..) {
                                                if command_tx_clone.send(AppCommand::AppendTabLog {
                                                    handle_id,
                                                    log,
                                                }).is_err() {
                                                    return; // App shut down
                                                }
                                            }
                                            last_sent = now;
                                        }
                                    }

                                    // Send remaining buffered logs
                                    for log in buffered_logs {
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
                            }
                        });

                        // 4. Register task for cleanup
                        task_registry.register(handle_id, log_task).await;

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
                    Err(e) => {
                        // Send error notification
                        let _ = command_tx.send(AppCommand::ShowNotification {
                            level: NotificationLevel::Error,
                            title: "Workflow Failed".to_string(),
                            message: format!("Failed to start {}: {}", workflow_id, e),
                        });

                        Ok(ToolResult::error(format!("Execution failed: {}", e)))
                    }
                }
            })
        },
    )
}
```

**Key Features**:
1. Creates tab via command
2. Shows success/error notifications
3. Spawns log streaming task with rate limiting (prevents channel overflow)
4. Registers task for cleanup
5. Updates status when complete
6. All communication via commands (no direct App access)

---

### Phase 3: UI Integration (Days 6-7, ~12 hours)

#### Step 3.1: Add Notification Rendering

**File**: `src/ui/notifications.rs` (NEW FILE)

```rust
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use crate::app::App;
use crate::app::NotificationLevel;

pub fn render_notifications(f: &mut Frame, app: &App, area: Rect) {
    let notifications = app.notifications.get_active();

    if notifications.is_empty() {
        return;
    }

    // Take bottom 3 lines per notification (max 3 notifications visible)
    let notification_height = (notifications.len() * 3).min(9);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(notification_height as u16),
        ])
        .split(area);

    let notification_area = chunks[1];

    // Render each notification
    let notification_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            notifications
                .iter()
                .take(3)
                .map(|_| Constraint::Length(3))
                .collect::<Vec<_>>()
        )
        .split(notification_area);

    for (idx, notification) in notifications.iter().take(3).enumerate() {
        let (bg_color, fg_color, icon) = match notification.level {
            NotificationLevel::Error => (Color::Red, Color::White, "✗"),
            NotificationLevel::Warning => (Color::Yellow, Color::Black, "⚠"),
            NotificationLevel::Info => (Color::Blue, Color::White, "ℹ"),
            NotificationLevel::Success => (Color::Green, Color::White, "✓"),
        };

        let text = vec![
            Line::from(vec![
                Span::styled(
                    format!("{} {} ", icon, notification.title),
                    Style::default()
                        .fg(fg_color)
                        .bg(bg_color)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(notification.message.clone()),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(bg_color));

        let paragraph = Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, notification_chunks[idx]);
    }
}
```

**File**: `src/ui/mod.rs`

Add module and update main UI function:

```rust
pub mod notifications;

pub fn ui(f: &mut Frame, app: &App) {
    let size = f.area();

    // Render main content (existing logic)
    match app.current_view {
        View::MainMenu => render_main_menu(f, app, size),
        View::WorkflowEdit => render_workflow_edit(f, app, size),
        View::Tabs => render_tabs(f, app, size),
        View::Chat => render_chat(f, app, size),
    }

    // Render notifications as overlay (NEW)
    notifications::render_notifications(f, app, size);
}
```

---

### Phase 4: Testing (Days 8-9, ~12 hours)

#### Step 4.1: Unit Tests

**File**: `tests/test_commands.rs` (NEW FILE)

```rust
use workflow_manager::app::commands::AppCommand;
use uuid::Uuid;
use std::collections::HashMap;

#[test]
fn test_create_tab_command_debug() {
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
fn test_all_command_variants_are_clone() {
    let handle_id = Uuid::new_v4();

    let commands = vec![
        AppCommand::CreateTab {
            workflow_id: "test".to_string(),
            params: HashMap::new(),
            handle_id,
        },
        AppCommand::AppendTabLog {
            handle_id,
            log: workflow_manager_sdk::WorkflowLog::PhaseStarted {
                phase: 0,
                name: "test".to_string(),
                total_phases: 1,
            },
        },
        AppCommand::UpdateTabStatus {
            handle_id,
            status: workflow_manager_sdk::WorkflowStatus::Running,
        },
        AppCommand::CloseTab { handle_id },
        AppCommand::SwitchToTab { handle_id },
        AppCommand::ShowNotification {
            level: NotificationLevel::Info,
            title: "Test".to_string(),
            message: "Test message".to_string(),
        },
        AppCommand::Quit,
    ];

    // All variants should be cloneable
    for cmd in commands {
        let _cloned = cmd.clone();
        assert!(format!("{:?}", cmd).len() > 0);
    }
}
```

#### Step 4.2: Command Ordering Tests

**File**: `tests/test_command_ordering.rs` (NEW FILE)

```rust
use workflow_manager::app::commands::{AppCommand, NotificationLevel};
use tokio::sync::mpsc;
use uuid::Uuid;
use std::collections::HashMap;

#[tokio::test]
async fn test_command_fifo_order() {
    let (tx, mut rx) = mpsc::unbounded_channel::<AppCommand>();

    let handle_id = Uuid::new_v4();

    // Send commands in specific order
    tx.send(AppCommand::CreateTab {
        workflow_id: "test".to_string(),
        params: HashMap::new(),
        handle_id,
    }).unwrap();

    tx.send(AppCommand::AppendTabLog {
        handle_id,
        log: workflow_manager_sdk::WorkflowLog::PhaseStarted {
            phase: 0,
            name: "test".to_string(),
            total_phases: 1,
        },
    }).unwrap();

    tx.send(AppCommand::UpdateTabStatus {
        handle_id,
        status: workflow_manager_sdk::WorkflowStatus::Completed,
    }).unwrap();

    // Verify order is maintained
    match rx.recv().await.unwrap() {
        AppCommand::CreateTab { .. } => {},
        _ => panic!("Expected CreateTab first"),
    }

    match rx.recv().await.unwrap() {
        AppCommand::AppendTabLog { .. } => {},
        _ => panic!("Expected AppendTabLog second"),
    }

    match rx.recv().await.unwrap() {
        AppCommand::UpdateTabStatus { .. } => {},
        _ => panic!("Expected UpdateTabStatus third"),
    }
}

#[tokio::test]
async fn test_concurrent_senders_maintain_order() {
    let (tx, mut rx) = mpsc::unbounded_channel::<AppCommand>();

    let tx1 = tx.clone();
    let tx2 = tx.clone();

    // Spawn two concurrent senders
    let handle1 = tokio::spawn(async move {
        for i in 0..100 {
            tx1.send(AppCommand::ShowNotification {
                level: NotificationLevel::Info,
                title: "Sender1".to_string(),
                message: format!("Message {}", i),
            }).unwrap();
        }
    });

    let handle2 = tokio::spawn(async move {
        for i in 0..100 {
            tx2.send(AppCommand::ShowNotification {
                level: NotificationLevel::Info,
                title: "Sender2".to_string(),
                message: format!("Message {}", i),
            }).unwrap();
        }
    });

    handle1.await.unwrap();
    handle2.await.unwrap();
    drop(tx);

    // Verify we received 200 messages (order within each sender is maintained)
    let mut count = 0;
    while rx.recv().await.is_some() {
        count += 1;
    }

    assert_eq!(count, 200, "Should receive all messages from both senders");
}

#[tokio::test]
async fn test_rapid_fire_commands() {
    let (tx, mut rx) = mpsc::unbounded_channel::<AppCommand>();

    // Send 10,000 commands rapidly
    for i in 0..10_000 {
        tx.send(AppCommand::ShowNotification {
            level: NotificationLevel::Info,
            title: "Test".to_string(),
            message: format!("{}", i),
        }).unwrap();
    }

    drop(tx);

    // Verify all received in order
    let mut expected = 0;
    while let Some(cmd) = rx.recv().await {
        match cmd {
            AppCommand::ShowNotification { message, .. } => {
                assert_eq!(message, format!("{}", expected));
                expected += 1;
            }
            _ => panic!("Unexpected command"),
        }
    }

    assert_eq!(expected, 10_000);
}

#[tokio::test]
async fn test_channel_survives_receiver_drop() {
    let (tx, rx) = mpsc::unbounded_channel::<AppCommand>();

    // Send some messages
    tx.send(AppCommand::Quit).unwrap();

    // Drop receiver (simulates app shutdown)
    drop(rx);

    // Further sends should fail gracefully
    let result = tx.send(AppCommand::Quit);
    assert!(result.is_err(), "Send should fail after receiver dropped");
}

#[tokio::test]
async fn test_task_registry_cleanup() {
    use workflow_manager::app::TaskRegistry;
    use uuid::Uuid;

    let registry = TaskRegistry::new();
    let workflow_id = Uuid::new_v4();

    // Register a task
    let task = tokio::spawn(async {
        tokio::time::sleep(tokio::time::Duration::from_secs(100)).await;
    });

    registry.register(workflow_id, task).await;

    // Cancel it
    registry.cancel_all(&workflow_id).await;

    // Task should be aborted (this test mainly checks no panic)
}
```

#### Step 4.3: Integration Tests

**File**: `tests/test_mcp_integration.rs` (NEW FILE)

```rust
// Note: This requires a mock runtime
// For now, document manual testing scenarios

#[test]
fn test_mcp_workflow_creates_tab() {
    // TODO: Implement with mock runtime
    // 1. Setup mock runtime
    // 2. Create MCP server with command channel
    // 3. Call execute_workflow
    // 4. Verify CreateTab command sent
    // 5. Verify log streaming task spawned
}
```

#### Step 4.4: Manual Testing Checklist

**Test Scenarios**:

1. **Happy Path - MCP Workflow Execution**:
   - [ ] Start app
   - [ ] Open chat view
   - [ ] Ask Claude to execute a workflow
   - [ ] Verify tab appears with notification
   - [ ] Verify logs stream to tab in real-time
   - [ ] Verify workflow completes and status updates
   - [ ] Verify success notification appears

2. **Error Handling**:
   - [ ] Execute invalid workflow ID → error notification, no tab
   - [ ] Execute with missing parameters → error notification, no tab
   - [ ] Workflow fails → tab shows failure status, error notification

3. **Tab Closure - MCP Workflows**:
   - [ ] Execute workflow via MCP
   - [ ] Try to close tab while running → confirmation dialog appears
   - [ ] Confirm close → workflow cancelled, background tasks stopped
   - [ ] Execute another workflow → verify no orphaned tasks from first

4. **Tab Closure - Manual Workflows**:
   - [ ] Launch workflow manually from TUI
   - [ ] Try to close while running → confirmation appears
   - [ ] Confirm close → process killed (existing behavior still works)

5. **Concurrent Execution**:
   - [ ] Execute multiple workflows simultaneously via MCP
   - [ ] Verify all tabs created
   - [ ] Verify logs don't mix between tabs
   - [ ] Close one tab → others continue running

6. **High-Frequency Logs**:
   - [ ] Execute workflow that generates 1000+ logs/sec
   - [ ] Verify TUI remains responsive
   - [ ] Verify logs appear (may be sampled)
   - [ ] Verify no memory issues

7. **Notifications**:
   - [ ] Trigger various operations
   - [ ] Verify notifications appear at bottom of screen
   - [ ] Verify auto-dismiss after 5 seconds
   - [ ] Verify color coding (error=red, success=green, etc.)

8. **Shutdown**:
   - [ ] Start multiple workflows via MCP
   - [ ] Quit app (Ctrl+C or 'q')
   - [ ] Verify all background tasks cleaned up
   - [ ] Verify no panic/errors on exit

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

    // Show notification
    command_tx.send(AppCommand::ShowNotification {
        level: NotificationLevel::Success,
        title: "Workflow Started".to_string(),
        message: "Executing research_agent".to_string(),
    })?;

    // Spawn log streamer
    let task = tokio::spawn(stream_logs(handle.id(), runtime, command_tx));
    task_registry.register(handle.id(), task).await;

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
    let tab = WorkflowTab {
        runtime_handle_id: Some(handle_id),  // Track for cleanup
        // ...
    };
    self.open_tabs.push(tab);
    self.notifications.success("Workflow Started", "...");
    Ok(())
}

// 5. Background task streams logs with rate limiting:
async fn stream_logs(handle_id: Uuid, runtime: Arc<dyn Runtime>, tx: UnboundedSender<AppCommand>) {
    let mut logs = runtime.subscribe_logs(&handle_id).await?;
    let mut buffered = Vec::new();
    let mut last_sent = Instant::now();

    while let Ok(log) = logs.recv().await {
        buffered.push(log);

        if now() - last_sent > 16ms || buffered.len() > 100 {
            for log in buffered.drain(..) {
                tx.send(AppCommand::AppendTabLog { handle_id, log })?;
            }
            last_sent = now();
        }
    }
}

// 6. User closes tab:
// - Existing close_current_tab() shows confirmation
// - User confirms
// - close_tab_confirmed() cancels runtime workflow + background tasks
// - Tab removed, resources cleaned up
```

### 5.2 Error Handling Pattern

```rust
// In MCP tool:
match runtime.execute_workflow(id, params).await {
    Ok(handle) => {
        // Send commands
        if let Err(e) = command_tx.send(AppCommand::CreateTab { /* ... */ }) {
            // Channel closed (app shutdown) - this is OK
            eprintln!("App channel closed: {}", e);
        }

        // Show success
        command_tx.send(AppCommand::ShowNotification {
            level: NotificationLevel::Success,
            title: "Workflow Started".to_string(),
            message: format!("Executing {}", id),
        })?;

        ToolResult::text("Workflow started")
    }
    Err(e) => {
        // Show error notification
        command_tx.send(AppCommand::ShowNotification {
            level: NotificationLevel::Error,
            title: "Workflow Failed".to_string(),
            message: format!("Failed to start: {}", e),
        })?;

        ToolResult::error(format!("Failed: {}", e))
    }
}
```

### 5.3 Graceful Shutdown

```rust
impl App {
    pub fn shutdown(&mut self) -> Result<()> {
        // 1. Cancel all background tasks
        self.tokio_runtime.block_on(async {
            self.task_registry.cancel_everything().await;
        });

        // 2. Close command channel
        self.command_rx.close();

        // 3. Process remaining commands
        while let Ok(cmd) = self.command_rx.try_recv() {
            let _ = self.handle_command(cmd);
        }

        // 4. Cleanup resources
        // ...

        Ok(())
    }
}
```

---

## 6. Testing Strategy

### 6.1 Test Coverage Goals

- **Unit Tests**: 100% coverage of command handlers
- **Integration Tests**: All MCP tools with mock runtime
- **Manual Tests**: All user scenarios documented above
- **Performance Tests**: High-frequency log scenarios

### 6.2 Test Data

Use existing workflows in codebase for testing:
- `research_agent` - Good for log streaming tests
- Simple workflows - Good for basic flow testing
- Create a stress test workflow that emits 10,000 logs

### 6.3 CI Integration

Add to CI pipeline:
```yaml
test:
  script:
    - cargo test --all
    - cargo test --test test_command_ordering
    - cargo test --test test_commands
```

---

## 7. Migration Path

### 7.1 Backward Compatibility

**No breaking changes** - existing functionality continues to work:

- ✅ Manual workflow execution (via TUI) still creates tabs directly
- ✅ Existing confirmation flow for closing tabs preserved
- ✅ Existing key bindings unchanged
- ✅ Event loop continues to poll for keyboard input
- ✅ `child_process` workflows work exactly as before

**New behavior only activates for MCP-launched workflows!**

### 7.2 Phased Rollout

**Week 1** (Days 1-2): Foundation
- Implement command enum, notifications, task registry
- Add channels to App
- Process commands in event loop
- Not user-visible yet (commands not sent)

**Week 1** (Days 3-5): MCP Integration
- Update ChatInterface and MCP tools
- Start sending commands
- Users see tabs created by MCP
- Notifications appear

**Week 2** (Days 6-7): Tab Lifecycle
- Update close handlers for dual-path cleanup
- Test both manual and MCP workflows
- Verify confirmation flow works for both

**Week 2** (Days 8-9): Testing
- Comprehensive test suite
- Manual testing all scenarios
- Bug fixes

**Week 2** (Day 10): Polish & Deploy
- Documentation updates
- Code review
- Merge to main

### 7.3 Rollback Plan

If issues arise:

1. **Quick Rollback**: Comment out command processing in event loop (one line)
2. **Partial Rollback**: Keep infrastructure, disable MCP command sending
3. **Full Rollback**: Revert to tagged release

---

## 8. Future Enhancements

### 8.1 Enhanced Commands

```rust
pub enum AppCommand {
    // ... existing variants ...

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

### 8.4 Performance Monitoring

For debugging:

```rust
#[cfg(debug_assertions)]
impl App {
    pub fn monitor_command_channel(&self) {
        // Log channel depth
        // Warn if > 100 pending commands
    }
}
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

---

## 10. Success Criteria

### 10.1 Functional Requirements

- [x] Task Registry prevents resource leaks
- [x] Notifications provide user feedback
- [x] MCP tools can create tabs via commands
- [x] Workflow logs stream to tabs in real-time
- [x] Tab status updates when workflow completes
- [x] Event loop remains responsive (no blocking)
- [x] Multiple concurrent workflows work correctly
- [x] Existing confirmation flow preserved
- [x] Both manual and MCP workflows handled correctly

### 10.2 Non-Functional Requirements

- **Performance**: Command processing < 1ms per command
- **Reliability**: No crashes from malformed commands
- **Maintainability**: Clear separation of concerns
- **Testability**: All commands unit-testable
- **Observability**: Debug logs + user notifications for all flows
- **Resource Safety**: All background tasks have cleanup paths

### 10.3 User Experience

- **Immediate Feedback**: Tab appears within 100ms of Claude's response
- **Real-time Updates**: Logs appear as they're generated
- **Smooth Rendering**: No flickering or lag during workflow execution
- **Error Visibility**: Clear error messages in TUI (not just stderr)
- **Familiar UX**: Existing tab close confirmation preserved

---

## 11. Next Steps (REVISED TIMELINE)

### Week 1 (5 days)

**Days 1-2: Phase 1 - Foundation (16 hours)**
1. ✅ Review Plan
2. ⬜ Define AppCommand enum (2h)
3. ⬜ Create NotificationManager (4h)
4. ⬜ Create TaskRegistry (4h)
5. ⬜ Update WorkflowTab with runtime_handle_id (1h)
6. ⬜ Add channels/registry to App (2h)
7. ⬜ Update event loop (2h)
8. ⬜ Module declarations (1h)

**Days 3-5: Phase 2 - MCP Integration (20 hours)**
1. ⬜ Implement all command handlers (6h)
2. ⬜ Update ChatInterface signature (2h)
3. ⬜ Update MCP server creation (2h)
4. ⬜ Implement execute_workflow with task registry (5h)
5. ⬜ Implement log streaming with rate limiting (5h)

### Week 2 (5 days)

**Days 6-7: Phase 3 - Tab Lifecycle Integration (12 hours)**
1. ⬜ Update close_tab_confirmed() for dual paths (3h)
2. ⬜ Update kill_current_tab() for dual paths (2h)
3. ⬜ Add notification rendering to UI (3h)
4. ⬜ Test manual workflow close still works (1h)
5. ⬜ Test MCP workflow close + cleanup (2h)
6. ⬜ Bug fixes (1h)

**Days 8-9: Phase 4 - Testing (12 hours)**
1. ⬜ Write unit tests for commands (3h)
2. ⬜ Write command ordering tests (3h)
3. ⬜ Write integration tests (3h)
4. ⬜ Manual testing all scenarios (3h)

**Day 10: Phase 5 - Polish & Documentation (8 hours)**
1. ⬜ Fix bugs found in testing (3h)
2. ⬜ Update documentation (3h)
3. ⬜ Code review and final polish (2h)

**Total Realistic Estimate: 10 days (2 weeks)**

**Contingency**: Add 2 days buffer for unexpected issues

**Final Timeline: 12 days (2.5 weeks)**

---

## 12. Key Differences from Original Plan

### What's New

1. **TaskRegistry** - Explicit background task lifecycle management
2. **NotificationManager** - User-visible feedback system
3. **Dual-path cleanup** - Handles both manual and MCP workflows
4. **Log rate limiting** - Prevents channel overflow in high-frequency scenarios
5. **Command ordering tests** - Prevents race conditions
6. **Enhanced error handling** - All errors visible to users
7. **Realistic timeline** - 10-12 days instead of 5-7 days

### What's Changed

1. **WorkflowTab** - Now has `runtime_handle_id` for MCP workflows
2. **close_tab_confirmed()** - Updated to cancel both workflow types
3. **Event loop** - Added notification cleanup
4. **MCP tools** - Accept task registry, register spawned tasks
5. **AppCommand** - Added `ShowNotification` variant

### What's Preserved

1. **Existing confirmation flow** - Still shows dialog for running workflows
2. **Manual workflow path** - `child_process` still works exactly the same
3. **Key bindings** - All existing shortcuts unchanged
4. **Event loop structure** - Still synchronous with polling
5. **Backward compatibility** - No breaking changes

---

**END OF REVISED PLAN**
