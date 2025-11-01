use claude_agent_sdk::mcp::{SdkMcpServer, SdkMcpTool, ToolResult};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use workflow_manager_sdk::WorkflowRuntime;

use crate::models::WorkflowHistory;

/// Create the workflow manager MCP server with all tools
pub fn create_workflow_mcp_server(
    runtime: Arc<dyn WorkflowRuntime>,
    history: Arc<Mutex<WorkflowHistory>>,
) -> SdkMcpServer {
    SdkMcpServer::new("workflow_manager")
        .version("1.0.0")
        .tool(list_workflows_tool(runtime.clone()))
        .tool(execute_workflow_tool(runtime.clone()))
        .tool(get_workflow_logs_tool(runtime.clone()))
        .tool(get_workflow_status_tool(runtime.clone()))
        .tool(cancel_workflow_tool(runtime))
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
fn execute_workflow_tool(runtime: Arc<dyn WorkflowRuntime>) -> SdkMcpTool {
    SdkMcpTool::new(
        "execute_workflow",
        "Execute a workflow with provided parameters",
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

                match runtime.execute_workflow(workflow_id, params_map).await {
                    Ok(handle) => {
                        let result = json!({
                            "handle_id": handle.id().to_string(),
                            "workflow_id": handle.workflow_id,
                            "status": "running"
                        });
                        Ok(ToolResult::text(
                            serde_json::to_string_pretty(&result).unwrap(),
                        ))
                    }
                    Err(e) => Ok(ToolResult::error(format!("Execution failed: {}", e))),
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

                let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

                match runtime.subscribe_logs(&handle_id).await {
                    Ok(mut logs_rx) => {
                        let mut logs = Vec::new();
                        while logs.len() < limit {
                            match logs_rx.try_recv() {
                                Ok(log) => logs.push(log),
                                Err(_) => break,
                            }
                        }
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
        let server = create_workflow_mcp_server(runtime, history);
        println!("MCP Server created: {}", server.name());
    }
}
