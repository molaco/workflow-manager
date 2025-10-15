//! File browser and dropdown functionality

use std::path::{Path, PathBuf};
use workflow_manager_sdk::FieldType;

use super::*;

impl App {
    pub fn open_file_browser(&mut self) {
        // Get the current field type
        if let View::WorkflowEdit(idx) = self.current_view {
            if let Some(workflow) = self.workflows.get(idx) {
                if let Some(field) = workflow.info.fields.get(self.edit_field_index) {
                    // Only open for file_path and state_file fields
                    if matches!(field.field_type, FieldType::FilePath { .. } | FieldType::StateFile { .. }) {
                        self.show_file_browser = true;
                        self.file_browser_search.clear();
                        self.load_file_browser_items();
                    }
                }
            }
        }
    }

    pub fn close_file_browser(&mut self) {
        self.show_file_browser = false;
        self.file_browser_items.clear();
        self.file_browser_selected = 0;
        self.file_browser_search.clear();
    }

    pub fn load_file_browser_items(&mut self) {
        let base_dir = if self.edit_buffer.is_empty() {
            self.current_dir.clone()
        } else {
            let path = if PathBuf::from(&self.edit_buffer).is_absolute() {
                PathBuf::from(&self.edit_buffer)
            } else {
                self.current_dir.join(&self.edit_buffer)
            };

            // If path is a directory, use it directly
            // If path is a file (or doesn't exist), use its parent
            if path.is_dir() {
                path
            } else {
                path.parent().unwrap_or(&self.current_dir).to_path_buf()
            }
        };

        let mut items = Vec::new();

        // Add parent directory
        if let Some(parent) = base_dir.parent() {
            items.push(parent.to_path_buf());
        }

        // Read directory
        if let Ok(entries) = std::fs::read_dir(&base_dir) {
            for entry in entries.flatten() {
                items.push(entry.path());
            }
        }

        // Sort: directories first, then files
        items.sort_by(|a, b| {
            match (a.is_dir(), b.is_dir()) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.file_name().cmp(&b.file_name()),
            }
        });

        self.file_browser_items = items;
        self.file_browser_selected = 0;
    }

    pub fn file_browser_next(&mut self) {
        if self.file_browser_selected < self.file_browser_items.len().saturating_sub(1) {
            self.file_browser_selected += 1;
        }
    }

    pub fn file_browser_previous(&mut self) {
        if self.file_browser_selected > 0 {
            self.file_browser_selected -= 1;
        }
    }

    pub fn file_browser_select(&mut self) {
        if let Some(path) = self.file_browser_items.get(self.file_browser_selected) {
            if path.is_dir() {
                // Navigate into directory
                self.current_dir = path.clone();
                self.edit_buffer = path.to_string_lossy().to_string();
                self.load_file_browser_items();
            } else {
                // Select file
                self.edit_buffer = path.to_string_lossy().to_string();
                self.close_file_browser();
            }
        }
    }

    pub fn complete_path(&mut self) {
        // Show dropdown with matching paths
        let partial = self.edit_buffer.clone();

        let (base_dir, prefix_str) = if partial.is_empty() {
            (self.current_dir.clone(), String::new())
        } else if partial.ends_with('/') || partial.ends_with('\\') {
            // Path ends with slash - we're inside a directory, show all contents
            let path = PathBuf::from(&partial);
            let dir = if path.is_absolute() {
                path
            } else {
                self.current_dir.join(path)
            };
            (dir, String::new())
        } else {
            let path = PathBuf::from(&partial);
            if let Some(parent) = path.parent() {
                let dir = if parent.as_os_str().is_empty() {
                    self.current_dir.clone()
                } else if path.is_absolute() {
                    parent.to_path_buf()
                } else {
                    self.current_dir.join(parent)
                };
                let prefix = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                (dir, prefix)
            } else {
                (self.current_dir.clone(), partial)
            }
        };

        let prefix = prefix_str.as_str();

        if let Ok(entries) = std::fs::read_dir(&base_dir) {
            let mut matches: Vec<PathBuf> = entries
                .flatten()
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .map(|s| s.starts_with(prefix))
                        .unwrap_or(false)
                })
                .map(|e| e.path())
                .collect();

            // Sort: directories first, then files
            matches.sort_by(|a, b| {
                match (a.is_dir(), b.is_dir()) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.file_name().cmp(&b.file_name()),
                }
            });

            // Add parent directory as first item (if it exists)
            if let Some(parent) = base_dir.parent() {
                matches.insert(0, parent.to_path_buf());
            }

            if !matches.is_empty() {
                self.dropdown_items = matches;
                self.dropdown_selected = 0;
                self.show_dropdown = true;
            }
        }
    }

    pub fn dropdown_next(&mut self) {
        if self.dropdown_selected < self.dropdown_items.len().saturating_sub(1) {
            self.dropdown_selected += 1;
        }
    }

    pub fn dropdown_previous(&mut self) {
        if self.dropdown_selected > 0 {
            self.dropdown_selected -= 1;
        }
    }

    pub fn dropdown_select(&mut self) {
        // Check if we're showing history or file paths
        if !self.history_items.is_empty() {
            // History dropdown
            if let Some(value) = self.history_items.get(self.dropdown_selected) {
                self.edit_buffer = value.clone();
                self.close_dropdown();
            }
        } else if let Some(path) = self.dropdown_items.get(self.dropdown_selected) {
            // File path dropdown
            let mut path_str = path.to_string_lossy().to_string();

            if path.is_dir() {
                // For directories, ensure trailing slash
                if !path_str.ends_with('/') && !path_str.ends_with('\\') {
                    path_str.push('/');
                }
                self.edit_buffer = path_str;
                self.complete_path();
            } else {
                // For files, close the dropdown
                self.edit_buffer = path_str;
                self.close_dropdown();
            }
        }
    }

    pub fn close_dropdown(&mut self) {
        self.show_dropdown = false;
        self.dropdown_items.clear();
        self.dropdown_selected = 0;
        self.history_items.clear();
    }

    pub fn show_history_dropdown(&mut self) {
        // Get current workflow and field
        if let View::WorkflowEdit(idx) = self.current_view {
            if let Some(workflow) = self.workflows.get(idx) {
                if let Some(field) = workflow.info.fields.get(self.edit_field_index) {
                    // Get history for this workflow + field
                    if let Some(workflow_history) = self.history.workflows.get(&workflow.info.id) {
                        if let Some(field_history) = workflow_history.get(&field.name) {
                            if !field_history.is_empty() {
                                self.history_items = field_history.clone();
                                self.dropdown_selected = 0;
                                self.show_dropdown = true;
                            }
                        }
                    }
                }
            }
        }
    }
}
