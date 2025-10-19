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

        // Create broadcast channel for logs (capacity 100)
        let (logs_tx, _) = broadcast::channel(100);

        // Generate execution ID
        let exec_id = Uuid::new_v4();

        // Store execution state
        let state = ExecutionState {
            workflow_id: id.to_string(),
            status: WorkflowStatus::Running,
            child: Some(child),
            logs_tx: logs_tx.clone(),
            binary_path: workflow.binary_path.clone(),
        };
        self.executions.lock().unwrap().insert(exec_id, state);

        // Spawn stderr parser task
        let executions = self.executions.clone();
        let exec_id_clone = exec_id;

        tokio::spawn(async move {
            if let Err(e) = parse_workflow_stderr(exec_id_clone, executions).await {
                eprintln!("Error parsing workflow stderr: {}", e);
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

/// Parse workflow stderr for __WF_EVENT__:<JSON> messages
async fn parse_workflow_stderr(
    exec_id: Uuid,
    executions: Arc<Mutex<HashMap<Uuid, ExecutionState>>>,
) -> Result<()> {
    // Get stderr handle
    let stderr = {
        let mut execs = executions.lock().unwrap();
        let state = execs
            .get_mut(&exec_id)
            .ok_or_else(|| anyhow!("Execution not found"))?;
        state
            .child
            .as_mut()
            .and_then(|c| c.stderr.take())
            .ok_or_else(|| anyhow!("No stderr available"))?
    };

    // Wrap in tokio async reader
    let stderr = tokio::process::ChildStderr::from_std(stderr)?;
    let reader = BufReader::new(stderr);
    let mut lines = reader.lines();

    // Parse lines
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(json_str) = line.strip_prefix("__WF_EVENT__:") {
            if let Ok(log) = serde_json::from_str::<WorkflowLog>(json_str) {
                // Broadcast to subscribers
                let execs = executions.lock().unwrap();
                if let Some(state) = execs.get(&exec_id) {
                    let _ = state.logs_tx.send(log);
                }
            }
        }
    }

    // Mark as completed
    let mut execs = executions.lock().unwrap();
    if let Some(state) = execs.get_mut(&exec_id) {
        state.status = WorkflowStatus::Completed;
    }

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
