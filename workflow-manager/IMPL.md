# Workflow Manager: MCP Integration & API Architecture Specification

## Table of Contents
1. [Introduction](#introduction)
2. [Current Architecture](#current-architecture)
3. [Proposed Service API](#proposed-service-api)
4. [MCP Server Implementation](#mcp-server-implementation)
5. [Type System Status](#type-system-status)
6. [Critical Gaps](#critical-gaps)
7. [Recommended Architecture Evolution](#recommended-architecture-evolution)
8. [Implementation Roadmap](#implementation-roadmap)
9. [API Readiness Assessment](#api-readiness-assessment)
10. [Actionable Recommendations](#actionable-recommendations)

## Introduction

This document provides a comprehensive specification for integrating the Workflow Manager with the Model Context Protocol (MCP) and evolving its architecture to support API-driven execution. The Workflow Manager currently operates as a TUI application with process-based workflow execution, and this specification outlines the path to expose its functionality through MCP tools and future HTTP/WebSocket APIs.

### Purpose

- Define the architecture for MCP integration using in-process patterns
- Specify a service layer (`WorkflowRuntime`) to unify TUI and MCP execution models
- Identify type system limitations blocking API readiness
- Provide a phased implementation roadmap with time estimates

### Scope

This specification covers:
- Current architecture analysis and execution model
- Service API design for workflow discovery, execution, and monitoring
- MCP server implementation patterns and tool definitions
- Type system gaps requiring refactoring
- Production hardening requirements (resource controls, persistence, multi-tenancy)

## Current Architecture

### Execution Model

The Workflow Manager uses a **process-based execution model** where all workflows run as separate processes spawned via `std::process::Command`. This provides strong isolation but introduces complexity in state management and communication.

**Key characteristics:**

- **Communication protocol**: Structured events transmitted over stderr using the `__WF_EVENT__:<JSON>` format
- **State hierarchy**: Three-level tracking system: `Phase → Task → Agent`
- **Binary discovery**: Workflow executables respond to `--workflow-metadata` flag with JSON schema describing inputs
- **State management**: In-memory `Arc<Mutex<Vec<WorkflowPhase>>>` per tab (non-persistent across application restarts)

**Process lifecycle:**
1. Discovery: Scan directories for workflow binaries
2. Metadata extraction: Execute `binary --workflow-metadata` to get schema
3. Execution: Spawn process with constructed CLI arguments
4. Monitoring: Parse stderr for `__WF_EVENT__` messages
5. State tracking: Build hierarchical phase/task/agent structure dynamically

### Three-Layer Design

The architecture follows a clean separation of concerns:

1. **Domain Layer** (`workflow-manager-sdk`): Runtime-agnostic types
   - Core types: `WorkflowDefinition`, `WorkflowLog`, `WorkflowStatus`, `FieldSchema`
   - Location: workflow-manager-sdk/src/lib.rs:40-65
   - Dependencies: Minimal (serde, serde_json, chrono)

2. **Service Layer**: `WorkflowRuntime` facade (proposed) providing unified API
   - Abstracts execution model from consumers
   - Provides sync operations (list, validate) and async operations (execute, subscribe)
   - Enables both TUI and MCP to share execution logic

3. **Adapter Layer**: TUI and MCP server consuming same `WorkflowRuntime`
   - TUI: Terminal interface with tab-based workflow management
   - MCP: In-process JSONRPC server exposing workflow tools

### Discovery & Execution Flow

**Discovery** (src/discovery.rs:15):
- Scan `~/.workflow-manager/workflows/` and `../target/debug`
- Execute each binary with `--workflow-metadata`
- Parse JSON response to extract `FieldSchema[]`
- Cache metadata for UI rendering and validation

**Execution** (main.rs:502-563):
1. Build CLI arguments from `HashMap<String, String>` inputs
2. Spawn process via `std::process::Command`
3. Attach stdout/stderr readers
4. Parse structured events from stderr
5. Forward stdout to display buffers

**State tracking** (main.rs:1255-1400):
- `handle_workflow_event()` dynamically builds hierarchy
- 15+ `WorkflowLog` variants cover all lifecycle events
- Phase/Task/Agent status updates propagate to UI
- Session restore via JSON (main.rs:229-328)

## Proposed Service API

### WorkflowRuntime Interface

The `WorkflowRuntime` provides a unified API for both TUI and MCP consumers, abstracting the process-based execution model.

#### Synchronous Operations

```rust
pub trait WorkflowRuntime {
    /// List all discovered workflows with metadata
    fn list_workflows(&self) -> Result<Vec<WorkflowMetadata>>;

    /// Get detailed metadata for a specific workflow
    fn get_workflow_metadata(&self, id: &str) -> Result<WorkflowMetadata>;

    /// Validate inputs against workflow schema before execution
    fn validate_workflow_inputs(
        &self,
        id: &str,
        params: HashMap<String, String>
    ) -> Result<()>;

    /// Get current status of a running workflow
    fn get_workflow_status(&self, handle_id: &Uuid) -> Result<WorkflowStatus>;
}
```

**Design rationale:**
- Discovery and validation must be synchronous for UI responsiveness
- Metadata includes `FieldSchema[]` for automatic form generation and validation
- Pre-execution validation prevents runtime errors from invalid inputs

#### Asynchronous Operations

```rust
pub trait WorkflowRuntime {
    /// Execute a workflow asynchronously
    async fn execute_workflow(
        &self,
        id: &str,
        params: HashMap<String, String>
    ) -> Result<WorkflowHandle>;

    /// Subscribe to logs from a running workflow
    async fn subscribe_logs(
        &self,
        handle_id: &Uuid
    ) -> Result<impl Stream<Item = WorkflowLog>>;
}
```

**Design rationale:**
- Workflows are long-running (seconds to hours), requiring async execution
- Logs stream via broadcast channels (tokio) to support multiple subscribers
- Returns immediately with `WorkflowHandle` for tracking

### WorkflowHandle

The `WorkflowHandle` provides access to workflow execution state and logs:

```rust
pub struct WorkflowHandle {
    id: Uuid,
    workflow_id: String,
    logs: tokio::sync::broadcast::Receiver<WorkflowLog>,
    completion: tokio::sync::oneshot::Receiver<WorkflowResult>,
}

impl WorkflowHandle {
    /// Get a stream of workflow logs
    pub fn logs(&mut self) -> impl Stream<Item = WorkflowLog> + '_;

    /// Wait for workflow completion
    pub async fn wait_completion(&mut self) -> Result<WorkflowResult>;

    /// Get the unique execution ID
    pub fn id(&self) -> &Uuid;
}
```

**Channel sizing:**
- Bounded channels (32-100 capacity) prevent memory leaks from slow consumers
- Oldest logs dropped on overflow (acceptable for streaming use cases)
- Persistent storage required for complete history

### Data Flow

**1. Initialization:**
```
Discovery → FieldSchema → Validation → Spawn process with unique UUID
```

**2. Runtime:**
```
Stderr parsing → Hierarchical state updates → Emit WorkflowLog variants
                                            ↓
                            Broadcast to subscribers (TUI, MCP, API)
```

**3. Persistence:**
- Current: Session restore via JSON (main.rs:229-328)
- Future: Database storage of execution history and artifacts

### Supported Field Types

The `FieldSchema` system supports six field types with JSON Schema conversion:

| Type | Purpose | Validation |
|------|---------|------------|
| `Text` | Free-form string input | Max length, regex patterns |
| `Number` | Integer/float input | Min/max, step |
| `FilePath` | Local file selection | Existence check, extension filter |
| `Select` | Dropdown/enum | Fixed set of options |
| `PhaseSelector` | Multi-phase selection | Valid phase indices |
| `StateFile` | Cross-workflow state | Compatibility check |

**MCP integration:**
- `FieldSchema` → JSON Schema conversion for automatic tool registration
- MCP clients can generate forms from schemas
- Type safety enforced at SDK boundary

## MCP Server Implementation

### In-Process Pattern

The MCP integration uses the **in-process pattern** via `SdkMcpServer`, avoiding subprocess overhead:

```rust
use crate::mcp::{SdkMcpServer, SdkMcpTool};

let server = SdkMcpServer::new("workflow_manager")
    .description("Execute and manage workflows")
    .tools(vec![
        list_workflows_tool(runtime.clone()),
        execute_workflow_tool(runtime.clone()),
        get_workflow_logs_tool(runtime.clone()),
    ])
    .build()?;
```

**Advantages:**
- JSONRPC over channels (no network/IPC serialization)
- Direct access to `WorkflowRuntime` via `Arc<dyn WorkflowRuntime>`
- Lower latency than subprocess-based MCP servers

**Naming convention:**
- Tools exposed as `mcp__{server_name}__{tool_name}`
- Example: `mcp__workflow_manager__execute_workflow`

### Three Core Tools

#### 1. list_workflows

**Purpose:** Discover all available workflows with their metadata.

```rust
fn list_workflows_tool(runtime: Arc<dyn WorkflowRuntime>) -> SdkMcpTool {
    SdkMcpTool::new(
        "list_workflows",
        "List all available workflows with their metadata and input schemas",
        serde_json::json!({"type": "object", "properties": {}}),
        move |_params| {
            let runtime = runtime.clone();
            async move {
                let workflows = runtime.list_workflows()?;
                let result = serde_json::to_value(&workflows)?;
                Ok(ToolResult::success(result))
            }
        }
    )
}
```

**Returns:**
```json
[
  {
    "id": "research_agent",
    "name": "Research Agent",
    "description": "Performs web research on multiple topics",
    "fields": [
      {
        "name": "topics",
        "field_type": "Text",
        "required": true,
        "description": "Comma-separated research topics"
      }
    ]
  }
]
```

#### 2. execute_workflow

**Purpose:** Start asynchronous workflow execution.

```rust
fn execute_workflow_tool(runtime: Arc<dyn WorkflowRuntime>) -> SdkMcpTool {
    SdkMcpTool::new(
        "execute_workflow",
        "Execute a workflow with provided parameters",
        serde_json::json!({
            "type": "object",
            "properties": {
                "workflow_id": {"type": "string"},
                "parameters": {"type": "object"}
            },
            "required": ["workflow_id", "parameters"]
        }),
        move |params| {
            let runtime = runtime.clone();
            async move {
                let workflow_id = params["workflow_id"].as_str().unwrap();
                let parameters = params["parameters"].as_object().unwrap()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
                    .collect();

                let handle = runtime.execute_workflow(workflow_id, parameters).await?;

                Ok(ToolResult::success(serde_json::json!({
                    "handle_id": handle.id(),
                    "status": "running"
                })))
            }
        }
    )
}
```

**Input:**
```json
{
  "workflow_id": "research_agent",
  "parameters": {
    "topics": "quantum computing, AI safety"
  }
}
```

**Returns:**
```json
{
  "handle_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "running"
}
```

#### 3. get_workflow_logs

**Purpose:** Retrieve logs from a running or completed workflow.

```rust
fn get_workflow_logs_tool(runtime: Arc<dyn WorkflowRuntime>) -> SdkMcpTool {
    SdkMcpTool::new(
        "get_workflow_logs",
        "Get logs from a workflow execution",
        serde_json::json!({
            "type": "object",
            "properties": {
                "handle_id": {"type": "string", "format": "uuid"}
            },
            "required": ["handle_id"]
        }),
        move |params| {
            let runtime = runtime.clone();
            async move {
                let handle_id = Uuid::parse_str(params["handle_id"].as_str().unwrap())?;
                let mut logs_stream = runtime.subscribe_logs(&handle_id).await?;

                let mut logs = Vec::new();
                while let Some(log) = logs_stream.next().await {
                    logs.push(log);
                }

                Ok(ToolResult::success(serde_json::to_value(&logs)?))
            }
        }
    )
}
```

**Input:**
```json
{
  "handle_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**Returns:**
```json
[
  {
    "type": "WorkflowStarted",
    "workflow_id": "research_agent",
    "timestamp": "2025-10-14T20:00:00Z"
  },
  {
    "type": "PhaseStarted",
    "phase_id": 0,
    "name": "Research Phase",
    "timestamp": "2025-10-14T20:00:01Z"
  }
]
```

### State Management

**Concurrency patterns:**

```rust
// Shared state across tool handlers
let state = Arc::new(Mutex::new(WorkflowState::new()));

// Clone Arc into each tool handler closure
let execute_state = state.clone();
let logs_state = state.clone();

// TUI: Arc<Mutex<>> for thread-safe buffers (main.rs:1108-1253)
// MCP: tokio::sync for async-safe state with unique UUIDs
```

**Best practices:**
- Use `Arc<tokio::sync::Mutex<>>` for async contexts
- Use `Arc<std::sync::Mutex<>>` for sync contexts
- Store `HashMap<Uuid, WorkflowHandle>` for handle tracking
- Clean up completed handles periodically

### Error Handling

**Domain errors:**
```rust
// Workflow not found
return Ok(ToolResult::error("Workflow 'foo' not found"));

// Validation failure
return Ok(ToolResult::error("Required field 'topics' missing"));
```

**System errors:**
```rust
// Process spawn failure
Err(anyhow!("Failed to spawn workflow process"))

// Channel closed
Err(anyhow!("Workflow execution channel closed unexpectedly"))
```

**Validation:**
- Automatic via JSON schema for tool inputs
- Manual via `validate_workflow_inputs()` for workflow parameters
- Return structured error messages with field names

### Registration Checklist

**Complete MCP integration in 5 steps:**

1. **Create tools:**
```rust
let tools = vec![
    SdkMcpTool::new(name, desc, schema, handler),
    // ... repeat for each tool
];
```

2. **Build server:**
```rust
let server = SdkMcpServer::new("workflow_manager")
    .description("Workflow execution and management")
    .tools(tools)
    .build()?;
```

3. **Register server:**
```rust
McpServerConfig::Sdk(SdkMcpServerMarker {
    name: "workflow_manager".to_string(),
    instance: Arc::new(server)
})
```

4. **Allow tools:**
```rust
allowed_tools: vec![
    ToolName::new("mcp__workflow_manager__*")
]
```

5. **Handle requests:**
```rust
// Background task
tokio::spawn(async move {
    loop {
        let request = mcp_rx.recv().await?;
        let response = server.handle_request(request).await?;
        mcp_tx.send(response).await?;
    }
});
```

**Reference:**
- MCP protocol: src/mcp/{server,tool,protocol}.rs
- Client integration: src/client/mod.rs:266-279, 750-820

## Type System Status

### ✅ Production-Ready

The following types are stable and ready for API use:

- **`WorkflowMetadata`**: Complete workflow schema with fields, description, version
- **`WorkflowLog`**: 15+ event variants covering full lifecycle (PhaseStarted, TaskCompleted, AgentProgress, etc.)
- **`WorkflowStatus`**: Enum for Running/Completed/Failed states
- **Discovery system**: Binary scanning and metadata extraction fully functional

These types require no changes for MCP integration.

### ⚠️ Architectural Blockers

#### 1. PhaseSelector (lib.rs:40-65)

**Problem:**
- Current implementation uses CLI string format `"0,1,2"` for phase selection
- Incompatible with JSON Schema array type `[0, 1, 2]`
- MCP clients expect structured types, not strings

**Required fix:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PhaseInput {
    Indices(Vec<usize>),      // API: [0, 1, 2]
    String(String),           // CLI: "0,1,2"
}

impl PhaseInput {
    pub fn to_indices(&self) -> Result<Vec<usize>> {
        match self {
            PhaseInput::Indices(v) => Ok(v.clone()),
            PhaseInput::String(s) => s.split(',')
                .map(|x| x.trim().parse())
                .collect()
        }
    }
}
```

**Impact:** Blocks automatic JSON Schema generation for workflows using phase selectors.

**Effort:** 2-3 hours (refactor + tests)

#### 2. StateFile Architecture

**Problem:**
- Current design conflates three concerns:
  1. File discovery (where to find state files)
  2. User input (file selection/upload)
  3. State tracking (execution artifacts)
- Assumes local filesystem access (breaks for remote APIs)

**Required split:**
```rust
// User input layer (API boundary)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileInput {
    Path(PathBuf),              // Local filesystem
    Url(String),                // Remote file
    Content(Vec<u8>),           // Inline upload
    Reference(String),          // Previous execution artifact
}

// Validation metadata (workflow schema)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateFileConstraint {
    pub compatible_workflows: Vec<String>,
    pub required_fields: Vec<String>,
    pub max_age: Option<Duration>,
}

// Internal tracking (runtime state)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateFileReference {
    pub execution_id: Uuid,
    pub file_path: PathBuf,
    pub created_at: DateTime<Utc>,
}
```

**Impact:** Blocks remote execution and multi-user scenarios.

**Effort:** 1-2 weeks (requires refactoring all state file usage)

#### 3. JSON Schema Generation

**Problem:**
- No `FieldType::to_json_schema()` implementation
- Manual schema construction in tool definitions
- Breaks automatic tool generation

**Required implementation:**
```rust
impl FieldType {
    pub fn to_json_schema(&self) -> serde_json::Value {
        match self {
            FieldType::Text => json!({"type": "string"}),
            FieldType::Number => json!({"type": "number"}),
            FieldType::FilePath => json!({
                "type": "string",
                "format": "path"
            }),
            FieldType::Select(options) => json!({
                "type": "string",
                "enum": options
            }),
            FieldType::PhaseSelector => json!({
                "type": "array",
                "items": {"type": "integer"}
            }),
            FieldType::StateFile => json!({"type": "string"}),
        }
    }
}
```

**Impact:** Blocks automatic MCP tool creation from workflow metadata.

**Effort:** 4-6 hours (implementation + tests)

#### 4. Versioning

**Problem:**
- No `schema_version` field in `WorkflowMetadata`
- Cannot detect format changes or perform migrations
- Risk of silent breakage across versions

**Required addition:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub fields: Vec<FieldSchema>,

    #[serde(default = "default_version")]
    pub schema_version: String,  // Semver: "1.0.0"
}

fn default_version() -> String {
    "1.0.0".to_string()
}
```

**Impact:** Blocks production deployment without migration strategy.

**Effort:** 2-3 hours (add field + update all workflows)

## Critical Gaps

### Execution Layer

#### No Resource Controls

**Problem:**
- Workflows can consume unlimited CPU, memory, disk
- No execution time limits
- No process sandboxing or isolation

**Required:**
```rust
pub struct ExecutionLimits {
    pub max_cpu_percent: u8,           // 0-100
    pub max_memory_mb: usize,          // MB
    pub max_disk_mb: usize,            // MB
    pub timeout: Duration,             // Wall clock
}

// Implementation approaches:
// 1. tokio::timeout for wall clock limits
// 2. cgroups v2 for CPU/memory/disk (Linux)
// 3. resource_limits crate for cross-platform
```

**Risk:** Resource exhaustion in production environments.

**Effort:** 3-4 weeks (research + cross-platform implementation)

#### Forceful Cancellation

**Problem:**
- `child.kill()` sends SIGKILL immediately (src/main.rs:1844)
- No grace period for cleanup
- Risk of corrupted state files

**Required:**
```rust
pub async fn graceful_shutdown(
    child: &mut Child,
    timeout: Duration
) -> Result<()> {
    // 1. Send SIGTERM
    send_signal(child.id(), Signal::SIGTERM)?;

    // 2. Wait with timeout
    match tokio::time::timeout(timeout, child.wait()).await {
        Ok(Ok(status)) => Ok(()),
        _ => {
            // 3. Force kill if timeout
            child.kill().await?;
            Err(anyhow!("Process killed after timeout"))
        }
    }
}
```

**Risk:** Data loss or corruption on cancellation.

**Effort:** 1-2 days (implementation + testing)

#### No Execution Persistence

**Problem:**
- Only TUI session restore exists (main.rs:229-328)
- No database storage of execution history
- Logs lost on application restart

**Required:**
- SQLite for local deployments
- PostgreSQL for production/multi-user
- Schema: `executions`, `workflow_logs`, `artifacts`

**Risk:** Cannot audit or replay workflow executions.

**Effort:** 2-3 weeks (schema design + migration system)

#### Stub Implementation

**Location:** main.rs:2153 (`load_discovered_workflows()`)

**Problem:**
- Discovery system not integrated with execution
- Metadata caching incomplete
- No hot reload on workflow changes

**Required:**
- File watcher for workflow directory changes
- LRU cache with TTL for metadata
- Background discovery refresh

**Effort:** 1-2 weeks

### Multi-tenancy

**Problem:**
- No user isolation or authentication
- All workflows visible to all users
- Shared execution namespace

**Required for production:**
- User authentication (OAuth/JWT)
- Per-user workflow visibility
- Execution quotas and rate limits
- Audit logging

**Effort:** 4-6 weeks (authentication + authorization system)

## Recommended Architecture Evolution

### Create `workflow-manager-api` Crate

**Rationale:**
- **Preserves SDK purity**: Minimal dependencies (serde, chrono only)
- **Prevents circular dependencies**: Unidirectional flow (SDK ← API)
- **Future extensibility**: Supports HTTP, gRPC, WebSocket without SDK changes

**Structure:**
```
workflow-manager-api/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── client.rs          // WorkflowApiClient trait
│   ├── endpoints.rs       // HTTP endpoint definitions
│   ├── error.rs           // API-specific errors
│   ├── types.rs           // Request/Response DTOs
│   └── streaming.rs       // WebSocket/SSE support

Dependencies:
  workflow-manager-sdk = { path = "../workflow-manager-sdk" }
  reqwest = "0.11"
  tokio = { version = "1", features = ["full"] }
  serde = { version = "1", features = ["derive"] }
  serde_json = "1"
```

**Benefits:**
1. SDK remains embeddable in workflows (no heavy deps)
2. API crate can use async runtime, HTTP libs freely
3. Multiple API implementations (REST, gRPC) share SDK types
4. Clear versioning boundary (SDK 1.x can support API 2.x)

### Extract Execution Engine

**Current problem:**
- Execution logic tightly coupled to TUI (tab-based state)
- `Arc<Mutex<Vec<WorkflowPhase>>>` per tab (main.rs:1108)
- Cannot reuse for MCP/API without duplication

**Proposed structure:**
```rust
// workflow-manager-core/src/execution.rs
pub struct ExecutionEngine {
    executions: Arc<Mutex<HashMap<Uuid, ExecutionState>>>,
    runtime: tokio::runtime::Runtime,
}

pub struct ExecutionState {
    id: Uuid,
    workflow_id: String,
    status: WorkflowStatus,
    phases: Vec<WorkflowPhase>,
    logs: tokio::sync::broadcast::Sender<WorkflowLog>,
    child: Option<Child>,
}

impl ExecutionEngine {
    pub async fn execute(
        &self,
        workflow: &WorkflowMetadata,
        params: HashMap<String, String>
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let (log_tx, _) = broadcast::channel(100);

        // Spawn process
        let child = self.spawn_workflow(workflow, params)?;

        // Store state
        let state = ExecutionState {
            id, workflow_id: workflow.id.clone(),
            status: WorkflowStatus::Running,
            phases: Vec::new(),
            logs: log_tx,
            child: Some(child),
        };
        self.executions.lock().unwrap().insert(id, state);

        // Start log consumer
        self.start_log_consumer(id).await?;

        Ok(id)
    }
}
```

**Benefits:**
1. TUI becomes thin adapter over `ExecutionEngine`
2. MCP tools delegate to same engine
3. Future API server uses same engine
4. Testable in isolation

**Migration path:**
1. Extract `ExecutionEngine` to `src/execution.rs`
2. Refactor TUI to use engine (1-2 weeks)
3. Implement MCP tools using engine (1 week)
4. Add persistence layer to engine (2-3 weeks)

**Effort:** 3-4 weeks total

## Implementation Roadmap

### Phase 1: MCP Integration (13-18 hours)

**Goal:** Enable Claude Desktop to execute workflows via MCP tools.

**Tasks:**

1. **Service layer** (4-6 hours)
   - Define `WorkflowRuntime` trait
   - Implement `ProcessBasedRuntime` using existing discovery/execution
   - Add broadcast channels for log streaming
   - Write unit tests for runtime API

2. **TUI refactor** (3-4 hours)
   - Replace direct process spawning with `WorkflowRuntime` calls
   - Migrate to `subscribe_logs()` for log consumption
   - Test tab management with new API
   - Verify session restore works

3. **MCP server** (4-5 hours)
   - Implement three tools: `list_workflows`, `execute_workflow`, `get_workflow_logs`
   - Register server in MCP client
   - Add tool allowlist configuration
   - Test end-to-end with Claude Desktop

4. **Testing** (2-3 hours)
   - Integration tests for MCP tools
   - Manual testing: workflow discovery, execution, log streaming
   - Error handling validation

**Deliverables:**
- Working MCP server with three tools
- TUI using new `WorkflowRuntime` API
- Documentation: MCP setup guide

**Blockers:** None (uses existing types)

### Phase 2: Type System (Weeks 1-2)

**Goal:** Fix type system blockers for API readiness.

**Tasks:**

1. **JSON Schema generation** (2-3 hours)
   - Implement `FieldType::to_json_schema()`
   - Add tests for all field types
   - Update MCP tool registration to use generated schemas

2. **Schema versioning** (2-3 hours)
   - Add `schema_version` field with `#[serde(default)]`
   - Update all workflow binaries to emit version
   - Write migration guide for future versions

3. **PhaseSelector fix** (4-6 hours)
   - Create `PhaseInput` enum (array + string variants)
   - Update CLI parsing to support both formats
   - Add JSON Schema generation for array format
   - Test with existing workflows

4. **Testing** (2-3 hours)
   - Unit tests for schema generation
   - Integration tests with MCP tools
   - Backward compatibility tests

**Deliverables:**
- `FieldType::to_json_schema()` implementation
- Versioned metadata format
- PhaseSelector supporting JSON arrays

**Blockers:** None

### Phase 3: API Foundation (Weeks 2-6)

**Goal:** Production-ready workflow execution API.

**Tasks:**

1. **Create `workflow-manager-api` crate** (1 week)
   - Define HTTP endpoints (REST)
   - Implement `WorkflowApiClient` trait
   - Add request/response DTOs
   - Write OpenAPI spec

2. **Implement core endpoints** (2 weeks)
   - `GET /workflows` - List workflows
   - `POST /workflows/{id}/execute` - Start execution
   - `GET /executions/{id}` - Get status
   - `GET /executions/{id}/logs` - Stream logs
   - Add authentication middleware

3. **Fix StateFile blocker** (1 week)
   - Split into `FileInput`, `StateFileConstraint`, `StateFileReference`
   - Update workflows using state files
   - Add remote file fetching
   - Test with API uploads

4. **Dual execution modes** (1 week)
   - Process-based: Current model (isolation)
   - Library-based: Direct function calls (low latency)
   - Add mode selection to workflow metadata
   - Benchmark performance difference

5. **Testing** (1 week)
   - Integration tests for all endpoints
   - Load testing (concurrent executions)
   - Security testing (input validation, auth)

**Deliverables:**
- `workflow-manager-api` crate with REST endpoints
- OpenAPI specification
- StateFile refactored for remote use
- Performance benchmarks

**Blockers:**
- Requires Phase 2 completion (type system fixes)
- May need execution engine extraction for library mode

### Phase 4: Production Hardening (Medium-term)

**Goal:** Enterprise-ready deployment with persistence and resource controls.

**Tasks:**

1. **Extract execution engine** (2-3 weeks)
   - Create `workflow-manager-core` crate
   - Implement `ExecutionEngine` with UUID-based state
   - Migrate TUI and MCP to use engine
   - Add comprehensive tests

2. **Resource limits** (3-4 weeks)
   - Start with `tokio::timeout` for wall clock
   - Add CPU/memory/disk limits via cgroups (Linux)
   - Cross-platform fallback using `resource_limits` crate
   - Test limit enforcement

3. **Graceful shutdown** (1 week)
   - SIGTERM → timeout → SIGKILL pattern
   - Cleanup handlers for state files
   - Test cancellation scenarios

4. **Persistence layer** (2-3 weeks)
   - Schema design: `executions`, `workflow_logs`, `artifacts`
   - SQLite implementation for local use
   - PostgreSQL implementation for production
   - Migration system

5. **WebSocket streaming** (1-2 weeks)
   - Implement `/api/executions/{id}/stream` endpoint
   - Add WebSocket authentication
   - Test with concurrent clients

6. **Multi-tenancy** (4-6 weeks)
   - User authentication (OAuth/JWT)
   - Per-user workflow isolation
   - Execution quotas and rate limits
   - Audit logging

**Deliverables:**
- `workflow-manager-core` crate
- Resource control system
- Persistent execution history
- Multi-user support

**Blockers:**
- Requires Phase 3 completion (API foundation)

## API Readiness Assessment

### Overall: 70%

The codebase has strong foundations but requires focused work on type system and execution architecture before production API deployment.

| Component | Status | Notes | Timeline |
|-----------|--------|-------|----------|
| Core types | ✅ Ready | `WorkflowMetadata`, `WorkflowLog`, `WorkflowStatus` stable | - |
| Discovery system | ✅ Ready | Binary scanning, metadata extraction functional | - |
| Execution logic | ⚠️ Needs extraction | Coupled to TUI, requires `ExecutionEngine` refactor | 1-2 weeks |
| PhaseSelector | 🚫 Blocked | String format incompatible with JSON Schema | 2-3 weeks |
| StateFile | 🚫 Blocked | Conflates discovery/input/tracking, assumes local FS | 2-3 weeks |
| JSON Schema | 📦 Missing | No `to_json_schema()` implementation | 4-6 hours |
| Versioning | 📦 Missing | No `schema_version` field for migration | 2-3 hours |
| Resource controls | 📦 Missing | No CPU/memory/disk limits, no timeouts | 3-4 weeks |
| Graceful shutdown | 📦 Missing | Uses SIGKILL, no cleanup grace period | 1 week |
| Persistence | 📦 Missing | No database storage of execution history | 2-3 weeks |
| Multi-user support | 📦 Missing | No auth, isolation, or quotas | 4-6 weeks |

### Readiness by Use Case

**MCP Integration (Claude Desktop):**
- **Status:** 90% ready
- **Timeline:** 2 weeks
- **Blockers:** Need `WorkflowRuntime` abstraction (Phase 1)

**HTTP API (Single user, local):**
- **Status:** 60% ready
- **Timeline:** 6-8 weeks
- **Blockers:** Type system fixes (Phase 2), API crate (Phase 3)

**HTTP API (Multi-user, production):**
- **Status:** 40% ready
- **Timeline:** 12-16 weeks
- **Blockers:** All of above + persistence, resource limits, auth (Phase 4)

### Risk Assessment

**High priority (blocks deployment):**
1. StateFile architecture (breaks remote execution)
2. No resource controls (security/stability risk)
3. No persistence (cannot audit/replay)

**Medium priority (degrades UX):**
1. PhaseSelector format (manual schema workarounds)
2. Forceful cancellation (data loss risk)
3. No multi-tenancy (single-user limitation)

**Low priority (nice-to-have):**
1. Library-based execution (latency optimization)
2. WebSocket streaming (HTTP SSE sufficient)
3. Hot reload (restart acceptable)

## Actionable Recommendations

### Immediate Actions (Next 2 weeks)

1. **Implement Phase 1 (MCP Integration)**
   - Create `WorkflowRuntime` trait and `ProcessBasedRuntime` implementation
   - Refactor TUI to use new runtime API
   - Build MCP server with three core tools
   - Test with Claude Desktop
   - **Owner:** Backend team
   - **Effort:** 13-18 hours

2. **Fix JSON Schema generation**
   - Implement `FieldType::to_json_schema()`
   - Update MCP tool registration
   - Add unit tests
   - **Owner:** SDK team
   - **Effort:** 2-3 hours

3. **Add schema versioning**
   - Add `schema_version` field to `WorkflowMetadata`
   - Update all workflow binaries
   - **Owner:** SDK team
   - **Effort:** 2-3 hours

### Short-term (Weeks 3-8)

4. **Fix PhaseSelector blocker**
   - Create `PhaseInput` enum supporting arrays
   - Update CLI parsing
   - Test backward compatibility
   - **Owner:** SDK team
   - **Effort:** 4-6 hours

5. **Create `workflow-manager-api` crate**
   - Define REST endpoints
   - Implement HTTP handlers
   - Write OpenAPI spec
   - **Owner:** API team
   - **Effort:** 1 week

6. **Fix StateFile architecture**
   - Split into `FileInput`, `StateFileConstraint`, `StateFileReference`
   - Update workflows using state files
   - Add remote file support
   - **Owner:** SDK + Backend team
   - **Effort:** 1 week

7. **Extract execution engine**
   - Create `ExecutionEngine` in `workflow-manager-core`
   - Migrate TUI and MCP to use engine
   - Add tests
   - **Owner:** Backend team
   - **Effort:** 2-3 weeks

### Medium-term (Weeks 9-16)

8. **Implement resource controls**
   - Add `tokio::timeout` for wall clock
   - Add cgroups support (Linux)
   - Cross-platform fallback
   - **Owner:** Infrastructure team
   - **Effort:** 3-4 weeks

9. **Add persistence layer**
   - Design database schema
   - Implement SQLite backend
   - Add PostgreSQL support
   - **Owner:** Backend + DB team
   - **Effort:** 2-3 weeks

10. **Implement graceful shutdown**
    - SIGTERM → timeout → SIGKILL flow
    - Add cleanup handlers
    - **Owner:** Backend team
    - **Effort:** 1 week

### Long-term (Weeks 17+)

11. **Build multi-tenancy system**
    - User authentication
    - Workflow isolation
    - Quotas and rate limits
    - **Owner:** Full-stack team
    - **Effort:** 4-6 weeks

12. **Add WebSocket streaming**
    - Implement `/api/executions/{id}/stream`
    - Add client SDK
    - **Owner:** API + Frontend team
    - **Effort:** 1-2 weeks

### Continuous Improvements

- **Documentation:** Update after each phase completion
- **Testing:** Maintain 80%+ coverage for new code
- **Monitoring:** Add metrics for execution time, resource usage, error rates
- **Performance:** Benchmark each phase, optimize bottlenecks

### Decision Points

**After Phase 1 (MCP Integration):**
- **Decision:** Proceed with full API development or iterate on MCP tools?
- **Criteria:** User feedback, usage metrics, resource availability

**After Phase 3 (API Foundation):**
- **Decision:** Deploy single-user API or wait for multi-tenancy?
- **Criteria:** Customer demand, security requirements, team capacity

**During Phase 4 (Production Hardening):**
- **Decision:** SQLite or PostgreSQL first?
- **Criteria:** Deployment target (local vs. cloud), data volume expectations

---

**Last updated:** 2025-10-14
**Status:** Draft specification
**Next review:** After Phase 1 completion
