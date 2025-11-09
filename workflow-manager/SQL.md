# SQLite Persistent Workflow Execution History

**Goal**: Store workflow executions in SQLite database so they persist across app restarts and are queryable by Claude indefinitely.

**Status**: Planning
**Priority**: High - Required for Claude to access completed workflows
**Complexity**: Medium - Database schema + runtime integration

---

## Problem Statement

**Current State:**
- Workflow executions stored in `Arc<Mutex<HashMap<Uuid, ExecutionState>>>`
- In-memory only - lost when app closes
- Claude can't query workflows from previous sessions

**Desired State:**
- Executions persisted to SQLite database (`~/.workflow-manager/executions.db`)
- Restored on app startup
- Claude can query any workflow by handle_id, even after app restart
- Efficient storage and retrieval of execution history

---

## Database Schema

### Table: `executions`

Primary table storing workflow execution metadata.

```sql
CREATE TABLE IF NOT EXISTS executions (
    -- Primary key
    id TEXT PRIMARY KEY,                    -- UUID (handle_id)

    -- Workflow info
    workflow_id TEXT NOT NULL,              -- Which workflow was executed
    workflow_name TEXT,                     -- Human-readable name

    -- Execution lifecycle
    status TEXT NOT NULL,                   -- "Running", "Completed", "Failed", "NotStarted"
    start_time TEXT NOT NULL,               -- ISO 8601 timestamp
    end_time TEXT,                          -- ISO 8601 timestamp (NULL if running)

    -- Results
    exit_code INTEGER,                      -- Process exit code (NULL if running)

    -- Metadata
    binary_path TEXT,                       -- Path to workflow binary
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for fast queries
CREATE INDEX IF NOT EXISTS idx_executions_workflow_id ON executions(workflow_id);
CREATE INDEX IF NOT EXISTS idx_executions_status ON executions(status);
CREATE INDEX IF NOT EXISTS idx_executions_start_time ON executions(start_time DESC);
```

### Table: `execution_params`

Stores input parameters used for each execution.

```sql
CREATE TABLE IF NOT EXISTS execution_params (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    execution_id TEXT NOT NULL,             -- Foreign key to executions.id
    param_name TEXT NOT NULL,               -- Field name (e.g., "query")
    param_value TEXT NOT NULL,              -- Field value

    FOREIGN KEY(execution_id) REFERENCES executions(id) ON DELETE CASCADE,
    UNIQUE(execution_id, param_name)
);

CREATE INDEX IF NOT EXISTS idx_params_execution_id ON execution_params(execution_id);
```

### Table: `execution_logs`

Stores workflow logs (both structured and raw output).

```sql
CREATE TABLE IF NOT EXISTS execution_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    execution_id TEXT NOT NULL,             -- Foreign key to executions.id
    sequence INTEGER NOT NULL,              -- Order within execution (0, 1, 2, ...)
    timestamp TEXT NOT NULL,                -- When log was emitted
    log_type TEXT NOT NULL,                 -- "PhaseStarted", "TaskProgress", "RawOutput", etc.
    log_data TEXT NOT NULL,                 -- JSON serialized WorkflowLog

    FOREIGN KEY(execution_id) REFERENCES executions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_logs_execution_id ON execution_logs(execution_id, sequence);
```

### Table: `schema_version`

Tracks database schema version for migrations.

```sql
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Initial version
INSERT OR IGNORE INTO schema_version (version) VALUES (1);
```

---

## Data Model Changes

### 1. Update `ExecutionState` Struct

**Add new fields for persistence:**

```rust
struct ExecutionState {
    workflow_id: String,
    workflow_name: String,                   // NEW
    status: WorkflowStatus,
    child: Option<Child>,
    logs_tx: Sender<WorkflowLog>,
    binary_path: PathBuf,
    logs_buffer: Arc<Mutex<Vec<WorkflowLog>>>,

    // NEW FIELDS FOR PERSISTENCE
    start_time: DateTime<Local>,            // When execution started
    end_time: Option<DateTime<Local>>,      // When execution finished
    params: HashMap<String, String>,        // Input parameters used
    exit_code: Option<i32>,                 // Process exit code
}
```

### 2. Create `DatabaseConnection` Wrapper

**File:** `src/database.rs`

```rust
use rusqlite::{Connection, Result};
use std::path::PathBuf;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new(path: PathBuf) -> Result<Self> {
        let conn = Connection::open(path)?;
        Ok(Self { conn })
    }

    pub fn initialize_schema(&self) -> Result<()> {
        // Create tables
        self.conn.execute_batch(include_str!("../schema.sql"))?;
        Ok(())
    }

    // CRUD operations for executions
    pub fn insert_execution(&self, exec: &PersistedExecution) -> Result<()>;
    pub fn update_execution(&self, id: &Uuid, status: WorkflowStatus, end_time: DateTime<Local>, exit_code: Option<i32>) -> Result<()>;
    pub fn get_execution(&self, id: &Uuid) -> Result<Option<PersistedExecution>>;
    pub fn list_executions(&self, limit: usize, offset: usize) -> Result<Vec<PersistedExecution>>;

    // CRUD operations for logs
    pub fn insert_log(&self, exec_id: &Uuid, sequence: usize, log: &WorkflowLog) -> Result<()>;
    pub fn get_logs(&self, exec_id: &Uuid, limit: Option<usize>) -> Result<Vec<WorkflowLog>>;

    // CRUD operations for params
    pub fn insert_params(&self, exec_id: &Uuid, params: &HashMap<String, String>) -> Result<()>;
    pub fn get_params(&self, exec_id: &Uuid) -> Result<HashMap<String, String>>;
}
```

### 3. Create `PersistedExecution` Struct

**For serialization to/from database:**

```rust
#[derive(Debug, Clone)]
pub struct PersistedExecution {
    pub id: Uuid,
    pub workflow_id: String,
    pub workflow_name: String,
    pub status: WorkflowStatus,
    pub start_time: DateTime<Local>,
    pub end_time: Option<DateTime<Local>>,
    pub exit_code: Option<i32>,
    pub binary_path: PathBuf,
    pub params: HashMap<String, String>,
    pub logs: Vec<WorkflowLog>,  // Loaded separately, not in main query
}

impl PersistedExecution {
    // Convert from ExecutionState
    pub fn from_execution_state(id: Uuid, state: &ExecutionState) -> Self {
        Self {
            id,
            workflow_id: state.workflow_id.clone(),
            workflow_name: state.workflow_name.clone(),
            status: state.status.clone(),
            start_time: state.start_time,
            end_time: state.end_time,
            exit_code: state.exit_code,
            binary_path: state.binary_path.clone(),
            params: state.params.clone(),
            logs: state.logs_buffer.lock().unwrap().clone(),
        }
    }

    // Convert to ExecutionState (for loading from DB)
    pub fn to_execution_state(&self) -> ExecutionState {
        let (logs_tx, _) = broadcast::channel(1);
        ExecutionState {
            workflow_id: self.workflow_id.clone(),
            workflow_name: self.workflow_name.clone(),
            status: self.status.clone(),
            child: None,  // Can't restore running process
            logs_tx,
            binary_path: self.binary_path.clone(),
            logs_buffer: Arc::new(Mutex::new(self.logs.clone())),
            start_time: self.start_time,
            end_time: self.end_time,
            params: self.params.clone(),
            exit_code: self.exit_code,
        }
    }
}
```

---

## Implementation Plan

### Phase 1: Database Setup & Schema

**Task 1.1: Add Dependencies**
- File: `Cargo.toml`
- Add: `rusqlite = "0.31"`
- Add: `chrono = { version = "0.4", features = ["serde"] }` (if not already present)

**Task 1.2: Create Database Module**
- File: `src/database.rs`
- Implement `Database` struct with connection management
- Implement schema initialization
- Implement basic CRUD operations

**Task 1.3: Create SQL Schema File**
- File: `src/schema.sql` (or embed in database.rs)
- Define all tables as shown above
- Add indexes for performance

**Task 1.4: Database Location**
- Use existing config directory: `~/.workflow-manager/`
- Database file: `~/.workflow-manager/executions.db`
- Create directory if doesn't exist

---

### Phase 2: Runtime Integration

**Task 2.1: Add Database to ProcessBasedRuntime**

```rust
pub struct ProcessBasedRuntime {
    workflows: Arc<Mutex<HashMap<String, DiscoveredWorkflow>>>,
    executions: Arc<Mutex<HashMap<Uuid, ExecutionState>>>,
    database: Arc<Database>,  // NEW
}

impl ProcessBasedRuntime {
    pub fn new() -> Result<Self> {
        // Initialize database
        let db_path = dirs::home_dir()
            .ok_or_else(|| anyhow!("Could not find home directory"))?
            .join(".workflow-manager")
            .join("executions.db");

        std::fs::create_dir_all(db_path.parent().unwrap())?;

        let database = Database::new(db_path)?;
        database.initialize_schema()?;

        let runtime = Self {
            workflows: Arc::new(Mutex::new(HashMap::new())),
            executions: Arc::new(Mutex::new(HashMap::new())),
            database: Arc::new(database),
        };

        // Load past executions from database
        runtime.restore_from_database()?;

        Ok(runtime)
    }
}
```

**Task 2.2: Implement restore_from_database()**

```rust
fn restore_from_database(&self) -> Result<()> {
    // Load recent executions (last 100? or all non-running?)
    let persisted = self.database.list_executions(100, 0)?;

    let mut executions = self.executions.lock().unwrap();
    for persisted_exec in persisted {
        // Convert to ExecutionState
        let state = persisted_exec.to_execution_state();
        executions.insert(persisted_exec.id, state);
    }

    Ok(())
}
```

**Task 2.3: Update execute_workflow() to Store Start**

In `execute_workflow()` after creating ExecutionState:

```rust
// Store execution state in memory
self.executions.lock().unwrap().insert(exec_id, state);

// NEW: Persist to database immediately
let persisted = PersistedExecution::from_execution_state(exec_id, &state);
self.database.insert_execution(&persisted)?;
self.database.insert_params(&exec_id, &params)?;
```

**Task 2.4: Update wait_for_process_exit() to Store Completion**

In `wait_for_process_exit()` after updating status:

```rust
// Update status
state.status = if exit_status.success() {
    WorkflowStatus::Completed
} else {
    WorkflowStatus::Failed
};
state.end_time = Some(chrono::Local::now());
state.exit_code = exit_status.code();

// NEW: Persist completion to database
self.database.update_execution(
    &exec_id,
    state.status.clone(),
    state.end_time.unwrap(),
    state.exit_code
)?;
```

**Task 2.5: Store Logs Incrementally**

In log parsers (`parse_workflow_stderr`, `parse_workflow_stdout`):

```rust
if let Some(log) = log {
    // Broadcast to real-time subscribers
    let _ = logs_tx.send(log.clone());

    // Store in buffer for historical retrieval
    let mut buffer = logs_buffer.lock().unwrap();
    let sequence = buffer.len();
    buffer.push(log.clone());

    // NEW: Persist log to database
    // (May want to batch this for performance - see optimization below)
    database.insert_log(&exec_id, sequence, &log)?;
}
```

---

### Phase 3: Query Operations

**Task 3.1: Update get_logs() Implementation**

```rust
async fn get_logs(&self, handle_id: &Uuid, limit: Option<usize>) -> WorkflowResult<Vec<WorkflowLog>> {
    // Try in-memory first (for running workflows)
    let executions = self.executions.lock().unwrap();
    if let Some(state) = executions.get(handle_id) {
        let logs = state.logs_buffer.lock().unwrap();
        let logs_vec = if let Some(limit) = limit {
            logs.iter().rev().take(limit).rev().cloned().collect()
        } else {
            logs.clone()
        };
        return Ok(logs_vec);
    }

    // Not in memory, load from database
    self.database.get_logs(handle_id, limit)
        .map_err(|e| anyhow!("Failed to load logs from database: {}", e))
}
```

**Task 3.2: Update get_status() Implementation**

```rust
async fn get_status(&self, handle_id: &Uuid) -> WorkflowResult<WorkflowStatus> {
    // Try in-memory first
    let executions = self.executions.lock().unwrap();
    if let Some(state) = executions.get(handle_id) {
        return Ok(state.status.clone());
    }

    // Not in memory, load from database
    self.database.get_execution(handle_id)?
        .map(|exec| exec.status)
        .ok_or_else(|| anyhow!("Execution not found: {}", handle_id))
}
```

**Task 3.3: Add New MCP Tool - list_execution_history**

```rust
fn list_execution_history_tool(runtime: Arc<dyn WorkflowRuntime>) -> SdkMcpTool {
    SdkMcpTool::new(
        "list_execution_history",
        "List recent workflow executions with pagination",
        json!({
            "type": "object",
            "properties": {
                "limit": {"type": "integer", "default": 10},
                "offset": {"type": "integer", "default": 0},
                "workflow_id": {"type": "string", "description": "Filter by workflow type"}
            }
        }),
        move |params| {
            // Query database for execution list
            // Return: handle_id, workflow_name, status, start_time, end_time
        }
    )
}
```

---

### Phase 4: Performance Optimizations

**Optimization 1: Batch Log Inserts**

Instead of inserting one log at a time:

```rust
// Buffer logs in memory
let mut pending_logs = Vec::new();

// Every N logs or every T seconds, batch insert
if pending_logs.len() >= 50 || last_flush > 5.seconds() {
    database.batch_insert_logs(&exec_id, &pending_logs)?;
    pending_logs.clear();
}
```

**Optimization 2: Lazy Load Logs**

Don't load all logs when restoring executions:

```rust
// Only load execution metadata on startup
// Load logs on-demand when get_logs() is called
struct PersistedExecution {
    // ... metadata fields
    logs: Option<Vec<WorkflowLog>>,  // None until explicitly loaded
}
```

**Optimization 3: Limit Restored Executions**

Don't load all executions on startup:

```rust
// Only load recent 100 completed executions
// Load others on-demand if queried
fn restore_from_database(&self) -> Result<()> {
    let recent = self.database.list_executions(100, 0)?;
    // Only keep Completed/Failed, skip Running (stale from crash)
}
```

**Optimization 4: Add Cleanup/Archiving**

Periodically archive old executions:

```rust
// Delete executions older than 30 days
fn cleanup_old_executions(&self, days: i64) -> Result<()> {
    let cutoff = chrono::Local::now() - chrono::Duration::days(days);
    self.database.delete_executions_before(cutoff)?;
}
```

---

### Phase 5: Migration & Backwards Compatibility

**Task 5.1: Handle Missing Database**

If database doesn't exist on first run:
- Create it automatically
- Initialize schema
- No migration needed (fresh install)

**Task 5.2: Handle Schema Changes**

For future schema updates:
- Check `schema_version` table
- Run migration scripts if needed
- Example: Adding new column

```rust
fn migrate_to_v2(&self) -> Result<()> {
    let version = self.get_schema_version()?;
    if version < 2 {
        self.conn.execute("ALTER TABLE executions ADD COLUMN new_field TEXT", [])?;
        self.set_schema_version(2)?;
    }
    Ok(())
}
```

**Task 5.3: Handle In-Progress Workflows After Restart**

Executions with status=Running should be marked as Failed/Unknown:

```rust
fn restore_from_database(&self) -> Result<()> {
    let executions = self.database.list_executions(100, 0)?;

    for mut exec in executions {
        // Mark stale running workflows as failed
        if exec.status == WorkflowStatus::Running {
            exec.status = WorkflowStatus::Failed;
            exec.end_time = Some(exec.start_time); // Approximate
            self.database.update_execution(
                &exec.id,
                WorkflowStatus::Failed,
                exec.start_time,
                None
            )?;
        }
        // Load into memory
        self.executions.lock().unwrap().insert(exec.id, exec.to_execution_state());
    }
}
```

---

## File Structure Changes

```
src/
├── database.rs           # NEW: Database connection & operations
├── schema.sql            # NEW: SQL schema definitions (or embedded in database.rs)
├── runtime.rs            # MODIFIED: Add database integration
├── models/
│   └── execution.rs      # NEW: PersistedExecution struct
├── mcp_tools.rs          # MODIFIED: Add list_execution_history tool
└── main.rs               # No changes needed
```

---

## Testing Strategy

### Unit Tests

**Test 1: Database CRUD**
```rust
#[test]
fn test_insert_and_retrieve_execution() {
    let db = Database::new_in_memory();
    let exec = create_test_execution();
    db.insert_execution(&exec).unwrap();
    let retrieved = db.get_execution(&exec.id).unwrap();
    assert_eq!(retrieved.unwrap().id, exec.id);
}
```

**Test 2: Log Storage**
```rust
#[test]
fn test_store_and_retrieve_logs() {
    let db = Database::new_in_memory();
    let exec_id = Uuid::new_v4();
    let logs = create_test_logs();
    for (i, log) in logs.iter().enumerate() {
        db.insert_log(&exec_id, i, log).unwrap();
    }
    let retrieved = db.get_logs(&exec_id, None).unwrap();
    assert_eq!(retrieved.len(), logs.len());
}
```

**Test 3: Restore from Database**
```rust
#[test]
fn test_restore_executions_on_startup() {
    let db = create_populated_database();
    let runtime = ProcessBasedRuntime::new_with_db(db);
    runtime.restore_from_database().unwrap();
    // Verify executions loaded
}
```

### Integration Tests

**Test 4: End-to-End Persistence**
1. Start app
2. Launch workflow
3. Wait for completion
4. Close app
5. Restart app
6. Query workflow by handle_id
7. Verify logs and status retrieved

**Test 5: Claude Query After Restart**
1. Launch workflow via TUI
2. Note handle_id
3. Restart app
4. Ask Claude for status of handle_id
5. Verify Claude can access it

---

## Rollout Plan

### Step 1: Add Database Infrastructure (Non-Breaking)
- Add rusqlite dependency
- Create database.rs module
- Create schema
- No runtime integration yet

### Step 2: Parallel Write (Dual Write)
- Write to both HashMap AND database
- Read only from HashMap (existing behavior)
- Verify writes working

### Step 3: Enable Restore
- Load from database on startup
- Still write to both
- Test with restarts

### Step 4: Optimize
- Add batching, lazy loading
- Add cleanup tools
- Monitor performance

---

## Success Criteria

- ✅ Executions persist across app restarts
- ✅ Claude can query any workflow by handle_id, even after restart
- ✅ Logs stored and retrievable from database
- ✅ No performance degradation for running workflows
- ✅ Database file stays reasonable size (<100MB for 1000 executions)
- ✅ Existing MCP tools (get_status, get_logs) work unchanged
- ✅ New tool (list_execution_history) provides searchable history

---

## Future Enhancements

1. **Search & Filter**
   - Search logs by text
   - Filter by workflow type, date range, status
   - Full-text search on logs

2. **Analytics**
   - Success/failure rates per workflow
   - Average execution time
   - Most used workflows

3. **Export**
   - Export execution as JSON
   - Share workflow run with others

4. **Cleanup UI**
   - View execution history in TUI
   - Delete old executions
   - Archive to separate file

---

## Estimated Effort

- **Phase 1 (Database Setup)**: 4-6 hours
- **Phase 2 (Runtime Integration)**: 6-8 hours
- **Phase 3 (Query Operations)**: 3-4 hours
- **Phase 4 (Optimizations)**: 4-6 hours
- **Phase 5 (Migration)**: 2-3 hours
- **Testing**: 4-6 hours

**Total**: ~25-35 hours

---

## Dependencies

- `rusqlite = "0.31"` - SQLite bindings
- `chrono = { version = "0.4", features = ["serde"] }` - Timestamps
- Existing: `serde`, `serde_json`, `uuid`

---

## Risk Mitigation

**Risk 1: Database Corruption**
- Mitigation: Regular backups, WAL mode, proper error handling

**Risk 2: Performance Degradation**
- Mitigation: Batch inserts, indexes, lazy loading, benchmarking

**Risk 3: Migration Failures**
- Mitigation: Schema versioning, backup before migration, rollback plan

**Risk 4: Disk Space**
- Mitigation: Cleanup tools, log limits, compression, archiving

---

## Questions to Resolve

1. **Log Retention**: How many logs to keep per execution? (All? Last 1000? Last 10MB?)
2. **Cleanup Policy**: Auto-delete executions older than N days? Manual only?
3. **Load Strategy**: Load all on startup? Load recent N? Load on-demand?
4. **Batch Size**: How many logs to batch before inserting? (50? 100? Time-based?)
5. **Indexes**: Which queries are most common? Optimize for what?

---

## Next Steps

1. Review this plan
2. Answer questions above
3. Start with Phase 1 - database setup
4. Iterate incrementally
