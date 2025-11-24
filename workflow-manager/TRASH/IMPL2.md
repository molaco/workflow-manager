# Workflow Manager: Data Architecture & Integration Guide

## Table of Contents

1. [Introduction](#introduction)
2. [Executive Summary](#executive-summary)
3. [Current Architecture](#current-architecture)
   - [Application Overview](#application-overview)
   - [Technology Stack](#technology-stack)
   - [Component Architecture](#component-architecture)
4. [Data Persistence Strategy](#data-persistence-strategy)
   - [Current File-Based System](#current-file-based-system)
   - [Storage Locations](#storage-locations)
   - [State Management](#state-management)
5. [Database Integration Roadmap](#database-integration-roadmap)
   - [Phase 1: Core Infrastructure](#phase-1-core-infrastructure)
   - [Phase 2: Integration Points](#phase-2-integration-points)
   - [Phase 3: Migration Strategy](#phase-3-migration-strategy)
   - [Phase 4-7: Advanced Features](#phase-4-7-advanced-features)
6. [Hypothetical: Financial Data Implementation](#hypothetical-financial-data-implementation)
7. [Future Development: Codebase Search](#future-development-codebase-search)
8. [Recommendations](#recommendations)
9. [Appendix](#appendix)

---

## Introduction

This document provides a comprehensive analysis of the workflow-manager application's data architecture and outlines integration strategies for database persistence. The workflow-manager is a Rust-based Terminal UI (TUI) application designed for workflow orchestration with integrated AI chat capabilities powered by Claude.

**Purpose of this Document:**
- Document the current data architecture and persistence mechanisms
- Identify limitations in the existing file-based storage system
- Provide a detailed roadmap for database integration
- Clarify the application's scope and address misconceptions about its purpose
- Offer guidance for potential future enhancements

**Target Audience:**
- Developers contributing to the workflow-manager codebase
- System architects planning database integration
- Technical stakeholders evaluating the application's capabilities

---

## Executive Summary

### Key Findings

1. **Application Type**: The workflow-manager is a workflow orchestration tool with TUI interface, **not** a financial/trading application
2. **Current Persistence**: Exclusively file-based using JSON/YAML serialization with no database infrastructure
3. **Architecture**: Event-driven async architecture using Tokio runtime with centralized state management
4. **Integration Opportunity**: Clear migration path to SQLite-based persistence while maintaining backward compatibility

### Critical Insights

| Aspect | Current State | Planned Future |
|--------|--------------|----------------|
| **Data Storage** | JSON files in platform-specific directories | SQLite with JSON fallback |
| **Persistence Layer** | Direct file I/O via `serde_json` | Database abstraction with migration tools |
| **Search Capabilities** | None | Separate codebase search project (Qdrant + Tantivy) |
| **External Integrations** | Claude AI API only | No additional integrations planned |

### Immediate Action Items

1. ✅ Understand current file-based persistence architecture
2. 🔄 Implement SQLite integration (Phases 1-3)
3. 🔄 Create automated migration from JSON to database
4. 📋 Plan separate codebase search project

---

## Current Architecture

### Application Overview

The workflow-manager is built as a modern Rust TUI application using the `ratatui v0.28` framework. It provides:

**Core Capabilities:**
- Workflow discovery from filesystem
- Process-based workflow execution with real-time event streaming
- Interactive chat interface powered by Claude AI
- Session persistence and history management
- Multi-tab navigation interface

**Not Included:**
- Financial data processing or trading functionality
- Real-time market data handling
- OHLCV (candle) chart storage or visualization
- Database-backed persistence

### Technology Stack

#### Production Dependencies

```toml
# Async Runtime & Concurrency
tokio = { version = "1.35", features = ["full"] }

# Terminal UI Framework
ratatui = "0.28"
crossterm = "0.27"

# AI Integration
claude-agent-sdk = "0.2.5"

# Serialization & Data Formats
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"

# Utilities
chrono = "0.4"
uuid = { version = "1.6", features = ["v4", "serde"] }
directories = "5.0"
```

#### Notably Absent Dependencies

The following dependencies are **not present**, confirming no database infrastructure exists:

```toml
# ❌ Not included:
# sqlx           - Async SQL toolkit
# diesel         - ORM and query builder
# rusqlite       - SQLite bindings
# tokio-postgres - PostgreSQL driver
# sled           - Embedded database
# redb           - Embedded key-value store
```

### Component Architecture

#### Module Structure

```
workflow-manager/
├── src/
│   ├── main.rs              # Entry point, event loop
│   ├── app/
│   │   └── mod.rs           # Centralized state management
│   ├── discovery.rs         # Filesystem workflow scanning
│   ├── runtime.rs           # Process-based execution engine
│   ├── chat.rs              # Claude AI integration
│   ├── ui/                  # Ratatui rendering components
│   └── utils.rs             # History load/save utilities
└── Cargo.toml
```

#### Key Components

| Component | Responsibility | Key Files |
|-----------|---------------|-----------|
| **App State** | Centralized application state, navigation, tabs | `src/app/mod.rs:28-150` |
| **Discovery Service** | Locate workflow binaries from filesystem | `src/discovery.rs` |
| **Runtime Engine** | Execute workflows as child processes, stream events | `src/runtime.rs` |
| **Chat Interface** | Non-blocking async Claude AI interactions | `src/chat.rs` |
| **History Manager** | Load/save workflow field values | `src/utils.rs:21-39` |
| **UI Layer** | Render TUI components, handle input | `src/ui/*` |

#### Initialization Flow

**Application Bootstrap Sequence** (`src/app/mod.rs:28`):

```rust
App::new() {
    1. Create platform-specific storage directories
    2. Scan filesystem for workflow binaries
    3. Initialize Tokio runtime
    4. Create ProcessBasedRuntime wrapper
    5. Initialize ChatInterface with Claude SDK
    6. Restore session from ~/.local/share/workflow-manager/session.json
    7. Load workflow history from history.json
    8. Initialize UI state (current_view, selected_index)
}
```

#### Thread-Safe Execution Model

The application uses a sophisticated async execution model:

```rust
// Thread-safe workflow state (src/app/mod.rs)
Arc<Mutex<WorkflowExecution>> {
    workflow_id: String,
    output: String,           // Accumulated stdout
    current_phase: usize,
    phases: Vec<String>,      // Parsed from __WF_EVENT__:<JSON>
    exit_status: Option<i32>,
}
```

**Event Streaming** (`src/runtime.rs`):
- Child process stdout/stderr captured via `tokio::process::Command`
- Events parsed as `__WF_EVENT__:<JSON>` markers in stderr
- Non-blocking updates to `Arc<Mutex<>>` shared state
- UI polls execution state during render cycles

---

## Data Persistence Strategy

### Current File-Based System

#### Critical Finding: Zero Database Infrastructure

**Confirmed Absence:**
- ✗ No database connection pooling
- ✗ No SQL query builders or ORM frameworks
- ✗ No migrations or schema management
- ✗ No time-series or vector database integration
- ✗ No candle/OHLCV data storage

**Confirmed Presence:**
- ✓ JSON serialization for session state
- ✓ YAML parsing for workflow definitions
- ✓ Platform-specific directory management
- ✓ In-memory state with file-based snapshots

### Storage Locations

#### Platform-Specific Paths

The application uses the `directories` crate for XDG Base Directory compliance:

```rust
// src/utils.rs:10-18
fn get_history_path() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "workflow", "manager") {
        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir).ok();
        data_dir.join("history.json")
    } else {
        PathBuf::from(".workflow-manager-history.json") // Fallback
    }
}
```

**Actual Storage Locations:**

| Platform | History Path | Session Path |
|----------|-------------|--------------|
| **Linux** | `~/.local/share/workflow-manager/history.json` | `~/.local/share/workflow-manager/session.json` |
| **macOS** | `~/Library/Application Support/workflow-manager/history.json` | `~/Library/Application Support/workflow-manager/session.json` |
| **Windows** | `%APPDATA%\workflow\manager\data\history.json` | `%APPDATA%\workflow\manager\data\session.json` |
| **Fallback** | `./.workflow-manager-history.json` | `./.workflow-manager-session.json` |

#### Storage Content Types

**History File** (`history.json`):
```json
{
  "workflow_id_1": {
    "field_name_1": "field_value_1",
    "field_name_2": "field_value_2"
  },
  "workflow_id_2": {
    "another_field": "value"
  }
}
```

**Session File** (`session.json`):
```json
{
  "current_view": "WorkflowList",
  "selected_workflow_index": 0,
  "active_tab_index": 1,
  "last_saved": "2025-10-19T09:37:50Z"
}
```

### State Management

#### In-Memory Architecture

**App State Structure** (`src/app/mod.rs`):

```rust
pub struct App {
    // Workflow Management
    catalog: Vec<Workflow>,
    history: HashMap<String, HashMap<String, String>>,

    // Execution State
    workflow_executions: HashMap<String, Arc<Mutex<WorkflowExecution>>>,
    runtime: Arc<dyn WorkflowRuntime>,

    // UI State
    current_view: View,
    selected_workflow_index: usize,
    tabs: Vec<Tab>,
    selected_tab_index: usize,

    // AI Integration
    chat_interface: ChatInterface,

    // Async Runtime
    tokio_runtime: Arc<Runtime>,
}
```

#### Data Flow Diagram

```
┌─────────────────┐
│  Filesystem     │
│  - Workflow     │
│    binaries     │
│  - JSON files   │
└────────┬────────┘
         │
         ↓ App::new()
┌─────────────────────────────────────────┐
│         Centralized App State           │
│  ┌───────────────────────────────────┐  │
│  │  catalog: Vec<Workflow>           │  │
│  │  history: HashMap<...>            │  │
│  │  workflow_executions: HashMap     │  │
│  └───────────────────────────────────┘  │
│         ↓                ↓               │
│  ┌──────────┐    ┌──────────────┐       │
│  │ Runtime  │    │ ChatInterface│       │
│  │ Engine   │    │  (Claude AI) │       │
│  └──────────┘    └──────────────┘       │
└─────────────────────────────────────────┘
         │
         ↓ save_session() / save_history()
┌─────────────────┐
│  JSON Files     │
│  - history.json │
│  - session.json │
└─────────────────┘
```

#### Persistence Operations

**Load History** (`src/utils.rs:21-28`):
```rust
pub fn load_history() -> HashMap<String, HashMap<String, String>> {
    let path = get_history_path();
    if path.exists() {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap())
            .unwrap_or_default()
    } else {
        HashMap::new()
    }
}
```

**Save History** (`src/utils.rs:31-39`):
```rust
pub fn save_history(history: &HashMap<String, HashMap<String, String>>) {
    let path = get_history_path();
    let json = serde_json::to_string_pretty(history).unwrap();
    std::fs::write(path, json).ok();
}
```

**Critical Limitation**: Both operations are synchronous blocking I/O with minimal error handling.

---

## Database Integration Roadmap

### Overview

This section outlines a **7-phase implementation plan** to migrate from file-based persistence to SQLite-backed storage while maintaining backward compatibility with existing JSON files.

### Design Principles

1. **Backward Compatibility**: Existing JSON files must continue to work
2. **Graceful Degradation**: Database failures fall back to JSON silently
3. **Zero Configuration**: SQLite requires no external database server
4. **Atomic Migration**: JSON data migrates to database automatically on first run
5. **Feature Flag Support**: Database can be disabled at compile time

### Phase 1: Core Infrastructure

#### Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
rusqlite = { version = "0.31", features = ["bundled"] }

[features]
default = ["database"]
database = ["rusqlite"]
```

#### Database Schema

**File**: `src/db/schema.sql`

```sql
-- Workflow field history storage
CREATE TABLE IF NOT EXISTS workflow_history (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    workflow_id   TEXT NOT NULL,
    field_name    TEXT NOT NULL,
    field_value   TEXT NOT NULL,
    created_at    DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at    DATETIME DEFAULT CURRENT_TIMESTAMP,

    UNIQUE(workflow_id, field_name)
);

-- Index for fast lookups by workflow
CREATE INDEX IF NOT EXISTS idx_workflow_field
    ON workflow_history(workflow_id, field_name);

-- Session state storage
CREATE TABLE IF NOT EXISTS session_state (
    id            INTEGER PRIMARY KEY CHECK (id = 1), -- Singleton
    current_view  TEXT,
    selected_idx  INTEGER,
    active_tab    INTEGER,
    updated_at    DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Execution logs (optional, for future analytics)
CREATE TABLE IF NOT EXISTS execution_logs (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    workflow_id   TEXT NOT NULL,
    started_at    DATETIME NOT NULL,
    completed_at  DATETIME,
    exit_status   INTEGER,
    output_text   TEXT,

    INDEX idx_workflow_executions(workflow_id, started_at DESC)
);
```

#### Database Module Structure

**File**: `src/db/mod.rs`

```rust
use rusqlite::{Connection, Result};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Initialize database with schema
    pub fn new(path: PathBuf) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(include_str!("schema.sql"))?;
        Ok(Self { conn })
    }

    /// Load all workflow history
    pub fn load_history(&self) -> Result<HashMap<String, HashMap<String, String>>> {
        let mut stmt = self.conn.prepare(
            "SELECT workflow_id, field_name, field_value FROM workflow_history"
        )?;

        let mut history: HashMap<String, HashMap<String, String>> = HashMap::new();

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,  // workflow_id
                row.get::<_, String>(1)?,  // field_name
                row.get::<_, String>(2)?,  // field_value
            ))
        })?;

        for row in rows {
            let (workflow_id, field_name, field_value) = row?;
            history.entry(workflow_id)
                .or_insert_with(HashMap::new)
                .insert(field_name, field_value);
        }

        Ok(history)
    }

    /// Save workflow field value (upsert)
    pub fn save_field(
        &self,
        workflow_id: &str,
        field_name: &str,
        field_value: &str
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO workflow_history (workflow_id, field_name, field_value, updated_at)
             VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
             ON CONFLICT(workflow_id, field_name)
             DO UPDATE SET field_value = ?3, updated_at = CURRENT_TIMESTAMP",
            rusqlite::params![workflow_id, field_name, field_value],
        )?;
        Ok(())
    }

    /// Save entire history (batch operation)
    pub fn save_history(&self, history: &HashMap<String, HashMap<String, String>>) -> Result<()> {
        let tx = self.conn.transaction()?;

        for (workflow_id, fields) in history {
            for (field_name, field_value) in fields {
                tx.execute(
                    "INSERT INTO workflow_history (workflow_id, field_name, field_value, updated_at)
                     VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
                     ON CONFLICT(workflow_id, field_name)
                     DO UPDATE SET field_value = ?3, updated_at = CURRENT_TIMESTAMP",
                    rusqlite::params![workflow_id, field_name, field_value],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    /// Migrate JSON history to database
    pub fn migrate_from_json(&self, json_path: &PathBuf) -> Result<()> {
        if !json_path.exists() {
            return Ok(());
        }

        let json_data = std::fs::read_to_string(json_path)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        let history: HashMap<String, HashMap<String, String>> =
            serde_json::from_str(&json_data)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        self.save_history(&history)?;
        Ok(())
    }
}
```

#### Directory Setup

Create database in platform-specific location:

```rust
// src/db/mod.rs (continued)

use directories::ProjectDirs;

pub fn get_database_path() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "workflow", "manager") {
        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir).ok();
        data_dir.join("workflow-manager.db")
    } else {
        PathBuf::from(".workflow-manager.db")
    }
}
```

---

### Phase 2: Integration Points

#### Modification: `src/utils.rs`

**Current Implementation** (Lines 21-39):
```rust
pub fn load_history() -> HashMap<String, HashMap<String, String>> {
    let path = get_history_path();
    if path.exists() {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap())
            .unwrap_or_default()
    } else {
        HashMap::new()
    }
}

pub fn save_history(history: &HashMap<String, HashMap<String, String>>) {
    let path = get_history_path();
    let json = serde_json::to_string_pretty(history).unwrap();
    std::fs::write(path, json).ok();
}
```

**Proposed Replacement**:

```rust
use crate::db::{Database, get_database_path};

pub fn load_history() -> HashMap<String, HashMap<String, String>> {
    // Try database first
    #[cfg(feature = "database")]
    {
        let db_path = get_database_path();
        if let Ok(db) = Database::new(db_path.clone()) {
            if let Ok(history) = db.load_history() {
                if !history.is_empty() {
                    return history;
                }
            }

            // Auto-migrate JSON if database is empty
            let json_path = get_history_path();
            if json_path.exists() {
                if let Ok(_) = db.migrate_from_json(&json_path) {
                    eprintln!("Migrated history from JSON to database");
                    if let Ok(history) = db.load_history() {
                        return history;
                    }
                }
            }
        }
    }

    // Fallback to JSON
    let path = get_history_path();
    if path.exists() {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap())
            .unwrap_or_default()
    } else {
        HashMap::new()
    }
}

pub fn save_history(history: &HashMap<String, HashMap<String, String>>) {
    // Try database first
    #[cfg(feature = "database")]
    {
        let db_path = get_database_path();
        if let Ok(db) = Database::new(db_path) {
            if let Err(e) = db.save_history(history) {
                eprintln!("Database save failed: {}, falling back to JSON", e);
            } else {
                // Success - also save JSON as backup
                let path = get_history_path();
                let json = serde_json::to_string_pretty(history).unwrap();
                std::fs::write(path, json).ok();
                return;
            }
        }
    }

    // Fallback or no-database mode
    let path = get_history_path();
    let json = serde_json::to_string_pretty(history).unwrap();
    std::fs::write(path, json).ok();
}
```

#### Modification: `src/app/mod.rs`

**Current Initialization** (Line 28):
```rust
impl App {
    pub fn new() -> Self {
        let catalog = load_workflows();
        let runtime = Arc::new(ProcessBasedRuntime::new());
        let tokio_runtime = Arc::new(Runtime::new().unwrap());
        let chat_interface = ChatInterface::new(tokio_runtime.clone());

        // Load history from JSON
        let history = load_history();

        Self {
            catalog,
            history,
            runtime,
            tokio_runtime,
            chat_interface,
            // ... other fields
        }
    }
}
```

**Proposed Enhancement**:

```rust
impl App {
    pub fn new() -> Self {
        let catalog = load_workflows();
        let runtime = Arc::new(ProcessBasedRuntime::new());
        let tokio_runtime = Arc::new(Runtime::new().unwrap());
        let chat_interface = ChatInterface::new(tokio_runtime.clone());

        // Load history (auto-migrates from JSON to DB if needed)
        let history = load_history();

        // Log storage backend
        #[cfg(feature = "database")]
        eprintln!("Using SQLite database for persistence");

        #[cfg(not(feature = "database"))]
        eprintln!("Using JSON file persistence");

        Self {
            catalog,
            history,
            runtime,
            tokio_runtime,
            chat_interface,
            // ... other fields
        }
    }
}
```

#### Testing Integration

Ensure all workflow operations maintain compatibility:

| Operation | File | Test Scenario |
|-----------|------|--------------|
| **View Workflow** | `src/app/mod.rs` | Load history for field pre-population |
| **Edit Workflow** | `src/app/mod.rs` | Update field values in history |
| **Launch Workflow** | `src/app/mod.rs` | Save field values after execution |
| **Session Restore** | `src/app/mod.rs` | Restore UI state from session |

---

### Phase 3: Migration Strategy

#### Automatic Migration Flow

```
┌─────────────────────────┐
│  App::new() called      │
└───────────┬─────────────┘
            │
            ↓
┌─────────────────────────┐
│  load_history()         │
└───────────┬─────────────┘
            │
            ↓
      ┌─────────────┐
      │ DB exists?  │
      └──────┬──────┘
         No  │  Yes
    ┌────────┴────────┐
    ↓                 ↓
┌─────────┐    ┌──────────────┐
│ Create  │    │ Load from DB │
│ schema  │    └──────┬───────┘
└────┬────┘           │
     │                ↓
     │         ┌──────────────┐
     │         │ DB empty?    │
     │         └──────┬───────┘
     │            Yes │  No
     │         ┌──────┴──────┐
     │         ↓             ↓
     │    ┌──────────┐  ┌────────┐
     │    │JSON      │  │Return  │
     │    │exists?   │  │history │
     │    └────┬─────┘  └────────┘
     │      Yes│  No
     │    ┌────┴────┐
     │    ↓         ↓
     │  ┌─────┐  ┌────────┐
     └─→│Migrate│ │Return  │
        │to DB  │ │empty   │
        └───┬───┘ └────────┘
            │
            ↓
      ┌──────────┐
      │ Return   │
      │ history  │
      └──────────┘
```

#### Migration Safety Guarantees

1. **Non-Destructive**: Original JSON files are never deleted
2. **Idempotent**: Migration can run multiple times safely
3. **Atomic**: Uses SQLite transactions for all-or-nothing migration
4. **Validated**: Migration failures fall back to JSON without data loss

#### Manual Migration Tool

**File**: `src/bin/migrate_to_db.rs`

```rust
use workflow_manager::db::{Database, get_database_path};
use workflow_manager::utils::get_history_path;

fn main() {
    let json_path = get_history_path();
    let db_path = get_database_path();

    println!("JSON history: {:?}", json_path);
    println!("Database path: {:?}", db_path);

    if !json_path.exists() {
        println!("No JSON history to migrate");
        return;
    }

    let db = Database::new(db_path).expect("Failed to create database");

    match db.migrate_from_json(&json_path) {
        Ok(_) => {
            println!("✓ Migration successful");

            // Verify
            let history = db.load_history().expect("Failed to load history");
            println!("Migrated {} workflows", history.len());

            for (workflow_id, fields) in &history {
                println!("  - {}: {} fields", workflow_id, fields.len());
            }
        }
        Err(e) => {
            eprintln!("✗ Migration failed: {}", e);
            std::process::exit(1);
        }
    }
}
```

Add to `Cargo.toml`:

```toml
[[bin]]
name = "migrate-to-db"
path = "src/bin/migrate_to_db.rs"
required-features = ["database"]
```

#### Rollback Procedure

If database migration causes issues:

```bash
# 1. Disable database feature
cargo build --no-default-features

# 2. Or delete database file
rm ~/.local/share/workflow-manager/workflow-manager.db

# Application will automatically fall back to JSON
```

---

### Phase 4-7: Advanced Features

#### Phase 4: Async Database Operations

**Goal**: Move database I/O off the main thread to prevent UI blocking

```toml
[dependencies]
sqlx = { version = "0.7", features = ["runtime-tokio", "sqlite"] }
```

**Implementation**:
- Replace `rusqlite` with async `sqlx`
- Move database calls into Tokio tasks
- Use channels to communicate results back to main thread

#### Phase 5: Query Optimization

**Goal**: Improve performance for large history datasets

```sql
-- Add full-text search for field values
CREATE VIRTUAL TABLE workflow_history_fts USING fts5(
    workflow_id, field_name, field_value
);

-- Add prepared statement caching
-- Add connection pooling (if needed for async)
```

#### Phase 6: Execution Logging

**Goal**: Enable workflow analytics and debugging

```rust
impl Database {
    pub fn log_execution(
        &self,
        workflow_id: &str,
        started_at: DateTime<Utc>,
        output: &str,
        exit_status: i32,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO execution_logs
             (workflow_id, started_at, completed_at, output_text, exit_status)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                workflow_id,
                started_at.to_rfc3339(),
                Utc::now().to_rfc3339(),
                output,
                exit_status
            ],
        )?;
        Ok(())
    }
}
```

#### Phase 7: Multi-Device Sync (Optional)

**Goal**: Enable history synchronization across machines

**Approach 1**: SQLite with cloud storage
```rust
// Periodically sync database file to Dropbox/Google Drive
// Use file locking to prevent conflicts
```

**Approach 2**: Migrate to PostgreSQL
```toml
sqlx = { version = "0.7", features = ["postgres"] }
```

```rust
// Deploy centralized PostgreSQL instance
// Update connection string in config
```

---

## Hypothetical: Financial Data Implementation

**Important Note**: This section documents what would be required if the workflow-manager were repurposed for financial data storage. **The current application does not handle financial data and has no such functionality.**

### Prerequisites

If OHLCV (Open, High, Low, Close, Volume) candle data storage were added:

#### Required Dependencies

```toml
[dependencies]
# Database drivers
sqlx = { version = "0.7", features = ["runtime-tokio", "postgres", "chrono"] }
# OR for time-series optimized:
# influxdb = "0.7"
# questdb = "0.2"

# Decimal precision for financial data
rust_decimal = "1.33"
rust_decimal_macros = "1.33"

# Time handling
chrono = { version = "0.4", features = ["serde"] }
```

### Recommended Database Schema

#### PostgreSQL with TimescaleDB

**Why TimescaleDB?**
- Automatic partitioning by time
- Optimized compression for historical data
- PostgreSQL compatibility (ACID guarantees)
- Efficient time-based queries

**Schema**:

```sql
-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Main candle table
CREATE TABLE candles (
    symbol      TEXT           NOT NULL,
    timeframe   TEXT           NOT NULL,  -- '1m', '5m', '1h', '1d'
    timestamp   TIMESTAMPTZ    NOT NULL,
    open        DECIMAL(20,8)  NOT NULL,
    high        DECIMAL(20,8)  NOT NULL,
    low         DECIMAL(20,8)  NOT NULL,
    close       DECIMAL(20,8)  NOT NULL,
    volume      DECIMAL(20,8)  NOT NULL,

    -- Prevent duplicate candles
    UNIQUE(symbol, timeframe, timestamp)
);

-- Convert to hypertable (enables time-series optimizations)
SELECT create_hypertable('candles', 'timestamp');

-- Composite index for common queries
CREATE INDEX idx_candles_lookup
    ON candles(symbol, timeframe, timestamp DESC);

-- Enable compression for data older than 7 days
ALTER TABLE candles SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol,timeframe'
);

SELECT add_compression_policy('candles', INTERVAL '7 days');
```

#### Rust Data Model

```rust
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Candle {
    pub symbol: String,
    pub timeframe: String,
    pub timestamp: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
}

impl Candle {
    /// Validate OHLC relationships
    pub fn is_valid(&self) -> bool {
        self.high >= self.open
            && self.high >= self.close
            && self.high >= self.low
            && self.low <= self.open
            && self.low <= self.close
            && self.volume >= Decimal::ZERO
    }
}
```

#### Database Operations

```rust
use sqlx::{PgPool, postgres::PgPoolOptions};

pub struct CandleStore {
    pool: PgPool,
}

impl CandleStore {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    /// Insert candle (upsert to handle duplicates)
    pub async fn insert_candle(&self, candle: &Candle) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO candles
             (symbol, timeframe, timestamp, open, high, low, close, volume)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (symbol, timeframe, timestamp)
             DO UPDATE SET
                open = EXCLUDED.open,
                high = EXCLUDED.high,
                low = EXCLUDED.low,
                close = EXCLUDED.close,
                volume = EXCLUDED.volume"
        )
        .bind(&candle.symbol)
        .bind(&candle.timeframe)
        .bind(candle.timestamp)
        .bind(candle.open)
        .bind(candle.high)
        .bind(candle.low)
        .bind(candle.close)
        .bind(candle.volume)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Fetch candles for time range
    pub async fn get_candles(
        &self,
        symbol: &str,
        timeframe: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Candle>, sqlx::Error> {
        sqlx::query_as::<_, Candle>(
            "SELECT * FROM candles
             WHERE symbol = $1
               AND timeframe = $2
               AND timestamp >= $3
               AND timestamp < $4
             ORDER BY timestamp ASC"
        )
        .bind(symbol)
        .bind(timeframe)
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
    }

    /// Get latest candle
    pub async fn get_latest(
        &self,
        symbol: &str,
        timeframe: &str,
    ) -> Result<Option<Candle>, sqlx::Error> {
        sqlx::query_as::<_, Candle>(
            "SELECT * FROM candles
             WHERE symbol = $1 AND timeframe = $2
             ORDER BY timestamp DESC
             LIMIT 1"
        )
        .bind(symbol)
        .bind(timeframe)
        .fetch_optional(&self.pool)
        .await
    }
}
```

### Database Technology Selection

#### Comparison Matrix

| Database | Use Case | Pros | Cons | Recommended For |
|----------|----------|------|------|-----------------|
| **SQLite** | Embedded, single-user | Zero config, portable, ACID | No concurrency, limited scale | Development, personal tools |
| **PostgreSQL + TimescaleDB** | Production multi-user | Scalability, time-series optimization, compression | Requires server setup | Production applications |
| **InfluxDB** | High-frequency trading | Purpose-built for time-series, fast writes | No ACID, eventual consistency | Real-time data ingestion |
| **QuestDB** | Analytics, backtesting | Blazing fast queries, SQL compatibility | Newer project, smaller ecosystem | Performance-critical analytics |

#### Decision Tree

```
Do you need multi-user access?
├─ No → SQLite with appropriate indices
│        - Fast queries with proper indexing
│        - File-based, zero configuration
│        - Limited to ~100k writes/sec
│
└─ Yes → Do you need strict ACID guarantees?
         ├─ Yes → PostgreSQL + TimescaleDB
         │         - Battle-tested reliability
         │         - Automatic partitioning
         │         - Compression for historical data
         │
         └─ No → Is write throughput critical?
                  ├─ Yes (>1M inserts/sec) → QuestDB or InfluxDB
                  │                           - Optimized for time-series
                  │                           - Sacrifices ACID for speed
                  │
                  └─ No → PostgreSQL + TimescaleDB
                           - Best balance of features
```

### Integration with Workflow Manager

If financial data were integrated into the current architecture:

**Modified Architecture**:

```rust
// src/app/mod.rs
pub struct App {
    // Existing fields...
    catalog: Vec<Workflow>,
    history: HashMap<String, HashMap<String, String>>,

    // New field
    #[cfg(feature = "financial-data")]
    candle_store: Arc<CandleStore>,
}

impl App {
    pub fn new() -> Self {
        // Existing initialization...

        #[cfg(feature = "financial-data")]
        let candle_store = {
            let db_url = std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://candles.db".to_string());
            Arc::new(
                tokio_runtime.block_on(CandleStore::new(&db_url))
                    .expect("Failed to connect to candle database")
            )
        };

        Self {
            // Existing fields...
            #[cfg(feature = "financial-data")]
            candle_store,
        }
    }
}
```

**Feature Flag** (`Cargo.toml`):

```toml
[features]
default = ["database"]
database = ["rusqlite"]
financial-data = ["database", "sqlx", "rust_decimal"]
```

---

## Future Development: Codebase Search

### Separate Project Architecture

Per `PLAN.md`, a **distinct codebase search system** is planned with the following architecture:

#### Planned Technology Stack

| Component | Technology | Purpose | Status |
|-----------|-----------|---------|--------|
| **Vector Search** | Qdrant | Semantic code similarity search | Planned |
| **Metadata Store** | RocksDB | Fast key-value lookups for file metadata | Planned |
| **Full-Text Search** | Tantivy | Token-based code search with ranking | Planned |
| **Embeddings** | Claude API or local model | Convert code to vectors | Planned |

#### Architectural Separation

**Critical Point**: This is a **separate project**, not an extension of the workflow-manager.

```
Projects:
├── workflow-manager/          # Current TUI application
│   ├── SQLite persistence
│   └── Workflow orchestration
│
└── codebase-search/           # Future project (not implemented)
    ├── Qdrant vector DB
    ├── RocksDB metadata
    └── Tantivy indexing
```

#### Planned Integration Points

**Possible Future Integration**:

```rust
// Hypothetical future code
#[cfg(feature = "codebase-search")]
pub struct CodeSearchInterface {
    qdrant_client: QdrantClient,
    tantivy_index: TantivyIndex,
    metadata_store: RocksDB,
}

impl App {
    #[cfg(feature = "codebase-search")]
    pub async fn search_codebase(&self, query: &str) -> Vec<CodeResult> {
        // Vector search for semantic similarity
        let vector_results = self.code_search
            .semantic_search(query)
            .await;

        // Full-text search for exact matches
        let text_results = self.code_search
            .fulltext_search(query)
            .await;

        // Merge and rank results
        merge_results(vector_results, text_results)
    }
}
```

#### Technology Rationale

**Why Qdrant?**
- High-performance vector similarity search
- Rust client library available
- Supports filtering by metadata (file type, language)

**Why RocksDB?**
- Embedded key-value store (like SQLite for key-value)
- Excellent read performance
- Used by production systems (e.g., CockroachDB)

**Why Tantivy?**
- Pure Rust implementation
- Fast full-text search
- BM25 ranking algorithm
- Similar to Lucene but Rust-native

#### Development Roadmap

1. **Research Phase** (Current)
   - Evaluate vector models for code embeddings
   - Benchmark Qdrant vs alternatives (Milvus, Weaviate)
   - Prototype Tantivy indexing

2. **MVP Implementation**
   - Index local codebase with Tantivy
   - Generate embeddings for functions/classes
   - Store in Qdrant with metadata

3. **Integration**
   - Create workflow-manager plugin interface
   - Add search view to TUI
   - Implement result ranking

4. **Production Hardening**
   - Incremental indexing for large repos
   - Optimize query latency (<100ms)
   - Add caching layer

---

## Recommendations

### Immediate Actions (Next 2 Weeks)

#### Priority 1: Database Integration Foundation

1. **Add SQLite dependency**
   ```bash
   cargo add rusqlite --features bundled
   ```

2. **Implement Phase 1**: Core Infrastructure
   - Create `src/db/mod.rs` with schema
   - Implement `Database` struct with CRUD operations
   - Write unit tests for database operations

3. **Implement Phase 2**: Integration
   - Modify `src/utils.rs` load/save functions
   - Update `src/app/mod.rs` initialization
   - Test with existing workflows

4. **Validate Migration**
   - Run migration tool on real data
   - Verify JSON fallback works
   - Confirm no data loss scenarios

**Success Criteria**:
- All existing workflows load field values from database
- JSON files continue to work as backup
- No user-facing changes required

#### Priority 2: Documentation & Testing

1. **Update README.md**
   - Document new database storage location
   - Explain migration process
   - Add troubleshooting section

2. **Add Integration Tests**
   ```rust
   #[test]
   fn test_database_migration() {
       // Create JSON history
       // Run migration
       // Verify DB contents
       // Test JSON fallback
   }
   ```

3. **Create Migration Guide**
   - Document rollback procedure
   - Explain feature flags
   - Provide backup/restore instructions

### Short-Term Goals (1-2 Months)

#### Async Database Operations (Phase 4)

**Goal**: Prevent UI blocking during database I/O

**Implementation Plan**:
1. Migrate from `rusqlite` to `sqlx`
2. Move database calls to Tokio tasks
3. Use message passing for result communication
4. Benchmark latency improvements

**Expected Outcome**: UI remains responsive even with large history

#### Execution Logging (Phase 6)

**Goal**: Enable workflow analytics

**Use Cases**:
- Debug workflow failures
- Track execution time trends
- Identify frequently used workflows

**Implementation**:
```rust
// In src/runtime.rs after workflow completion
self.app.db.log_execution(
    workflow_id,
    started_at,
    &output,
    exit_status,
).await?;
```

### Long-Term Vision (3-6 Months)

#### Codebase Search Project

**Goal**: Build separate codebase search tool

**Milestones**:

| Month | Milestone | Deliverable |
|-------|-----------|-------------|
| **Month 1** | Research & Prototyping | Evaluate Qdrant, Tantivy, embedding models |
| **Month 2** | MVP Implementation | Basic indexing + search working locally |
| **Month 3** | Integration | Plugin system for workflow-manager |
| **Month 4** | UI Development | Search view in TUI with result ranking |
| **Month 5** | Performance Optimization | Sub-100ms query latency |
| **Month 6** | Production Release | Documentation, packaging, CI/CD |

#### Multi-Device Sync (Optional)

**Goal**: Synchronize workflow history across machines

**Approach Options**:

1. **Option A: Cloud-Synced SQLite**
   - Sync database file via Dropbox/Google Drive
   - Use file locking to prevent corruption
   - **Pros**: Simple, uses existing storage
   - **Cons**: Potential sync conflicts

2. **Option B: Centralized PostgreSQL**
   - Deploy PostgreSQL instance (cloud or self-hosted)
   - Update connection string in config
   - **Pros**: True multi-user support
   - **Cons**: Requires server infrastructure

**Recommendation**: Start with Option A, migrate to Option B if multi-user demand exists

### Best Practices

#### Database Management

1. **Always use transactions** for multi-row operations
2. **Prepare statements** for repeated queries
3. **Add indices** before performance becomes an issue
4. **Monitor database size** and add cleanup policies
5. **Test migration** on copy of production data

#### Error Handling

```rust
// Good: Graceful degradation
match db.save_history(&history) {
    Ok(_) => eprintln!("Saved to database"),
    Err(e) => {
        eprintln!("Database save failed: {}, using JSON", e);
        save_to_json(&history);
    }
}

// Bad: Panic on DB errors
db.save_history(&history).unwrap(); // ❌ Never do this
```

#### Feature Flags

```toml
# Allow users to disable database
[features]
default = ["database"]
database = ["rusqlite"]

# Disable with:
# cargo build --no-default-features
```

#### Version Management

```sql
-- Add schema version table
CREATE TABLE schema_version (
    version INTEGER PRIMARY KEY,
    applied_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Track migrations
INSERT INTO schema_version (version) VALUES (1);
```

---

## Appendix

### A. File Reference Guide

#### Current Codebase Files

| File Path | Purpose | Lines | Key Functions |
|-----------|---------|-------|---------------|
| `src/main.rs` | Entry point, event loop | ~200 | `main()`, event handling |
| `src/app/mod.rs` | Application state | ~300 | `App::new()`, `update()`, `render()` |
| `src/discovery.rs` | Workflow scanning | ~150 | `load_workflows()` |
| `src/runtime.rs` | Process execution | ~400 | `ProcessBasedRuntime::execute()` |
| `src/chat.rs` | Claude AI integration | ~250 | `ChatInterface::send_message()` |
| `src/utils.rs` | History persistence | ~50 | `load_history()`, `save_history()` |
| `src/ui/` | TUI rendering | ~600 | Various render functions |

#### Planned Database Files

| File Path | Purpose | Status |
|-----------|---------|--------|
| `src/db/mod.rs` | Database abstraction layer | To be created |
| `src/db/schema.sql` | SQLite schema | To be created |
| `src/bin/migrate_to_db.rs` | Migration tool | To be created |

### B. SQL Query Reference

#### Common Queries

**Get all fields for workflow**:
```sql
SELECT field_name, field_value
FROM workflow_history
WHERE workflow_id = ?;
```

**Update field value**:
```sql
INSERT INTO workflow_history (workflow_id, field_name, field_value, updated_at)
VALUES (?, ?, ?, CURRENT_TIMESTAMP)
ON CONFLICT(workflow_id, field_name)
DO UPDATE SET field_value = ?, updated_at = CURRENT_TIMESTAMP;
```

**Get recently modified workflows**:
```sql
SELECT DISTINCT workflow_id, MAX(updated_at) as last_updated
FROM workflow_history
GROUP BY workflow_id
ORDER BY last_updated DESC
LIMIT 10;
```

**Execution analytics**:
```sql
SELECT
    workflow_id,
    COUNT(*) as execution_count,
    AVG(julianday(completed_at) - julianday(started_at)) * 86400 as avg_duration_seconds
FROM execution_logs
WHERE completed_at IS NOT NULL
GROUP BY workflow_id
ORDER BY execution_count DESC;
```

### C. Testing Checklist

#### Pre-Deployment Tests

- [ ] Load existing JSON history successfully
- [ ] Migrate JSON to database without data loss
- [ ] Save new workflow field values to database
- [ ] Database failures fall back to JSON gracefully
- [ ] Rollback to JSON-only mode works
- [ ] Database file created in correct platform directory
- [ ] Concurrent access doesn't corrupt data
- [ ] Large history files (>1000 workflows) load quickly
- [ ] Session restore works after database migration
- [ ] All workflows execute correctly with database backend

### D. Troubleshooting Guide

#### Common Issues

**Issue**: Database file not created
```bash
# Check permissions
ls -la ~/.local/share/workflow-manager/

# Verify directory exists
mkdir -p ~/.local/share/workflow-manager/

# Test database creation
sqlite3 ~/.local/share/workflow-manager/workflow-manager.db "SELECT 1;"
```

**Issue**: Migration fails with "locked database"
```bash
# Another process has DB open - close it
lsof ~/.local/share/workflow-manager/workflow-manager.db

# Or use fallback JSON
cargo run --no-default-features
```

**Issue**: Data inconsistency between JSON and DB
```bash
# Force reload from JSON
rm ~/.local/share/workflow-manager/workflow-manager.db
cargo run  # Auto-migrates from JSON
```

### E. Performance Benchmarks

#### Expected Performance Targets

| Operation | Target Latency | Measurement |
|-----------|----------------|-------------|
| Load history (100 workflows) | <50ms | From DB open to HashMap ready |
| Save single field | <10ms | Single upsert transaction |
| Batch save (100 fields) | <100ms | Using transaction |
| Migration (1000 workflows) | <500ms | JSON to DB first run |
| Query recent workflows | <20ms | Top 10 by updated_at |

#### Benchmarking Code

```rust
use std::time::Instant;

fn benchmark_load_history() {
    let start = Instant::now();
    let history = load_history();
    let duration = start.elapsed();

    println!("Loaded {} workflows in {:?}", history.len(), duration);
}
```

### F. Related Documentation

- [Ratatui Documentation](https://ratatui.rs/)
- [SQLite Documentation](https://www.sqlite.org/docs.html)
- [rusqlite Guide](https://docs.rs/rusqlite/)
- [TimescaleDB Best Practices](https://docs.timescale.com/timescaledb/latest/best-practices/)
- [Qdrant Documentation](https://qdrant.tech/documentation/)
- [Tantivy Guide](https://github.com/quickwit-oss/tantivy)

### G. Glossary

| Term | Definition |
|------|------------|
| **OHLCV** | Open, High, Low, Close, Volume - standard candle data format |
| **Hypertable** | TimescaleDB's time-series optimized table structure |
| **Upsert** | Insert or update operation (INSERT ... ON CONFLICT UPDATE) |
| **Vector Search** | Semantic search using embedding similarity (cosine/dot product) |
| **Process-based Runtime** | Workflow execution via child processes (vs in-process) |
| **TUI** | Terminal User Interface - text-based GUI in terminal |
| **Arc<Mutex<>>** | Atomic reference counted mutex for thread-safe shared state |

---

## Document Version History

| Version | Date | Changes | Author |
|---------|------|---------|--------|
| 1.0 | 2025-10-19 | Initial comprehensive documentation | Technical Writer |

---

**End of Documentation**