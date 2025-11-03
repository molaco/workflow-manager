//! Command handler implementations for App
//!
//! This module implements the handle_command method and all related
//! command processing logic.

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use uuid::Uuid;
use workflow_manager_sdk::WorkflowLog;

use super::{App, AppCommand, View, WorkflowTab, WorkflowPane};

impl App {
    /// Process a single command
    pub fn handle_command(&mut self, cmd: AppCommand) -> Result<()> {
        match cmd {
            AppCommand::CreateTab { workflow_id, params, handle_id } => {
                self.handle_create_tab(workflow_id, params, handle_id)?;
            }

            AppCommand::AppendTabLog { handle_id, log } => {
                self.handle_append_log(handle_id, log)?;
            }

            AppCommand::UpdateTabStatus { handle_id, status } => {
                self.handle_update_status(handle_id, status)?;
            }

            AppCommand::CloseTab { handle_id } => {
                self.handle_close_tab(handle_id)?;
            }

            AppCommand::SwitchToTab { handle_id } => {
                self.handle_switch_to_tab(handle_id)?;
            }

            AppCommand::ShowNotification { level, title, message } => {
                self.notifications.push(level, title, message);
            }

            AppCommand::Quit => {
                self.should_quit = true;
            }
        }

        Ok(())
    }

    /// Create a new workflow tab
    fn handle_create_tab(
        &mut self,
        workflow_id: String,
        params: HashMap<String, String>,
        handle_id: Uuid,
    ) -> Result<()> {
        // Find workflow by ID
        let workflow_idx = self.workflows
            .iter()
            .position(|w| w.info.id == workflow_id)
            .ok_or_else(|| {
                // Show user-friendly error
                self.notifications.error(
                    "Workflow Not Found",
                    format!("Cannot create tab: workflow '{}' does not exist", workflow_id)
                );

                anyhow!("Workflow '{}' not found", workflow_id)
            })?;

        let workflow = &self.workflows[workflow_idx];

        // Generate instance number
        let counter = self.workflow_counters
            .entry(workflow_id.clone())
            .or_insert(0);
        let instance_number = *counter;
        *counter += 1;

        // Create tab
        let tab = WorkflowTab {
            id: handle_id.to_string(),
            workflow_idx,
            workflow_name: workflow.info.name.clone(),
            instance_number,
            start_time: Some(chrono::Local::now()),
            status: workflow_manager_sdk::WorkflowStatus::Running,

            // MCP workflows: no child_process, but have runtime_handle_id
            child_process: None,
            runtime_handle_id: Some(handle_id),

            exit_code: None,
            workflow_phases: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            workflow_output: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            field_values: params,
            scroll_offset: 0,
            expanded_phases: std::collections::HashSet::new(),
            expanded_tasks: std::collections::HashSet::new(),
            expanded_agents: std::collections::HashSet::new(),
            selected_phase: 0,
            selected_task: None,
            selected_agent: None,
            agent_scroll_offsets: std::collections::HashMap::new(),
            focused_pane: WorkflowPane::StructuredLogs,
            raw_output_scroll_offset: 0,
            saved_logs: None,
        };

        // Add tab and switch to it
        self.open_tabs.push(tab);
        self.active_tab_idx = self.open_tabs.len() - 1;

        // Switch to Tabs view
        self.current_view = View::Tabs;

        // Show success notification
        self.notifications.success(
            "Workflow Started",
            format!("Created tab for {} #{}", workflow.info.name, instance_number)
        );

        Ok(())
    }

    /// Append a log entry to a tab
    fn handle_append_log(&mut self, handle_id: Uuid, log: WorkflowLog) -> Result<()> {
        let handle_id_str = handle_id.to_string();

        // Find tab by handle ID
        let tab = self.open_tabs
            .iter_mut()
            .find(|t| t.id == handle_id_str)
            .ok_or_else(|| {
                // This is expected if tab was closed - don't show error to user
                // Just log it for debugging
                eprintln!("Received log for closed tab: {}", handle_id);
                anyhow!("Tab with handle {} not found", handle_id)
            })?;

        // Update structured logs (phases/tasks/agents)
        App::handle_workflow_event(log.clone(), &tab.workflow_phases);

        // ALSO update raw output buffer (for Raw Output pane)
        if let Ok(mut output) = tab.workflow_output.lock() {
            let formatted = Self::format_workflow_log(&log);
            if !formatted.is_empty() {
                output.push(formatted);
            }
        }

        Ok(())
    }

    /// Format a WorkflowLog for display in raw output
    fn format_workflow_log(log: &WorkflowLog) -> String {
        match log {
            WorkflowLog::PhaseStarted { phase, name, total_phases } => {
                format!("📋 Phase {}/{}: {}", phase + 1, total_phases, name)
            }
            WorkflowLog::PhaseCompleted { phase, name } => {
                format!("✅ Phase {} completed: {}", phase + 1, name)
            }
            WorkflowLog::PhaseFailed { phase, name, error } => {
                format!("❌ Phase {} failed: {} - {}", phase + 1, name, error)
            }
            WorkflowLog::TaskStarted { task_id, description, .. } => {
                format!("  ▶ Task {}: {}", task_id, description)
            }
            WorkflowLog::TaskProgress { task_id, message } => {
                format!("    • [{}] {}", task_id, message)
            }
            WorkflowLog::TaskCompleted { task_id, result } => {
                if let Some(r) = result {
                    format!("  ✓ Task {} completed: {}", task_id, r)
                } else {
                    format!("  ✓ Task {} completed", task_id)
                }
            }
            WorkflowLog::TaskFailed { task_id, error } => {
                format!("  ✗ Task {} failed: {}", task_id, error)
            }
            WorkflowLog::AgentStarted { agent_name, description, .. } => {
                format!("    🤖 Agent '{}': {}", agent_name, description)
            }
            WorkflowLog::AgentMessage { agent_name, message, .. } => {
                format!("       [{}] {}", agent_name, message)
            }
            WorkflowLog::AgentCompleted { agent_name, result, .. } => {
                if let Some(r) = result {
                    format!("    ✓ Agent '{}' completed: {}", agent_name, r)
                } else {
                    format!("    ✓ Agent '{}' completed", agent_name)
                }
            }
            WorkflowLog::AgentFailed { agent_name, error, .. } => {
                format!("    ✗ Agent '{}' failed: {}", agent_name, error)
            }
            WorkflowLog::StateFileCreated { phase, file_path, description } => {
                format!("  💾 Phase {}: Created {} - {}", phase + 1, file_path, description)
            }
            WorkflowLog::RawOutput { stream, line } => {
                // Match manual workflow behavior: stderr gets "ERROR:" prefix
                if stream == "stderr" {
                    format!("ERROR: {}", line)
                } else {
                    line.to_string()
                }
            }
        }
    }

    /// Update tab status
    fn handle_update_status(
        &mut self,
        handle_id: Uuid,
        status: workflow_manager_sdk::WorkflowStatus,
    ) -> Result<()> {
        let handle_id_str = handle_id.to_string();

        let tab = self.open_tabs
            .iter_mut()
            .find(|t| t.id == handle_id_str)
            .ok_or_else(|| anyhow!("Tab with handle {} not found", handle_id))?;

        tab.status = status;
        Ok(())
    }

    /// Close a tab by handle (bypasses confirmation)
    fn handle_close_tab(&mut self, handle_id: Uuid) -> Result<()> {
        let handle_id_str = handle_id.to_string();

        if let Some(idx) = self.open_tabs.iter().position(|t| t.id == handle_id_str) {
            let tab = &self.open_tabs[idx];

            // Cancel MCP workflow if applicable
            if let Some(runtime_handle_id) = tab.runtime_handle_id {
                if let Some(runtime) = &self.runtime {
                    let runtime = runtime.clone();
                    self.tokio_runtime.block_on(async {
                        let _ = runtime.cancel_workflow(&runtime_handle_id).await;
                    });
                }

                // Cancel background tasks
                self.tokio_runtime.block_on(async {
                    self.task_registry.cancel_all(&runtime_handle_id).await;
                });
            }

            // Remove tab
            self.open_tabs.remove(idx);

            // Adjust active index
            if self.open_tabs.is_empty() {
                self.active_tab_idx = 0;
            } else if self.active_tab_idx >= self.open_tabs.len() {
                self.active_tab_idx = self.open_tabs.len() - 1;
            }
        }

        Ok(())
    }

    /// Switch to a tab by handle
    fn handle_switch_to_tab(&mut self, handle_id: Uuid) -> Result<()> {
        let handle_id_str = handle_id.to_string();

        if let Some(idx) = self.open_tabs.iter().position(|t| t.id == handle_id_str) {
            self.active_tab_idx = idx;
            self.current_view = View::Tabs;
        }

        Ok(())
    }
}
