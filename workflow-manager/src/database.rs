//! SQLite database for persistent workflow execution history
//!
//! This module provides persistent storage for workflow executions, logs, and parameters.
//! Data persists across app restarts and enables historical queries via MCP tools.
//!
//! # Features
//!
//! - **Persistent Storage**: All workflow executions stored in SQLite database
//! - **Full History**: Logs, parameters, and metadata for every execution
//! - **Efficient Queries**: Indexed queries for fast retrieval by workflow ID, status, or time
//! - **Batch Operations**: Optimized batch inserts for high-frequency logging
//! - **Cleanup Tools**: Delete old executions to manage disk space
//! - **Statistics**: Get success/failure rates and execution counts per workflow
//!
//! # Database Schema
//!
//! The database consists of 4 tables:
//!
//! 1. **executions** - Core execution metadata (workflow ID, status, timestamps, exit code)
//! 2. **execution_params** - Input parameters used for each execution
//! 3. **execution_logs** - Structured logs (PhaseStarted, TaskProgress, RawOutput, etc.)
//! 4. **schema_version** - Database schema version for migrations
//!
//! # Example Usage
//!
//! ```rust,no_run
//! use workflow_manager::database::{Database, PersistedExecution};
//! use workflow_manager_sdk::{WorkflowStatus, WorkflowLog};
//! use std::path::PathBuf;
//! use std::collections::HashMap;
//! use chrono::Local;
//! use uuid::Uuid;
//!
//! # fn main() -> anyhow::Result<()> {
//! // Initialize database
//! let db_path = dirs::home_dir()
//!     .unwrap()
//!     .join(".workflow-manager")
//!     .join("executions.db");
//! let db = Database::new(db_path)?;
//! db.initialize_schema()?;
//!
//! // Create and store an execution
//! let exec_id = Uuid::new_v4();
//! let exec = PersistedExecution {
//!     id: exec_id,
//!     workflow_id: "web-search".to_string(),
//!     workflow_name: "Web Search".to_string(),
//!     status: WorkflowStatus::Running,
//!     start_time: Local::now(),
//!     end_time: None,
//!     exit_code: None,
//!     binary_path: PathBuf::from("/usr/local/bin/web-search"),
//!     created_at: Local::now(),
//!     updated_at: Local::now(),
//! };
//! db.insert_execution(&exec)?;
//!
//! // Store execution parameters
//! let mut params = HashMap::new();
//! params.insert("query".to_string(), "rust async programming".to_string());
//! db.insert_params(&exec_id, &params)?;
//!
//! // Store logs as they arrive
//! let log = WorkflowLog::PhaseStarted {
//!     phase: 1,
//!     name: "Search Phase".to_string(),
//!     total_phases: 3,
//! };
//! db.insert_log(&exec_id, 0, &log)?;
//!
//! // Update execution when complete
//! db.update_execution(
//!     &exec_id,
//!     WorkflowStatus::Completed,
//!     Some(Local::now()),
//!     Some(0)
//! )?;
//!
//! // Query historical data
//! let executions = db.list_executions(10, 0, None)?;
//! let logs = db.get_logs(&exec_id, Some(100))?;
//! let params = db.get_params(&exec_id)?;
//!
//! // Get statistics
//! let stats = db.get_workflow_stats("web-search")?;
//! println!("Total: {}, Completed: {}, Failed: {}",
//!     stats.total, stats.completed, stats.failed);
//! # Ok(())
//! # }
//! ```
//!
//! # Performance Considerations
//!
//! - Use `batch_insert_logs()` for high-frequency logging (50+ logs/second)
//! - Limit queries with pagination to avoid loading large result sets
//! - Run `delete_executions_before()` periodically to manage database size
//! - WAL mode is enabled by default for better concurrent access

use anyhow::{anyhow, Result};
use chrono::{DateTime, Local};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;
use workflow_manager_sdk::{WorkflowLog, WorkflowStatus};

/// Database wrapper for workflow execution persistence
pub struct Database {
    conn: Connection,
}

/// Serializable execution data for database storage
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
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

/// Log entry with metadata for database storage
#[derive(Debug, Clone)]
pub struct PersistedLog {
    pub execution_id: Uuid,
    pub sequence: usize,
    pub timestamp: DateTime<Local>,
    pub log_type: String,
    pub log_data: WorkflowLog,
}

impl Database {
    /// Create a new database connection at the specified path
    pub fn new(path: PathBuf) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrent access
        conn.pragma_update(None, "journal_mode", "WAL")?;

        // Enable foreign key constraints
        conn.pragma_update(None, "foreign_keys", "ON")?;

        Ok(Self { conn })
    }

    /// Create an in-memory database (for testing)
    #[cfg(test)]
    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self { conn })
    }

    /// Initialize database schema with all tables and indexes
    pub fn initialize_schema(&self) -> Result<()> {
        // Create executions table
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS executions (
                -- Primary key
                id TEXT PRIMARY KEY,

                -- Workflow info
                workflow_id TEXT NOT NULL,
                workflow_name TEXT,

                -- Execution lifecycle
                status TEXT NOT NULL,
                start_time TEXT NOT NULL,
                end_time TEXT,

                -- Results
                exit_code INTEGER,

                -- Metadata
                binary_path TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )?;

        // Create indexes for executions
        self.conn.execute_batch(
            r#"
            CREATE INDEX IF NOT EXISTS idx_executions_workflow_id ON executions(workflow_id);
            CREATE INDEX IF NOT EXISTS idx_executions_status ON executions(status);
            CREATE INDEX IF NOT EXISTS idx_executions_start_time ON executions(start_time DESC);
            "#,
        )?;

        // Create execution_params table
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS execution_params (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                execution_id TEXT NOT NULL,
                param_name TEXT NOT NULL,
                param_value TEXT NOT NULL,

                FOREIGN KEY(execution_id) REFERENCES executions(id) ON DELETE CASCADE,
                UNIQUE(execution_id, param_name)
            );
            "#,
        )?;

        // Create index for execution_params
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_params_execution_id ON execution_params(execution_id)",
            [],
        )?;

        // Create execution_logs table
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS execution_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                execution_id TEXT NOT NULL,
                sequence INTEGER NOT NULL,
                timestamp TEXT NOT NULL,
                log_type TEXT NOT NULL,
                log_data TEXT NOT NULL,

                FOREIGN KEY(execution_id) REFERENCES executions(id) ON DELETE CASCADE
            );
            "#,
        )?;

        // Create index for execution_logs
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_logs_execution_id ON execution_logs(execution_id, sequence)",
            [],
        )?;

        // Create schema_version table
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )?;

        // Insert initial schema version
        self.conn.execute(
            "INSERT OR IGNORE INTO schema_version (version) VALUES (1)",
            [],
        )?;

        // Run migrations
        self.migrate_to_v2()?;

        Ok(())
    }

    /// Migrate database schema to version 2 (chat input history)
    pub fn migrate_to_v2(&self) -> Result<()> {
        let current = self.get_schema_version()?;

        if current < 2 {
            self.conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS chat_input_history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    message TEXT NOT NULL,
                    timestamp TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );

                CREATE INDEX IF NOT EXISTS idx_chat_history_timestamp
                ON chat_input_history(timestamp DESC);

                UPDATE schema_version SET version = 2;
                "#,
            )?;
        }

        Ok(())
    }

    /// Get current schema version
    pub fn get_schema_version(&self) -> Result<i32> {
        let version: i32 = self
            .conn
            .query_row(
                "SELECT MAX(version) FROM schema_version",
                [],
                |row| row.get(0),
            )?;
        Ok(version)
    }

    /// Set schema version (for migrations)
    pub fn set_schema_version(&self, version: i32) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO schema_version (version) VALUES (?1)",
            params![version],
        )?;
        Ok(())
    }

    /// Insert a new execution record
    pub fn insert_execution(&self, exec: &PersistedExecution) -> Result<()> {
        let status_str = status_to_string(&exec.status);
        let start_time_str = exec.start_time.to_rfc3339();
        let end_time_str = exec.end_time.map(|dt| dt.to_rfc3339());
        let binary_path_str = exec.binary_path.to_string_lossy().to_string();

        self.conn.execute(
            r#"
            INSERT INTO executions (
                id, workflow_id, workflow_name, status, start_time, end_time,
                exit_code, binary_path, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                exec.id.to_string(),
                exec.workflow_id,
                exec.workflow_name,
                status_str,
                start_time_str,
                end_time_str,
                exec.exit_code,
                binary_path_str,
                exec.created_at.to_rfc3339(),
                exec.updated_at.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    /// Update execution status, end time, and exit code
    pub fn update_execution(
        &self,
        id: &Uuid,
        status: WorkflowStatus,
        end_time: Option<DateTime<Local>>,
        exit_code: Option<i32>,
    ) -> Result<()> {
        let status_str = status_to_string(&status);
        let end_time_str = end_time.map(|dt| dt.to_rfc3339());
        let updated_at = Local::now().to_rfc3339();

        self.conn.execute(
            r#"
            UPDATE executions
            SET status = ?1, end_time = ?2, exit_code = ?3, updated_at = ?4
            WHERE id = ?5
            "#,
            params![status_str, end_time_str, exit_code, updated_at, id.to_string()],
        )?;

        Ok(())
    }

    /// Get a single execution by ID
    pub fn get_execution(&self, id: &Uuid) -> Result<Option<PersistedExecution>> {
        let result = self
            .conn
            .query_row(
                r#"
                SELECT id, workflow_id, workflow_name, status, start_time, end_time,
                       exit_code, binary_path, created_at, updated_at
                FROM executions
                WHERE id = ?1
                "#,
                params![id.to_string()],
                map_execution_row,
            )
            .optional()?;

        Ok(result)
    }

    /// List executions with pagination and optional workflow filter
    pub fn list_executions(
        &self,
        limit: usize,
        offset: usize,
        workflow_id: Option<&str>,
    ) -> Result<Vec<PersistedExecution>> {
        let query = if workflow_id.is_some() {
            format!(
                r#"
                SELECT id, workflow_id, workflow_name, status, start_time, end_time,
                       exit_code, binary_path, created_at, updated_at
                FROM executions
                WHERE workflow_id = ?1
                ORDER BY start_time DESC
                LIMIT ?2 OFFSET ?3
                "#
            )
        } else {
            format!(
                r#"
                SELECT id, workflow_id, workflow_name, status, start_time, end_time,
                       exit_code, binary_path, created_at, updated_at
                FROM executions
                ORDER BY start_time DESC
                LIMIT ?1 OFFSET ?2
                "#
            )
        };

        let mut stmt = self.conn.prepare(&query)?;

        let executions = if let Some(wf_id) = workflow_id {
            stmt.query_map(params![wf_id, limit, offset], map_execution_row)?
                .collect::<Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![limit, offset], map_execution_row)?
                .collect::<Result<Vec<_>, _>>()?
        };

        Ok(executions)
    }

    /// Insert a log entry
    pub fn insert_log(&self, exec_id: &Uuid, sequence: usize, log: &WorkflowLog) -> Result<()> {
        let timestamp = Local::now().to_rfc3339();
        let log_type = log_type_from_log(log);
        let log_data = serde_json::to_string(log)?;

        self.conn.execute(
            r#"
            INSERT INTO execution_logs (execution_id, sequence, timestamp, log_type, log_data)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![exec_id.to_string(), sequence, timestamp, log_type, log_data],
        )?;

        Ok(())
    }

    /// Batch insert multiple logs (more efficient for bulk operations)
    pub fn batch_insert_logs(&self, exec_id: &Uuid, logs: &[(usize, WorkflowLog)]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO execution_logs (execution_id, sequence, timestamp, log_type, log_data)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
            )?;

            for (sequence, log) in logs {
                let timestamp = Local::now().to_rfc3339();
                let log_type = log_type_from_log(log);
                let log_data = serde_json::to_string(log)?;

                stmt.execute(params![
                    exec_id.to_string(),
                    sequence,
                    timestamp,
                    log_type,
                    log_data
                ])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    /// Get logs for a specific execution
    pub fn get_logs(&self, exec_id: &Uuid, limit: Option<usize>) -> Result<Vec<WorkflowLog>> {
        let query = if let Some(limit) = limit {
            format!(
                r#"
                SELECT log_data
                FROM execution_logs
                WHERE execution_id = ?1
                ORDER BY sequence ASC
                LIMIT {}
                "#,
                limit
            )
        } else {
            r#"
            SELECT log_data
            FROM execution_logs
            WHERE execution_id = ?1
            ORDER BY sequence ASC
            "#
            .to_string()
        };

        let mut stmt = self.conn.prepare(&query)?;
        let logs = stmt
            .query_map(params![exec_id.to_string()], |row| {
                let log_data: String = row.get(0)?;
                Ok(log_data)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Deserialize all logs
        let parsed_logs: Result<Vec<WorkflowLog>> = logs
            .into_iter()
            .map(|data| serde_json::from_str(&data).map_err(|e| anyhow!("Failed to parse log: {}", e)))
            .collect();

        parsed_logs
    }

    /// Get log count for an execution
    pub fn get_log_count(&self, exec_id: &Uuid) -> Result<usize> {
        let count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM execution_logs WHERE execution_id = ?1",
            params![exec_id.to_string()],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Insert execution parameters
    pub fn insert_params(&self, exec_id: &Uuid, params: &HashMap<String, String>) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO execution_params (execution_id, param_name, param_value)
                VALUES (?1, ?2, ?3)
                "#,
            )?;

            for (name, value) in params {
                stmt.execute(params![exec_id.to_string(), name, value])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    /// Get execution parameters
    pub fn get_params(&self, exec_id: &Uuid) -> Result<HashMap<String, String>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT param_name, param_value
            FROM execution_params
            WHERE execution_id = ?1
            "#,
        )?;

        let params_iter = stmt.query_map(params![exec_id.to_string()], |row| {
            let name: String = row.get(0)?;
            let value: String = row.get(1)?;
            Ok((name, value))
        })?;

        let params: HashMap<String, String> = params_iter.collect::<Result<_, _>>()?;
        Ok(params)
    }

    /// Delete executions older than the specified date
    pub fn delete_executions_before(&self, cutoff: DateTime<Local>) -> Result<usize> {
        let cutoff_str = cutoff.to_rfc3339();
        let deleted = self.conn.execute(
            "DELETE FROM executions WHERE start_time < ?1",
            params![cutoff_str],
        )?;
        Ok(deleted)
    }

    /// Delete a specific execution by ID
    pub fn delete_execution(&self, id: &Uuid) -> Result<()> {
        self.conn.execute(
            "DELETE FROM executions WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(())
    }

    /// Get execution statistics by workflow
    pub fn get_workflow_stats(&self, workflow_id: &str) -> Result<WorkflowStats> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                COUNT(*) as total,
                SUM(CASE WHEN status = 'Completed' THEN 1 ELSE 0 END) as completed,
                SUM(CASE WHEN status = 'Failed' THEN 1 ELSE 0 END) as failed,
                SUM(CASE WHEN status = 'Running' THEN 1 ELSE 0 END) as running
            FROM executions
            WHERE workflow_id = ?1
            "#,
        )?;

        let stats = stmt.query_row(params![workflow_id], |row| {
            Ok(WorkflowStats {
                total: row.get(0)?,
                completed: row.get(1)?,
                failed: row.get(2)?,
                running: row.get(3)?,
            })
        })?;

        Ok(stats)
    }

    /// Get all executions with a specific status
    pub fn get_executions_by_status(&self, status: WorkflowStatus) -> Result<Vec<PersistedExecution>> {
        let status_str = status_to_string(&status);
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, workflow_id, workflow_name, status, start_time, end_time,
                   exit_code, binary_path, created_at, updated_at
            FROM executions
            WHERE status = ?1
            ORDER BY start_time DESC
            "#,
        )?;

        let executions = stmt
            .query_map(params![status_str], map_execution_row)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(executions)
    }

    /// Insert a chat input message
    pub fn insert_chat_message(&self, message: &str) -> Result<i64> {
        let timestamp = Local::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO chat_input_history (message, timestamp) VALUES (?1, ?2)",
            params![message, timestamp],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Get recent chat history (returns in chronological order - oldest first)
    pub fn get_chat_history(&self, limit: usize) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT message FROM chat_input_history
             ORDER BY timestamp DESC
             LIMIT ?1"
        )?;

        let messages = stmt
            .query_map([limit], |row| row.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?;

        // Reverse to get chronological order (oldest first)
        Ok(messages.into_iter().rev().collect())
    }

    /// Delete old chat history (older than specified days)
    pub fn cleanup_old_chat_history(&self, days: i64) -> Result<usize> {
        use chrono::Duration;
        let cutoff = Local::now() - Duration::days(days);
        let cutoff_str = cutoff.to_rfc3339();

        let deleted = self.conn.execute(
            "DELETE FROM chat_input_history WHERE timestamp < ?1",
            params![cutoff_str],
        )?;

        Ok(deleted)
    }
}

/// Statistics for a workflow
#[derive(Debug, Clone)]
pub struct WorkflowStats {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub running: usize,
}

// Helper functions for mapping between database and Rust types

/// Convert WorkflowStatus to database string
fn status_to_string(status: &WorkflowStatus) -> &'static str {
    match status {
        WorkflowStatus::NotStarted => "NotStarted",
        WorkflowStatus::Running => "Running",
        WorkflowStatus::Completed => "Completed",
        WorkflowStatus::Failed => "Failed",
    }
}

/// Convert database string to WorkflowStatus
fn string_to_status(s: &str) -> Result<WorkflowStatus> {
    match s {
        "NotStarted" => Ok(WorkflowStatus::NotStarted),
        "Running" => Ok(WorkflowStatus::Running),
        "Completed" => Ok(WorkflowStatus::Completed),
        "Failed" => Ok(WorkflowStatus::Failed),
        _ => Err(anyhow!("Unknown workflow status: {}", s)),
    }
}

/// Extract log type from WorkflowLog for indexing
fn log_type_from_log(log: &WorkflowLog) -> String {
    match log {
        WorkflowLog::PhaseStarted { .. } => "PhaseStarted",
        WorkflowLog::PhaseCompleted { .. } => "PhaseCompleted",
        WorkflowLog::PhaseFailed { .. } => "PhaseFailed",
        WorkflowLog::TaskStarted { .. } => "TaskStarted",
        WorkflowLog::TaskProgress { .. } => "TaskProgress",
        WorkflowLog::TaskCompleted { .. } => "TaskCompleted",
        WorkflowLog::TaskFailed { .. } => "TaskFailed",
        WorkflowLog::AgentStarted { .. } => "AgentStarted",
        WorkflowLog::AgentMessage { .. } => "AgentMessage",
        WorkflowLog::AgentCompleted { .. } => "AgentCompleted",
        WorkflowLog::AgentFailed { .. } => "AgentFailed",
        WorkflowLog::StateFileCreated { .. } => "StateFileCreated",
        WorkflowLog::RawOutput { .. } => "RawOutput",
    }
    .to_string()
}

/// Map a database row to PersistedExecution
fn map_execution_row(row: &Row) -> rusqlite::Result<PersistedExecution> {
    let id_str: String = row.get(0)?;
    let workflow_id: String = row.get(1)?;
    let workflow_name: String = row.get(2)?;
    let status_str: String = row.get(3)?;
    let start_time_str: String = row.get(4)?;
    let end_time_str: Option<String> = row.get(5)?;
    let exit_code: Option<i32> = row.get(6)?;
    let binary_path_str: String = row.get(7)?;
    let created_at_str: String = row.get(8)?;
    let updated_at_str: String = row.get(9)?;

    let id = Uuid::parse_str(&id_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;

    let status = string_to_status(&status_str).map_err(|e| {
        rusqlite::Error::InvalidQuery
    })?;

    let start_time = DateTime::parse_from_rfc3339(&start_time_str)
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
        })?
        .with_timezone(&Local);

    let end_time = end_time_str
        .map(|s| DateTime::parse_from_rfc3339(&s))
        .transpose()
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e))
        })?
        .map(|dt| dt.with_timezone(&Local));

    let binary_path = PathBuf::from(binary_path_str);

    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e))
        })?
        .with_timezone(&Local);

    let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(e))
        })?
        .with_timezone(&Local);

    Ok(PersistedExecution {
        id,
        workflow_id,
        workflow_name,
        status,
        start_time,
        end_time,
        exit_code,
        binary_path,
        created_at,
        updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn create_test_execution(id: Uuid) -> PersistedExecution {
        let now = Local::now();
        PersistedExecution {
            id,
            workflow_id: "test-workflow".to_string(),
            workflow_name: "Test Workflow".to_string(),
            status: WorkflowStatus::Running,
            start_time: now,
            end_time: None,
            exit_code: None,
            binary_path: PathBuf::from("/usr/bin/test"),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn test_database_creation_and_schema() {
        let db = Database::new_in_memory().unwrap();
        db.initialize_schema().unwrap();

        let version = db.get_schema_version().unwrap();
        assert_eq!(version, 2); // Updated to v2 with chat_input_history table
    }

    #[test]
    fn test_insert_and_retrieve_execution() {
        let db = Database::new_in_memory().unwrap();
        db.initialize_schema().unwrap();

        let exec_id = Uuid::new_v4();
        let exec = create_test_execution(exec_id);
        db.insert_execution(&exec).unwrap();

        let retrieved = db.get_execution(&exec_id).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, exec_id);
        assert_eq!(retrieved.workflow_id, "test-workflow");
        assert_eq!(retrieved.workflow_name, "Test Workflow");
    }

    #[test]
    fn test_update_execution() {
        let db = Database::new_in_memory().unwrap();
        db.initialize_schema().unwrap();

        let exec_id = Uuid::new_v4();
        let exec = create_test_execution(exec_id);
        db.insert_execution(&exec).unwrap();

        let end_time = Local::now();
        db.update_execution(&exec_id, WorkflowStatus::Completed, Some(end_time), Some(0))
            .unwrap();

        let updated = db.get_execution(&exec_id).unwrap().unwrap();
        assert_eq!(updated.status, WorkflowStatus::Completed);
        assert!(updated.end_time.is_some());
        assert_eq!(updated.exit_code, Some(0));
    }

    #[test]
    fn test_list_executions() {
        let db = Database::new_in_memory().unwrap();
        db.initialize_schema().unwrap();

        // Insert multiple executions
        for i in 0..5 {
            let exec_id = Uuid::new_v4();
            let mut exec = create_test_execution(exec_id);
            exec.workflow_id = format!("workflow-{}", i % 2);
            db.insert_execution(&exec).unwrap();
        }

        // List all executions
        let all = db.list_executions(10, 0, None).unwrap();
        assert_eq!(all.len(), 5);

        // List with pagination
        let page1 = db.list_executions(2, 0, None).unwrap();
        assert_eq!(page1.len(), 2);

        let page2 = db.list_executions(2, 2, None).unwrap();
        assert_eq!(page2.len(), 2);

        // Filter by workflow
        let filtered = db.list_executions(10, 0, Some("workflow-0")).unwrap();
        assert!(filtered.len() >= 2);
    }

    #[test]
    fn test_store_and_retrieve_logs() {
        let db = Database::new_in_memory().unwrap();
        db.initialize_schema().unwrap();

        let exec_id = Uuid::new_v4();
        let exec = create_test_execution(exec_id);
        db.insert_execution(&exec).unwrap();

        // Insert logs
        let logs = vec![
            WorkflowLog::PhaseStarted {
                phase: 1,
                name: "Phase 1".to_string(),
                total_phases: 3,
            },
            WorkflowLog::TaskProgress {
                task_id: "task1".to_string(),
                message: "running".to_string(),
            },
            WorkflowLog::RawOutput {
                stream: "stdout".to_string(),
                line: "Hello, world!".to_string(),
            },
        ];

        for (i, log) in logs.iter().enumerate() {
            db.insert_log(&exec_id, i, log).unwrap();
        }

        // Retrieve logs
        let retrieved = db.get_logs(&exec_id, None).unwrap();
        assert_eq!(retrieved.len(), 3);

        // Test log count
        let count = db.get_log_count(&exec_id).unwrap();
        assert_eq!(count, 3);

        // Test limit
        let limited = db.get_logs(&exec_id, Some(2)).unwrap();
        assert_eq!(limited.len(), 2);
    }

    #[test]
    fn test_batch_insert_logs() {
        let db = Database::new_in_memory().unwrap();
        db.initialize_schema().unwrap();

        let exec_id = Uuid::new_v4();
        let exec = create_test_execution(exec_id);
        db.insert_execution(&exec).unwrap();

        // Create batch of logs
        let logs: Vec<(usize, WorkflowLog)> = (0..10)
            .map(|i| {
                (
                    i,
                    WorkflowLog::RawOutput {
                        stream: "stdout".to_string(),
                        line: format!("Line {}", i),
                    },
                )
            })
            .collect();

        db.batch_insert_logs(&exec_id, &logs).unwrap();

        let retrieved = db.get_logs(&exec_id, None).unwrap();
        assert_eq!(retrieved.len(), 10);
    }

    #[test]
    fn test_params_storage() {
        let db = Database::new_in_memory().unwrap();
        db.initialize_schema().unwrap();

        let exec_id = Uuid::new_v4();
        let exec = create_test_execution(exec_id);
        db.insert_execution(&exec).unwrap();

        // Insert params
        let mut params = HashMap::new();
        params.insert("query".to_string(), "test query".to_string());
        params.insert("max_results".to_string(), "10".to_string());

        db.insert_params(&exec_id, &params).unwrap();

        // Retrieve params
        let retrieved = db.get_params(&exec_id).unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved.get("query"), Some(&"test query".to_string()));
        assert_eq!(retrieved.get("max_results"), Some(&"10".to_string()));
    }

    #[test]
    fn test_delete_old_executions() {
        let db = Database::new_in_memory().unwrap();
        db.initialize_schema().unwrap();

        // Insert old execution
        let old_id = Uuid::new_v4();
        let mut old_exec = create_test_execution(old_id);
        old_exec.start_time = Local::now() - Duration::days(60);
        db.insert_execution(&old_exec).unwrap();

        // Insert recent execution
        let recent_id = Uuid::new_v4();
        let recent_exec = create_test_execution(recent_id);
        db.insert_execution(&recent_exec).unwrap();

        // Delete executions older than 30 days
        let cutoff = Local::now() - Duration::days(30);
        let deleted = db.delete_executions_before(cutoff).unwrap();
        assert_eq!(deleted, 1);

        // Verify only recent execution remains
        let all = db.list_executions(10, 0, None).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, recent_id);
    }

    #[test]
    fn test_workflow_stats() {
        let db = Database::new_in_memory().unwrap();
        db.initialize_schema().unwrap();

        let workflow_id = "test-workflow";

        // Insert various executions
        for i in 0..10 {
            let exec_id = Uuid::new_v4();
            let mut exec = create_test_execution(exec_id);
            exec.workflow_id = workflow_id.to_string();
            exec.status = match i % 3 {
                0 => WorkflowStatus::Completed,
                1 => WorkflowStatus::Failed,
                _ => WorkflowStatus::Running,
            };
            db.insert_execution(&exec).unwrap();
        }

        let stats = db.get_workflow_stats(workflow_id).unwrap();
        assert_eq!(stats.total, 10);
        assert!(stats.completed > 0);
        assert!(stats.failed > 0);
        assert!(stats.running > 0);
    }

    #[test]
    fn test_get_executions_by_status() {
        let db = Database::new_in_memory().unwrap();
        db.initialize_schema().unwrap();

        // Insert executions with different statuses
        for i in 0..6 {
            let exec_id = Uuid::new_v4();
            let mut exec = create_test_execution(exec_id);
            exec.status = if i % 2 == 0 {
                WorkflowStatus::Completed
            } else {
                WorkflowStatus::Failed
            };
            db.insert_execution(&exec).unwrap();
        }

        let completed = db.get_executions_by_status(WorkflowStatus::Completed).unwrap();
        assert_eq!(completed.len(), 3);

        let failed = db.get_executions_by_status(WorkflowStatus::Failed).unwrap();
        assert_eq!(failed.len(), 3);

        let running = db.get_executions_by_status(WorkflowStatus::Running).unwrap();
        assert_eq!(running.len(), 0);
    }

    #[test]
    fn test_cascade_delete() {
        let db = Database::new_in_memory().unwrap();
        db.initialize_schema().unwrap();

        let exec_id = Uuid::new_v4();
        let exec = create_test_execution(exec_id);
        db.insert_execution(&exec).unwrap();

        // Insert params and logs
        let mut params = HashMap::new();
        params.insert("test".to_string(), "value".to_string());
        db.insert_params(&exec_id, &params).unwrap();

        let log = WorkflowLog::RawOutput {
            stream: "stdout".to_string(),
            line: "test".to_string(),
        };
        db.insert_log(&exec_id, 0, &log).unwrap();

        // Delete execution
        db.delete_execution(&exec_id).unwrap();

        // Verify params and logs are also deleted (cascade)
        let params_result = db.get_params(&exec_id).unwrap();
        assert_eq!(params_result.len(), 0);

        let logs_result = db.get_logs(&exec_id, None).unwrap();
        assert_eq!(logs_result.len(), 0);
    }
}
