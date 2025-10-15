//! Data models for the workflow manager TUI
//!
//! This module re-exports all data structures from the app module for backwards compatibility.

// Re-export all public items from the app module
pub use crate::app::{
    App,
    View,
    WorkflowTab,
    WorkflowPhase,
    WorkflowTask,
    WorkflowAgent,
    WorkflowHistory,
    PhaseStatus,
    TaskStatus,
    AgentStatus,
};
