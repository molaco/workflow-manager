//! Navigation methods for workflow and tab hierarchies

use super::*;

impl App {
    pub fn next(&mut self) {
        match self.current_view {
            View::WorkflowList => {
                if self.selected < self.workflows.len().saturating_sub(1) {
                    self.selected += 1;
                }
            }
            View::WorkflowEdit(idx) => {
                // Navigate to next field
                if let Some(workflow) = self.workflows.get(idx) {
                    if self.edit_field_index < workflow.info.fields.len().saturating_sub(1) {
                        self.edit_field_index += 1;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn previous(&mut self) {
        match self.current_view {
            View::WorkflowList => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            View::WorkflowEdit(_) => {
                // Navigate to previous field
                if self.edit_field_index > 0 {
                    self.edit_field_index -= 1;
                }
            }
            _ => {}
        }
    }

    // Tab navigation methods
    pub fn navigate_tab_down(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }
        let tab = &mut self.open_tabs[self.active_tab_idx];

        if let Ok(phases) = tab.workflow_phases.lock() {
            if phases.is_empty() {
                return;
            }

            let mut just_exited_agent = false;

            // If agent is selected, try to move to next agent
            if let Some(ref agent_id) = tab.selected_agent.clone() {
                if let Some(ref task_id) = tab.selected_task {
                    if let Some(phase) = phases.get(tab.selected_phase) {
                        if let Some(task) = phase.tasks.iter().find(|t| &t.id == task_id) {
                            if let Some(agent_idx) =
                                task.agents.iter().position(|a| &a.id == agent_id)
                            {
                                if agent_idx + 1 < task.agents.len() {
                                    // Move to next agent in same task
                                    tab.selected_agent =
                                        Some(task.agents[agent_idx + 1].id.clone());
                                    return;
                                }
                            }
                        }
                    }
                }
                // No next agent, move to next task
                tab.selected_agent = None;
                just_exited_agent = true;
            }

            // If task is selected, try to move to next task or dive into agents
            if let Some(ref task_id) = tab.selected_task.clone() {
                if let Some(phase) = phases.get(tab.selected_phase) {
                    if let Some(task_idx) = phase.tasks.iter().position(|t| &t.id == task_id) {
                        let task = &phase.tasks[task_idx];

                        // If task is expanded and has agents, dive into first agent (but only if we didn't just exit an agent)
                        if !just_exited_agent
                            && tab.expanded_tasks.contains(task_id)
                            && !task.agents.is_empty()
                        {
                            tab.selected_agent = Some(task.agents[0].id.clone());
                            return;
                        }

                        // Move to next task in same phase
                        if task_idx + 1 < phase.tasks.len() {
                            tab.selected_task = Some(phase.tasks[task_idx + 1].id.clone());
                            return;
                        }
                    }
                }
                // No next task in this phase, move to next phase
                tab.selected_task = None;
                if tab.selected_phase + 1 < phases.len() {
                    tab.selected_phase += 1;
                }
                return;
            }

            // Navigate phases or dive into tasks
            let phase = &phases[tab.selected_phase];

            // If current phase is expanded and has tasks, dive into first task
            if tab.expanded_phases.contains(&tab.selected_phase) && !phase.tasks.is_empty() {
                tab.selected_task = Some(phase.tasks[0].id.clone());
                return;
            }

            // Move to next phase
            if tab.selected_phase + 1 < phases.len() {
                tab.selected_phase += 1;
            }
        }
    }

    pub fn navigate_tab_up(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }
        let tab = &mut self.open_tabs[self.active_tab_idx];

        if let Ok(phases) = tab.workflow_phases.lock() {
            if phases.is_empty() {
                return;
            }

            // If agent is selected, try to move to previous agent
            if let Some(ref agent_id) = tab.selected_agent.clone() {
                if let Some(ref task_id) = tab.selected_task {
                    if let Some(phase) = phases.get(tab.selected_phase) {
                        if let Some(task) = phase.tasks.iter().find(|t| &t.id == task_id) {
                            if let Some(agent_idx) =
                                task.agents.iter().position(|a| &a.id == agent_id)
                            {
                                if agent_idx > 0 {
                                    // Move to previous agent
                                    tab.selected_agent =
                                        Some(task.agents[agent_idx - 1].id.clone());
                                    return;
                                } else {
                                    // Move back to task level
                                    tab.selected_agent = None;
                                    return;
                                }
                            }
                        }
                    }
                }
            }

            // If task is selected, try to move to previous task
            if let Some(ref task_id) = tab.selected_task.clone() {
                if let Some(phase) = phases.get(tab.selected_phase) {
                    if let Some(task_idx) = phase.tasks.iter().position(|t| &t.id == task_id) {
                        if task_idx > 0 {
                            // Move to previous task
                            let prev_task = &phase.tasks[task_idx - 1];
                            tab.selected_task = Some(prev_task.id.clone());

                            // If previous task is expanded and has agents, select last agent
                            if tab.expanded_tasks.contains(&prev_task.id)
                                && !prev_task.agents.is_empty()
                            {
                                tab.selected_agent =
                                    Some(prev_task.agents.last().unwrap().id.clone());
                            }
                            return;
                        } else {
                            // Move back to phase level
                            tab.selected_task = None;
                            return;
                        }
                    }
                }
            }

            // Navigate phases
            if tab.selected_phase > 0 {
                tab.selected_phase -= 1;

                // If moving to previous phase that's expanded with tasks, select last task
                if let Some(phase) = phases.get(tab.selected_phase) {
                    if tab.expanded_phases.contains(&tab.selected_phase) && !phase.tasks.is_empty()
                    {
                        let last_task = phase.tasks.last().unwrap();
                        tab.selected_task = Some(last_task.id.clone());

                        // If last task is expanded with agents, select last agent
                        if tab.expanded_tasks.contains(&last_task.id)
                            && !last_task.agents.is_empty()
                        {
                            tab.selected_agent = Some(last_task.agents.last().unwrap().id.clone());
                        }
                    }
                }
            }
        }
    }

    pub fn scroll_agent_messages_up(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }
        let tab = &mut self.open_tabs[self.active_tab_idx];

        if let Some(ref agent_id) = tab.selected_agent {
            let offset = tab
                .agent_scroll_offsets
                .entry(agent_id.clone())
                .or_insert(0);
            if *offset > 0 {
                *offset -= 1;
            }
        }
    }

    pub fn scroll_agent_messages_down(&mut self) {
        if self.open_tabs.is_empty() {
            return;
        }
        let tab = &mut self.open_tabs[self.active_tab_idx];

        if let Some(ref agent_id) = tab.selected_agent {
            // Find the agent to check message count
            if let Ok(phases) = tab.workflow_phases.lock() {
                for phase in phases.iter() {
                    for task in &phase.tasks {
                        if let Some(agent) = task.agents.iter().find(|a| &a.id == agent_id) {
                            let offset = tab
                                .agent_scroll_offsets
                                .entry(agent_id.clone())
                                .or_insert(0);
                            let window_size = 5;
                            let max_offset = agent.messages.len().saturating_sub(window_size);
                            if *offset < max_offset {
                                *offset += 1;
                            }
                            return;
                        }
                    }
                }
            }
        }
    }

    pub fn navigate_workflow_down(&mut self) {
        if let Ok(phases) = self.workflow_phases.lock() {
            if phases.is_empty() {
                return;
            }

            let mut just_exited_agent = false;

            // If we're on an agent, try to move to next agent in same task
            if let Some(ref agent_id) = self.selected_agent {
                let phase = &phases[self.selected_phase];
                if let Some(task) = phase
                    .tasks
                    .iter()
                    .find(|t| Some(&t.id) == self.selected_task.as_ref())
                {
                    let current_idx = task.agents.iter().position(|a| &a.id == agent_id);
                    if let Some(idx) = current_idx {
                        if idx + 1 < task.agents.len() {
                            self.selected_agent = Some(task.agents[idx + 1].id.clone());
                            return;
                        }
                    }
                }
                // No more agents, move to next task
                self.selected_agent = None;
                just_exited_agent = true;
            }

            // If we're on a task, try to move to next task in same phase or first agent if expanded
            if let Some(ref task_id) = self.selected_task {
                let phase = &phases[self.selected_phase];

                // Check if task is expanded and has agents (but only if we didn't just exit an agent)
                if !just_exited_agent && self.expanded_tasks.contains(task_id) {
                    if let Some(task) = phase.tasks.iter().find(|t| &t.id == task_id) {
                        if !task.agents.is_empty() {
                            self.selected_agent = Some(task.agents[0].id.clone());
                            return;
                        }
                    }
                }

                // Move to next task
                let current_idx = phase.tasks.iter().position(|t| &t.id == task_id);
                if let Some(idx) = current_idx {
                    if idx + 1 < phase.tasks.len() {
                        self.selected_task = Some(phase.tasks[idx + 1].id.clone());
                        return;
                    }
                }
                // No more tasks, move to next phase
                self.selected_task = None;
            }

            // Move to next phase or first task if expanded
            if self.expanded_phases.contains(&self.selected_phase) {
                let phase = &phases[self.selected_phase];
                if !phase.tasks.is_empty() && self.selected_task.is_none() {
                    self.selected_task = Some(phase.tasks[0].id.clone());
                    return;
                }
            }

            if self.selected_phase + 1 < phases.len() {
                self.selected_phase += 1;
                self.selected_task = None;
                self.selected_agent = None;
            }
        }
    }

    pub fn navigate_workflow_up(&mut self) {
        if let Ok(phases) = self.workflow_phases.lock() {
            if phases.is_empty() {
                return;
            }

            // If we're on an agent, try to move to previous agent
            if let Some(ref agent_id) = self.selected_agent {
                let phase = &phases[self.selected_phase];
                if let Some(task) = phase
                    .tasks
                    .iter()
                    .find(|t| Some(&t.id) == self.selected_task.as_ref())
                {
                    let current_idx = task.agents.iter().position(|a| &a.id == agent_id);
                    if let Some(idx) = current_idx {
                        if idx > 0 {
                            self.selected_agent = Some(task.agents[idx - 1].id.clone());
                            return;
                        }
                    }
                }
                // At first agent, move back to task
                self.selected_agent = None;
                return;
            }

            // If we're on a task, try to move to previous task or last agent of previous task
            if let Some(ref task_id) = self.selected_task {
                let phase = &phases[self.selected_phase];
                let current_idx = phase.tasks.iter().position(|t| &t.id == task_id);
                if let Some(idx) = current_idx {
                    if idx > 0 {
                        let prev_task = &phase.tasks[idx - 1];
                        self.selected_task = Some(prev_task.id.clone());
                        // If previous task is expanded with agents, jump to last agent
                        if self.expanded_tasks.contains(&prev_task.id)
                            && !prev_task.agents.is_empty()
                        {
                            self.selected_agent =
                                Some(prev_task.agents[prev_task.agents.len() - 1].id.clone());
                        }
                        return;
                    }
                }
                // At first task, move back to phase
                self.selected_task = None;
                return;
            }

            // Move to previous phase or last task if expanded
            if self.selected_phase > 0 {
                self.selected_phase -= 1;
                self.selected_task = None;
                self.selected_agent = None;

                // If new phase is expanded and has tasks, jump to last task
                if self.expanded_phases.contains(&self.selected_phase) {
                    let phase = &phases[self.selected_phase];
                    if !phase.tasks.is_empty() {
                        let last_task = &phase.tasks[phase.tasks.len() - 1];
                        self.selected_task = Some(last_task.id.clone());
                        // If last task is expanded with agents, jump to last agent
                        if self.expanded_tasks.contains(&last_task.id)
                            && !last_task.agents.is_empty()
                        {
                            self.selected_agent =
                                Some(last_task.agents[last_task.agents.len() - 1].id.clone());
                        }
                    }
                }
            }
        }
    }

    pub fn update_workflow_scroll(&mut self, viewport_height: usize) {
        // Calculate which line the selected item is on and adjust scroll to keep it visible
        if let Ok(phases) = self.workflow_phases.lock() {
            let mut current_line = 0;
            let mut selected_line = 0;

            for phase in phases.iter() {
                // Check if this phase is selected
                if self.selected_phase == phase.id
                    && self.selected_task.is_none()
                    && self.selected_agent.is_none()
                {
                    selected_line = current_line;
                }
                current_line += 1; // Phase header

                if self.expanded_phases.contains(&phase.id) {
                    for task in &phase.tasks {
                        // Check if this task is selected
                        if self.selected_phase == phase.id
                            && Some(&task.id) == self.selected_task.as_ref()
                            && self.selected_agent.is_none()
                        {
                            selected_line = current_line;
                        }
                        current_line += 1; // Task header

                        if self.expanded_tasks.contains(&task.id) {
                            // Count task messages
                            current_line += task.messages.len();

                            for agent in &task.agents {
                                // Check if this agent is selected
                                if Some(&agent.id) == self.selected_agent.as_ref() {
                                    selected_line = current_line;
                                }
                                current_line += 1; // Agent header

                                if self.expanded_agents.contains(&agent.id) {
                                    current_line += agent.messages.len();
                                }
                            }
                        }
                    }

                    // Count output files
                    if !phase.output_files.is_empty() {
                        current_line += 1; // "Output files:" header
                        current_line += phase.output_files.len();
                    }
                }

                current_line += 1; // Empty line after phase
            }

            // Adjust scroll offset to keep selected line visible
            // Leave some padding at top and bottom
            let padding = 2;
            let visible_lines = viewport_height.saturating_sub(2); // Account for borders

            if selected_line < self.workflow_scroll_offset + padding {
                // Selected line is above visible area, scroll up
                self.workflow_scroll_offset = selected_line.saturating_sub(padding);
            } else if selected_line
                >= self.workflow_scroll_offset + visible_lines.saturating_sub(padding)
            {
                // Selected line is below visible area, scroll down
                self.workflow_scroll_offset = selected_line
                    .saturating_sub(visible_lines.saturating_sub(padding).saturating_sub(1));
            }
        }
    }
}
