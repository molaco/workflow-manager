//! Command pattern for App communication
//!
//! This module defines the AppCommand enum which represents all possible
//! commands that can be sent to the App from MCP tools or other async tasks.

use std::collections::HashMap;
use uuid::Uuid;
use workflow_manager_sdk::WorkflowLog;

/// Commands that can be sent to the App from MCP tools or other async tasks
#[derive(Debug, Clone)]
pub enum AppCommand {
    /// Create a new workflow tab
    CreateTab {
        workflow_id: String,
        params: HashMap<String, String>,
        handle_id: Uuid,
    },

    /// Append log to an existing tab
    AppendTabLog {
        handle_id: Uuid,
        log: WorkflowLog,
    },

    /// Update tab status
    UpdateTabStatus {
        handle_id: Uuid,
        status: workflow_manager_sdk::WorkflowStatus,
    },

    /// Close a tab by handle (bypasses confirmation - assumes MCP has approval)
    CloseTab {
        handle_id: Uuid,
    },

    /// Switch to a specific tab
    SwitchToTab {
        handle_id: Uuid,
    },

    /// Show a notification to the user
    ShowNotification {
        level: NotificationLevel,
        title: String,
        message: String,
    },

    /// Quit the application
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NotificationLevel {
    Info,
    Success,
    Warning,
    Error,
}
