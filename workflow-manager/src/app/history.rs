//! History and session management

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use workflow_manager_sdk::WorkflowStatus;

use super::*;

impl App {
    // Session persistence
    pub fn save_session(&self) {
        #[derive(Serialize)]
        struct SavedTab {
            workflow_idx: usize,
            workflow_name: String,
            instance_number: usize,
            field_values: HashMap<String, String>,
            status: String,
            saved_logs: Vec<String>,
        }

        let saved_tabs: Vec<SavedTab> = self.open_tabs.iter()
            .map(|t| SavedTab {
                workflow_idx: t.workflow_idx,
                workflow_name: t.workflow_name.clone(),
                instance_number: t.instance_number,
                field_values: t.field_values.clone(),
                status: format!("{:?}", t.status),
                saved_logs: {
                    let mut logs = Vec::new();
                    if let Ok(output) = t.workflow_output.lock() {
                        logs = output.clone();
                    }
                    logs
                },
            })
            .collect();

        if let Some(data_dir) = directories::ProjectDirs::from("", "", "workflow-manager") {
            let session_path = data_dir.data_dir().join("session.json");
            if let Ok(json) = serde_json::to_string_pretty(&saved_tabs) {
                let _ = std::fs::write(session_path, json);
            }
        }
    }

    pub fn restore_session(&mut self) {
        #[derive(Deserialize)]
        struct SavedTab {
            workflow_idx: usize,
            workflow_name: String,
            instance_number: usize,
            field_values: HashMap<String, String>,
            status: String,
            saved_logs: Vec<String>,
        }

        if let Some(data_dir) = directories::ProjectDirs::from("", "", "workflow-manager") {
            let session_path = data_dir.data_dir().join("session.json");
            if let Ok(json) = std::fs::read_to_string(session_path) {
                if let Ok(saved_tabs) = serde_json::from_str::<Vec<SavedTab>>(&json) {
                    for saved in saved_tabs {
                        if saved.workflow_idx >= self.workflows.len() {
                            continue;
                        }

                        let status = match saved.status.as_str() {
                            "Completed" => WorkflowStatus::Completed,
                            "Failed" => WorkflowStatus::Failed,
                            _ => WorkflowStatus::NotStarted,
                        };

                        let tab = WorkflowTab {
                            id: format!("restored_{}", saved.instance_number),
                            workflow_idx: saved.workflow_idx,
                            workflow_name: saved.workflow_name,
                            instance_number: saved.instance_number,
                            start_time: None,
                            status,
                            child_process: None,
                            exit_code: None,
                            workflow_phases: Arc::new(Mutex::new(Vec::new())),
                            workflow_output: Arc::new(Mutex::new(saved.saved_logs)),
                            field_values: saved.field_values,
                            scroll_offset: 0,
                            expanded_phases: HashSet::new(),
                            expanded_tasks: HashSet::new(),
                            expanded_agents: HashSet::new(),
                            selected_phase: 0,
                            selected_task: None,
                            selected_agent: None,
                            agent_scroll_offsets: HashMap::new(),
                            saved_logs: None,
                        };

                        self.open_tabs.push(tab);

                        // Update counter
                        let workflow = &self.workflows[saved.workflow_idx];
                        let counter = self.workflow_counters
                            .entry(workflow.info.id.clone())
                            .or_insert(0);
                        if saved.instance_number >= *counter {
                            *counter = saved.instance_number + 1;
                        }
                    }
                }
            }
        }
    }

    pub fn load_latest_values_from_history(&mut self, workflow_idx: usize) {
        // Load most recent values from history for this workflow
        if let Some(workflow) = self.workflows.get(workflow_idx) {
            let workflow_id = &workflow.info.id;

            if let Some(workflow_history) = self.history.workflows.get(workflow_id) {
                // Load the most recent value for each field
                for field in &workflow.info.fields {
                    if let Some(field_history) = workflow_history.get(&field.name) {
                        if let Some(latest_value) = field_history.first() {
                            self.field_values.insert(field.name.clone(), latest_value.clone());
                        }
                    }
                }
            }
        }
    }

    pub fn save_to_history(&mut self) {
        // Save current field values to history when launching workflow
        let idx = match self.current_view {
            View::WorkflowEdit(idx) | View::WorkflowDetail(idx) => idx,
            _ => return,
        };

        if let Some(workflow) = self.workflows.get(idx) {
            let workflow_id = workflow.info.id.clone();

            // Get or create workflow history
            let workflow_history = self.history.workflows
                .entry(workflow_id)
                .or_insert_with(HashMap::new);

            // Save each field value
            for (field_name, value) in &self.field_values {
                if !value.is_empty() {
                    let field_history = workflow_history
                        .entry(field_name.clone())
                        .or_insert_with(Vec::new);

                    // Add to history if not already present
                    if !field_history.contains(value) {
                        field_history.insert(0, value.clone());

                        // Keep max 10 history items
                        if field_history.len() > 10 {
                            field_history.truncate(10);
                        }
                    } else {
                        // Move to front
                        if let Some(pos) = field_history.iter().position(|v| v == value) {
                            let val = field_history.remove(pos);
                            field_history.insert(0, val);
                        }
                    }
                }
            }

            // Save history to file
            let _ = crate::utils::save_history(&self.history);
        }
    }
}
