//! History and session management

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use super::*;

impl App {
    // Session persistence
    pub fn save_session(&self) {
        #[derive(Serialize)]
        struct MinimalSession {
            active_tab_idx: usize,
            pinned_executions: Vec<String>, // Store UUIDs as strings
        }

        // Save all open tabs as "pinned" executions
        let pinned_executions: Vec<String> = self
            .open_tabs
            .iter()
            .map(|t| t.runtime_handle_id.to_string())
            .collect();

        let session = MinimalSession {
            active_tab_idx: self.active_tab_idx,
            pinned_executions,
        };

        if let Some(data_dir) = directories::ProjectDirs::from("", "", "workflow-manager") {
            let session_path = data_dir.data_dir().join("session.json");
            if let Ok(json) = serde_json::to_string_pretty(&session) {
                let _ = std::fs::write(session_path, json);
            }
        }
    }

    pub fn restore_session(&mut self) {
        #[derive(Deserialize)]
        struct MinimalSession {
            active_tab_idx: usize,
            pinned_executions: Vec<String>,
        }

        // Get runtime reference - if not available, can't restore from database
        let runtime = match &self.runtime {
            Some(r) => r.clone(),
            None => return, // No runtime means no database access
        };

        if let Some(data_dir) = directories::ProjectDirs::from("", "", "workflow-manager") {
            let session_path = data_dir.data_dir().join("session.json");
            if let Ok(json) = std::fs::read_to_string(&session_path) {
                if let Ok(session) = serde_json::from_str::<MinimalSession>(&json) {
                    // Restore each pinned execution from database
                    for handle_id_str in session.pinned_executions {
                        if let Ok(handle_id) = Uuid::parse_str(&handle_id_str) {
                            if let Some(tab) = self.create_tab_from_database(&runtime, &handle_id) {
                                self.open_tabs.push(tab);
                            }
                        }
                    }

                    // Restore active tab index (clamp to valid range)
                    if !self.open_tabs.is_empty() {
                        self.active_tab_idx = session.active_tab_idx.min(self.open_tabs.len() - 1);
                    }
                }
            }
        }
    }

    /// Create a WorkflowTab from database using handle_id
    fn create_tab_from_database(
        &mut self,
        runtime: &Arc<dyn workflow_manager_sdk::WorkflowRuntime>,
        handle_id: &Uuid,
    ) -> Option<WorkflowTab> {
        // Use tokio runtime to call async database methods
        let execution = self.tokio_runtime.block_on(async {
            // Get execution from database via runtime
            // Query database directly since runtime doesn't expose get_execution yet
            // We need to access the ProcessBasedRuntime's database
            // For now, we'll use list_executions with filtering to find our execution
            match runtime.list_executions(1000, 0, None).await {
                Ok(executions) => executions.into_iter().find(|e| e.id == *handle_id),
                Err(_) => None,
            }
        })?;

        // Find workflow index by workflow_id
        let workflow_idx = self
            .workflows
            .iter()
            .position(|w| w.info.id == execution.workflow_id)?;

        // Get params from database
        let field_values = self.tokio_runtime.block_on(async {
            // Similar issue - we need database access
            // For now return empty, will fix in next iteration
            HashMap::new()
        });

        // Get logs from database and process them properly
        let (workflow_phases, raw_output) = self.tokio_runtime.block_on(async {
            match runtime.get_logs(handle_id, None).await {
                Ok(workflow_logs) => {
                    // Create phases structure for structured logs
                    let phases = Arc::new(Mutex::new(Vec::new()));
                    let mut raw_logs = Vec::new();

                    // Process each log
                    for log in workflow_logs {
                        // Process structured logs (phases, tasks, agents)
                        App::handle_workflow_event(log.clone(), &phases);

                        // Only add RawOutput to text buffer (same as live execution)
                        if let workflow_manager_sdk::WorkflowLog::RawOutput { line, .. } = &log {
                            raw_logs.push(line.clone());
                        }
                    }

                    (phases, raw_logs)
                }
                Err(_) => (Arc::new(Mutex::new(Vec::new())), Vec::new()),
            }
        });

        // Generate instance number
        let workflow = &self.workflows[workflow_idx];
        let counter = self
            .workflow_counters
            .entry(workflow.info.id.clone())
            .or_insert(0);
        let instance_number = *counter;
        *counter += 1;

        Some(WorkflowTab {
            id: format!("restored_{}", handle_id),
            workflow_idx,
            workflow_name: execution.workflow_name.clone(),
            instance_number,
            start_time: Some(execution.start_time),
            status: execution.status,
            runtime_handle_id: execution.id, // Use REAL handle_id from database!
            exit_code: execution.exit_code,
            workflow_phases, // Use properly processed phases
            workflow_output: Arc::new(Mutex::new(raw_output)), // Only raw stdout/stderr
            field_values,
            scroll_offset: 0,
            expanded_phases: HashSet::new(),
            expanded_tasks: HashSet::new(),
            expanded_agents: HashSet::new(),
            selected_phase: 0,
            selected_task: None,
            selected_agent: None,
            agent_scroll_offsets: HashMap::new(),
            focused_pane: WorkflowPane::StructuredLogs,
            raw_output_scroll_offset: 0,
            saved_logs: None,
        })
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
                            self.field_values
                                .insert(field.name.clone(), latest_value.clone());
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
            let workflow_history = self.history.workflows.entry(workflow_id).or_default();

            // Save each field value
            for (field_name, value) in &self.field_values {
                if !value.is_empty() {
                    let field_history = workflow_history.entry(field_name.clone()).or_default();

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
