use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::broadcast;
use uuid::Uuid;
use workflow_manager_sdk::{
    async_trait, FullWorkflowMetadata, WorkflowHandle, WorkflowLog, WorkflowResult,
    WorkflowRuntime, WorkflowStatus,
};

use crate::discovery::{discover_workflows, DiscoveredWorkflow};

/// Internal execution state for a running workflow
struct ExecutionState {
    workflow_id: String,
    status: WorkflowStatus,
    child: Option<Child>,
    logs_tx: broadcast::Sender<WorkflowLog>,
    binary_path: PathBuf,
    /// Persistent buffer of all logs for historical retrieval
    logs_buffer: Arc<Mutex<Vec<WorkflowLog>>>,
}

/// Process-based workflow runtime implementation
pub struct ProcessBasedRuntime {
    /// Discovered workflows cache (id -> workflow)
    workflows: Arc<Mutex<HashMap<String, DiscoveredWorkflow>>>,
    /// Active executions (uuid -> state)
    executions: Arc<Mutex<HashMap<Uuid, ExecutionState>>>,
}

impl ProcessBasedRuntime {
    /// Create a new runtime and discover workflows
    pub fn new() -> Result<Self> {
        let workflows = discover_workflows();
        let workflows_map: HashMap<String, DiscoveredWorkflow> = workflows
            .into_iter()
            .map(|w| (w.metadata.id.clone(), w))
            .collect();

        Ok(Self {
            workflows: Arc::new(Mutex::new(workflows_map)),
            executions: Arc::new(Mutex::new(HashMap::new())),
        })
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
        let mut cmd = self.build_command(&workflow, params);
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
            status: WorkflowStatus::Running,
            child: Some(child),
            logs_tx: logs_tx.clone(),
            binary_path: workflow.binary_path.clone(),
            logs_buffer: logs_buffer.clone(),
        };
        self.executions.lock().unwrap().insert(exec_id, state);

        // Spawn stderr parser task
        let executions_stderr = self.executions.clone();
        let exec_id_stderr = exec_id;
        tokio::spawn(async move {
            if let Err(e) = parse_workflow_stderr(exec_id_stderr, executions_stderr.clone()).await {
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
        let exec_id_stdout = exec_id;
        tokio::spawn(async move {
            if let Err(e) = parse_workflow_stdout(exec_id_stdout, executions_stdout.clone()).await {
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
        let exec_id_wait = exec_id;
        tokio::spawn(async move {
            if let Err(e) = wait_for_process_exit(exec_id_wait, executions_wait).await {
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
        let executions = self.executions.lock().unwrap();
        let state = executions
            .get(handle_id)
            .ok_or_else(|| anyhow!("Execution not found: {}", handle_id))?;

        let buffer = state.logs_buffer.lock().unwrap();
        let logs = if let Some(limit) = limit {
            buffer.iter().rev().take(limit).rev().cloned().collect()
        } else {
            buffer.clone()
        };

        Ok(logs)
    }

    async fn get_status(&self, handle_id: &Uuid) -> WorkflowResult<WorkflowStatus> {
        let executions = self.executions.lock().unwrap();
        let state = executions
            .get(handle_id)
            .ok_or_else(|| anyhow!("Execution not found: {}", handle_id))?;
        Ok(state.status.clone())
    }

    async fn cancel_workflow(&self, handle_id: &Uuid) -> WorkflowResult<()> {
        let mut executions = self.executions.lock().unwrap();
        let state = executions
            .get_mut(handle_id)
            .ok_or_else(|| anyhow!("Execution not found: {}", handle_id))?;

        if let Some(mut child) = state.child.take() {
            let _ = child.kill();
            state.status = WorkflowStatus::Failed;
        }

        Ok(())
    }
}

/// Parse workflow stderr for __WF_EVENT__:<JSON> messages and raw output
async fn parse_workflow_stderr(
    exec_id: Uuid,
    executions: Arc<Mutex<HashMap<Uuid, ExecutionState>>>,
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

            // Store in buffer for historical retrieval
            if let Ok(mut buffer) = logs_buffer.lock() {
                buffer.push(log);
            }
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

    // Parse lines without holding the main executions lock
    while let Ok(Some(line)) = lines.next_line().await {
        // All stdout is raw output
        let log = WorkflowLog::RawOutput {
            stream: "stdout".to_string(),
            line,
        };

        // Broadcast to real-time subscribers
        let _ = logs_tx.send(log.clone());

        // Store in buffer for historical retrieval
        if let Ok(mut buffer) = logs_buffer.lock() {
            buffer.push(log);
        }
    }

    Ok(())
}

/// Wait for workflow process to exit and update status accordingly
async fn wait_for_process_exit(
    exec_id: Uuid,
    executions: Arc<Mutex<HashMap<Uuid, ExecutionState>>>,
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
