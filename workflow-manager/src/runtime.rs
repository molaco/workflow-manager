use anyhow::{anyhow, Result};
use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::broadcast;
use uuid::Uuid;
use workflow_manager_sdk::{
    async_trait, ExecutionSummary, FullWorkflowMetadata, WorkflowHandle, WorkflowLog,
    WorkflowResult, WorkflowRuntime, WorkflowStatus,
};

use crate::database::{Database, PersistedExecution};
use crate::discovery::{discover_workflows, DiscoveredWorkflow};

/// Internal execution state for a running workflow
pub struct ExecutionState {
    pub workflow_id: String,
    pub workflow_name: String,
    pub status: WorkflowStatus,
    pub child: Option<Child>,
    pub logs_tx: broadcast::Sender<WorkflowLog>,
    pub binary_path: PathBuf,
    /// Persistent buffer of all logs for historical retrieval
    pub logs_buffer: Arc<Mutex<Vec<WorkflowLog>>>,
    pub start_time: DateTime<Local>,
    pub end_time: Option<DateTime<Local>>,
    pub params: HashMap<String, String>,
    pub exit_code: Option<i32>,
}

/// Process-based workflow runtime implementation
pub struct ProcessBasedRuntime {
    /// Discovered workflows cache (id -> workflow)
    workflows: Arc<Mutex<HashMap<String, DiscoveredWorkflow>>>,
    /// Active executions (uuid -> state)
    executions: Arc<Mutex<HashMap<Uuid, ExecutionState>>>,
    /// SQLite database for persistent workflow execution history
    database: Arc<Mutex<Database>>,
}

impl ProcessBasedRuntime {
    /// Create runtime with pre-discovered workflows (avoids duplicate discovery)
    pub fn new_with_workflows(workflows: Vec<DiscoveredWorkflow>) -> Result<Self> {
        let workflows_map: HashMap<String, DiscoveredWorkflow> = workflows
            .into_iter()
            .map(|w| (w.metadata.id.clone(), w))
            .collect();

        // Initialize database (same as new())
        let db_path = dirs::home_dir()
            .ok_or_else(|| anyhow!("Could not find home directory"))?
            .join(".workflow-manager")
            .join("executions.db");

        std::fs::create_dir_all(db_path.parent().unwrap())?;

        let database = Database::new(db_path)?;
        database.initialize_schema()?;

        let runtime = Self {
            workflows: Arc::new(Mutex::new(workflows_map)),
            executions: Arc::new(Mutex::new(HashMap::new())),
            database: Arc::new(Mutex::new(database)),
        };

        // Restore from database
        if let Err(e) = runtime.restore_from_database() {
            eprintln!("Warning: Failed to restore executions from database: {}", e);
        }

        Ok(runtime)
    }

    /// Create a new runtime and discover workflows
    pub fn new() -> Result<Self> {
        let workflows = discover_workflows();
        Self::new_with_workflows(workflows)
    }

    /// Refresh workflow discovery
    pub fn refresh_workflows(&self) -> Result<()> {
        let workflows = discover_workflows();
        let workflows_map: HashMap<String, DiscoveredWorkflow> = workflows
            .into_iter()
            .map(|w| (w.metadata.id.clone(), w))
            .collect();

        *self.workflows.lock().unwrap() = workflows_map;
        Ok(())
    }

    /// Get reference to database for chat history
    pub fn get_database(&self) -> Arc<Mutex<Database>> {
        self.database.clone()
    }

    /// Clean up completed/failed workflow executions
    /// Removes execution state for workflows that have finished, freeing memory
    pub fn cleanup_completed_executions(&self) {
        let mut execs = self.executions.lock().unwrap();
        execs.retain(|_, state| {
            matches!(state.status, WorkflowStatus::Running)
        });
    }

    /// Build CLI command from parameters
    fn build_command(
        &self,
        workflow: &DiscoveredWorkflow,
        params: HashMap<String, String>,
    ) -> Command {
        let mut cmd = Command::new(&workflow.binary_path);

        for field in &workflow.fields {
            if let Some(value) = params.get(&field.name) {
                if !value.is_empty() {
                    cmd.arg(&field.cli_arg).arg(value);
                }
            }
        }

        cmd
    }

    /// Restore past executions from database on startup
    fn restore_from_database(&self) -> Result<()> {
        let db = self.database.lock().unwrap();

        // Load recent completed executions (last 100)
        let persisted = db.list_executions(100, 0, None)?;

        let mut executions = self.executions.lock().unwrap();
        for mut exec in persisted {
            // Skip Running status - these are stale from previous session
            if exec.status == WorkflowStatus::Running {
                // Mark as Failed since app was restarted
                exec.status = WorkflowStatus::Failed;
                exec.end_time = Some(exec.start_time); // Approximate end time

                // Update in database
                db.update_execution(
                    &exec.id,
                    WorkflowStatus::Failed,
                    exec.end_time,
                    None
                )?;
            }

            // Load logs from database for this execution
            let logs = db.get_logs(&exec.id, None)?;

            // Load params from database for this execution
            let params = db.get_params(&exec.id)?;

            // Convert to ExecutionState and load into memory
            let (logs_tx, _) = broadcast::channel(1000);
            let state = ExecutionState {
                workflow_id: exec.workflow_id.clone(),
                workflow_name: exec.workflow_name.clone(),
                status: exec.status.clone(),
                child: None, // Cannot restore running process
                logs_tx,
                binary_path: exec.binary_path.clone(),
                logs_buffer: Arc::new(Mutex::new(logs)),
                start_time: exec.start_time,
                end_time: exec.end_time,
                params,
                exit_code: exec.exit_code,
            };
            executions.insert(exec.id, state);
        }

        Ok(())
    }
}

#[async_trait]
impl WorkflowRuntime for ProcessBasedRuntime {
    fn list_workflows(&self) -> WorkflowResult<Vec<FullWorkflowMetadata>> {
        let workflows = self.workflows.lock().unwrap();
        let result = workflows
            .values()
            .map(|w| FullWorkflowMetadata {
                metadata: w.metadata.clone(),
                fields: w.fields.clone(),
            })
            .collect();
        Ok(result)
    }

    fn get_workflow_metadata(&self, id: &str) -> WorkflowResult<FullWorkflowMetadata> {
        let workflows = self.workflows.lock().unwrap();
        workflows
            .get(id)
            .map(|w| FullWorkflowMetadata {
                metadata: w.metadata.clone(),
                fields: w.fields.clone(),
            })
            .ok_or_else(|| format!("Workflow '{}' not found", id).into())
    }

    fn validate_workflow_inputs(
        &self,
        id: &str,
        params: HashMap<String, String>,
    ) -> WorkflowResult<()> {
        let workflows = self.workflows.lock().unwrap();
        let workflow = workflows
            .get(id)
            .ok_or_else(|| format!("Workflow '{}' not found", id))?;

        // Check required fields
        for field in &workflow.fields {
            if field.required && !params.contains_key(&field.name) {
                return Err(format!("Required field '{}' missing", field.name).into());
            }
        }

        Ok(())
    }

    async fn execute_workflow(
        &self,
        id: &str,
        params: HashMap<String, String>,
    ) -> WorkflowResult<WorkflowHandle> {
        // Validate inputs
        self.validate_workflow_inputs(id, params.clone())?;

        // Get workflow
        let workflow = {
            let workflows = self.workflows.lock().unwrap();
            workflows
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Workflow '{}' not found", id))?
        };

        // Build command
        let mut cmd = self.build_command(&workflow, params.clone());
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        // Spawn process
        let child = cmd
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn workflow process: {}", e))?;

        // Create broadcast channel for logs (capacity 1000)
        // Increased from 100 to reduce lagging in high-frequency workflows
        let (logs_tx, _) = broadcast::channel(1000);

        // Generate execution ID
        let exec_id = Uuid::new_v4();

        // Store execution state
        let logs_buffer = Arc::new(Mutex::new(Vec::new()));
        let state = ExecutionState {
            workflow_id: id.to_string(),
            workflow_name: workflow.metadata.name.clone(),
            status: WorkflowStatus::Running,
            child: Some(child),
            logs_tx: logs_tx.clone(),
            binary_path: workflow.binary_path.clone(),
            logs_buffer: logs_buffer.clone(),
            start_time: Local::now(),
            end_time: None,
            params: params.clone(),
            exit_code: None,
        };
        self.executions.lock().unwrap().insert(exec_id, state);

        // Persist execution to database
        {
            let db = self.database.lock().unwrap();
            let persisted = PersistedExecution {
                id: exec_id,
                workflow_id: workflow.metadata.id.clone(),
                workflow_name: workflow.metadata.name.clone(),
                status: WorkflowStatus::Running,
                start_time: Local::now(),
                end_time: None,
                exit_code: None,
                binary_path: workflow.binary_path.clone(),
                created_at: Local::now(),
                updated_at: Local::now(),
            };

            if let Err(e) = db.insert_execution(&persisted) {
                eprintln!("Warning: Failed to persist execution to database: {}", e);
            }

            // Persist params
            if let Err(e) = db.insert_params(&exec_id, &params) {
                eprintln!("Warning: Failed to persist params to database: {}", e);
            }
        }

        // Spawn stderr parser task
        let executions_stderr = self.executions.clone();
        let database_stderr = self.database.clone();
        let exec_id_stderr = exec_id;
        tokio::spawn(async move {
            if let Err(e) = parse_workflow_stderr(exec_id_stderr, executions_stderr.clone(), database_stderr).await {
                eprintln!("Error parsing workflow stderr: {}", e);
                // Mark execution as failed
                let mut execs = executions_stderr.lock().unwrap();
                if let Some(state) = execs.get_mut(&exec_id_stderr) {
                    state.status = WorkflowStatus::Failed;
                }
            }
        });

        // Spawn stdout parser task
        let executions_stdout = self.executions.clone();
        let database_stdout = self.database.clone();
        let exec_id_stdout = exec_id;
        tokio::spawn(async move {
            if let Err(e) = parse_workflow_stdout(exec_id_stdout, executions_stdout.clone(), database_stdout).await {
                eprintln!("Error parsing workflow stdout: {}", e);
                // Mark execution as failed
                let mut execs = executions_stdout.lock().unwrap();
                if let Some(state) = execs.get_mut(&exec_id_stdout) {
                    state.status = WorkflowStatus::Failed;
                }
            }
        });

        // Spawn task to wait for process exit and update status
        let executions_wait = self.executions.clone();
        let database_wait = self.database.clone();
        let exec_id_wait = exec_id;
        tokio::spawn(async move {
            if let Err(e) = wait_for_process_exit(exec_id_wait, executions_wait, database_wait).await {
                eprintln!("Error waiting for process exit: {}", e);
            }
        });

        Ok(WorkflowHandle::new(exec_id, id.to_string()))
    }

    async fn subscribe_logs(
        &self,
        handle_id: &Uuid,
    ) -> WorkflowResult<broadcast::Receiver<WorkflowLog>> {
        let executions = self.executions.lock().unwrap();
        let state = executions
            .get(handle_id)
            .ok_or_else(|| anyhow!("Execution not found: {}", handle_id))?;
        Ok(state.logs_tx.subscribe())
    }

    async fn get_logs(&self, handle_id: &Uuid, limit: Option<usize>) -> WorkflowResult<Vec<WorkflowLog>> {
        // Try in-memory first (for running workflows)
        {
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
        }

        // Not in memory, try database
        let db = self.database.lock().unwrap();
        db.get_logs(handle_id, limit)
            .map_err(|e| e.into())
    }

    async fn get_status(&self, handle_id: &Uuid) -> WorkflowResult<WorkflowStatus> {
        // Try in-memory first
        {
            let executions = self.executions.lock().unwrap();
            if let Some(state) = executions.get(handle_id) {
                return Ok(state.status.clone());
            }
        }

        // Not in memory, try database
        let db = self.database.lock().unwrap();
        db.get_execution(handle_id)
            .map_err(|e: anyhow::Error| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?
            .map(|exec| exec.status)
            .ok_or_else(|| anyhow!("Execution not found: {}", handle_id).into())
    }

    async fn cancel_workflow(&self, handle_id: &Uuid) -> WorkflowResult<()> {
        let mut executions = self.executions.lock().unwrap();
        let state = executions
            .get_mut(handle_id)
            .ok_or_else(|| anyhow!("Execution not found: {}", handle_id))?;

        if let Some(mut child) = state.child.take() {
            let _ = child.kill();
            state.status = WorkflowStatus::Failed;
            state.end_time = Some(Local::now());
            // exit_code remains None when killed
        }

        Ok(())
    }

    async fn list_executions(
        &self,
        limit: usize,
        offset: usize,
        workflow_id: Option<String>,
    ) -> WorkflowResult<Vec<ExecutionSummary>> {
        // Query database
        let db = self.database.lock().unwrap();
        let persisted = db
            .list_executions(limit, offset, workflow_id.as_deref())
            .map_err(|e| anyhow!("Failed to list executions from database: {}", e))?;

        // Convert to lightweight summaries
        let summaries = persisted
            .iter()
            .map(|exec| ExecutionSummary {
                id: exec.id,
                workflow_id: exec.workflow_id.clone(),
                workflow_name: exec.workflow_name.clone(),
                status: exec.status.clone(),
                start_time: exec.start_time,
                end_time: exec.end_time,
                exit_code: exec.exit_code,
            })
            .collect();

        Ok(summaries)
    }

    async fn get_params(&self, handle_id: &Uuid) -> WorkflowResult<HashMap<String, String>> {
        // First check if execution is in memory (running)
        {
            let executions = self.executions.lock().unwrap();
            if let Some(state) = executions.get(handle_id) {
                return Ok(state.params.clone());
            }
        }

        // Not in memory, query database
        let db = self.database.lock().unwrap();
        db.get_params(handle_id)
            .map_err(|e| anyhow!("Failed to get params from database: {}", e).into())
    }
}

/// Parse workflow stderr for __WF_EVENT__:<JSON> messages and raw output
async fn parse_workflow_stderr(
    exec_id: Uuid,
    executions: Arc<Mutex<HashMap<Uuid, ExecutionState>>>,
    database: Arc<Mutex<Database>>,
) -> Result<()> {
    // Get stderr handle and clone necessary state once to avoid locking on every line
    let (stderr, logs_tx, logs_buffer) = {
        let mut execs = executions.lock().unwrap();
        let state = execs
            .get_mut(&exec_id)
            .ok_or_else(|| anyhow!("Execution not found"))?;
        let stderr = state
            .child
            .as_mut()
            .and_then(|c| c.stderr.take())
            .ok_or_else(|| anyhow!("No stderr available"))?;

        // Clone state we need for parsing to avoid holding lock
        (stderr, state.logs_tx.clone(), state.logs_buffer.clone())
    };

    // Wrap in tokio async reader
    let stderr = tokio::process::ChildStderr::from_std(stderr)?;
    let reader = BufReader::new(stderr);
    let mut lines = reader.lines();

    // Batch logging state
    let mut pending_logs: Vec<(usize, WorkflowLog)> = Vec::new();
    let mut last_flush = std::time::Instant::now();

    // Parse lines without holding the main executions lock
    while let Ok(Some(line)) = lines.next_line().await {
        let log = if let Some(json_str) = line.strip_prefix("__WF_EVENT__:") {
            // Structured log event
            serde_json::from_str::<WorkflowLog>(json_str).ok()
        } else {
            // Raw stderr output
            Some(WorkflowLog::RawOutput {
                stream: "stderr".to_string(),
                line,
            })
        };

        if let Some(log) = log {
            // Broadcast to real-time subscribers
            let _ = logs_tx.send(log.clone());

            // Store in buffer for historical retrieval and get sequence number
            let sequence = if let Ok(mut buffer) = logs_buffer.lock() {
                let seq = buffer.len();
                buffer.push(log.clone());
                seq
            } else {
                continue;
            };

            // Add to pending batch
            pending_logs.push((sequence, log));

            // Flush if batch is full or time elapsed
            if pending_logs.len() >= 50 || last_flush.elapsed() > std::time::Duration::from_secs(5) {
                let db = database.lock().unwrap();
                if let Err(e) = db.batch_insert_logs(&exec_id, &pending_logs) {
                    eprintln!("Warning: Failed to batch insert logs: {}", e);
                }
                pending_logs.clear();
                last_flush = std::time::Instant::now();
            }
        }
    }

    // Flush remaining logs
    if !pending_logs.is_empty() {
        let db = database.lock().unwrap();
        if let Err(e) = db.batch_insert_logs(&exec_id, &pending_logs) {
            eprintln!("Warning: Failed to flush remaining logs: {}", e);
        }
    }

    // Status is now updated by wait_for_process_exit based on exit code
    // No longer marking as completed here to avoid race condition

    Ok(())
}

/// Parse workflow stdout for raw output
async fn parse_workflow_stdout(
    exec_id: Uuid,
    executions: Arc<Mutex<HashMap<Uuid, ExecutionState>>>,
    database: Arc<Mutex<Database>>,
) -> Result<()> {
    // Get stdout handle and clone necessary state once to avoid locking on every line
    let (stdout, logs_tx, logs_buffer) = {
        let mut execs = executions.lock().unwrap();
        let state = execs
            .get_mut(&exec_id)
            .ok_or_else(|| anyhow!("Execution not found"))?;
        let stdout = state
            .child
            .as_mut()
            .and_then(|c| c.stdout.take())
            .ok_or_else(|| anyhow!("No stdout available"))?;

        // Clone state we need for parsing to avoid holding lock
        (stdout, state.logs_tx.clone(), state.logs_buffer.clone())
    };

    // Wrap in tokio async reader
    let stdout = tokio::process::ChildStdout::from_std(stdout)?;
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    // Batch logging state
    let mut pending_logs: Vec<(usize, WorkflowLog)> = Vec::new();
    let mut last_flush = std::time::Instant::now();

    // Parse lines without holding the main executions lock
    while let Ok(Some(line)) = lines.next_line().await {
        // All stdout is raw output
        let log = WorkflowLog::RawOutput {
            stream: "stdout".to_string(),
            line,
        };

        // Broadcast to real-time subscribers
        let _ = logs_tx.send(log.clone());

        // Store in buffer for historical retrieval and get sequence number
        let sequence = if let Ok(mut buffer) = logs_buffer.lock() {
            let seq = buffer.len();
            buffer.push(log.clone());
            seq
        } else {
            continue;
        };

        // Add to pending batch
        pending_logs.push((sequence, log));

        // Flush if batch is full or time elapsed
        if pending_logs.len() >= 50 || last_flush.elapsed() > std::time::Duration::from_secs(5) {
            let db = database.lock().unwrap();
            if let Err(e) = db.batch_insert_logs(&exec_id, &pending_logs) {
                eprintln!("Warning: Failed to batch insert logs: {}", e);
            }
            pending_logs.clear();
            last_flush = std::time::Instant::now();
        }
    }

    // Flush remaining logs
    if !pending_logs.is_empty() {
        let db = database.lock().unwrap();
        if let Err(e) = db.batch_insert_logs(&exec_id, &pending_logs) {
            eprintln!("Warning: Failed to flush remaining logs: {}", e);
        }
    }

    Ok(())
}

/// Wait for workflow process to exit and update status accordingly
async fn wait_for_process_exit(
    exec_id: Uuid,
    executions: Arc<Mutex<HashMap<Uuid, ExecutionState>>>,
    database: Arc<Mutex<Database>>,
) -> Result<()> {
    // Take the child process from the execution state
    let mut child = {
        let mut execs = executions.lock().unwrap();
        let state = execs
            .get_mut(&exec_id)
            .ok_or_else(|| anyhow!("Execution not found"))?;
        state
            .child
            .take()
            .ok_or_else(|| anyhow!("No child process available"))?
    };

    // Wait for the process to exit
    let exit_status = child.wait()?;

    // Update status based on exit code
    let mut execs = executions.lock().unwrap();
    if let Some(state) = execs.get_mut(&exec_id) {
        state.status = if exit_status.success() {
            WorkflowStatus::Completed
        } else {
            WorkflowStatus::Failed
        };
        state.end_time = Some(Local::now());
        state.exit_code = exit_status.code();

        // Persist completion to database
        let db = database.lock().unwrap();
        if let Err(e) = db.update_execution(
            &exec_id,
            state.status.clone(),
            state.end_time,
            state.exit_code,
        ) {
            eprintln!("Warning: Failed to update execution in database: {}", e);
        }
    }

    // Note: ExecutionState is kept in HashMap for historical log retrieval
    // The broadcast channel will close naturally when parser tasks complete and drop their senders

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_runtime_creation() {
        let runtime = ProcessBasedRuntime::new().unwrap();
        let workflows = runtime.list_workflows().unwrap();
        println!("Found {} workflows", workflows.len());
        assert!(!workflows.is_empty());
    }

    #[tokio::test]
    async fn test_workflow_discovery() {
        let runtime = ProcessBasedRuntime::new().unwrap();
        let workflows = runtime.list_workflows().unwrap();

        for workflow in workflows {
            println!(
                "Workflow: {} ({})",
                workflow.metadata.name, workflow.metadata.id
            );
            println!("  Description: {}", workflow.metadata.description);
            println!("  Fields: {}", workflow.fields.len());
        }
    }
}
