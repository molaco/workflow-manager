//! Research workflow module
//!
//! This module provides functionality for executing research workflows with Claude Agent SDK.
//! It supports multi-phase execution: codebase analysis, prompt generation, research execution,
//! YAML validation, and documentation synthesis.

pub mod types;
pub mod cli;
pub mod phase0_analyze;
pub mod phase1_prompts;
pub mod phase2_research;
pub mod phase3_validate;
pub mod phase4_synthesize;
pub mod workflow;

// Re-export commonly used types
pub use types::{CodebaseAnalysis, PromptsData, ResearchPrompt, ResearchResult};
pub use workflow::{run_research_workflow, WorkflowConfig};
