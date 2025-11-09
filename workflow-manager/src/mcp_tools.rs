use claude_agent_sdk::mcp::{SdkMcpServer, SdkMcpTool, ToolResult};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use workflow_manager_sdk::WorkflowRuntime;

use crate::models::WorkflowHistory;
use crate::app::{AppCommand, NotificationLevel, TaskRegistry};

/// Create the workflow manager MCP server with all tools
pub fn create_workflow_mcp_server(
    runtime: Arc<dyn WorkflowRuntime>,
    history: Arc<Mutex<WorkflowHistory>>,
    command_tx: tokio::sync::mpsc::UnboundedSender<AppCommand>,
    task_registry: TaskRegistry,
) -> SdkMcpServer {
    SdkMcpServer::new("workflow_manager")
        .version("1.0.0")
        .tool(list_workflows_tool(runtime.clone()))
        .tool(execute_workflow_tool(
            runtime.clone(),
            command_tx.clone(),
            task_registry.clone(),
        ))
        .tool(get_workflow_logs_tool(runtime.clone()))
        .tool(get_workflow_status_tool(runtime.clone()))
        .tool(cancel_workflow_tool(runtime.clone()))
        .tool(list_execution_history_tool(runtime))
        .tool(get_workflow_history_tool(history))
}

/// Tool: list_workflows
fn list_workflows_tool(runtime: Arc<dyn WorkflowRuntime>) -> SdkMcpTool {
    SdkMcpTool::new(
        "list_workflows",
        "List all available workflows with their metadata and input schemas",
        json!({"type": "object", "properties": {}}),
        move |_params| {
            let runtime = runtime.clone();
            Box::pin(async move {
                match runtime.list_workflows() {
                    Ok(workflows) => match serde_json::to_string_pretty(&workflows) {
                        Ok(json) => Ok(ToolResult::text(json)),
                        Err(e) => Ok(ToolResult::error(format!("Serialization error: {}", e))),
                    },
                    Err(e) => Ok(ToolResult::error(format!(
                        "Failed to list workflows: {}",
                        e
                    ))),
                }
            })
        },
    )
}

/// Tool: execute_workflow
fn execute_workflow_tool(
    runtime: Arc<dyn WorkflowRuntime>,
    command_tx: tokio::sync::mpsc::UnboundedSender<AppCommand>,
    task_registry: TaskRegistry,
) -> SdkMcpTool {
    SdkMcpTool::new(
        "execute_workflow",
        "Execute a workflow with provided parameters. Creates a tab in the TUI and streams logs in real-time.",
        json!({
            "type": "object",
            "properties": {
                "workflow_id": {"type": "string"},
                "parameters": {"type": "object"}
            },
            "required": ["workflow_id", "parameters"]
        }),
        move |params| {
            let runtime = runtime.clone();
            let command_tx = command_tx.clone();
            let task_registry = task_registry.clone();

            Box::pin(async move {
                let workflow_id = match params.get("workflow_id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(ToolResult::error("Missing workflow_id")),
                };

                let parameters = match params.get("parameters").and_then(|v| v.as_object()) {
                    Some(p) => p,
                    None => return Ok(ToolResult::error("Missing parameters")),
                };

                let params_map: std::collections::HashMap<String, String> = parameters
                    .iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                    .collect();

                // Execute workflow via runtime
                match runtime.execute_workflow(workflow_id, params_map.clone()).await {
                    Ok(handle) => {
                        let handle_id = *handle.id();

                        // 1. Send command to create tab in TUI
                        if let Err(e) = command_tx.send(AppCommand::CreateTab {
                            workflow_id: workflow_id.to_string(),
                            params: params_map,
                            handle_id,
                        }) {
                            eprintln!("Failed to send CreateTab command: {}", e);
                            return Ok(ToolResult::error("Failed to create tab"));
                        }

                        // 2. Send success notification
                        let _ = command_tx.send(AppCommand::ShowNotification {
                            level: NotificationLevel::Success,
                            title: "Workflow Started".to_string(),
                            message: format!("Executing {}", workflow_id),
                        });

                        // 3. Spawn task to stream logs to tab (no rate limiting - send immediately)
                        let log_task = tokio::spawn({
                            let runtime_clone = runtime.clone();
                            let command_tx_clone = command_tx.clone();

                            async move {
                                if let Ok(mut logs_rx) = runtime_clone.subscribe_logs(&handle_id).await {
                                    // Stream logs immediately as they arrive
                                    loop {
                                        match logs_rx.recv().await {
                                            Ok(log) => {
                                                if command_tx_clone.send(AppCommand::AppendTabLog {
                                                    handle_id,
                                                    log,
                                                }).is_err() {
                                                    // App shut down or tab closed
                                                    return;
                                                }
                                            }
                                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                                eprintln!("Warning: MCP log receiver lagged by {} messages for workflow {}", n, handle_id);
                                                // Send notification about lag
                                                let _ = command_tx_clone.send(AppCommand::ShowNotification {
                                                    level: NotificationLevel::Warning,
                                                    title: "Log Stream Lagged".to_string(),
                                                    message: format!("Skipped {} messages due to high log volume", n),
                                                });
                                                // Continue receiving - don't exit on lag
                                                continue;
                                            }
                                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                                // Channel closed, exit gracefully
                                                break;
                                            }
                                        }
                                    }
                                }

                                // When logs stream ends, update status
                                if let Ok(status) = runtime_clone.get_status(&handle_id).await {
                                    let _ = command_tx_clone.send(AppCommand::UpdateTabStatus {
                                        handle_id,
                                        status,
                                    });
                                }
                            }
                        });

                        // 4. Register task for cleanup
                        task_registry.register(handle_id, log_task).await;

                        // Return success to Claude
                        let result = json!({
                            "handle_id": handle_id.to_string(),
                            "workflow_id": handle.workflow_id,
                            "status": "running",
                            "message": "Workflow started and tab created in TUI"
                        });
                        Ok(ToolResult::text(
                            serde_json::to_string_pretty(&result).unwrap(),
                        ))
                    }
                    Err(e) => {
                        // Send error notification
                        let _ = command_tx.send(AppCommand::ShowNotification {
                            level: NotificationLevel::Error,
                            title: "Workflow Failed".to_string(),
                            message: format!("Failed to start {}: {}", workflow_id, e),
                        });

                        Ok(ToolResult::error(format!("Execution failed: {}", e)))
                    }
                }
            })
        },
    )
}

/// Tool: get_workflow_logs
fn get_workflow_logs_tool(runtime: Arc<dyn WorkflowRuntime>) -> SdkMcpTool {
    SdkMcpTool::new(
        "get_workflow_logs",
        "Get logs from a workflow execution",
        json!({
            "type": "object",
            "properties": {
                "handle_id": {"type": "string"},
                "limit": {"type": "integer", "default": 50}
            },
            "required": ["handle_id"]
        }),
        move |params| {
            let runtime = runtime.clone();
            Box::pin(async move {
                let handle_id_str = match params.get("handle_id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(ToolResult::error("Missing handle_id")),
                };

                let handle_id = match Uuid::parse_str(handle_id_str) {
                    Ok(id) => id,
                    Err(e) => return Ok(ToolResult::error(format!("Invalid UUID: {}", e))),
                };

                let limit = params.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);

                match runtime.get_logs(&handle_id, limit).await {
                    Ok(logs) => {
                        match serde_json::to_string_pretty(&logs) {
                            Ok(json) => Ok(ToolResult::text(json)),
                            Err(e) => Ok(ToolResult::error(format!("Serialization error: {}", e))),
                        }
                    }
                    Err(e) => Ok(ToolResult::error(format!("Failed to get logs: {}", e))),
                }
            })
        },
    )
}

/// Tool: get_workflow_status
fn get_workflow_status_tool(runtime: Arc<dyn WorkflowRuntime>) -> SdkMcpTool {
    SdkMcpTool::new(
        "get_workflow_status",
        "Get the current status of a workflow execution",
        json!({
            "type": "object",
            "properties": {
                "handle_id": {"type": "string"}
            },
            "required": ["handle_id"]
        }),
        move |params| {
            let runtime = runtime.clone();
            Box::pin(async move {
                let handle_id_str = match params.get("handle_id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(ToolResult::error("Missing handle_id")),
                };

                let handle_id = match Uuid::parse_str(handle_id_str) {
                    Ok(id) => id,
                    Err(e) => return Ok(ToolResult::error(format!("Invalid UUID: {}", e))),
                };

                match runtime.get_status(&handle_id).await {
                    Ok(status) => {
                        let result = json!({
                            "handle_id": handle_id.to_string(),
                            "status": format!("{:?}", status)
                        });
                        Ok(ToolResult::text(
                            serde_json::to_string_pretty(&result).unwrap(),
                        ))
                    }
                    Err(e) => Ok(ToolResult::error(format!("Failed to get status: {}", e))),
                }
            })
        },
    )
}

/// Tool: cancel_workflow
fn cancel_workflow_tool(runtime: Arc<dyn WorkflowRuntime>) -> SdkMcpTool {
    SdkMcpTool::new(
        "cancel_workflow",
        "Cancel a running workflow execution",
        json!({
            "type": "object",
            "properties": {
                "handle_id": {"type": "string"}
            },
            "required": ["handle_id"]
        }),
        move |params| {
            let runtime = runtime.clone();
            Box::pin(async move {
                let handle_id_str = match params.get("handle_id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(ToolResult::error("Missing handle_id")),
                };

                let handle_id = match Uuid::parse_str(handle_id_str) {
                    Ok(id) => id,
                    Err(e) => return Ok(ToolResult::error(format!("Invalid UUID: {}", e))),
                };

                match runtime.cancel_workflow(&handle_id).await {
                    Ok(_) => {
                        let result = json!({
                            "handle_id": handle_id.to_string(),
                            "status": "cancelled"
                        });
                        Ok(ToolResult::text(
                            serde_json::to_string_pretty(&result).unwrap(),
                        ))
                    }
                    Err(e) => Ok(ToolResult::error(format!("Failed to cancel: {}", e))),
                }
            })
        },
    )
}

/// Tool: list_execution_history
///
/// TODO: Full implementation requires adding list_executions() method to WorkflowRuntime trait.
/// This is a skeleton implementation that provides the MCP interface.
fn list_execution_history_tool(runtime: Arc<dyn WorkflowRuntime>) -> SdkMcpTool {
    SdkMcpTool::new(
        "list_execution_history",
        "List recent workflow executions with pagination and optional filtering",
        json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of executions to return",
                    "default": 10
                },
                "offset": {
                    "type": "integer",
                    "description": "Number of executions to skip (for pagination)",
                    "default": 0
                },
                "workflow_id": {
                    "type": "string",
                    "description": "Filter by specific workflow type (optional)"
                }
            }
        }),
        move |params| {
            let _runtime = runtime.clone();
            Box::pin(async move {
                let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
                let offset = params.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let workflow_id = params.get("workflow_id").and_then(|v| v.as_str()).map(String::from);

                // TODO: Query via runtime - need to add list_executions() method to WorkflowRuntime trait
                // Expected signature: async fn list_executions(&self, limit: usize, offset: usize, workflow_id: Option<String>) -> Result<Vec<ExecutionRecord>>
                // For now, return placeholder response

                let result = json!({
                    "message": "list_execution_history implementation pending - requires WorkflowRuntime trait extension",
                    "requested_params": {
                        "limit": limit,
                        "offset": offset,
                        "workflow_id": workflow_id
                    }
                });
                Ok(ToolResult::text(serde_json::to_string_pretty(&result).unwrap()))
            })
        },
    )
}

/// Tool: get_workflow_history
fn get_workflow_history_tool(history: Arc<Mutex<WorkflowHistory>>) -> SdkMcpTool {
    SdkMcpTool::new(
        "get_workflow_history",
        "Get previous parameter values used for a workflow. Returns the history of field values from past executions.",
        json!({
            "type": "object",
            "properties": {
                "workflow_id": {
                    "type": "string",
                    "description": "The ID of the workflow to get history for"
                },
                "field_name": {
                    "type": "string",
                    "description": "Optional: Filter to a specific field name. If omitted, returns all field history."
                }
            },
            "required": ["workflow_id"]
        }),
        move |params| {
            let history = history.clone();
            Box::pin(async move {
                let workflow_id = match params.get("workflow_id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(ToolResult::error("Missing workflow_id")),
                };

                let field_name = params.get("field_name").and_then(|v| v.as_str());

                let history_lock = history.lock().await;

                // Get workflow history
                let workflow_history = match history_lock.workflows.get(workflow_id) {
                    Some(h) => h,
                    None => {
                        // No history for this workflow yet
                        let result = json!({
                            "workflow_id": workflow_id,
                            "field_history": {}
                        });
                        return Ok(ToolResult::text(
                            serde_json::to_string_pretty(&result).unwrap(),
                        ));
                    }
                };

                // Filter by field_name if provided
                let field_history = if let Some(field) = field_name {
                    // Return only the specified field
                    let mut filtered = serde_json::Map::new();
                    if let Some(values) = workflow_history.get(field) {
                        filtered.insert(field.to_string(), json!(values));
                    }
                    filtered
                } else {
                    // Return all fields
                    workflow_history
                        .iter()
                        .map(|(k, v)| (k.clone(), json!(v)))
                        .collect()
                };

                let result = json!({
                    "workflow_id": workflow_id,
                    "field_history": field_history
                });

                Ok(ToolResult::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                ))
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::ProcessBasedRuntime;

    #[tokio::test]
    async fn test_create_mcp_server() {
        let runtime = Arc::new(ProcessBasedRuntime::new().unwrap());
        let history = Arc::new(Mutex::new(WorkflowHistory::default()));
        let (command_tx, _command_rx) = tokio::sync::mpsc::unbounded_channel();
        let task_registry = TaskRegistry::new();
        let server = create_workflow_mcp_server(runtime, history, command_tx, task_registry);
        println!("MCP Server created: {}", server.name());
    }
}
