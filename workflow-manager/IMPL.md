# Implementation Plan: Complex Workflow Support

## Overview

Transform the workflow manager from supporting simple single-execution workflows to supporting complex, multi-phase, stateful workflows with hierarchical logging and real-time progress tracking.

## Goals

1. Support multi-phase workflows (like research_agent, tasks_agent)
2. Enable resumability from any phase using saved state files
3. Provide hierarchical log viewing (phases → tasks → agents)
4. Display real-time progress with expand/collapse functionality
5. Track concurrent execution with batch processing

## Architecture

### Core Concepts

**Phase-based Execution:**
- Workflows have multiple phases (0, 1, 2, 3, 4)
- Each phase can be run independently
- Phases communicate via intermediate YAML files
- User can resume from any phase using `--phases` flag

**Structured Logging:**
- Workflows emit structured events to stderr
- Format: `__WF_EVENT__:{"type":"phase_started","phase":2}`
- TUI parses these events for hierarchical display
- Human-readable logs go to stdout

**Hierarchical Progress:**
- Phase level: "Phase 2: Execute Research (3/5 complete)"
- Task level: "Task 3: API Documentation (in progress)"
- Agent level: "@files agent: Completed (5 files identified)"

**State Management:**
- Each phase outputs files (e.g., `codebase_analysis_20250113_143045.yaml`)
- TUI tracks these state files
- Can pass state files to resume from specific phases

## Implementation Steps

### Phase 1: Structured Logging Infrastructure

**1.1 Define WorkflowLog enum in workflow-manager-sdk**

```rust
// workflow-manager-sdk/src/lib.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowLog {
    PhaseStarted {
        phase: usize,
        name: String,
        total_phases: usize,
    },
    PhaseProgress {
        phase: usize,
        message: String,
        progress: Option<f32>, // 0.0 to 1.0
    },
    PhaseCompleted {
        phase: usize,
        output_file: Option<String>,
    },

    TaskStarted {
        phase: usize,
        task_id: String,
        description: String,
        total_tasks: Option<usize>,
    },
    TaskProgress {
        phase: usize,
        task_id: String,
        message: String,
        progress: Option<f32>,
    },
    TaskCompleted {
        phase: usize,
        task_id: String,
        duration_secs: Option<u64>,
    },

    AgentStarted {
        task_id: String,
        agent_name: String,
        description: String,
    },
    AgentProgress {
        task_id: String,
        agent_name: String,
        message: String,
    },
    AgentCompleted {
        task_id: String,
        agent_name: String,
        output: Option<String>,
    },

    Info { message: String },
    Warning { message: String },
    Error { message: String },
}

impl WorkflowLog {
    pub fn emit(&self) {
        eprintln!("__WF_EVENT__:{}", serde_json::to_string(self).unwrap());
    }
}
```

**1.2 Add helper macros for easy logging**

```rust
#[macro_export]
macro_rules! log_phase_started {
    ($phase:expr, $name:expr, $total:expr) => {
        WorkflowLog::PhaseStarted {
            phase: $phase,
            name: $name.to_string(),
            total_phases: $total,
        }.emit();
    };
}

// Similar macros for other event types
```

### Phase 2: Update Workflow Binaries

**2.1 Refactor research_agent example**

Add structured logging throughout:

```rust
// Phase 0
log_phase_started!(0, "Analyze Codebase", 5);
println!("PHASE 0: Analyzing Codebase Structure");
// ... existing code ...
log_phase_completed!(0, Some("codebase_analysis.yaml"));

// Phase 2 with concurrent tasks
log_phase_started!(2, "Execute Research", 5);
for (i, prompt) in prompts.iter().enumerate() {
    let task_id = format!("research_{}", i + 1);
    log_task_started!(2, &task_id, &prompt.title, Some(prompts.len()));

    // Execute research
    // ...

    log_task_completed!(2, &task_id, Some(duration));
}
log_phase_completed!(2, Some("research_results.yaml"));
```

**2.2 Refactor tasks_agent example**

Add agent-level logging:

```rust
// When invoking sub-agents
log_agent_started!(&task_id, "files", "Identify files to modify");

// During agent execution
log_agent_progress!(&task_id, "files", "Found 5 files...");

// After agent completes
log_agent_completed!(&task_id, "files", Some("5 files identified"));
```

**2.3 Update simple_query workflow**

Even simple workflows emit basic events:

```rust
log_phase_started!(0, "Query Claude", 1);
log_phase_progress!(0, "Sending query...", None);
// ... query execution ...
log_phase_completed!(0, None);
```

### Phase 3: TUI Hierarchical Log Viewer

**3.1 Add log parsing to main.rs**

```rust
#[derive(Debug, Clone)]
struct WorkflowPhase {
    id: usize,
    name: String,
    status: PhaseStatus,
    tasks: Vec<WorkflowTask>,
    output_file: Option<String>,
}

#[derive(Debug, Clone)]
struct WorkflowTask {
    id: String,
    description: String,
    status: TaskStatus,
    agents: Vec<WorkflowAgent>,
    duration: Option<Duration>,
}

#[derive(Debug, Clone)]
struct WorkflowAgent {
    name: String,
    status: AgentStatus,
    output: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum PhaseStatus {
    NotStarted,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
enum AgentStatus {
    Pending,
    Running,
    Completed,
}
```

**3.2 Update launch_workflow to parse structured logs**

```rust
fn launch_workflow(&mut self) {
    // ... existing setup ...

    // Clear and initialize phase structure
    if let Ok(mut output) = self.workflow_output.lock() {
        output.clear();
    }
    self.workflow_phases = Arc::new(Mutex::new(Vec::new()));

    // Spawn threads to read stdout AND stderr
    if let Some(stdout) = child.stdout.take() {
        let output = Arc::clone(&self.workflow_output);
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if let Ok(mut output) = output.lock() {
                        output.push(line);
                    }
                }
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        let output = Arc::clone(&self.workflow_output);
        let phases = Arc::clone(&self.workflow_phases);
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    // Check for structured event
                    if line.starts_with("__WF_EVENT__:") {
                        if let Some(json) = line.strip_prefix("__WF_EVENT__:") {
                            if let Ok(event) = serde_json::from_str::<WorkflowLog>(json) {
                                // Update phase structure
                                if let Ok(mut phases) = phases.lock() {
                                    update_phases(&mut phases, event);
                                }
                            }
                        }
                    }

                    // Also store raw stderr
                    if let Ok(mut output) = output.lock() {
                        output.push(format!("ERROR: {}", line));
                    }
                }
            }
        });
    }
}

fn update_phases(phases: &mut Vec<WorkflowPhase>, event: WorkflowLog) {
    match event {
        WorkflowLog::PhaseStarted { phase, name, .. } => {
            // Ensure phase exists
            while phases.len() <= phase {
                phases.push(WorkflowPhase {
                    id: phases.len(),
                    name: format!("Phase {}", phases.len()),
                    status: PhaseStatus::NotStarted,
                    tasks: Vec::new(),
                    output_file: None,
                });
            }
            phases[phase].name = name;
            phases[phase].status = PhaseStatus::InProgress;
        }
        WorkflowLog::TaskStarted { phase, task_id, description, .. } => {
            if let Some(p) = phases.get_mut(phase) {
                p.tasks.push(WorkflowTask {
                    id: task_id,
                    description,
                    status: TaskStatus::Running,
                    agents: Vec::new(),
                    duration: None,
                });
            }
        }
        WorkflowLog::AgentStarted { task_id, agent_name, .. } => {
            // Find task and add agent
            for phase in phases.iter_mut() {
                if let Some(task) = phase.tasks.iter_mut().find(|t| t.id == task_id) {
                    task.agents.push(WorkflowAgent {
                        name: agent_name,
                        status: AgentStatus::Running,
                        output: None,
                    });
                }
            }
        }
        // Handle other events...
        _ => {}
    }
}
```

**3.3 Create hierarchical rendering**

```rust
fn render_workflow_running(f: &mut Frame, area: Rect, app: &App, idx: usize) {
    let workflow = &app.workflows[idx];

    // Get phases structure
    let phases = if let Ok(phases) = app.workflow_phases.lock() {
        phases.clone()
    } else {
        Vec::new()
    };

    // Build hierarchical list
    let mut items = Vec::new();

    for phase in &phases {
        // Phase line
        let status_icon = match phase.status {
            PhaseStatus::NotStarted => "○",
            PhaseStatus::InProgress => "▶",
            PhaseStatus::Completed => "✓",
            PhaseStatus::Failed => "✗",
        };
        let phase_line = format!("{} Phase {}: {}", status_icon, phase.id, phase.name);
        items.push(ListItem::new(phase_line));

        // If expanded, show tasks
        if app.expanded_phases.contains(&phase.id) {
            for task in &phase.tasks {
                let task_icon = match task.status {
                    TaskStatus::Pending => "[ ]",
                    TaskStatus::Running => "[⚙]",
                    TaskStatus::Completed => "[✓]",
                    TaskStatus::Failed => "[✗]",
                };
                let task_line = format!("  {} {}", task_icon, task.description);
                items.push(ListItem::new(task_line));

                // If task expanded, show agents
                if app.expanded_tasks.contains(&task.id) {
                    for agent in &task.agents {
                        let agent_icon = match agent.status {
                            AgentStatus::Pending => "○",
                            AgentStatus::Running => "⚙",
                            AgentStatus::Completed => "✓",
                        };
                        let agent_line = format!("    {} @{}", agent_icon, agent.name);
                        items.push(ListItem::new(agent_line));
                    }
                }
            }
        }
    }

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Workflow Progress "));

    f.render_widget(list, area);
}
```

**3.4 Add expand/collapse keyboard handling**

```rust
// In keyboard handler for WorkflowRunning view
KeyCode::Char(' ') | KeyCode::Enter => {
    // Toggle expansion of selected item
    app.toggle_expansion();
}
KeyCode::Char('e') => {
    // Expand all
    app.expand_all();
}
KeyCode::Char('c') => {
    // Collapse all
    app.collapse_all();
}
```

**3.5 Add App state for expansions**

```rust
struct App {
    // ... existing fields ...
    workflow_phases: Arc<Mutex<Vec<WorkflowPhase>>>,
    expanded_phases: HashSet<usize>,
    expanded_tasks: HashSet<String>,
    selected_item: Option<SelectedItem>,
}

#[derive(Debug, Clone)]
enum SelectedItem {
    Phase(usize),
    Task(usize, String),  // phase_id, task_id
    Agent(String, String), // task_id, agent_name
}

impl App {
    fn toggle_expansion(&mut self) {
        match &self.selected_item {
            Some(SelectedItem::Phase(phase_id)) => {
                if self.expanded_phases.contains(phase_id) {
                    self.expanded_phases.remove(phase_id);
                } else {
                    self.expanded_phases.insert(*phase_id);
                }
            }
            Some(SelectedItem::Task(_, task_id)) => {
                if self.expanded_tasks.contains(task_id) {
                    self.expanded_tasks.remove(task_id);
                } else {
                    self.expanded_tasks.insert(task_id.clone());
                }
            }
            _ => {}
        }
    }
}
```

### Phase 4: State File Management

**4.1 Add state file tracking to WorkflowInfo**

```rust
// In workflow-manager-sdk
pub struct WorkflowInfo {
    // ... existing fields ...
    pub state_files: HashMap<String, Option<PathBuf>>,
}
```

**4.2 Detect state files from PhaseCompleted events**

```rust
WorkflowLog::PhaseCompleted { phase, output_file } => {
    if let Some(p) = phases.get_mut(phase) {
        p.status = PhaseStatus::Completed;
        p.output_file = output_file.clone();

        // Store in workflow state_files
        if let Some(file) = output_file {
            // Extract state file name (e.g., "analysis_file")
            let state_name = extract_state_name(&file);
            app.workflows[idx].info.state_files.insert(
                state_name,
                Some(PathBuf::from(file))
            );
        }
    }
}
```

**4.3 Show state files in WorkflowDetail view**

```rust
// Add section to workflow detail
if !workflow.info.state_files.is_empty() {
    items.push(Line::from(""));
    items.push(Line::from(Span::styled(
        "State Files:",
        Style::default().add_modifier(Modifier::BOLD)
    )));
    for (name, path) in &workflow.info.state_files {
        if let Some(p) = path {
            items.push(Line::from(format!("  {}: {}", name, p.display())));
        }
    }
}
```

### Phase 5: Enhanced Field Types

**5.1 Add PhaseSelector field type**

```rust
// In workflow-manager-sdk
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Number { min: Option<f64>, max: Option<f64> },
    FilePath { default_path: Option<String> },
    Select { options: Vec<String> },
    PhaseSelector { total_phases: usize }, // NEW: "0,1,2" or "2,3,4"
}
```

**5.2 Add StateFile field type**

```rust
pub enum FieldType {
    // ... existing ...
    StateFile {
        pattern: String,  // "codebase_analysis_*.yaml"
        phase: usize,     // Which phase generates this file
    },
}
```

**5.3 Update WorkflowEdit to handle new field types**

```rust
// PhaseSelector: Show checkboxes for phases
FieldType::PhaseSelector { total_phases } => {
    // Render as: [x] Phase 0  [ ] Phase 1  [x] Phase 2
    // User can toggle with space
}

// StateFile: Show file picker filtered by pattern
FieldType::StateFile { pattern, .. } => {
    // When Tab pressed, show files matching pattern
    // Fuzzy search through matching files
}
```

## Example: Updated research_agent workflow

```rust
#[derive(Parser, Debug, Clone, WorkflowDefinition)]
#[workflow(
    id = "research_agent",
    name = "Research Agent Workflow",
    description = "Multi-phase research workflow with codebase analysis"
)]
struct Args {
    /// Research objective
    #[arg(short, long)]
    #[field(
        label = "Objective",
        description = "[TEXT] Research question or objective",
        type = "text"
    )]
    input: Option<String>,

    /// Phases to execute
    #[arg(long, default_value = "0,1,2,3,4")]
    #[field(
        label = "Phases",
        description = "[PHASES] Select phases to run (0-4)",
        type = "phase_selector",
        total_phases = "5"
    )]
    phases: String,

    /// Batch size for concurrent execution
    #[arg(long, default_value = "1")]
    #[field(
        label = "Batch Size",
        description = "[NUMBER] Concurrent execution batch size",
        type = "number",
        min = "1",
        max = "10"
    )]
    batch_size: usize,

    /// Resume from saved analysis
    #[arg(long)]
    #[field(
        label = "Analysis File",
        description = "[STATE FILE] Resume with existing analysis",
        type = "state_file",
        pattern = "codebase_analysis_*.yaml",
        phase = "0"
    )]
    analysis_file: Option<String>,

    /// Resume from saved prompts
    #[arg(long)]
    #[field(
        label = "Prompts File",
        description = "[STATE FILE] Resume with existing prompts",
        type = "state_file",
        pattern = "research_prompts_*.yaml",
        phase = "1"
    )]
    prompts_file: Option<String>,

    // ... other fields ...

    #[arg(long, hide = true)]
    workflow_metadata: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.workflow_metadata {
        args.print_metadata();
        return Ok(());
    }

    use workflow_manager_sdk::WorkflowLog;

    // Parse phases
    let phases: Vec<usize> = args.phases
        .split(',')
        .filter_map(|p| p.trim().parse().ok())
        .collect();

    let mut codebase_analysis = None;
    let mut prompts_data = None;

    // Phase 0
    if phases.contains(&0) {
        WorkflowLog::PhaseStarted {
            phase: 0,
            name: "Analyze Codebase".to_string(),
            total_phases: 5,
        }.emit();

        println!("PHASE 0: Analyzing Codebase");
        // ... existing analysis code ...

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let analysis_path = format!("codebase_analysis_{}.yaml", timestamp);
        fs::write(&analysis_path, &analysis_yaml).await?;

        WorkflowLog::PhaseCompleted {
            phase: 0,
            output_file: Some(analysis_path),
        }.emit();

        codebase_analysis = Some(analysis);
    } else if let Some(file) = &args.analysis_file {
        // Load from file
        let content = fs::read_to_string(file).await?;
        codebase_analysis = Some(serde_yaml::from_str(&content)?);
    }

    // Phase 1
    if phases.contains(&1) {
        WorkflowLog::PhaseStarted {
            phase: 1,
            name: "Generate Prompts".to_string(),
            total_phases: 5,
        }.emit();

        println!("PHASE 1: Generating Research Prompts");
        // ... existing prompt generation ...

        WorkflowLog::PhaseCompleted {
            phase: 1,
            output_file: Some(prompts_path.to_string()),
        }.emit();

        prompts_data = Some(prompts);
    }

    // Phase 2 - with task tracking
    if phases.contains(&2) {
        WorkflowLog::PhaseStarted {
            phase: 2,
            name: "Execute Research".to_string(),
            total_phases: 5,
        }.emit();

        let prompts = prompts_data.as_ref().unwrap();

        for (i, prompt) in prompts.prompts.iter().enumerate() {
            let task_id = format!("research_{}", i + 1);

            WorkflowLog::TaskStarted {
                phase: 2,
                task_id: task_id.clone(),
                description: prompt.title.clone(),
                total_tasks: Some(prompts.prompts.len()),
            }.emit();

            // Execute research
            let result = execute_research_prompt(prompt, i + 1, &timestamp, None).await?;

            WorkflowLog::TaskCompleted {
                phase: 2,
                task_id,
                duration_secs: Some(60), // calculate actual duration
            }.emit();
        }

        WorkflowLog::PhaseCompleted {
            phase: 2,
            output_file: Some(results_path.to_string()),
        }.emit();
    }

    // ... Phases 3, 4 ...

    Ok(())
}
```

## Testing Plan

1. **Unit tests:** Test log parsing, phase tracking
2. **Integration tests:** Run research_agent with different phase combinations
3. **Manual testing:**
   - Run full workflow, verify all logs captured
   - Expand/collapse phases, tasks, agents
   - Resume from phase 2, verify state loaded correctly
   - Test with concurrent tasks (batch_size > 1)

## Future Enhancements (Not in this plan)

- Pause/Resume functionality
- Log persistence to disk/database
- Export logs to JSON/text
- Filter logs by level/phase/task/agent
- Search functionality in logs
- Real-time cost tracking
- Workflow templates/presets
- Workflow chaining (output of one → input of another)
