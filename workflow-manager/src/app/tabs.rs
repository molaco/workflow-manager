//! Tab management operations

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use workflow_manager_sdk::{WorkflowStatus, WorkflowLog};

use super::*;

impl App {
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

        // Kill process if running
        if let Some(tab) = self.open_tabs.get_mut(self.active_tab_idx) {
            if let Some(mut child) = tab.child_process.take() {
                let _ = child.kill();
            }
        }

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
            if let Some(mut child) = tab.child_process.take() {
                let _ = child.kill();
                tab.status = WorkflowStatus::Failed;
                if let Ok(mut output) = tab.workflow_output.lock() {
                    output.push(String::new());
                    output.push("⚠️ Workflow killed by user".to_string());
                }
            }
        }
    }

    pub fn rerun_current_tab(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }

        let tab = &mut self.open_tabs[self.active_tab_idx];

        // Kill existing process if running
        if let Some(mut child) = tab.child_process.take() {
            let _ = child.kill();
        }

        // Get workflow info
        let workflow_idx = tab.workflow_idx;
        let field_values = tab.field_values.clone();

        let workflow = match self.workflows.get(workflow_idx) {
            Some(w) => w,
            None => return,
        };

        let workflow_id = &workflow.info.id;
        let binary_path = PathBuf::from("../target/debug").join(workflow_id);

        // Build command arguments from saved field values
        let mut args = Vec::new();
        for field in &workflow.info.fields {
            if let Some(value) = field_values.get(&field.name) {
                if !value.is_empty() {
                    let arg_name = format!("--{}", field.name.replace('_', "-"));

                    if field.description.contains("[BOOL]") ||
                       value.eq_ignore_ascii_case("true") ||
                       value.eq_ignore_ascii_case("false") {
                        if value.eq_ignore_ascii_case("true") {
                            args.push(arg_name);
                        }
                    } else {
                        args.push(arg_name);
                        args.push(value.clone());
                    }
                }
            }
        }

        // Reset tab state
        tab.status = WorkflowStatus::Running;
        tab.exit_code = None;
        tab.start_time = Some(chrono::Local::now());

        // Clear output and phases
        if let Ok(mut output) = tab.workflow_output.lock() {
            output.clear();
        }
        if let Ok(mut phases) = tab.workflow_phases.lock() {
            phases.clear();
        }

        // Spawn new process
        match std::process::Command::new(&binary_path)
            .args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(mut child) => {
                let output_clone = Arc::clone(&tab.workflow_output);
                let phases_clone = Arc::clone(&tab.workflow_phases);

                // Spawn thread to read stdout
                if let Some(stdout) = child.stdout.take() {
                    let output = Arc::clone(&output_clone);
                    thread::spawn(move || {
                        use std::io::BufRead;
                        let reader = std::io::BufReader::new(stdout);
                        for line in reader.lines() {
                            if let Ok(line) = line {
                                if let Ok(mut output) = output.lock() {
                                    output.push(line);
                                }
                            }
                        }
                    });
                }

                // Spawn thread to read stderr and parse structured logs
                if let Some(stderr) = child.stderr.take() {
                    let output = Arc::clone(&output_clone);
                    let phases = phases_clone;
                    thread::spawn(move || {
                        use std::io::BufRead;
                        let reader = std::io::BufReader::new(stderr);
                        for line in reader.lines() {
                            if let Ok(line) = line {
                                if let Some(json_str) = line.strip_prefix("__WF_EVENT__:") {
                                    if let Ok(event) = serde_json::from_str::<WorkflowLog>(json_str) {
                                        Self::handle_workflow_event(event, &phases);
                                    }
                                } else {
                                    if let Ok(mut output) = output.lock() {
                                        output.push(format!("ERROR: {}", line));
                                    }
                                }
                            }
                        }
                    });
                }

                tab.child_process = Some(child);
            }
            Err(e) => {
                tab.status = WorkflowStatus::Failed;
                if let Ok(mut output) = tab.workflow_output.lock() {
                    output.push(format!("❌ Failed to rerun workflow: {}", e));
                    output.push(format!("   Binary path: {}", binary_path.display()));
                    output.push(format!("   Args: {:?}", args));
                }
            }
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
        else {
            if tab.expanded_phases.contains(&tab.selected_phase) {
                tab.expanded_phases.remove(&tab.selected_phase);
            } else {
                tab.expanded_phases.insert(tab.selected_phase);
            }
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

    // Poll all running tabs for process status (output is read by threads)
    pub fn poll_all_tabs(&mut self) {
        for tab in &mut self.open_tabs {
            if tab.status != WorkflowStatus::Running {
                continue;
            }

            // Check process status
            if let Some(child) = &mut tab.child_process {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        tab.status = if status.success() {
                            WorkflowStatus::Completed
                        } else {
                            WorkflowStatus::Failed
                        };
                        tab.exit_code = status.code();

                        // Add completion message
                        if let Ok(mut output) = tab.workflow_output.lock() {
                            output.push(String::new());
                            if tab.status == WorkflowStatus::Completed {
                                output.push("✅ Workflow completed successfully".to_string());

                                // Save to history on success
                                if let Some(workflow) = self.workflows.get(tab.workflow_idx) {
                                    for (field_name, value) in &tab.field_values {
                                        if !value.is_empty() {
                                            let workflow_history = self.history.workflows
                                                .entry(workflow.info.id.clone())
                                                .or_insert_with(HashMap::new);

                                            let field_history = workflow_history
                                                .entry(field_name.clone())
                                                .or_insert_with(Vec::new);

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
                                output.push(format!("❌ Workflow failed with exit code: {:?}", tab.exit_code));
                            }
                        }
                    }
                    Ok(None) => {
                        // Still running - threads are reading output
                    }
                    Err(_) => {
                        tab.status = WorkflowStatus::Failed;
                        if let Ok(mut output) = tab.workflow_output.lock() {
                            output.push("❌ Error checking workflow status".to_string());
                        }
                    }
                }
            }
        }
    }
}
