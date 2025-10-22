# Workflow Manager - Codebase Knowledge

Complete architectural and implementation knowledge for the Workflow Manager TUI application.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Module Organization](#module-organization)
- [Core Concepts](#core-concepts)
- [Data Flow](#data-flow)
- [Key Components](#key-components)
- [Design Patterns](#design-patterns)
- [Development Guide](#development-guide)
- [Testing](#testing)
- [Common Tasks](#common-tasks)

---

## Architecture Overview

### Tech Stack

- **Language**: Rust
- **UI Framework**: Ratatui (Terminal UI)
- **Backend**: crossterm (terminal control)
- **SDK**: workflow-manager-sdk / claude-agent-sdk-rust
- **Async Runtime**: Tokio
- **AI Integration**: Claude API via MCP (Model Context Protocol)

### High-Level Design

```
┌─────────────────────────────────────────────────────────┐
│                     main.rs (403 lines)                  │
│              Entry point + Event loop                    │
└────────────────────┬────────────────────────────────────┘
                     │
        ┌────────────┼────────────┬──────────────┐
        │            │            │              │
        ▼            ▼            ▼              ▼
   ┌────────┐  ┌────────┐  ┌─────────┐    ┌──────────┐
   │   UI   │  │  App   │  │ Runtime │    │ Discovery│
   │ Module │  │ Module │  │ Module  │    │  Module  │
   └────────┘  └────────┘  └─────────┘    └──────────┘
        │            │            │              │
        └────────────┴────────────┴──────────────┘
                     │
              ┌──────┴──────┐
              │   Models    │
              │  (Shared)   │
              └─────────────┘
```

### Design Philosophy

1. **Separation of Concerns**: UI, Logic, Data are strictly separated
2. **Domain-Driven Design**: Modules organized by business domain (tabs, navigation, workflow_ops)
3. **Event-Driven**: Main loop polls events and delegates to handlers
4. **Process-Based Execution**: Workflows run as separate processes
5. **Stateful TUI**: Rich terminal interface with multiple views and tabs

---

## Module Organization

### Root Source Files (1467 lines)

```
src/
├── main.rs (403 lines)          # Entry point, event loop, keyboard handling
├── models.rs (8 lines)          # Re-exports from app::models
├── utils.rs (64 lines)          # Utilities (history, workflow loading)
├── chat.rs (428 lines)          # Claude AI chat integration (async)
├── runtime.rs (285 lines)       # Process-based workflow execution
├── mcp_tools.rs (235 lines)     # MCP server and tools
└── discovery.rs (234 lines)     # Workflow binary discovery
```

### App Module (2135 lines)

```
src/app/
├── mod.rs (113 lines)           # Orchestration: new() + open_chat()
├── tabs.rs (333 lines)          # Tab management (9 methods)
├── navigation.rs (474 lines)    # Navigation & scrolling (9 methods)
├── file_browser.rs (241 lines)  # File browser & dropdown (12 methods)
├── history.rs (175 lines)       # Session persistence (4 methods)
└── workflow_ops.rs (799 lines)  # Workflow operations (15 methods)
```

**Total**: 51 App methods organized by domain

### App Models (213 lines)

```
src/app/models/
├── mod.rs (16 lines)            # Re-exports
├── app.rs (71 lines)            # App struct (60 fields)
├── workflow.rs (62 lines)       # Phase/Task/Agent + Status enums
├── tab.rs (42 lines)            # WorkflowTab (per-tab state)
├── view.rs (12 lines)           # View enum (routing)
└── history.rs (10 lines)        # WorkflowHistory struct
```

### UI Module (1597 lines)

```
src/ui/
├── mod.rs (108 lines)           # Main ui() orchestration
├── workflow_views.rs (597 lines) # List/Detail/Edit/Running views
├── tab_views.rs (446 lines)     # Tab bar/content/empty/confirmation
├── components.rs (212 lines)    # Dropdown/file browser/popups
├── header_footer.rs (136 lines) # Header & footer rendering
└── chat_view.rs (98 lines)      # Chat UI rendering
```

---

## Core Concepts

### 1. Views (Routing)

The app uses a view-based routing system:

```rust
pub enum View {
    WorkflowList,           // Browse available workflows
    WorkflowDetail(usize),  // View workflow details
    WorkflowEdit(usize),    // Edit workflow fields
    WorkflowRunning(usize), // [DEPRECATED] Legacy single workflow view
    Tabs,                   // Main tabbed interface (current default)
    Chat,                   // AI chat interface
}
```

**Navigation Flow**:
```
WorkflowList → (Enter) → WorkflowDetail → (E) → WorkflowEdit → (L) → New Tab
                                                                          ↓
Tabs ←──────────────────────────────────────────────────────────────────┘
  ↕ (A key)
Chat
```

### 2. Tabs System

Each tab represents a running or completed workflow instance:

```rust
pub struct WorkflowTab {
    // Identity
    pub id: String,                    // "research_20251014_120000"
    pub workflow_idx: usize,           // Index in App.workflows
    pub workflow_name: String,         // "Research Agent"
    pub instance_number: usize,        // #1, #2, #3...

    // Execution
    pub status: WorkflowStatus,
    pub child_process: Option<Child>,
    pub exit_code: Option<i32>,

    // Data (Arc<Mutex<>> for thread-safe updates)
    pub workflow_phases: Arc<Mutex<Vec<WorkflowPhase>>>,
    pub workflow_output: Arc<Mutex<Vec<String>>>,
    pub field_values: HashMap<String, String>,

    // UI State (per-tab!)
    pub scroll_offset: usize,
    pub expanded_phases: HashSet<usize>,
    pub expanded_tasks: HashSet<String>,
    pub expanded_agents: HashSet<String>,
    pub selected_phase: usize,
    pub selected_task: Option<String>,
    pub selected_agent: Option<String>,
    pub agent_scroll_offsets: HashMap<String, usize>,
}
```

**Key Features**:
- Multiple workflows can run simultaneously
- Each tab has independent UI state (scroll, expansions, selections)
- Tab titles show: "Workflow Name #N (Status)"
- Tabs persist across sessions

### 3. Workflow Hierarchy

Workflows execute in a hierarchical structure:

```
Workflow
  └── Phase 1: "Research"
       ├── Task 1.1: "Initialize"
       │    └── Agent: "coordinator" → Messages
       ├── Task 1.2: "Search"
       │    ├── Agent: "searcher_1" → Messages
       │    └── Agent: "searcher_2" → Messages
       └── Task 1.3: "Synthesize"
            └── Agent: "writer" → Messages
  └── Phase 2: "Output"
       └── ...
```

**Data Structures**:
```rust
pub struct WorkflowPhase {
    pub id: usize,
    pub name: String,
    pub status: PhaseStatus,  // NotStarted/Running/Completed/Failed
    pub tasks: Vec<WorkflowTask>,
    pub output_files: Vec<(String, String)>,
}

pub struct WorkflowTask {
    pub id: String,
    pub phase: usize,
    pub description: String,
    pub status: TaskStatus,
    pub agents: Vec<WorkflowAgent>,
    pub messages: Vec<String>,
    pub result: Option<String>,
}

pub struct WorkflowAgent {
    pub id: String,           // "task_id:agent_name"
    pub task_id: String,
    pub name: String,
    pub description: String,
    pub status: AgentStatus,
    pub messages: Vec<String>,
    pub result: Option<String>,
}
```

### 4. Workflow Discovery

Workflows are discovered from filesystem:

```rust
// In src/discovery.rs
pub fn discover_workflows() -> Vec<DiscoveredWorkflow> {
    // 1. Search known paths for executables
    // 2. Execute each with --info flag
    // 3. Parse WorkflowMetadata from stdout
    // 4. Return list of discovered workflows
}
```

**Search Paths**:
1. `$CARGO_MANIFEST_DIR/target/debug/` (development)
2. `$CARGO_MANIFEST_DIR/target/release/` (release builds)
3. Current directory
4. System PATH

**Workflow Binary Contract**:
```bash
$ workflow-binary --info
# Outputs JSON with WorkflowMetadata:
# - id, name, description
# - fields (inputs required)
# - phases (execution structure)
```

### 5. Session Persistence

The app saves and restores session state:

**Saved State** (`~/.local/share/workflow-manager/session.json`):
```json
{
  "tabs": [
    {
      "workflow_idx": 0,
      "workflow_name": "Research Agent",
      "instance_number": 1,
      "field_values": {"query": "rust async", "depth": "3"},
      "status": "Completed",
      "saved_logs": ["phase output...", "..."]
    }
  ],
  "active_tab_idx": 0
}
```

**History** (`~/.local/share/workflow-manager/history.json`):
```json
{
  "workflows": {
    "research_agent": {
      "query": ["rust async", "tokio runtime", "previous query"],
      "depth": ["3", "5", "2"]
    }
  }
}
```

---

## Data Flow

### 1. Application Startup

```
main()
  ├── enable_raw_mode()
  ├── setup terminal
  ├── App::new()
  │    ├── load_workflows() → discovers workflow binaries
  │    ├── load_history() → loads field history
  │    ├── ProcessBasedRuntime::new() → initializes runtime
  │    ├── ChatInterface::new() → starts Claude init in background
  │    ├── restore_session() → restores previous tabs
  │    └── current_view = View::Tabs
  └── run_app(terminal, app)
```

### 2. Main Event Loop

```rust
fn run_app(terminal, app) {
    loop {
        // 1. Poll all running workflow processes
        app.poll_all_tabs();

        // 2. Poll chat for async operations (non-blocking)
        if let Some(chat) = &mut app.chat {
            chat.poll_initialization();  // Check init completion
            chat.poll_response();         // Check message responses

            // Animate spinner during init or thinking
            if !chat.initialized || chat.waiting_for_response {
                chat.update_spinner();
            }
        }

        // 3. Render UI
        terminal.draw(|f| ui::ui(f, app))?;

        // 4. Handle keyboard input (50ms timeout)
        if event::poll(50ms)? {
            match event::read()? {
                // Dispatch to appropriate handler based on:
                // - Current view
                // - Modal state (confirmation, dropdown, file browser)
                // - Edit mode
            }
        }

        // 5. Check quit flag
        if app.should_quit {
            app.save_session();
            break;
        }
    }
}
```

### 3. Tab Polling (Real-time Updates)

```rust
// In src/app/tabs.rs
pub fn poll_all_tabs(&mut self) {
    for tab in &mut self.open_tabs {
        if let Some(child) = &mut tab.child_process {
            // 1. Check if process is still running
            match child.try_wait() {
                Ok(Some(status)) => {
                    tab.status = WorkflowStatus::Completed;
                    tab.exit_code = status.code();
                }
                Ok(None) => {
                    // Still running - poll stdout/stderr
                    // Parse structured logs: __WF_EVENT__:{json}
                    // Update workflow_phases via handle_workflow_event()
                }
                Err(e) => {
                    tab.status = WorkflowStatus::Failed;
                }
            }
        }
    }
}
```

### 4. Workflow Launch Flow

```
User in WorkflowEdit view → Presses 'L' → launch_workflow_in_tab()
  ├── Get workflow by idx
  ├── Validate field_values
  ├── Generate tab ID: "{workflow_id}_{timestamp}"
  ├── Increment workflow counter for display #
  ├── Build command: workflow-binary --field1=value1 --field2=value2
  ├── Spawn child process with stdout/stderr capture
  ├── Create WorkflowTab with:
  │    ├── Arc<Mutex<Vec<WorkflowPhase>>> (shared with polling thread)
  │    ├── Arc<Mutex<Vec<String>>> (shared output buffer)
  │    └── UI state (expansions, scroll, selections)
  ├── Add tab to open_tabs
  ├── Set active_tab_idx to new tab
  ├── current_view = View::Tabs
  └── Save field values to history
```

### 5. Structured Logging Protocol

Workflows communicate progress via structured logs on stderr:

**Workflow Binary Output**:
```rust
// In workflow code:
eprintln!("__WF_EVENT__:{}", serde_json::to_string(&event)?);
```

**Event Types** (WorkflowLog enum):
```rust
pub enum WorkflowLog {
    PhaseStart { phase_id, name },
    PhaseComplete { phase_id },
    PhaseFailed { phase_id, error },

    TaskStart { phase_id, task_id, description },
    TaskComplete { phase_id, task_id },
    TaskFailed { phase_id, task_id, error },

    AgentStart { phase_id, task_id, agent_id, name },
    AgentMessage { phase_id, task_id, agent_id, message },
    AgentComplete { phase_id, task_id, agent_id, result },
    AgentFailed { phase_id, task_id, agent_id, error },

    OutputFile { path, description },
}
```

**Processing** (in `app/workflow_ops.rs`):
```rust
fn handle_workflow_event(event: WorkflowLog, phases: &Arc<Mutex<Vec<WorkflowPhase>>>) {
    match event {
        PhaseStart { phase_id, name } => {
            // Create or update phase in phases vec
        }
        AgentMessage { agent_id, message, ... } => {
            // Find agent in hierarchy and append message
        }
        // ... etc
    }
}
```

### 6. File Browser Flow

```
User in Edit mode → Types '/' (empty buffer) → open_file_browser()
  ├── show_file_browser = true
  ├── load_file_browser_items()
  │    ├── Read current_dir entries
  │    ├── Filter by file_browser_search (fuzzy matching)
  │    └── Sort: directories first, then files
  └── Render modal overlay

User types characters → Updates file_browser_search → Re-filters items
User presses Enter → file_browser_select()
  ├── Get selected path
  ├── If directory: Update current_dir, reload items
  ├── If file: Insert path into edit_buffer
  └── close_file_browser()
```

### 7. History Dropdown Flow

```
User in Edit mode → Presses Tab → show_history_dropdown()
  ├── Get current workflow_id and field_name
  ├── Load previous values from history
  ├── Populate dropdown_items
  └── show_dropdown = true

User presses Enter → dropdown_select()
  ├── Get selected value from history
  ├── Set edit_buffer = selected_value
  └── close_dropdown()
```

---

## Key Components

### Main Event Loop (main.rs:54-399)

**Keyboard Handling Modes**:

1. **Close Confirmation** (lines 68-78)
   - Y/N to confirm tab close

2. **Dropdown Mode** (lines 80-95)
   - Up/Down/Tab: Navigate
   - Enter: Select
   - Esc: Cancel

3. **File Browser Mode** (lines 98-120)
   - j/k or Up/Down: Navigate
   - Enter: Select directory or file
   - Typing: Fuzzy search
   - Esc: Cancel

4. **Edit Mode** (lines 123-160)
   - Typing: Append to edit_buffer
   - Backspace: Delete char
   - Enter: save_edited_field()
   - Esc: cancel_editing()
   - Tab: Path completion or history dropdown
   - '/': Open file browser

5. **Chat Mode** (lines 161-213)
   - Typing: Append to chat input
   - Enter: Send message to Claude
   - Up/Down: Scroll messages
   - Esc: Exit to Tabs view

6. **Normal Navigation** (lines 215-386)
   - q/Q: Quit
   - j/k or Up/Down: Navigate
   - Tab/Shift+Tab: Switch tabs (Tabs view only)
   - Enter: Context-dependent action
   - Space: Toggle expand all
   - E: Edit workflow
   - L: Launch workflow
   - C: Close tab
   - K: Kill workflow
   - R: Rerun workflow
   - A: Open chat
   - Esc/b: Go back

**Important**: Shift+Tab is handled as `KeyCode::BackTab`, not `KeyCode::Tab` with SHIFT modifier (crossterm quirk).

### App Initialization (app/mod.rs:17-85)

**Sequence**:
1. Load workflows via discovery
2. Load history from disk
3. Create tokio runtime for async ops
4. Initialize all App struct fields
5. Create ProcessBasedRuntime
6. Create ChatInterface with runtime
7. Restore previous session (tabs)
8. Set initial view to Tabs

**Critical Fields**:
```rust
pub struct App {
    // Workflow catalog
    pub workflows: Vec<Workflow>,

    // Tab state
    pub open_tabs: Vec<WorkflowTab>,
    pub active_tab_idx: usize,
    pub workflow_counters: HashMap<String, usize>,  // For #N display

    // View routing
    pub current_view: View,

    // Modal overlays
    pub show_file_browser: bool,
    pub show_dropdown: bool,
    pub show_close_confirmation: bool,

    // Edit state
    pub is_editing: bool,
    pub edit_buffer: String,
    pub edit_field_index: usize,
    pub field_values: HashMap<String, String>,

    // Chat
    pub chat: Option<ChatInterface>,
    pub tokio_runtime: Runtime,

    // Runtime
    pub runtime: Option<Arc<dyn WorkflowRuntime>>,
}
```

### UI Rendering (ui/mod.rs:9-108)

**Main ui() Function Flow**:
```rust
pub fn ui(f: &mut Frame, app: &App) {
    let size = f.area();

    // 1. Choose which view to render
    match app.current_view {
        View::WorkflowList => render_workflow_list(f, size, app),
        View::WorkflowDetail(idx) => render_workflow_detail(f, size, app, idx),
        View::WorkflowEdit(idx) => render_workflow_edit(f, size, app, idx),
        View::Tabs => {
            if app.open_tabs.is_empty() {
                render_empty_tabs(f, size)
            } else {
                render_tab_content(f, size, app)
            }
        }
        View::Chat => render_chat(f, size, app),
        _ => {}
    }

    // 2. Render modal overlays (on top)
    if app.show_dropdown {
        render_dropdown(f, size, app);
    }
    if app.show_file_browser {
        render_file_browser(f, size, app);
    }
    if app.show_close_confirmation {
        render_close_confirmation(f, size);
    }
}
```

**Component Architecture**:
- Each render function takes `Frame`, `Rect`, `App`
- Uses Ratatui widgets (Block, Paragraph, List, etc.)
- Layout managed via `Layout::default()` with constraints
- Colors: Blue (selected), Gray (normal), Green (success), Red (error)

### Process-Based Runtime (runtime.rs)

**Purpose**: Execute workflow binaries as child processes

```rust
pub struct ProcessBasedRuntime {
    workflow_dir: PathBuf,
}

impl WorkflowRuntime for ProcessBasedRuntime {
    fn execute_workflow(&self, workflow: &Workflow, params: &HashMap<String, String>)
        -> Result<String>
    {
        // 1. Find workflow binary
        // 2. Build command with --field=value args
        // 3. Spawn process and wait
        // 4. Return stdout
    }

    fn discover_workflows(&self) -> Result<Vec<Workflow>> {
        // Delegates to discovery::discover_workflows()
    }
}
```

### Chat Integration (chat.rs)

**Non-Blocking Async Architecture**:

The chat interface uses background tokio tasks with channel-based communication to keep the UI responsive during Claude API calls.

```rust
pub struct ChatInterface {
    // State
    pub messages: Vec<ChatMessage>,
    pub input_buffer: String,
    pub initialized: bool,
    pub init_error: Option<String>,

    // Async components
    client: Option<Arc<Mutex<ClaudeSDKClient>>>,
    init_rx: Option<mpsc::UnboundedReceiver<InitResult>>,
    response_rx: Option<mpsc::UnboundedReceiver<ChatResponse>>,
    tokio_handle: tokio::runtime::Handle,

    // UI state
    pub waiting_for_response: bool,
    pub response_start_time: Option<Instant>,
    pub spinner_frame: usize,
    pub message_scroll: u16,
    pub log_scroll: u16,
    pub active_pane: ActivePane,

    // MCP runtime
    runtime: Arc<dyn WorkflowRuntime>,
}
```

**Initialization Flow** (automatic on startup):
```rust
impl ChatInterface {
    pub fn new(runtime: Arc<dyn WorkflowRuntime>, tokio_handle: tokio::runtime::Handle) -> Self {
        // 1. Create ChatInterface with empty messages
        // 2. Start background initialization task via start_initialization()
        // 3. Returns immediately (non-blocking)
    }

    fn start_initialization(&mut self, runtime, tokio_handle) {
        // Spawns tokio task to create Claude client + MCP server
        tokio_handle.spawn(async move {
            let result = initialize_internal(runtime).await;
            tx.send(result); // Send result via channel
        });
    }

    pub fn poll_initialization(&mut self) {
        // Called in main loop - non-blocking try_recv()
        // On success: stores client, sets initialized=true, adds greeting
    }
}
```

**Message Sending Flow** (non-blocking):
```rust
impl ChatInterface {
    pub fn send_message_async(&mut self, message: String) {
        // 1. Set waiting_for_response=true
        // 2. Create response channel
        // 3. Spawn background task
        tokio_handle.spawn(async move {
            let result = send_message_internal(client, message).await;
            tx.send(result); // Send ChatResponse via channel
        });
    }

    pub fn poll_response(&mut self) {
        // Called in main loop - non-blocking try_recv()
        // On success: adds response to messages, sets waiting_for_response=false
    }
}
```

**Main Loop Integration**:
```rust
// src/main.rs event loop
loop {
    if let Some(chat) = &mut app.chat {
        chat.poll_initialization();  // Check for init completion
        chat.poll_response();         // Check for message response

        // Animate spinner during init or thinking
        if !chat.initialized || chat.waiting_for_response {
            chat.update_spinner();
        }
    }

    terminal.draw(|f| ui::ui(f, app))?;
    // ... keyboard events ...
}
```

**MCP Tools Available**:
- `mcp__workflow_manager__list_workflows`: Get available workflows
- `mcp__workflow_manager__execute_workflow`: Run a workflow by ID with parameters
- `mcp__workflow_manager__get_workflow_logs`: Get execution logs
- `mcp__workflow_manager__get_workflow_status`: Check workflow status
- `mcp__workflow_manager__cancel_workflow`: Cancel running workflow

**Two-Pane UI Layout**:
- Left 60%: Chat messages with user/assistant conversation
- Right 40%: Tool call logs showing MCP tool invocations and results
- Independent scroll for each pane
- Tab key switches active pane

---

## Design Patterns

### 1. Module Re-exports

**Pattern**: Each module directory has a `mod.rs` that re-exports types:

```rust
// src/app/models/mod.rs
mod history;
mod workflow;
mod tab;
mod view;
mod app;

pub use history::*;
pub use workflow::*;
pub use tab::*;
pub use view::*;
pub use app::*;
```

**Benefits**:
- Clean public API: `use crate::app::models::App` (not `use crate::app::models::app::App`)
- Easy refactoring: Move files without breaking imports
- Encapsulation: Internal modules can stay private

### 2. Shared State via Arc<Mutex<>>

**Pattern**: Thread-safe shared state between main thread and background threads:

```rust
pub struct WorkflowTab {
    pub workflow_phases: Arc<Mutex<Vec<WorkflowPhase>>>,
    pub workflow_output: Arc<Mutex<Vec<String>>>,
    // ...
}

// Polling thread updates:
if let Ok(mut phases) = tab.workflow_phases.lock() {
    phases[0].status = PhaseStatus::Running;
}

// UI thread reads:
if let Ok(phases) = tab.workflow_phases.lock() {
    for phase in phases.iter() {
        // render...
    }
}
```

### 3. View-Based Routing

**Pattern**: Single `View` enum controls which UI to render:

```rust
pub enum View {
    WorkflowList,
    WorkflowDetail(usize),
    WorkflowEdit(usize),
    Tabs,
    Chat,
}

// Navigation:
app.current_view = View::WorkflowDetail(idx);

// Rendering:
match app.current_view {
    View::WorkflowList => render_workflow_list(),
    View::WorkflowDetail(idx) => render_workflow_detail(idx),
    // ...
}
```

### 4. Hierarchical Selection State

**Pattern**: Three-level selection for workflow hierarchy:

```rust
pub struct WorkflowTab {
    pub selected_phase: usize,           // Current phase index
    pub selected_task: Option<String>,   // Current task ID
    pub selected_agent: Option<String>,  // Current agent ID
}

// Navigation logic:
if selected_agent.is_some() {
    // Navigate within agent messages
} else if selected_task.is_some() {
    // Navigate within task agents
} else {
    // Navigate within phases
}
```

### 5. Fuzzy Matching

**Pattern**: Use skim fuzzy matcher for search:

```rust
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

let matcher = SkimMatcherV2::default();
let items: Vec<_> = all_items.into_iter()
    .filter(|item| {
        matcher.fuzzy_match(item.to_str().unwrap(), &search_query).is_some()
    })
    .collect();
```

### 6. Centered Modal Overlay

**Pattern**: Render modals in center of screen:

```rust
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
```

---

## Development Guide

### Building

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Run
cargo run --release

# Run with specific binary
./target/release/workflow-manager
```

### Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_workflow_discovery

# Run with output
cargo test -- --nocapture
```

### Code Quality

```bash
# Auto-fix issues
cargo fix --allow-dirty

# Lint
cargo clippy --all-targets

# Auto-fix clippy warnings
cargo clippy --fix --allow-dirty

# Format
cargo fmt

# Check formatting
cargo fmt -- --check
```

### Creating a New Workflow

1. **Create binary in `src/bin/my_workflow.rs`**:

```rust
use workflow_manager_sdk::{WorkflowDefinition, FieldType};
use clap::Parser;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    info: bool,

    #[arg(long)]
    my_field: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.info {
        // Output workflow metadata
        let def = WorkflowDefinition::new()
            .id("my_workflow")
            .name("My Workflow")
            .description("Does something cool")
            .field("my_field", FieldType::Text, "Input parameter");

        println!("{}", serde_json::to_string(&def.build())?);
        return Ok(());
    }

    // Execute workflow
    let input = args.my_field.expect("--my-field required");

    eprintln!("__WF_EVENT__:{}", serde_json::to_string(&WorkflowLog::PhaseStart {
        phase_id: 0,
        name: "Processing".to_string(),
    })?);

    // Do work...

    eprintln!("__WF_EVENT__:{}", serde_json::to_string(&WorkflowLog::PhaseComplete {
        phase_id: 0,
    })?);

    Ok(())
}
```

2. **Build and run**:
```bash
cargo build --release
# Workflow will be automatically discovered on next app launch
```

### Adding a New App Method

1. **Choose the right module** based on domain:
   - `tabs.rs`: Tab lifecycle, switching, closing
   - `navigation.rs`: Scrolling, moving selections
   - `file_browser.rs`: File/path operations
   - `history.rs`: Persistence, session management
   - `workflow_ops.rs`: Workflow execution, editing, launching

2. **Add method to appropriate `impl App` block**:

```rust
// In src/app/tabs.rs
impl App {
    pub fn my_new_method(&mut self) {
        // Implementation
    }
}
```

3. **Method is automatically available** due to re-exports in `app/mod.rs`

### Adding a New View

1. **Add variant to View enum** (`app/models/view.rs`):
```rust
pub enum View {
    // ... existing
    MyNewView,
}
```

2. **Add render function** (`ui/workflow_views.rs` or new file):
```rust
pub fn render_my_view(f: &mut Frame, area: Rect, app: &App) {
    // Render logic
}
```

3. **Add to ui() dispatcher** (`ui/mod.rs`):
```rust
match app.current_view {
    // ... existing
    View::MyNewView => render_my_view(f, size, app),
}
```

4. **Add keyboard handler** (`main.rs` in appropriate section)

### Adding a New Modal

1. **Add state to App** (`app/models/app.rs`):
```rust
pub struct App {
    // ... existing
    pub show_my_modal: bool,
    pub my_modal_data: Vec<String>,
}
```

2. **Add render function** (`ui/components.rs`):
```rust
pub fn render_my_modal(f: &mut Frame, area: Rect, app: &App) {
    let popup_area = centered_rect(60, 40, area);
    // Render modal
}
```

3. **Add to ui() overlay section** (`ui/mod.rs`):
```rust
if app.show_my_modal {
    render_my_modal(f, size, app);
}
```

4. **Add keyboard handler** in main event loop

---

## Testing

### Unit Tests

Located in respective modules:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_discovery() {
        let workflows = discover_workflows();
        assert!(!workflows.is_empty());
    }
}
```

### Integration Testing

Manual testing checklist:

**Workflow List View**:
- [ ] List displays all discovered workflows
- [ ] Arrow keys navigate
- [ ] Enter opens detail view

**Workflow Detail View**:
- [ ] Shows workflow metadata
- [ ] Shows required fields
- [ ] E key opens edit view
- [ ] b/Esc returns to list

**Workflow Edit View**:
- [ ] Enter on field starts editing
- [ ] Can type values
- [ ] Tab shows history dropdown (for text fields)
- [ ] / opens file browser (for file fields)
- [ ] L launches workflow in new tab
- [ ] Esc cancels editing

**Tabs View**:
- [ ] Empty state shows hint when no tabs
- [ ] Tab bar shows all open tabs
- [ ] Tab/Shift+Tab cycles through tabs
- [ ] Running workflows update in real-time
- [ ] Phases/tasks/agents display correctly
- [ ] Enter toggles expansion
- [ ] Space toggles expand all
- [ ] h/l or arrows scroll agent messages
- [ ] C prompts for close confirmation
- [ ] Y actually closes tab
- [ ] K kills running workflow
- [ ] R reruns workflow
- [ ] E edits workflow in current tab

**Chat View**:
- [ ] A key opens chat from Tabs view
- [ ] Can type messages
- [ ] Enter sends message
- [ ] Claude responds
- [ ] Tool calls execute workflows
- [ ] Up/Down scrolls message history
- [ ] Esc returns to Tabs view

**Session Persistence**:
- [ ] Quit and restart preserves tabs
- [ ] Field history persists
- [ ] Tab state (scroll, expansions) preserved

---

## Common Tasks

### Debugging Workflow Discovery

```bash
# Check what workflows are discovered
cargo run --release
# In app: View workflow list - should show all binaries

# Test workflow --info manually
./target/release/my_workflow --info
# Should output JSON metadata
```

### Debugging Structured Logs

Add debug output to workflow:
```rust
eprintln!("__WF_EVENT__:{}", serde_json::to_string(&event)?);
eprintln!("DEBUG: Regular stderr message");
```

Check parsing in `app/workflow_ops.rs::launch_workflow_in_tab()` around line 410.

### Debugging UI Rendering

Add temporary debug output:
```rust
pub fn render_my_view(f: &mut Frame, area: Rect, app: &App) {
    eprintln!("DEBUG: Rendering view, area = {:?}", area);
    // ... render
}
```

Note: Debug output goes to terminal after app exits.

### Finding Memory Leaks

```bash
# Run with valgrind
valgrind --leak-check=full ./target/release/workflow-manager

# Profile with heaptrack
heaptrack ./target/release/workflow-manager
```

### Performance Profiling

```bash
# Build with profiling symbols
cargo build --release --profile=release-with-debug

# Run with perf
perf record --call-graph dwarf ./target/release/workflow-manager
perf report
```

### Adding Keyboard Shortcuts

1. Find appropriate handler section in `main.rs`
2. Add match arm:
```rust
KeyCode::Char('x') | KeyCode::Char('X') => {
    if matches!(app.current_view, View::MyView) {
        app.my_action();
    }
}
```

3. Document in footer via `render_footer()` in `ui/header_footer.rs`

### Troubleshooting Common Issues

**"No workflows discovered"**:
- Check binaries exist in target/debug or target/release
- Run `cargo build` first
- Verify workflow implements `--info` flag

**"Tab doesn't update"**:
- Check workflow outputs structured logs (`__WF_EVENT__`)
- Verify polling is working (`poll_all_tabs()` called in loop)
- Check Arc<Mutex<>> is properly shared

**"Keyboard input not working"**:
- Check modal overlays aren't blocking (dropdown, file browser)
- Verify view is set correctly
- Check event handling order (modals first, then view-specific)

**"Session not persisting"**:
- Check directories crate creates ~/.local/share/workflow-manager/
- Verify save_session() called on quit
- Check JSON serialization doesn't fail

---

## Future Improvements

### Planned Features

1. **Workflow Templates**: Save common field configurations
2. **Tab Groups**: Organize tabs into groups
3. **Search/Filter**: Search across workflow output
4. **Export Results**: Export workflow output to files
5. **Keyboard Customization**: User-defined keybindings
6. **Themes**: Color scheme customization
7. **Workflow Scheduling**: Run workflows on schedule
8. **Remote Execution**: Run workflows on remote machines

### Known Limitations

1. **WorkflowRunning View**: Deprecated, kept for backwards compat
2. **Fixed Keybindings**: Not customizable yet
3. **Single Screen**: No split-screen support
4. **Limited Error Display**: Errors shown briefly, not persistent
5. **No Undo**: Tab close, kill operations not undoable

### Architecture Debt

1. **App Struct Size**: 60 fields, could be further modularized
2. **Global Tokio Runtime**: One runtime for entire app, could be per-component
3. **Polling Frequency**: Fixed 50ms, could be adaptive
4. **Lock Contention**: Arc<Mutex<>> on hot paths, consider lock-free structures

---

## Appendix

### Key Files Reference

| File | Lines | Purpose |
|------|-------|---------|
| main.rs | 403 | Entry point, event loop |
| app/mod.rs | 113 | App initialization |
| app/tabs.rs | 333 | Tab management |
| app/navigation.rs | 474 | Navigation logic |
| app/workflow_ops.rs | 799 | Workflow operations |
| ui/workflow_views.rs | 597 | Workflow UI |
| ui/tab_views.rs | 446 | Tab UI |
| runtime.rs | 285 | Process execution |
| discovery.rs | 234 | Workflow discovery |

### External Dependencies

```toml
[dependencies]
ratatui = "0.28"              # Terminal UI framework
crossterm = "0.28"            # Terminal control
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
clap = { version = "4.5", features = ["derive"] }
directories = "5.0"           # OS-specific paths
chrono = "0.4"                # Date/time
fuzzy-matcher = "0.3"         # Fuzzy search
claude_sdk = "0.1"            # Claude API (if using)
```

### Terminology

- **Workflow**: Executable binary that performs a task
- **Tab**: Running instance of a workflow
- **Phase**: Major step in workflow execution
- **Task**: Unit of work within a phase
- **Agent**: AI agent executing within a task
- **View**: UI screen/route
- **Modal**: Overlay UI component (dropdown, file browser)
- **Session**: Persistent app state across restarts

---

**Document Version**: 1.0
**Last Updated**: 2025-10-15
**Codebase Version**: After Phase 8 refactoring (commit d56afe0)
