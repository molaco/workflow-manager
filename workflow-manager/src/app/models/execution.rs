//! Execution persistence models for database serialization

use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;
use workflow_manager_sdk::{WorkflowLog, WorkflowStatus};

use crate::runtime::ExecutionState;

/// Persisted execution model for database storage
///
/// This struct represents a workflow execution that can be serialized to/from
/// a database. It contains all the metadata and state needed to persist
/// workflow executions across application restarts.
#[derive(Debug, Clone)]
pub struct PersistedExecution {
    /// Unique execution ID (handle_id)
    pub id: Uuid,

    /// Workflow type identifier
    pub workflow_id: String,

    /// Human-readable workflow name
    pub workflow_name: String,

    /// Current execution status
    pub status: WorkflowStatus,

    /// When the execution started
    pub start_time: DateTime<Local>,

    /// When the execution finished (None if still running)
    pub end_time: Option<DateTime<Local>>,

    /// Process exit code (None if still running or no exit code available)
    pub exit_code: Option<i32>,

    /// Path to the workflow binary
    pub binary_path: PathBuf,

    /// Input parameters used for this execution
    pub params: HashMap<String, String>,

    /// All logs emitted during execution (loaded separately from DB)
    pub logs: Vec<WorkflowLog>,
}

impl PersistedExecution {
    /// Convert from ExecutionState to PersistedExecution
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

    /// Convert from PersistedExecution to ExecutionState
    ///
    /// This is used when loading executions from the database on startup.
    /// Note: The child process and broadcast sender cannot be restored,
    /// so this is only suitable for completed/failed executions.
    pub fn to_execution_state(&self) -> ExecutionState {
        use tokio::sync::broadcast;

        let (logs_tx, _) = broadcast::channel(1000);

        ExecutionState {
            workflow_id: self.workflow_id.clone(),
            workflow_name: self.workflow_name.clone(),
            status: self.status.clone(),
            child: None, // Cannot restore running process
            logs_tx,
            binary_path: self.binary_path.clone(),
            logs_buffer: std::sync::Arc::new(std::sync::Mutex::new(self.logs.clone())),
            start_time: self.start_time,
            end_time: self.end_time,
            params: self.params.clone(),
            exit_code: self.exit_code,
        }
    }
}
