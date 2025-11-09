//! Tab management operations

use workflow_manager_sdk::WorkflowStatus;

use super::*;

impl App {
    // Helper to strip ANSI color codes from strings
    fn strip_ansi_codes(s: &str) -> String {
        // Regex to match ANSI escape sequences
        // Pattern: ESC [ ... m (where ... is parameters like 1;36)
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '\x1b' {
                // Check if this is an ANSI escape sequence
                if chars.peek() == Some(&'[') {
                    chars.next(); // consume '['
                    // Skip until we find 'm' or reach end
                    while let Some(&next_ch) = chars.peek() {
                        chars.next();
                        if next_ch == 'm' {
                            break;
                        }
                    }
                } else {
                    // Not an escape sequence, keep the character
                    result.push(ch);
                }
            } else {
                result.push(ch);
            }
        }

        result
    }

    // Tab navigation
    pub fn next_tab(&mut self) {
        if !self.open_tabs.is_empty() {
            self.active_tab_idx = (self.active_tab_idx + 1) % self.open_tabs.len();
        }
    }

    pub fn previous_tab(&mut self) {
        if !self.open_tabs.is_empty() {
            self.active_tab_idx = if self.active_tab_idx == 0 {
                self.open_tabs.len() - 1
            } else {
                self.active_tab_idx - 1
            };
        }
    }

    // Tab management actions
    pub fn close_current_tab(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }

        let tab = &self.open_tabs[self.active_tab_idx];

        // If running, show confirmation
        if tab.status == WorkflowStatus::Running {
            self.show_close_confirmation = true;
            return;
        }

        // Close tab directly if not running
        self.close_tab_confirmed();
    }

    pub fn close_tab_confirmed(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }

        let tab = &self.open_tabs[self.active_tab_idx];
        let handle_id = tab.runtime_handle_id;

        // Unified path: Cancel ALL workflows via runtime
        if let Some(runtime) = &self.runtime {
            let runtime = runtime.clone();
            self.tokio_runtime.block_on(async {
                if let Err(e) = runtime.cancel_workflow(&handle_id).await {
                    eprintln!("Failed to cancel workflow {}: {}", handle_id, e);
                }
            });
        }

        // Cancel background tasks (log streamers, etc.)
        self.tokio_runtime.block_on(async {
            self.task_registry.cancel_all(&handle_id).await;
        });

        // Remove tab
        self.open_tabs.remove(self.active_tab_idx);

        // Adjust active index
        if self.open_tabs.is_empty() {
            self.active_tab_idx = 0;
        } else if self.active_tab_idx >= self.open_tabs.len() {
            self.active_tab_idx = self.open_tabs.len() - 1;
        }

        self.show_close_confirmation = false;
    }

    pub fn kill_current_tab(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }

        if let Some(tab) = self.open_tabs.get_mut(self.active_tab_idx) {
            let handle_id = tab.runtime_handle_id;

            // Unified path: Cancel ALL workflows via runtime
            if let Some(runtime) = &self.runtime {
                let runtime = runtime.clone();
                self.tokio_runtime.block_on(async {
                    let _ = runtime.cancel_workflow(&handle_id).await;
                });
            }

            // Cancel background tasks
            self.tokio_runtime.block_on(async {
                self.task_registry.cancel_all(&handle_id).await;
            });

            tab.status = WorkflowStatus::Failed;
            if let Ok(mut output) = tab.workflow_output.lock() {
                output.push(String::new());
                output.push("⚠️ Workflow killed by user".to_string());
            }
        }
    }

    pub fn rerun_current_tab(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }

        let tab = &mut self.open_tabs[self.active_tab_idx];
        let handle_id = tab.runtime_handle_id;

        // Cancel existing workflow if running
        if let Some(runtime) = &self.runtime {
            let runtime = runtime.clone();
            self.tokio_runtime.block_on(async {
                let _ = runtime.cancel_workflow(&handle_id).await;
            });
        }

        // TODO: Implement rerun via runtime.execute_workflow()
        // For now, just show error message
        tab.status = WorkflowStatus::Failed;
        if let Ok(mut output) = tab.workflow_output.lock() {
            output.clear();
            output.push("❌ Rerun not yet implemented with runtime-based execution".to_string());
            output.push("   Please close this tab and create a new one.".to_string());
        }
    }

    pub fn toggle_tab_item(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }
        let tab = &mut self.open_tabs[self.active_tab_idx];

        // If agent is selected, toggle agent expansion
        if let Some(ref agent_id) = tab.selected_agent {
            if tab.expanded_agents.contains(agent_id) {
                tab.expanded_agents.remove(agent_id);
            } else {
                tab.expanded_agents.insert(agent_id.clone());
            }
        }
        // If task is selected, toggle task expansion
        else if let Some(ref task_id) = tab.selected_task {
            if tab.expanded_tasks.contains(task_id) {
                tab.expanded_tasks.remove(task_id);
            } else {
                tab.expanded_tasks.insert(task_id.clone());
            }
        }
        // Otherwise, toggle phase expansion
        else if tab.expanded_phases.contains(&tab.selected_phase) {
            tab.expanded_phases.remove(&tab.selected_phase);
        } else {
            tab.expanded_phases.insert(tab.selected_phase);
        }
    }

    pub fn toggle_tab_expand_all(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }
        let tab = &mut self.open_tabs[self.active_tab_idx];

        if let Ok(phases) = tab.workflow_phases.lock() {
            let all_expanded = phases.iter().all(|p| tab.expanded_phases.contains(&p.id));

            if all_expanded {
                // Collapse all
                tab.expanded_phases.clear();
                tab.expanded_tasks.clear();
                tab.expanded_agents.clear();
            } else {
                // Expand all
                for phase in phases.iter() {
                    tab.expanded_phases.insert(phase.id);
                    for task in &phase.tasks {
                        tab.expanded_tasks.insert(task.id.clone());
                        for agent in &task.agents {
                            tab.expanded_agents.insert(agent.id.clone());
                        }
                    }
                }
            }
        }
    }

    // Poll all running tabs for process status (logs are streamed via runtime)
    pub fn poll_all_tabs(&mut self) {
        if self.runtime.is_none() {
            return;
        }

        let runtime = self.runtime.as_ref().unwrap().clone();

        for tab in &mut self.open_tabs {
            if tab.status != WorkflowStatus::Running {
                continue;
            }

            let handle_id = tab.runtime_handle_id;

            // Check workflow status via runtime
            let status = self.tokio_runtime.block_on(async {
                runtime.get_status(&handle_id).await
            });

            if let Ok(new_status) = status {
                if new_status != WorkflowStatus::Running {
                    tab.status = new_status.clone();

                    // Add completion message
                    if let Ok(mut output) = tab.workflow_output.lock() {
                        output.push(String::new());
                        if new_status == WorkflowStatus::Completed {
                            output.push("✅ Workflow completed successfully".to_string());

                            // Save to history on success
                            if let Some(workflow) = self.workflows.get(tab.workflow_idx) {
                                for (field_name, value) in &tab.field_values {
                                    if !value.is_empty() {
                                        let workflow_history = self
                                            .history
                                            .workflows
                                            .entry(workflow.info.id.clone())
                                            .or_default();

                                        let field_history = workflow_history
                                            .entry(field_name.clone())
                                            .or_default();

                                        if !field_history.contains(value) {
                                            field_history.insert(0, value.clone());
                                            if field_history.len() > 10 {
                                                field_history.truncate(10);
                                            }
                                        }
                                    }
                                }
                                let _ = crate::utils::save_history(&self.history);
                            }
                        } else {
                            output.push("❌ Workflow failed".to_string());
                        }
                    }
                }
            }
        }
    }
}
