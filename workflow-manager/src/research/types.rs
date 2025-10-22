//! Data structures for research workflows

use serde::{Deserialize, Serialize};

/// Codebase analysis data - flexible YAML structure
pub type CodebaseAnalysis = serde_yaml::Value;

/// A single research prompt with focus areas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchPrompt {
    pub title: String,
    pub query: String,
    pub focus: Vec<String>,
}

/// Collection of research prompts with objective
#[derive(Debug, Serialize, Deserialize)]
pub struct PromptsData {
    pub objective: String,
    pub prompts: Vec<ResearchPrompt>,
}

/// Result of a single research execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchResult {
    pub title: String,
    pub query: String,
    pub response_file: String,
    pub focus: Vec<String>,
}
