//! Data structures for research workflows
//!
//! This module contains the core data types used throughout the research workflow.

use serde::{Deserialize, Serialize};

/// Codebase analysis data - flexible YAML structure
///
/// This type alias allows for flexible YAML content from Phase 0 codebase analysis.
/// The analysis typically includes:
/// - File statistics (counts and lines by extension)
/// - Directory structure
/// - Entry points and configuration files
/// - Dependencies and frameworks
/// - Architecture patterns
pub type CodebaseAnalysis = serde_yaml::Value;

/// A single research prompt with focus areas
///
/// Generated in Phase 1, each prompt targets a specific aspect of the research objective.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchPrompt {
    /// Title of the research prompt
    pub title: String,
    /// Query/question to be executed by the research agent
    pub query: String,
    /// Focus areas or topics to concentrate on
    pub focus: Vec<String>,
}

/// Collection of research prompts with objective
///
/// Container for all research prompts generated in Phase 1.
#[derive(Debug, Serialize, Deserialize)]
pub struct PromptsData {
    /// Original research objective provided by the user
    pub objective: String,
    /// List of generated research prompts
    pub prompts: Vec<ResearchPrompt>,
}

/// Result of a single research execution
///
/// Tracks the output from executing one research prompt in Phase 2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchResult {
    /// Title of the research task
    pub title: String,
    /// Original query that was executed
    pub query: String,
    /// Path to the YAML file containing the detailed research findings
    pub response_file: String,
    /// Focus areas that were covered
    pub focus: Vec<String>,
}
