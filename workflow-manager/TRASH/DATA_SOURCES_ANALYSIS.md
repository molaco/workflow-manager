# Workflow Manager: Data Source Architecture Analysis

## Executive Summary

The Workflow Manager codebase implements a **process-based workflow execution model** with a **multi-layer data storage architecture**. Rather than fetching from databases, APIs, or WebSockets, the system uses a hybrid approach combining:

1. **Local file system storage** for persistent data (history, session state)
2. **Inter-process communication (IPC)** via stderr for workflow events
3. **In-memory data structures** with Arc/Mutex for concurrent access
4. **Built-in workflow binary discovery** from local filesystem
5. **MCP Server integration** for external tool access via Claude SDK

---

## 1. LOCAL STORAGE & PERSISTENCE LAYER

### File Locations

**History File** (`/home/molaco/Documents/japanese/workflow-manager/src/utils.rs`):
```rust
pub fn history_file_path() -> PathBuf {
    use directories::ProjectDirs;
    
    if let Some(proj_dirs) = ProjectDirs::from("com", "workflow-manager", "workflow-manager") {
        proj_dirs.data_dir().join("history.json")
    } else {
        PathBuf::from(".workflow-manager-history.json")
    }
}

pub fn load_history() -> WorkflowHistory {
    let path = history_file_path();
    if let Ok(content) = std::fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        WorkflowHistory::default()
    }
}

pub fn save_history(history: &WorkflowHistory) -> Result<()> {
    let path = history_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(history)?;
    std::fs::write(path, content)?;
    Ok(())
}
```

**Session File** (from `src/app/history.rs`):
```rust
if let Some(data_dir) = directories::ProjectDirs::from("", "", "workflow-manager") {
    let session_path = data_dir.data_dir().join("session.json");
    // Saves tab state, field values, and logs
}
```

### Data Structures

**WorkflowHistory** (`src/app/models/history.rs`):
```rust
pub struct WorkflowHistory {
    pub workflows: HashMap<String, HashMap<String, Vec<String>>>,
    // Structure: workflow_id -> field_name -> list of values (up to 10 recent)
}
```

**WorkflowTab** (`src/app/models/tab.rs`):
```rust
pub struct WorkflowTab {
    pub id: String,
    pub workflow_idx: usize,
    pub field_values: HashMap<String, String>,
    pub workflow_output: Arc<Mutex<Vec<String>>>,      // In-memory
    pub workflow_phases: Arc<Mutex<Vec<WorkflowPhase>>>, // In-memory
    pub saved_logs: Option<Vec<String>>,
    // ... UI state for tabs
}
```

### Data Flow (Persistence)

1. **On Startup** → Load workflows → Load history → Restore session
2. **During Execution** → Keep data in Arc<Mutex<Vec>> (thread-safe)
3. **On Completion** → Save to history file
4. **On Exit** → Save session state

Code Location: `/home/molaco/Documents/japanese/workflow-manager/src/app/history.rs:save_session()` & `restore_session()`

---

## 2. WORKFLOW DISCOVERY & LOADING

### Source Discovery Strategy

**Search Paths** (`src/discovery.rs:get_search_paths()`):
```rust
fn get_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    
    // 1. Built-in: Same directory as workflow-manager binary
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            paths.push(exe_dir.to_path_buf());
        }
    }
    
    // 2. User workflows: ~/.workflow-manager/workflows/
    if let Ok(home) = std::env::var("HOME") {
        paths.push(PathBuf::from(home).join(".workflow-manager/workflows"));
    }
    
    paths
}
```

### Conditional Logic for Data Source Selection

The system **selects workflows** based on:

1. **Filesystem Search**: Scan directories for executable binaries
2. **Metadata Extraction**: Run each binary with `--workflow-metadata` flag
3. **Filtering Logic** (`src/discovery.rs`):
```rust
// Skip the TUI binary itself
if filename == "workflow-manager" {
    continue;
}

// Skip build artifacts (hashes)
if filename.contains('-') {
    if let Some(after_dash) = filename.split('-').next_back() {
        if after_dash.len() > 10 && after_dash.chars().all(|c| c.is_ascii_hexdigit()) {
            continue; // Skip hash suffix
        }
    }
}

// Check if executable
if !is_executable(&path) {
    continue;
}

// Extract metadata
if let Ok(workflow) = extract_workflow_metadata(&path) {
    workflows.push(workflow);
}
```

### Metadata Extraction Process

**Dynamic Loading** (`src/discovery.rs:extract_workflow_metadata()`):
```rust
fn extract_workflow_metadata(binary_path: &Path) -> Result<DiscoveredWorkflow> {
    let output = Command::new(binary_path)
        .arg("--workflow-metadata")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();
    
    let json = String::from_utf8(output.stdout)?;
    let full_metadata: FullWorkflowMetadata = serde_json::from_str(&json)?;
    
    Ok(DiscoveredWorkflow {
        metadata: full_metadata.metadata,
        fields: full_metadata.fields,
        binary_path: binary_path.to_path_buf(),
    })
}
```

**File Path**: `/home/molaco/Documents/japanese/workflow-manager/src/discovery.rs`

---

## 3. DATA SOURCE TYPES & ENUMS

### WorkflowSource Enum (Data Origin)

**Location**: `/home/molaco/Documents/japanese/workflow-manager-sdk/src/lib.rs`

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowSource {
    BuiltIn,      // Internal workflows from src/bin/
    UserDefined,  // Custom workflows from ~/.workflow-manager/workflows/
}

pub struct Workflow {
    pub info: WorkflowInfo,
    pub source: WorkflowSource,
}
```

### Field Type Enum (Configuration)

**Location**: `workflow-manager-sdk/src/lib.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Number { min: Option<i64>, max: Option<i64> },
    FilePath { pattern: Option<String> },
    Select { options: Vec<String> },
    PhaseSelector { total_phases: usize },
    StateFile { pattern: String, phase: Option<usize> },
}
```

---

## 4. INTER-PROCESS COMMUNICATION (IPC) FOR DATA

### Structured Event Logging

**WorkflowLog Enum** (`workflow-manager-sdk/src/lib.rs`):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowLog {
    PhaseStarted { phase: usize, name: String, total_phases: usize },
    PhaseCompleted { phase: usize, name: String },
    PhaseFailed { phase: usize, name: String, error: String },
    TaskStarted { phase: usize, task_id: String, description: String, total_tasks: Option<usize> },
    TaskProgress { task_id: String, message: String },
    TaskCompleted { task_id: String, result: Option<String> },
    TaskFailed { task_id: String, error: String },
    AgentStarted { task_id: String, agent_name: String, description: String },
    AgentMessage { task_id: String, agent_name: String, message: String },
    AgentCompleted { task_id: String, agent_name: String, result: Option<String> },
    AgentFailed { task_id: String, agent_name: String, error: String },
    StateFileCreated { phase: usize, file_path: String, description: String },
}

impl WorkflowLog {
    pub fn emit(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            eprintln!("__WF_EVENT__:{}", json);
            let _ = std::io::stderr().flush();
        }
    }
}
```

### Event Processing

**Real-time Parsing** (`src/app/workflow_ops.rs` ~line 180-204):
```rust
// Spawn thread to read stderr and parse structured logs
if let Some(stderr) = child.stderr.take() {
    let output = Arc::clone(&output_clone);
    let phases = Arc::clone(&self.workflow_phases);
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                if let Some(json_str) = line.strip_prefix("__WF_EVENT__:") {
                    if let Ok(event) = serde_json::from_str::<WorkflowLog>(json_str) {
                        Self::handle_workflow_event(event, &phases);
                    }
                }
            }
        }
    });
}
```

**File Paths**: 
- Event definition: `/home/molaco/Documents/japanese/workflow-manager-sdk/src/lib.rs`
- Event processing: `/home/molaco/Documents/japanese/workflow-manager/src/app/workflow_ops.rs`

---

## 5. RUNTIME & WORKFLOW EXECUTION

### WorkflowRuntime Trait (Abstraction)

**Location**: `workflow-manager-sdk/src/lib.rs` (lines 396-431)

```rust
#[async_trait]
pub trait WorkflowRuntime: Send + Sync {
    fn list_workflows(&self) -> WorkflowResult<Vec<FullWorkflowMetadata>>;
    fn get_workflow_metadata(&self, id: &str) -> WorkflowResult<FullWorkflowMetadata>;
    fn validate_workflow_inputs(&self, id: &str, params: HashMap<String, String>) -> WorkflowResult<()>;
    
    async fn execute_workflow(
        &self,
        id: &str,
        params: HashMap<String, String>,
    ) -> WorkflowResult<WorkflowHandle>;
    
    async fn subscribe_logs(&self, handle_id: &Uuid) -> WorkflowResult<broadcast::Receiver<WorkflowLog>>;
    async fn get_status(&self, handle_id: &Uuid) -> WorkflowResult<WorkflowStatus>;
    async fn cancel_workflow(&self, handle_id: &Uuid) -> WorkflowResult<()>;
}
```

### ProcessBasedRuntime Implementation

**Location**: `/home/molaco/Documents/japanese/workflow-manager/src/runtime.rs`

```rust
pub struct ProcessBasedRuntime {
    workflows: Arc<Mutex<HashMap<String, DiscoveredWorkflow>>>,  // Cached workflows
    executions: Arc<Mutex<HashMap<Uuid, ExecutionState>>>,      // Running processes
}

impl ProcessBasedRuntime {
    pub fn new() -> Result<Self> {
        let workflows = discover_workflows();  // Load from filesystem
        let workflows_map: HashMap<String, DiscoveredWorkflow> = workflows
            .into_iter()
            .map(|w| (w.metadata.id.clone(), w))
            .collect();
        
        Ok(Self {
            workflows: Arc::new(Mutex::new(workflows_map)),
            executions: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    
    pub fn refresh_workflows(&self) -> Result<()> {
        let workflows = discover_workflows();  // Re-discover
        // Update internal cache
    }
}
```

---

## 6. CONDITIONAL DATA SOURCE SELECTION LOGIC

### Decision Flow for Data Retrieval

**App Initialization** (`src/app/mod.rs` lines 25-93):

```rust
pub fn new() -> Self {
    // 1. Load workflows from discovered sources
    let workflows = crate::utils::load_workflows();
    
    // 2. Load history from local disk
    let history = crate::utils::load_history();
    
    // 3. Initialize runtime for workflow execution
    match crate::runtime::ProcessBasedRuntime::new() {
        Ok(runtime) => {
            let runtime_arc = Arc::new(runtime) as Arc<dyn workflow_manager_sdk::WorkflowRuntime>;
            app.runtime = Some(runtime_arc.clone());
            // Initialize chat interface with MCP tools
            app.chat = Some(ChatInterface::new(runtime_arc, ...));
        }
        Err(e) => {
            eprintln!("Warning: Failed to initialize workflow runtime: {}", e);
        }
    }
}
```

### Load Strategies

1. **Built-in Data** (Workflows, Metadata):
   - Source: Filesystem discovery
   - Location: `src/bin/` + `~/.workflow-manager/workflows/`
   - Trigger: Application startup + manual refresh

2. **User Data** (History, Session):
   - Source: Local JSON files
   - Location: `~/.local/share/workflow-manager/` (or fallback `.workflow-manager-history.json`)
   - Trigger: On startup (load), on workflow completion (save)

3. **Runtime State** (Executing Workflows):
   - Source: Process-based (spawned child processes)
   - Storage: Arc<Mutex<Vec>> (in-memory thread-safe)
   - Events: Via stderr IPC with JSON events

4. **MCP Tools** (External Operations):
   - Source: Claude SDK with MCP server
   - Location: `src/mcp_tools.rs`
   - Available tools: `list_workflows`, `execute_workflow`, `get_workflow_logs`, `get_workflow_status`, `cancel_workflow`

---

## 7. ASYNC/AWAIT & NON-BLOCKING OPERATIONS

### Chat Interface Initialization

**Location**: `/home/molaco/Documents/japanese/workflow-manager/src/chat.rs` (lines 121-169)

```rust
fn start_initialization(&mut self, runtime: Arc<dyn WorkflowRuntime>, tokio_handle: tokio::runtime::Handle) {
    let (tx, rx) = mpsc::unbounded_channel();
    self.init_rx = Some(rx);
    
    tokio_handle.spawn(async move {
        let result = Self::initialize_internal(runtime).await;
        let _ = tx.send(result);
    });
}

pub fn poll_initialization(&mut self) {
    if let Some(rx) = &mut self.init_rx {
        match rx.try_recv() {
            Ok(InitResult::Success(client)) => {
                self.client = Some(client);
                self.initialized = true;
            }
            Ok(InitResult::Error(error)) => {
                self.init_error = Some(error);
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                // Still initializing...
            }
            _ => {}
        }
    }
}
```

### Async Message Sending

**Location**: `src/chat.rs` (lines 196-220)

```rust
pub fn send_message_async(&mut self, message: String) {
    self.waiting_for_response = true;
    let (tx, rx) = mpsc::unbounded_channel();
    self.response_rx = Some(rx);
    
    if let Some(client) = self.client.clone() {
        self.tokio_handle.spawn(async move {
            let result = Self::send_message_internal(client, message).await;
            let _ = tx.send(result);
        });
    }
}

pub fn poll_response(&mut self) {
    if let Some(rx) = &mut self.response_rx {
        match rx.try_recv() {
            Ok(response) => {
                self.waiting_for_response = false;
                // Process response
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                // Still waiting...
            }
            _ => {}
        }
    }
}
```

---

## 8. CONFIGURATION & FLAGS THAT CONTROL DATA SOURCES

### Cargo Dependencies

**File**: `/home/molaco/Documents/japanese/workflow-manager/Cargo.toml`

```toml
[dependencies]
claude-agent-sdk = { path = "../claude-agent-sdk-rust", features = ["tracing-support"] }
workflow-manager-sdk = { path = "../workflow-manager-sdk" }
tokio = { workspace = true, features = ["fs", "process", "io-util"] }
reqwest = { version = "0.11", features = ["json"] }  # HTTP client available but not actively used
serde_json = { workspace = true }
directories = "5.0"  # For XDG paths
```

### Environment Variables

- `HOME`: Used to locate `~/.workflow-manager/workflows/` directory
- Implicit: Tokio runtime configuration (multi-threaded)

### Conditional Compilation

**Executable Platform Detection** (`src/discovery.rs` lines 111-132):
```rust
#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("exe"))
        .unwrap_or(false)
}
```

---

## 9. DATA FLOW DIAGRAM

```
┌─────────────────────────────────────────────────────────────────┐
│                      APP STARTUP                                │
└────────────────┬────────────────────────────────────────────────┘
                 │
     ┌───────────┴──────────────┬─────────────────┐
     │                          │                 │
     v                          v                 v
┌─────────────────┐    ┌─────────────────┐  ┌──────────────┐
│ Discover From FS│    │ Load History    │  │ Init Runtime │
│  - Built-in     │    │ from ~/.local/   │  │  - Workflows │
│  - User         │    │  share/          │  │  - Discovery │
└────────┬────────┘    └────────┬────────┘  └──────┬───────┘
         │                      │                  │
         └──────────────────────┼──────────────────┘
                                │
                    ┌───────────v─────────────┐
                    │  App State Initialized  │
                    └───────────┬─────────────┘
                                │
                ┌───────────────┴───────────────┐
                │                               │
                v                               v
        ┌──────────────┐            ┌──────────────────┐
        │ User Selects │            │ Chat Interface   │
        │ Workflow     │            │ Init (Async)     │
        └──────┬───────┘            └──────┬───────────┘
               │                           │ (background)
               v                           v
        ┌──────────────────────────────────────────┐
        │ Load Latest Values from History          │
        │ (for field pre-population)               │
        └──────┬───────────────────────────────────┘
               │
               v
        ┌──────────────────────────────────────────┐
        │ User Launches Workflow                    │
        │ → Spawns child process                   │
        │ → Saves to history (on completion)       │
        │ → Streams events via __WF_EVENT__ stderr │
        └──────┬───────────────────────────────────┘
               │
               v
        ┌──────────────────────────────────────────┐
        │ Real-time Event Parsing & UI Update      │
        │ (from stderr IPC)                         │
        └──────────────────────────────────────────┘
```

---

## 10. NO DATABASE/API/WEBSOCKET PATTERNS FOUND

The codebase analysis reveals:

| Data Source Type | Status | Evidence |
|-----------------|--------|----------|
| **Database** (SQLite, PostgreSQL, etc.) | NOT USED | No `sqlx`, `rusqlite`, `diesel`, `tokio_postgres` in dependencies |
| **HTTP/REST API** | Partial | `reqwest` imported but **not actively used** in data fetching |
| **WebSocket** | NOT USED | No `tokio-tungstenite`, `ws`, or WebSocket code found |
| **GraphQL** | NOT USED | No GraphQL dependencies |
| **gRPC** | NOT USED | No `tonic` or gRPC code |
| **File System** | ACTIVELY USED | History, session, workflow discovery |
| **Process IPC** | ACTIVELY USED | stderr JSON event streaming |
| **In-Memory Cache** | ACTIVELY USED | Arc<Mutex> for runtime state |
| **Local Storage** | ACTIVELY USED | XDG directories, JSON files |

---

## 11. KEY FILE LOCATIONS SUMMARY

| Component | File Path |
|-----------|-----------|
| **Workflow Discovery** | `/home/molaco/Documents/japanese/workflow-manager/src/discovery.rs` |
| **Runtime Implementation** | `/home/molaco/Documents/japanese/workflow-manager/src/runtime.rs` |
| **Chat Interface** | `/home/molaco/Documents/japanese/workflow-manager/src/chat.rs` |
| **MCP Tools** | `/home/molaco/Documents/japanese/workflow-manager/src/mcp_tools.rs` |
| **History Management** | `/home/molaco/Documents/japanese/workflow-manager/src/app/history.rs` |
| **Workflow Execution** | `/home/molaco/Documents/japanese/workflow-manager/src/app/workflow_ops.rs` |
| **Utils** | `/home/molaco/Documents/japanese/workflow-manager/src/utils.rs` |
| **Data Models** | `/home/molaco/Documents/japanese/workflow-manager/src/app/models/` |
| **SDK Traits** | `/home/molaco/Documents/japanese/workflow-manager-sdk/src/lib.rs` |

---

## CONCLUSION

The Workflow Manager uses a **deliberately minimal data source architecture**:

- **Primary pattern**: Filesystem-based discovery + local JSON storage
- **Real-time communication**: Structured JSON events via process stderr
- **Concurrency model**: Arc<Mutex> for thread-safe state management
- **External integrations**: MCP server for Claude SDK tool access
- **No external dependencies**: No active database, API, or WebSocket usage

This design prioritizes **simplicity, portability, and decoupling** over external service dependencies.

