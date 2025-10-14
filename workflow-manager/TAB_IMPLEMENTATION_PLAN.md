# Tab Interface Implementation Plan

## Executive Summary

**Goal:** Transform the workflow manager from single-workflow execution to a tabbed interface supporting multiple concurrent workflows.

**Scope:** Major architectural refactor touching ~80% of `src/main.rs`

**Estimated Changes:** 15-20 code modifications, ~500-800 lines of new/changed code

**Risk Level:** High (breaking changes to core state management)

---

## Current Architecture Analysis

### Current State Model
```rust
struct App {
    workflows: Vec<Workflow>,              // Catalog (unchanged)
    current_view: View,                     // Single view state

    // Single workflow execution state
    workflow_output: Arc<Mutex<Vec<String>>>,
    workflow_phases: Arc<Mutex<Vec<WorkflowPhase>>>,
    workflow_running: bool,

    // Single workflow UI state
    expanded_phases: HashSet<usize>,
    expanded_tasks: HashSet<String>,
    selected_phase: usize,
    workflow_scroll_offset: usize,

    // Shared state (unchanged)
    field_values: HashMap<String, String>,
    history: WorkflowHistory,
    // ... file browser, dropdown, etc.
}

enum View {
    WorkflowList,
    WorkflowDetail(usize),    // workflow catalog index
    WorkflowEdit(usize),      // workflow catalog index
    WorkflowRunning(usize),   // workflow catalog index
}
```

### Problems with Current Design
1. **Single workflow only** - Can't run multiple at once
2. **Blocking execution** - UI switches to running view, can't browse others
3. **Lost state** - Closing running workflow loses all output
4. **No session persistence** - App restart loses everything

---

## New Architecture Design

### New State Model

```rust
// New: Per-tab state container
struct WorkflowTab {
    // Identity
    id: String,                               // Unique: "research_20251014_120000"
    workflow_idx: usize,                      // Index in App.workflows catalog
    workflow_name: String,                    // "Research Agent Workflow"
    instance_number: usize,                   // Counter for display: #1, #2, #3
    start_time: Option<DateTime<Local>>,      // When launched

    // Execution state
    status: WorkflowStatus,                   // NotStarted/Running/Completed/Failed
    child_process: Option<Child>,             // Running process handle
    stdout_reader: Option<BufReader<ChildStdout>>,
    stderr_reader: Option<BufReader<ChildStderr>>,
    exit_code: Option<i32>,                   // For completed/failed

    // Workflow data (per tab)
    workflow_phases: Arc<Mutex<Vec<WorkflowPhase>>>,
    workflow_output: Arc<Mutex<Vec<String>>>,
    field_values: HashMap<String, String>,    // Config used for this run

    // UI state (per tab)
    scroll_offset: usize,
    expanded_phases: HashSet<usize>,
    expanded_tasks: HashSet<String>,
    expanded_agents: HashSet<String>,
    selected_phase: usize,
    selected_task: Option<String>,
    selected_agent: Option<String>,

    // Session persistence
    saved_logs: Option<Vec<String>>,          // For restored tabs
}

struct App {
    // Catalog (unchanged)
    workflows: Vec<Workflow>,

    // Tab management (NEW)
    open_tabs: Vec<WorkflowTab>,
    active_tab_idx: usize,
    workflow_counters: HashMap<String, usize>,  // workflow_id -> next instance #

    // View state (CHANGED)
    current_view: View,
    show_close_confirmation: bool,             // NEW

    // Shared state (mostly unchanged)
    selected: usize,                          // For WorkflowList
    edit_field_index: usize,                  // For WorkflowEdit
    edit_buffer: String,
    is_editing: bool,
    field_values: HashMap<String, String>,    // Temp during edit
    show_file_browser: bool,
    // ... file browser state
    show_dropdown: bool,
    // ... dropdown state
    history: WorkflowHistory,
    history_items: Vec<String>,
    current_dir: PathBuf,

    // Removed (now per-tab):
    // workflow_output: Arc<Mutex<Vec<String>>>,      ← REMOVE
    // workflow_running: bool,                        ← REMOVE
    // workflow_phases: Arc<Mutex<Vec<WorkflowPhase>>>, ← REMOVE
    // expanded_phases: HashSet<usize>,               ← REMOVE
    // expanded_tasks: HashSet<String>,               ← REMOVE
    // expanded_agents: HashSet<String>,              ← REMOVE
    // selected_phase: usize,                         ← REMOVE
    // selected_task: Option<String>,                 ← REMOVE
    // selected_agent: Option<String>,                ← REMOVE
    // workflow_scroll_offset: usize,                 ← REMOVE
}

enum View {
    WorkflowList,              // Browse catalog (for new tab)
    WorkflowDetail(usize),     // View workflow info (for new tab)
    WorkflowEdit(usize),       // Configure before launch (for new tab)
    Tabs,                      // NEW: Main view with tabs
}
```

### Data Flow

#### Current Flow
```
WorkflowList → WorkflowEdit → WorkflowRunning (blocks)
                                     ↓
                            [process completes]
                                     ↓
                            Back to WorkflowList (output lost)
```

#### New Flow
```
Initial: Tabs (empty) → Press Ctrl+T
                             ↓
              WorkflowList (in special context)
                             ↓
              Select workflow → WorkflowEdit
                             ↓
              Launch → New Tab (running) → Back to Tabs view
                             ↓
              [process runs in background]
                             ↓
              Tab status updates (●→✓/✗)

Switch tabs: Tab/Shift+Tab (output preserved)
Close tab: Ctrl+W or C (with confirmation if running)
Rerun: R (creates new tab with same config)
Kill: K (terminates process)
```

---

## Implementation Phases

### Phase 1: Add Core Structures (Non-breaking)
**Goal:** Add new types without removing old ones

**Changes:**
1. Add `WorkflowTab` struct (lines ~86)
2. Add `open_tabs`, `active_tab_idx`, `workflow_counters` to `App`
3. Add `View::Tabs` variant
4. Keep all existing fields temporarily

**Testing:** Should compile, no behavior change yet

---

### Phase 2: Session Persistence (Non-breaking)
**Goal:** Add save/restore logic

**Changes:**
1. Create `SavedTab` struct
2. Add `save_session()` function
3. Add `restore_session()` function
4. Call `restore_session()` in `App::new()`
5. Call `save_session()` on quit

**Testing:** Creates/reads session.json, no UI change

---

### Phase 3: Tab Rendering (New UI)
**Goal:** Render tab bar and tab content

**Changes:**
1. Create `render_tab_bar()` function
2. Create `render_tab_content()` function
3. Create `render_empty_tabs()` function
4. Update `main()` render logic to show tabs
5. Update `render_workflow_running()` to use `WorkflowTab` state

**Testing:** Should see tab bar (empty initially)

---

### Phase 4: Tab Navigation (Minimal)
**Goal:** Basic tab switching

**Changes:**
1. Add `next_tab()` method
2. Add `previous_tab()` method
3. Handle `Tab` and `Shift+Tab` keys in `View::Tabs`
4. Add horizontal scroll logic

**Testing:** Can navigate between empty tabs (if any)

---

### Phase 5: Launch Workflow in Tab (Core Feature)
**Goal:** Create tabs from workflow launches

**Changes:**
1. Create `launch_workflow_in_tab()` method
2. Update `launch_workflow()` to use new method
3. Add counter-based naming logic
4. Switch view to `View::Tabs` after launch

**Testing:** Can launch workflow, creates tab, shows in tab bar

---

### Phase 6: Multi-Tab Polling (Critical)
**Goal:** Monitor all running workflows concurrently

**Changes:**
1. Create `poll_all_tabs()` method
2. Update main event loop to call `poll_all_tabs()`
3. Add `read_tab_output()` method
4. Update structured log parsing to use tab's state

**Testing:** Multiple workflows can run concurrently, output captured per-tab

---

### Phase 7: Tab Management Actions
**Goal:** Close, kill, rerun tabs

**Changes:**
1. Add `close_tab()` method with confirmation
2. Add `kill_tab()` method
3. Add `rerun_tab()` method
4. Handle `C`, `K`, `R` keys in `View::Tabs`
5. Handle `Ctrl+W` for close

**Testing:** Can close/kill/rerun tabs

---

### Phase 8: New Tab Workflow
**Goal:** Ctrl+T shows WorkflowList

**Changes:**
1. Handle `Ctrl+T` to switch to `View::WorkflowList`
2. Update `View::WorkflowList` rendering to show in tab context
3. Update `View::WorkflowEdit` to work in tab context
4. Return to `View::Tabs` after launch or cancel

**Testing:** Can open new tab, select workflow, configure, launch

---

### Phase 9: Migration & Cleanup (Breaking)
**Goal:** Remove old single-workflow fields

**Changes:**
1. Remove old fields from `App` struct
2. Update all methods to use tab state
3. Remove `View::WorkflowRunning` variant
4. Update all match statements

**Testing:** Full compile, all features work

---

### Phase 10: Session Restore Logic
**Goal:** Restore tabs on startup

**Changes:**
1. For completed/failed: Load saved logs, set status
2. For not-run: Create tab in NotStarted state
3. Show appropriate view based on status

**Testing:** Restart app, tabs restored correctly

---

## Detailed File Changes

### File: `src/main.rs`

#### Section 1: Add WorkflowTab (after line 86)
```rust
#[derive(Debug, Clone)]
struct WorkflowTab {
    id: String,
    workflow_idx: usize,
    workflow_name: String,
    instance_number: usize,
    start_time: Option<DateTime<Local>>,
    status: WorkflowStatus,
    child_process: Option<Child>,
    stdout_reader: Option<BufReader<ChildStdout>>,
    stderr_reader: Option<BufReader<ChildStderr>>,
    exit_code: Option<i32>,
    workflow_phases: Arc<Mutex<Vec<WorkflowPhase>>>,
    workflow_output: Arc<Mutex<Vec<String>>>,
    field_values: HashMap<String, String>,
    scroll_offset: usize,
    expanded_phases: HashSet<usize>,
    expanded_tasks: HashSet<String>,
    expanded_agents: HashSet<String>,
    selected_phase: usize,
    selected_task: Option<String>,
    selected_agent: Option<String>,
    saved_logs: Option<Vec<String>>,
}
```

#### Section 2: Update View enum (line 88)
```rust
#[derive(Debug, Clone, PartialEq)]
enum View {
    WorkflowList,
    WorkflowDetail(usize),
    WorkflowEdit(usize),
    Tabs,  // NEW
}
```

#### Section 3: Add to App struct (line 95)
```rust
struct App {
    workflows: Vec<Workflow>,

    // NEW: Tab management
    open_tabs: Vec<WorkflowTab>,
    active_tab_idx: usize,
    workflow_counters: HashMap<String, usize>,
    show_close_confirmation: bool,

    selected: usize,
    current_view: View,
    // ... rest unchanged for Phase 1
}
```

#### Section 4: Update App::new() (line 134)
```rust
fn new() -> Self {
    let workflows = load_workflows();
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    let history = load_history();

    let mut app = Self {
        workflows,
        open_tabs: Vec::new(),  // NEW
        active_tab_idx: 0,      // NEW
        workflow_counters: HashMap::new(),  // NEW
        show_close_confirmation: false,     // NEW
        // ... rest
    };

    app.restore_session();  // NEW

    // If tabs restored, start in Tabs view
    if !app.open_tabs.is_empty() {
        app.current_view = View::Tabs;
    }

    app
}
```

#### Section 5: Add session persistence (new methods)
```rust
impl App {
    fn save_session(&self) {
        #[derive(Serialize)]
        struct SavedTab {
            workflow_idx: usize,
            workflow_name: String,
            instance_number: usize,
            field_values: HashMap<String, String>,
            status: String,
            saved_logs: Vec<String>,
        }

        let saved_tabs: Vec<SavedTab> = self.open_tabs.iter()
            .map(|t| SavedTab {
                workflow_idx: t.workflow_idx,
                workflow_name: t.workflow_name.clone(),
                instance_number: t.instance_number,
                field_values: t.field_values.clone(),
                status: format!("{:?}", t.status),
                saved_logs: {
                    let mut logs = Vec::new();
                    if let Ok(output) = t.workflow_output.lock() {
                        logs = output.clone();
                    }
                    logs
                },
            })
            .collect();

        if let Some(data_dir) = directories::ProjectDirs::from("", "", "workflow-manager") {
            let session_path = data_dir.data_dir().join("session.json");
            if let Ok(json) = serde_json::to_string_pretty(&saved_tabs) {
                let _ = std::fs::write(session_path, json);
            }
        }
    }

    fn restore_session(&mut self) {
        #[derive(Deserialize)]
        struct SavedTab {
            workflow_idx: usize,
            workflow_name: String,
            instance_number: usize,
            field_values: HashMap<String, String>,
            status: String,
            saved_logs: Vec<String>,
        }

        if let Some(data_dir) = directories::ProjectDirs::from("", "", "workflow-manager") {
            let session_path = data_dir.data_dir().join("session.json");
            if let Ok(json) = std::fs::read_to_string(session_path) {
                if let Ok(saved_tabs) = serde_json::from_str::<Vec<SavedTab>>(&json) {
                    for saved in saved_tabs {
                        if saved.workflow_idx >= self.workflows.len() {
                            continue;
                        }

                        let status = match saved.status.as_str() {
                            "Completed" => WorkflowStatus::Completed,
                            "Failed" => WorkflowStatus::Failed,
                            _ => WorkflowStatus::NotStarted,
                        };

                        let tab = WorkflowTab {
                            id: format!("restored_{}", saved.instance_number),
                            workflow_idx: saved.workflow_idx,
                            workflow_name: saved.workflow_name,
                            instance_number: saved.instance_number,
                            start_time: None,
                            status,
                            child_process: None,
                            stdout_reader: None,
                            stderr_reader: None,
                            exit_code: None,
                            workflow_phases: Arc::new(Mutex::new(Vec::new())),
                            workflow_output: Arc::new(Mutex::new(saved.saved_logs)),
                            field_values: saved.field_values,
                            scroll_offset: 0,
                            expanded_phases: HashSet::new(),
                            expanded_tasks: HashSet::new(),
                            expanded_agents: HashSet::new(),
                            selected_phase: 0,
                            selected_task: None,
                            selected_agent: None,
                            saved_logs: None,
                        };

                        self.open_tabs.push(tab);

                        // Update counter
                        let workflow = &self.workflows[saved.workflow_idx];
                        let counter = self.workflow_counters
                            .entry(workflow.info.id.clone())
                            .or_insert(0);
                        if saved.instance_number >= *counter {
                            *counter = saved.instance_number + 1;
                        }
                    }
                }
            }
        }
    }
}
```

#### Section 6: Add tab rendering (new functions)
```rust
fn render_tab_bar(f: &mut Frame, area: Rect, app: &App) {
    // Calculate visible tabs
    let max_chars = area.width as usize - 10;
    let mut current_width = 0;
    let mut first_visible = 0;

    // Find scroll window to keep active tab visible
    for (i, tab) in app.open_tabs.iter().enumerate() {
        let tab_width = tab.workflow_name.len() + 5; // " #N ● "

        if i < app.active_tab_idx {
            if current_width + tab_width > max_chars / 2 {
                first_visible = i + 1;
                current_width = 0;
            } else {
                current_width += tab_width;
            }
        }
    }

    // Build tab titles
    let mut spans = Vec::new();

    for (i, tab) in app.open_tabs.iter().skip(first_visible).enumerate() {
        let real_idx = i + first_visible;
        let is_active = real_idx == app.active_tab_idx;

        // Truncate name
        let name = if tab.workflow_name.len() > 10 {
            format!("{}...", &tab.workflow_name[..7])
        } else {
            tab.workflow_name.clone()
        };

        // Status icon
        let icon = match tab.status {
            WorkflowStatus::Running => "●",
            WorkflowStatus::Completed => "✓",
            WorkflowStatus::Failed => "✗",
            WorkflowStatus::NotStarted => "○",
        };

        let title = format!("{} #{} {} ", name, tab.instance_number, icon);

        let style = if is_active {
            Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        spans.push(Span::styled(title, style));

        // Check if we've exceeded width
        let total_width: usize = spans.iter().map(|s| s.content.len()).sum();
        if total_width > max_chars {
            break;
        }
    }

    // Add [+ New] button
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        "[+ New]",
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    ));

    let tabs_line = Line::from(spans);
    let separator = Line::from("━".repeat(area.width as usize));

    let paragraph = Paragraph::new(vec![tabs_line, separator]);
    f.render_widget(paragraph, area);
}

fn render_empty_tabs(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(""),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "No workflows running",
            Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD)
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press [Ctrl+T] or click [+ New]",
            Style::default().fg(Color::Cyan)
        )),
        Line::from(Span::styled(
            "to start a new workflow",
            Style::default().fg(Color::Cyan)
        )),
    ];

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}
```

#### Section 7: Update main render loop (around line 2200+)
```rust
// In main() rendering section
match app.current_view {
    View::WorkflowList => {
        render_workflow_list(f, chunks[0], &app);
        render_footer(f, chunks[1], &app);
    }
    View::WorkflowDetail(idx) => {
        render_workflow_detail(f, chunks[0], &app, idx);
        render_footer(f, chunks[1], &app);
    }
    View::WorkflowEdit(idx) => {
        render_workflow_edit(f, chunks[0], &mut app, idx);
        render_footer(f, chunks[1], &app);
    }
    View::Tabs => {
        // Split screen: tab bar + content
        let tab_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),     // Tab bar
                Constraint::Min(0),        // Content
                Constraint::Length(3),     // Footer
            ])
            .split(chunks[0]);

        render_tab_bar(f, tab_chunks[0], &app);

        if app.open_tabs.is_empty() {
            render_empty_tabs(f, tab_chunks[1]);
        } else if let Some(tab) = app.open_tabs.get(app.active_tab_idx) {
            render_tab_content(f, tab_chunks[1], &app, tab);
        }

        render_footer(f, tab_chunks[2], &app);
    }
}
```

#### Section 8: Add tab navigation (new methods)
```rust
impl App {
    fn next_tab(&mut self) {
        if !self.open_tabs.is_empty() {
            self.active_tab_idx = (self.active_tab_idx + 1) % self.open_tabs.len();
        }
    }

    fn previous_tab(&mut self) {
        if !self.open_tabs.is_empty() {
            self.active_tab_idx = if self.active_tab_idx == 0 {
                self.open_tabs.len() - 1
            } else {
                self.active_tab_idx - 1
            };
        }
    }

    fn close_current_tab(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }

        let tab = &self.open_tabs[self.active_tab_idx];

        // If running, show confirmation
        if tab.status == WorkflowStatus::Running {
            self.show_close_confirmation = true;
            return;
        }

        // Close tab
        self.close_tab_confirmed();
    }

    fn close_tab_confirmed(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }

        // Kill process if running
        if let Some(tab) = self.open_tabs.get_mut(self.active_tab_idx) {
            if let Some(mut child) = tab.child_process.take() {
                let _ = child.kill();
            }
        }

        // Remove tab
        self.open_tabs.remove(self.active_tab_idx);

        // Adjust active index
        if self.open_tabs.is_empty() {
            // No tabs left, stay in Tabs view showing empty state
            self.active_tab_idx = 0;
        } else if self.active_tab_idx >= self.open_tabs.len() {
            self.active_tab_idx = self.open_tabs.len() - 1;
        }

        self.show_close_confirmation = false;
    }
}
```

#### Section 9: Update key handling (around line 250+)
```rust
// In handle_key() or event loop
match app.current_view {
    View::Tabs => {
        if app.show_close_confirmation {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    app.close_tab_confirmed();
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    app.show_close_confirmation = false;
                }
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    app.previous_tab();
                }
                KeyCode::Tab => {
                    app.next_tab();
                }
                KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.current_view = View::WorkflowList;
                }
                KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.close_current_tab();
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    app.close_current_tab();
                }
                KeyCode::Char('k') | KeyCode::Char('K') => {
                    if let Some(tab) = app.open_tabs.get_mut(app.active_tab_idx) {
                        if let Some(mut child) = tab.child_process.take() {
                            let _ = child.kill();
                            tab.status = WorkflowStatus::Failed;
                        }
                    }
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    app.rerun_current_tab();
                }
                KeyCode::Char('q') => {
                    app.should_quit = true;
                }
                // Forward other keys to tab content
                _ => {
                    if let Some(tab) = app.open_tabs.get_mut(app.active_tab_idx) {
                        // Handle navigation within tab (arrow keys, etc.)
                        handle_tab_content_key(app, tab, key);
                    }
                }
            }
        }
    }
    // ... other views
}
```

#### Section 10: Update launch_workflow (around line 400+)
```rust
fn launch_workflow_in_tab(&mut self, workflow_idx: usize) {
    let workflow = &self.workflows[workflow_idx];

    // Get next instance number
    let instance_number = {
        let counter = self.workflow_counters
            .entry(workflow.info.id.clone())
            .or_insert(0);
        *counter += 1;
        *counter
    };

    // Create tab
    let tab_id = format!("{}_{}", workflow.info.id, chrono::Local::now().format("%Y%m%d_%H%M%S"));

    let mut tab = WorkflowTab {
        id: tab_id,
        workflow_idx,
        workflow_name: workflow.info.name.clone(),
        instance_number,
        start_time: Some(chrono::Local::now()),
        status: WorkflowStatus::Running,
        child_process: None,
        stdout_reader: None,
        stderr_reader: None,
        exit_code: None,
        workflow_phases: Arc::new(Mutex::new(Vec::new())),
        workflow_output: Arc::new(Mutex::new(Vec::new())),
        field_values: self.field_values.clone(),
        scroll_offset: 0,
        expanded_phases: HashSet::new(),
        expanded_tasks: HashSet::new(),
        expanded_agents: HashSet::new(),
        selected_phase: 0,
        selected_task: None,
        selected_agent: None,
        saved_logs: None,
    };

    // Build command (use existing build_workflow_command logic)
    let mut command = Command::new(/* ... */);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    // Spawn process
    match command.spawn() {
        Ok(mut child) => {
            tab.stdout_reader = child.stdout.take().map(BufReader::new);
            tab.stderr_reader = child.stderr.take().map(BufReader::new);
            tab.child_process = Some(child);

            // Add tab and switch to it
            self.open_tabs.push(tab);
            self.active_tab_idx = self.open_tabs.len() - 1;
            self.current_view = View::Tabs;
        }
        Err(e) => {
            eprintln!("Failed to launch workflow: {}", e);
        }
    }
}
```

#### Section 11: Add poll_all_tabs (new method)
```rust
impl App {
    fn poll_all_tabs(&mut self) {
        for tab in &mut self.open_tabs {
            if tab.status != WorkflowStatus::Running {
                continue;
            }

            // Check process status
            if let Some(child) = &mut tab.child_process {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        tab.status = if status.success() {
                            WorkflowStatus::Completed
                        } else {
                            WorkflowStatus::Failed
                        };
                        tab.exit_code = status.code();

                        // Save to history on success
                        if tab.status == WorkflowStatus::Completed {
                            self.save_field_values_to_history(tab.workflow_idx, &tab.field_values);
                        }
                    }
                    Ok(None) => {
                        // Still running - read output
                        self.read_tab_output(tab);
                    }
                    Err(_) => {
                        tab.status = WorkflowStatus::Failed;
                    }
                }
            }
        }
    }

    fn read_tab_output(&self, tab: &mut WorkflowTab) {
        use std::io::BufRead;

        // Read stdout
        if let Some(reader) = &mut tab.stdout_reader {
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                if let Ok(mut output) = tab.workflow_output.lock() {
                    output.push(line.clone());
                }
                line.clear();
            }
        }

        // Read stderr (structured logs)
        if let Some(reader) = &mut tab.stderr_reader {
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                if line.contains("__WF_EVENT__:") {
                    // Parse structured log
                    if let Some(json_start) = line.find("__WF_EVENT__:") {
                        let json_str = &line[json_start + 13..];
                        if let Ok(event) = serde_json::from_str::<WorkflowLog>(json_str) {
                            self.handle_workflow_event(event, &tab.workflow_phases);
                        }
                    }
                } else {
                    if let Ok(mut output) = tab.workflow_output.lock() {
                        output.push(line.clone());
                    }
                }
                line.clear();
            }
        }
    }
}
```

#### Section 12: Update main event loop (in main())
```rust
// In main() event loop
loop {
    // Poll all tabs (fast, 50-100ms)
    if app.current_view == View::Tabs {
        app.poll_all_tabs();
    }

    // Render
    terminal.draw(|f| {
        // ... existing render logic
    })?;

    // Handle events with timeout
    if event::poll(Duration::from_millis(50))? {
        if let Event::Key(key) = event::read()? {
            // ... existing key handling
        }
    }

    if app.should_quit {
        app.save_session();
        break;
    }
}
```

---

## Testing Strategy

### Unit Tests
1. `WorkflowTab` creation and state transitions
2. Counter increment logic
3. Tab navigation (next/prev with wrapping)
4. Session save/restore

### Integration Tests
1. Launch workflow → creates tab → runs to completion
2. Switch tabs while running → state preserved
3. Close running tab with confirmation
4. Rerun tab creates new instance with incremented counter
5. Kill tab terminates process
6. Restore session on startup

### Manual Testing Checklist
- [ ] Launch single workflow
- [ ] Launch multiple workflows concurrently
- [ ] Switch between tabs with Tab/Shift+Tab
- [ ] Close completed tab (no confirmation)
- [ ] Close running tab (shows confirmation)
- [ ] Kill running workflow with K
- [ ] Rerun workflow with R (creates new tab)
- [ ] Open new tab with Ctrl+T
- [ ] Quit and restart (session restored)
- [ ] Empty tabs state shows hint

---

## Rollback Plan

### If Implementation Fails
1. **Git branch**: Create `feature/tabs` before starting
2. **Backup**: Copy current `main.rs` to `main.rs.backup`
3. **Rollback**: `git checkout main -- src/main.rs`

### Incremental Rollback
Each phase is independent. If Phase N fails:
1. Revert changes from Phase N
2. Keep Phases 1 through N-1
3. Continue with working partial implementation

---

## Risk Mitigation

### High-Risk Areas
1. **Process management**: Multiple Child processes, potential leaks
   - Mitigation: Ensure `kill()` on drop, test with many tabs

2. **Thread safety**: Arc<Mutex<>> on per-tab state
   - Mitigation: Minimize lock scope, avoid deadlocks

3. **Session restore**: Corrupted JSON, invalid workflow indices
   - Mitigation: Validate all data, skip invalid tabs

4. **Memory usage**: Storing all output in memory
   - Mitigation: Consider output size limits, log rotation

### Medium-Risk Areas
1. **UI responsiveness**: 50ms polling may cause lag
   - Mitigation: Profile, adjust timeout if needed

2. **Tab bar overflow**: Many tabs exceed screen width
   - Mitigation: Horizontal scrolling implemented

---

## Success Criteria

### Minimum Viable Product
- ✅ Can launch workflows in tabs
- ✅ Can switch between tabs
- ✅ Multiple workflows run concurrently
- ✅ Can close tabs
- ✅ Session persists on restart

### Full Feature Set
- ✅ All MVP features
- ✅ Confirmation on close running tab
- ✅ Kill running workflow with K
- ✅ Rerun workflow with R
- ✅ Counter-based naming
- ✅ Horizontal scroll for many tabs
- ✅ Empty tabs hint
- ✅ Proper status icons (●/✓/✗)

---

## Estimated Timeline

| Phase | Task | Lines Changed | Time Estimate |
|-------|------|---------------|---------------|
| 1 | Core structures | ~50 | 30 min |
| 2 | Session persistence | ~100 | 1 hour |
| 3 | Tab rendering | ~150 | 1.5 hours |
| 4 | Tab navigation | ~50 | 30 min |
| 5 | Launch in tab | ~80 | 1 hour |
| 6 | Multi-tab polling | ~100 | 1.5 hours |
| 7 | Tab actions | ~80 | 1 hour |
| 8 | New tab workflow | ~60 | 45 min |
| 9 | Migration | ~100 | 1 hour |
| 10 | Restore logic | ~60 | 45 min |
| **TOTAL** | | **~830 lines** | **9.5 hours** |

---

## Next Steps

1. **Review this plan** - Identify any concerns or changes needed
2. **Approve to proceed** - Give go-ahead to start implementation
3. **Execute phases 1-10** - Implement incrementally
4. **Test each phase** - Verify before moving to next
5. **Final review** - Full testing of complete feature

---

## Questions for Review

1. Does the architecture make sense for your use case?
2. Are there any features missing from the plan?
3. Should we adjust the polling frequency (50ms)?
4. Any concerns about memory usage with unlimited tabs?
5. Should we add tab limits or resource constraints?

---

**Ready to proceed? Please review and approve!**
