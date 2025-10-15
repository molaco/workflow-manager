//! Utility functions for workflow and history management

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use workflow_manager_sdk::{Workflow, WorkflowInfo, WorkflowSource, WorkflowStatus};

use crate::models::WorkflowHistory;

/// Get the path to the history file
pub fn history_file_path() -> PathBuf {
    use directories::ProjectDirs;

    if let Some(proj_dirs) = ProjectDirs::from("com", "workflow-manager", "workflow-manager") {
        proj_dirs.data_dir().join("history.json")
    } else {
        PathBuf::from(".workflow-manager-history.json")
    }
}

/// Load workflow history from disk
pub fn load_history() -> WorkflowHistory {
    let path = history_file_path();
    if let Ok(content) = std::fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        WorkflowHistory::default()
    }
}

/// Save workflow history to disk
pub fn save_history(history: &WorkflowHistory) -> Result<()> {
    let path = history_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(history)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Load all workflows (built-in and discovered)
pub fn load_workflows() -> Vec<Workflow> {
    let mut workflows = Vec::new();

    // Load built-in workflows using discovery module
    workflows.extend(crate::discovery::discover_workflows().into_iter().map(|dw| {
        Workflow {
            info: WorkflowInfo {
                id: dw.metadata.id.clone(),
                name: dw.metadata.name.clone(),
                description: dw.metadata.description.clone(),
                status: WorkflowStatus::NotStarted,
                metadata: dw.metadata,
                fields: dw.fields,
                progress_messages: vec![],
            },
            source: WorkflowSource::BuiltIn,
        }
    }));

    workflows
}
