// Re-export the derive macro
pub use workflow_manager_macros::WorkflowDefinition;

// Re-export claude-agent-sdk for convenience
pub use claude_agent_sdk;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Local};

// Re-export async trait for convenience
pub use async_trait::async_trait;

/// Workflow metadata (id, name, description)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// Complete workflow metadata with fields (for JSON export)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullWorkflowMetadata {
    #[serde(flatten)]
    pub metadata: WorkflowMetadata,
    pub fields: Vec<FieldSchema>,
}

/// Field schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldSchema {
    pub name: String,
    pub field_type: FieldType,
    pub label: String,
    pub description: String,
    pub cli_arg: String,
    pub required: bool,
    pub default: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_for_phases: Option<Vec<usize>>,
}

/// Field type enum
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Number {
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<i64>,
    },
    FilePath {
        #[serde(skip_serializing_if = "Option::is_none")]
        pattern: Option<String>,
    },
    Select {
        options: Vec<String>,
    },
    PhaseSelector {
        total_phases: usize,
    },
    StateFile {
        pattern: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        phase: Option<usize>,
    },
}

/// Trait that workflows must implement (auto-implemented by derive macro)
pub trait WorkflowDefinition {
    fn metadata() -> WorkflowMetadata;
    fn fields() -> Vec<FieldSchema>;
    fn print_metadata(&self);
}

/// Workflow status for TUI tracking
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkflowStatus {
    NotStarted,
    Running,
    Completed,
    Failed,
}

/// Progress messages that workflows can send to the TUI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowProgress {
    Started { message: String },
    Progress { message: String },
    Completed { message: String },
    Failed { error: String },
}

/// Complete workflow information for TUI display
#[derive(Debug, Clone)]
pub struct WorkflowInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: WorkflowStatus,
    pub metadata: WorkflowMetadata,
    pub fields: Vec<FieldSchema>,
    pub progress_messages: Vec<String>,
}

/// Workflow source type
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowSource {
    BuiltIn,
    UserDefined,
}

/// Complete workflow struct for TUI
#[derive(Debug, Clone)]
pub struct Workflow {
    pub info: WorkflowInfo,
    pub source: WorkflowSource,
}

/// Structured logging events emitted by workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowLog {
    /// Phase started
    PhaseStarted {
        phase: usize,
        name: String,
        total_phases: usize,
    },
    /// Phase completed
    PhaseCompleted {
        phase: usize,
        name: String,
    },
    /// Phase failed
    PhaseFailed {
        phase: usize,
        name: String,
        error: String,
    },
    /// Task started
    TaskStarted {
        phase: usize,
        task_id: String,
        description: String,
        total_tasks: Option<usize>,
    },
    /// Task progress update
    TaskProgress {
        task_id: String,
        message: String,
    },
    /// Task completed
    TaskCompleted {
        task_id: String,
        result: Option<String>,
    },
    /// Task failed
    TaskFailed {
        task_id: String,
        error: String,
    },
    /// Agent started (sub-agent within a task)
    AgentStarted {
        task_id: String,
        agent_name: String,
        description: String,
    },
    /// Agent message (streaming text)
    AgentMessage {
        task_id: String,
        agent_name: String,
        message: String,
    },
    /// Agent completed
    AgentCompleted {
        task_id: String,
        agent_name: String,
        result: Option<String>,
    },
    /// Agent failed
    AgentFailed {
        task_id: String,
        agent_name: String,
        error: String,
    },
    /// State file created (intermediate outputs)
    StateFileCreated {
        phase: usize,
        file_path: String,
        description: String,
    },
    /// Raw output from workflow process (stdout/stderr)
    RawOutput {
        stream: String,  // "stdout" or "stderr"
        line: String,
    },
}

impl WorkflowLog {
    /// Emit this log event to stderr for TUI parsing
    pub fn emit(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            use std::io::Write;
            eprintln!("__WF_EVENT__:{}", json);
            // Force flush stderr in async/concurrent contexts
            let _ = std::io::stderr().flush();
        }
    }
}

/// Helper macros for workflow logging
#[macro_export]
macro_rules! log_phase_start {
    ($phase:expr, $name:expr, $total:expr) => {
        $crate::WorkflowLog::PhaseStarted {
            phase: $phase,
            name: $name.to_string(),
            total_phases: $total,
        }
        .emit();
    };
}

#[macro_export]
macro_rules! log_phase_complete {
    ($phase:expr, $name:expr) => {
        $crate::WorkflowLog::PhaseCompleted {
            phase: $phase,
            name: $name.to_string(),
        }
        .emit();
    };
}

#[macro_export]
macro_rules! log_phase_failed {
    ($phase:expr, $name:expr, $error:expr) => {
        $crate::WorkflowLog::PhaseFailed {
            phase: $phase,
            name: $name.to_string(),
            error: $error.to_string(),
        }
        .emit();
    };
}

#[macro_export]
macro_rules! log_task_start {
    ($phase:expr, $task_id:expr, $desc:expr) => {
        $crate::WorkflowLog::TaskStarted {
            phase: $phase,
            task_id: $task_id.to_string(),
            description: $desc.to_string(),
            total_tasks: None,
        }
        .emit();
    };
    ($phase:expr, $task_id:expr, $desc:expr, $total:expr) => {
        $crate::WorkflowLog::TaskStarted {
            phase: $phase,
            task_id: $task_id.to_string(),
            description: $desc.to_string(),
            total_tasks: Some($total),
        }
        .emit();
    };
}

#[macro_export]
macro_rules! log_task_progress {
    ($task_id:expr, $msg:expr) => {
        $crate::WorkflowLog::TaskProgress {
            task_id: $task_id.to_string(),
            message: $msg.to_string(),
        }
        .emit();
    };
}

#[macro_export]
macro_rules! log_task_complete {
    ($task_id:expr) => {
        $crate::WorkflowLog::TaskCompleted {
            task_id: $task_id.to_string(),
            result: None,
        }
        .emit();
    };
    ($task_id:expr, $result:expr) => {
        $crate::WorkflowLog::TaskCompleted {
            task_id: $task_id.to_string(),
            result: Some($result.to_string()),
        }
        .emit();
    };
}

#[macro_export]
macro_rules! log_task_failed {
    ($task_id:expr, $error:expr) => {
        $crate::WorkflowLog::TaskFailed {
            task_id: $task_id.to_string(),
            error: $error.to_string(),
        }
        .emit();
    };
}

#[macro_export]
macro_rules! log_agent_start {
    ($task_id:expr, $agent:expr, $desc:expr) => {
        $crate::WorkflowLog::AgentStarted {
            task_id: $task_id.to_string(),
            agent_name: $agent.to_string(),
            description: $desc.to_string(),
        }
        .emit();
    };
}

#[macro_export]
macro_rules! log_agent_message {
    ($task_id:expr, $agent:expr, $msg:expr) => {
        $crate::WorkflowLog::AgentMessage {
            task_id: $task_id.to_string(),
            agent_name: $agent.to_string(),
            message: $msg.to_string(),
        }
        .emit();
    };
}

#[macro_export]
macro_rules! log_agent_complete {
    ($task_id:expr, $agent:expr) => {
        $crate::WorkflowLog::AgentCompleted {
            task_id: $task_id.to_string(),
            agent_name: $agent.to_string(),
            result: None,
        }
        .emit();
    };
    ($task_id:expr, $agent:expr, $result:expr) => {
        $crate::WorkflowLog::AgentCompleted {
            task_id: $task_id.to_string(),
            agent_name: $agent.to_string(),
            result: Some($result.to_string()),
        }
        .emit();
    };
}

#[macro_export]
macro_rules! log_agent_failed {
    ($task_id:expr, $agent:expr, $error:expr) => {
        $crate::WorkflowLog::AgentFailed {
            task_id: $task_id.to_string(),
            agent_name: $agent.to_string(),
            error: $error.to_string(),
        }
        .emit();
    };
}

#[macro_export]
macro_rules! log_state_file {
    ($phase:expr, $path:expr, $desc:expr) => {
        $crate::WorkflowLog::StateFileCreated {
            phase: $phase,
            file_path: $path.to_string(),
            description: $desc.to_string(),
        }
        .emit();
    };
}

/// Workflow execution handle for tracking async execution
#[derive(Debug, Clone)]
pub struct WorkflowHandle {
    pub id: Uuid,
    pub workflow_id: String,
}

impl WorkflowHandle {
    pub fn new(id: Uuid, workflow_id: String) -> Self {
        Self { id, workflow_id }
    }

    pub fn id(&self) -> &Uuid {
        &self.id
    }
}

/// Result type for workflow operations
pub type WorkflowResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Lightweight execution summary for listing (excludes logs and params)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    pub id: Uuid,
    pub workflow_id: String,
    pub workflow_name: String,
    pub status: WorkflowStatus,
    pub start_time: DateTime<Local>,
    pub end_time: Option<DateTime<Local>>,
    pub exit_code: Option<i32>,
}

/// Runtime trait for workflow discovery and execution
/// This provides a unified API for both TUI and MCP consumers
#[async_trait]
pub trait WorkflowRuntime: Send + Sync {
    /// List all discovered workflows with metadata
    fn list_workflows(&self) -> WorkflowResult<Vec<FullWorkflowMetadata>>;

    /// Get detailed metadata for a specific workflow
    fn get_workflow_metadata(&self, id: &str) -> WorkflowResult<FullWorkflowMetadata>;

    /// Validate inputs against workflow schema before execution
    fn validate_workflow_inputs(
        &self,
        id: &str,
        params: HashMap<String, String>,
    ) -> WorkflowResult<()>;

    /// Execute a workflow asynchronously
    async fn execute_workflow(
        &self,
        id: &str,
        params: HashMap<String, String>,
    ) -> WorkflowResult<WorkflowHandle>;

    /// Subscribe to logs from a running workflow (real-time stream)
    async fn subscribe_logs(
        &self,
        handle_id: &Uuid,
    ) -> WorkflowResult<tokio::sync::broadcast::Receiver<WorkflowLog>>;

    /// Get historical logs from a workflow execution (for MCP tools)
    async fn get_logs(&self, handle_id: &Uuid, limit: Option<usize>) -> WorkflowResult<Vec<WorkflowLog>>;

    /// Get current status of a running workflow
    async fn get_status(&self, handle_id: &Uuid) -> WorkflowResult<WorkflowStatus>;

    /// Cancel a running workflow
    async fn cancel_workflow(&self, handle_id: &Uuid) -> WorkflowResult<()>;

    /// List workflow executions with pagination and optional filtering
    ///
    /// # Arguments
    /// * `limit` - Maximum number of executions to return
    /// * `offset` - Number of executions to skip (for pagination)
    /// * `workflow_id` - Optional filter by workflow type
    ///
    /// # Returns
    /// Vector of execution summaries, ordered by start_time descending (newest first)
    async fn list_executions(
        &self,
        limit: usize,
        offset: usize,
        workflow_id: Option<String>,
    ) -> WorkflowResult<Vec<ExecutionSummary>>;

    /// Get parameters used for a specific workflow execution
    ///
    /// # Arguments
    /// * `handle_id` - The execution UUID
    ///
    /// # Returns
    /// HashMap of parameter names to values used in the execution
    async fn get_params(&self, handle_id: &Uuid) -> WorkflowResult<HashMap<String, String>>;
}
