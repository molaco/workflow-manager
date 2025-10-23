// Re-export the derive macro
pub use workflow_manager_macros::WorkflowDefinition;

// Re-export claude-agent-sdk for convenience
pub use claude_agent_sdk;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

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

// ============================================================================
// Console Logging Macros (for task planner and CLI workflows)
// ============================================================================
// These macros provide colored console output for human-readable logs,
// complementing the structured WorkflowLog events used by the TUI.
// ============================================================================

/// Logs the start of a workflow phase/step with a header and description.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_phase_start_console;
/// log_phase_start_console!(1, "Main Orchestrator", "Generate tasks overview");
/// ```
///
/// Outputs:
/// ```text
/// ═══ STEP 1: Main Orchestrator ═══
/// Generate tasks overview
/// ```
#[macro_export]
macro_rules! log_phase_start_console {
    ($phase:expr, $title:expr, $description:expr) => {
        println!("\x1b[1;36m═══ STEP {}: {} ═══\x1b[0m", $phase, $title);
        println!("\x1b[36m{}\x1b[0m", $description);
    };
}

/// Logs the completion of a workflow phase/step.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_phase_complete_console;
/// log_phase_complete_console!(1);
/// ```
///
/// Outputs:
/// ```text
/// ✓ Step 1 complete
/// ```
#[macro_export]
macro_rules! log_phase_complete_console {
    ($phase:expr) => {
        println!("\x1b[32m✓ Step {} complete\x1b[0m", $phase);
    };
}

/// Logs the start of a batch operation.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_batch_start;
/// log_batch_start!(2, 5, 3);
/// ```
///
/// Outputs:
/// ```text
/// → Executing Batch 2/5 (3 tasks)
/// ```
#[macro_export]
macro_rules! log_batch_start {
    ($batch_num:expr, $total_batches:expr, $num_tasks:expr) => {
        println!(
            "\x1b[36m→ Executing Batch {}/{} ({} tasks)\x1b[0m",
            $batch_num, $total_batches, $num_tasks
        );
    };
}

/// Logs the completion of a batch operation.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_batch_complete;
/// log_batch_complete!(2);
/// ```
///
/// Outputs:
/// ```text
/// ✓ Batch 2 complete
/// ```
#[macro_export]
macro_rules! log_batch_complete {
    ($batch_num:expr) => {
        println!("\x1b[32m✓ Batch {} complete\x1b[0m", $batch_num);
    };
}

/// Logs the start of parallel execution.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_parallel_start;
/// log_parallel_start!(3, "tasks");
/// ```
///
/// Outputs:
/// ```text
/// → Running 3 tasks in parallel
/// ```
#[macro_export]
macro_rules! log_parallel_start {
    ($num_items:expr, $item_type:expr) => {
        println!(
            "\x1b[36m→ Running {} {} in parallel\x1b[0m",
            $num_items, $item_type
        );
    };
}

/// Logs the completion of parallel execution.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_parallel_complete;
/// log_parallel_complete!(3, "tasks");
/// ```
///
/// Outputs:
/// ```text
/// ✓ 3 tasks completed
/// ```
#[macro_export]
macro_rules! log_parallel_complete {
    ($num_items:expr, $item_type:expr) => {
        println!(
            "\x1b[32m✓ {} {} completed\x1b[0m",
            $num_items, $item_type
        );
    };
}

/// Logs delegation to a sub-agent.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_delegate_to;
/// log_delegate_to!("orchestrator", "files");
/// ```
///
/// Outputs:
/// ```text
///   → Delegating to @files agent...
/// ```
#[macro_export]
macro_rules! log_delegate_to {
    ($parent_agent:expr, $sub_agent_name:expr) => {
        println!(
            "\x1b[36m  → Delegating to @{} agent...\x1b[0m",
            $sub_agent_name
        );
    };
}

/// Logs completion of a sub-agent delegation.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_delegate_complete;
/// log_delegate_complete!("files");
/// ```
///
/// Outputs:
/// ```text
///   ✓ @files agent complete
/// ```
#[macro_export]
macro_rules! log_delegate_complete {
    ($sub_agent_name:expr) => {
        println!(
            "\x1b[32m  ✓ @{} agent complete\x1b[0m",
            $sub_agent_name
        );
    };
}

/// Logs individual agent/task statistics.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_stats;
/// log_stats!(1250, 3, 0.0234, 1234, 567);
/// ```
///
/// Outputs:
/// ```text
/// Statistics: 1250ms, 3 turns, $0.0234 (tokens: 1234 in / 567 out)
/// ```
#[macro_export]
macro_rules! log_stats {
    ($duration_ms:expr, $turns:expr, $cost_usd:expr, $input_tokens:expr, $output_tokens:expr) => {
        println!(
            "\x1b[2mStatistics: {}ms, {} turns, ${:.4} (tokens: {} in / {} out)\x1b[0m",
            $duration_ms, $turns, $cost_usd, $input_tokens, $output_tokens
        );
    };
}

/// Logs aggregate statistics for multiple items.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_aggregate_stats;
/// log_aggregate_stats!(5, 6234, 15, 0.1145);
/// ```
///
/// Outputs:
/// ```text
/// Total: 5 items, 6.2s, 15 turns, $0.1145
/// ```
#[macro_export]
macro_rules! log_aggregate_stats {
    ($item_count:expr, $total_duration_ms:expr, $total_turns:expr, $total_cost_usd:expr) => {
        println!(
            "\x1b[1mTotal: {} items, {:.1}s, {} turns, ${:.4}\x1b[0m",
            $item_count,
            $total_duration_ms as f64 / 1000.0,
            $total_turns,
            $total_cost_usd
        );
    };
}

/// Logs a review summary with approval counts.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_review_summary;
/// log_review_summary!(8, 2, 10);
/// ```
///
/// Outputs:
/// ```text
/// Review: ✓ 8 approved, ✗ 2 need revision (10 total)
/// ```
#[macro_export]
macro_rules! log_review_summary {
    ($approved:expr, $needs_revision:expr, $total:expr) => {
        println!(
            "\x1b[1mReview: \x1b[32m✓ {} approved\x1b[0m, \x1b[31m✗ {} need revision\x1b[0m ({} total)",
            $approved, $needs_revision, $total
        );
    };
}

/// Logs a specific review issue for a task.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_review_issue;
/// log_review_issue!(3, "Missing test coverage for edge cases");
/// ```
///
/// Outputs:
/// ```text
///   ✗ Task 3: Missing test coverage for edge cases
/// ```
#[macro_export]
macro_rules! log_review_issue {
    ($task_id:expr, $issue_text:expr) => {
        println!(
            "\x1b[31m  ✗ Task {}: {}\x1b[0m",
            $task_id, $issue_text
        );
    };
}

/// Logs progress of an operation.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_progress;
/// log_progress!(3, 5, "tasks");
/// ```
///
/// Outputs:
/// ```text
/// Progress: 3/5 tasks
/// ```
#[macro_export]
macro_rules! log_progress {
    ($current:expr, $total:expr, $item_type:expr) => {
        println!(
            "\x1b[36mProgress: {}/{} {}\x1b[0m",
            $current, $total, $item_type
        );
    };
}

/// Logs the number of items found.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_found;
/// log_found!(14, "tasks to expand");
/// ```
///
/// Outputs:
/// ```text
/// Found 14 tasks to expand
/// ```
#[macro_export]
macro_rules! log_found {
    ($count:expr, $item_type:expr) => {
        println!("\x1b[36mFound {} {}\x1b[0m", $count, $item_type);
    };
}

/// Logs an informational message.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_info;
/// log_info!("Loading tasks_overview_template...");
/// ```
///
/// Outputs:
/// ```text
/// ℹ Loading tasks_overview_template...
/// ```
#[macro_export]
macro_rules! log_info {
    ($message:expr) => {
        println!("\x1b[36mℹ {}\x1b[0m", $message);
    };
    ($fmt:expr, $($arg:tt)*) => {
        println!("\x1b[36mℹ {}\x1b[0m", format!($fmt, $($arg)*));
    };
}

/// Logs a warning message.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_warning;
/// log_warning!("Circular dependency detected");
/// ```
///
/// Outputs:
/// ```text
/// ⚠ Warning: Circular dependency detected
/// ```
#[macro_export]
macro_rules! log_warning {
    ($message:expr) => {
        println!("\x1b[33m⚠ Warning: {}\x1b[0m", $message);
    };
    ($fmt:expr, $($arg:tt)*) => {
        println!("\x1b[33m⚠ Warning: {}\x1b[0m", format!($fmt, $($arg)*));
    };
}

/// Logs that a file has been saved.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_file_saved;
/// log_file_saved!("./tasks_overview.yaml");
/// ```
///
/// Outputs:
/// ```text
/// ✓ Saved: ./tasks_overview.yaml
/// ```
#[macro_export]
macro_rules! log_file_saved {
    ($path:expr) => {
        println!("\x1b[32m✓ Saved: {}\x1b[0m", $path);
    };
}

/// Logs the start of streaming mode file writing.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_streaming_start;
/// log_streaming_start!("./tasks.yaml");
/// ```
///
/// Outputs:
/// ```text
/// → Streaming mode: Writing directly to ./tasks.yaml
/// ```
#[macro_export]
macro_rules! log_streaming_start {
    ($path:expr) => {
        println!(
            "\x1b[36m→ Streaming mode: Writing directly to {}\x1b[0m",
            $path
        );
    };
}

/// Logs the completion of streaming mode file writing.
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_streaming_complete;
/// log_streaming_complete!("./tasks.yaml");
/// ```
///
/// Outputs:
/// ```text
/// ✓ Streamed to: ./tasks.yaml
/// ```
#[macro_export]
macro_rules! log_streaming_complete {
    ($path:expr) => {
        println!("\x1b[32m✓ Streamed to: {}\x1b[0m", $path);
    };
}

/// Logs a debug message (intended to be used conditionally).
///
/// # Example
/// ```
/// use workflow_manager_sdk::log_debug;
/// log_debug!("Parsing 5 batches from execution plan");
/// let count = 42;
/// log_debug!("Processing {} items", count);
/// ```
///
/// Outputs:
/// ```text
/// [DEBUG] Parsing 5 batches from execution plan
/// [DEBUG] Processing 42 items
/// ```
#[macro_export]
macro_rules! log_debug {
    ($message:expr) => {
        println!("\x1b[2m[DEBUG] {}\x1b[0m", $message);
    };
    ($fmt:expr, $($arg:tt)*) => {
        println!("\x1b[2m[DEBUG] {}\x1b[0m", format!($fmt, $($arg)*));
    };
}

// ============================================================================
// End of Console Logging Macros
// ============================================================================

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

    /// Subscribe to logs from a running workflow
    async fn subscribe_logs(
        &self,
        handle_id: &Uuid,
    ) -> WorkflowResult<tokio::sync::broadcast::Receiver<WorkflowLog>>;

    /// Get current status of a running workflow
    async fn get_status(&self, handle_id: &Uuid) -> WorkflowResult<WorkflowStatus>;

    /// Cancel a running workflow
    async fn cancel_workflow(&self, handle_id: &Uuid) -> WorkflowResult<()>;
}
