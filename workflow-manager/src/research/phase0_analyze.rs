//! Phase 0: Codebase analysis
//!
//! Analyzes codebase structure and generates a structured analysis using Claude agents.
//!
//! This phase uses Claude with tool access (Read, Glob, Grep, Bash) to:
//! - Count files by extension with exact metrics
//! - Map directory structure
//! - Identify entry points and configuration files
//! - Extract dependencies and frameworks
//! - Detect architecture patterns
//!
//! The result is a comprehensive YAML analysis saved to `OUTPUT/codebase_analysis_<timestamp>.yaml`.

use crate::research::types::CodebaseAnalysis;
use claude_agent_sdk::{query, ClaudeAgentOptions, ContentBlock, Message};
use futures::StreamExt;
use std::path::Path;
use workflow_manager_sdk::{
    log_agent_complete, log_agent_failed, log_agent_message, log_agent_start,
};

/// Analyze codebase structure and generate comprehensive overview
pub async fn analyze_codebase(codebase_path: &Path) -> anyhow::Result<CodebaseAnalysis> {
    let task_id = "analyze";
    let agent_name = "Suborchestrator Agent";

    log_agent_start!(task_id, agent_name, "Analyzing codebase structure");

    let analysis_prompt = format!(
        r#"Analyze the codebase at {} and provide a structured overview.

# CRITICAL: Use Bash tools for EXACT counts, not estimates!

# Required Analysis

## 1. File Statistics (MUST BE EXACT - USE BASH COMMANDS)
Run these commands to get ACCURATE counts:
  - Count .rs files: find . -name "*.rs" -type f | wc -l
  - Count .rs lines: find . -name "*.rs" -type f -exec wc -l {{}} + | tail -1
  - Count .py files: find . -name "*.py" -type f | wc -l
  - Count .py lines: find . -name "*.py" -type f -exec wc -l {{}} + | tail -1
  - Repeat for ALL extensions found (.md, .yaml, .toml, .js, etc.)

Output format:
  - File count by extension (exact numbers from find | wc -l)
  - Total lines of code per extension (exact numbers from wc -l)
  - Identify test files vs source files (use grep or file paths)

## 2. Directory Structure
- Map top 3 directory levels (use tree or ls -R)
- Identify purpose of each major directory
- Note any special directories (docs, examples, tests, benchmarks)

## 3. Entry Points & Configuration
- Main executable files (main.rs, __main__.py, index.js)
- Build configs (Cargo.toml, package.json, pyproject.toml, CMakeLists.txt)
- CI/CD configs (.github/workflows)
- Documentation roots (README.md, docs/)

## 4. Dependencies & Frameworks
- External dependencies (from manifest files)
- Internal module/crate structure
- Framework detection (web frameworks, ML libraries, etc.)

## 5. Architecture Patterns
- Project type (library, application, monorepo, workspace)
- Module organization (monolithic, modular, microservices)
- Notable patterns (MVC, layered, plugin-based)

## 6. Key Components
- Core modules/packages
- Public APIs or interfaces
- Notable implementation files

# Output Format
Provide analysis as YAML with proper structure.

BEFORE generating YAML:
1. Verify all file counts are exact (not estimated)
2. Use actual command outputs for line counts
3. Double-check numbers match bash command results
4. **CRITICAL: Check for duplicate keys at EVERY level - each key must appear ONCE**
5. If you need multiple values, use YAML arrays/lists: `key:\n  - value1\n  - value2`

Be comprehensive and ACCURATE with all metrics."#,
        codebase_path.display()
    );

    let options = ClaudeAgentOptions::builder()
        .system_prompt(
            r#"You are a codebase analyst. Your analysis must be ACCURATE and COMPREHENSIVE.

CRITICAL REQUIREMENTS:
- Use Bash tools (find, wc, grep) to get EXACT counts, never estimate
- Run 'find . -name "*.ext" | wc -l' for precise file counts
- Run 'find . -name "*.ext" -exec wc -l {} + | tail -1' for total line counts
- Count ALL files, do not sample or approximate
- Verify your numbers before outputting YAML

YAML OUTPUT REQUIREMENTS (CRITICAL):
- NO DUPLICATE KEYS at any level - each key must appear only once
- Use arrays/lists for multiple items under one key
- Example: WRONG: "documentation: a\ndocumentation: b"  CORRECT: "documentation:\n  - a\n  - b"
- Validate YAML structure before outputting - check for any repeated keys
- Proper YAML indentation (2 spaces, no tabs)

Provide detailed structural analysis with accurate, verified metrics."#
                .to_string(),
        )
        .allowed_tools(vec![
            "Read".to_string(),
            "Glob".to_string(),
            "Grep".to_string(),
            "Bash".to_string(),
        ])
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    let stream = query(&analysis_prompt, Some(options)).await?;
    let mut stream = Box::pin(stream);

    let mut response_text = String::new();

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    if let ContentBlock::Text { text } = block {
                        // Collect raw text without printing
                        response_text.push_str(text);
                        // Emit structured agent message for TUI
                        log_agent_message!(task_id, agent_name, text);
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

    // Extract and parse YAML
    let yaml_content = extract_yaml(&response_text);
    let analysis: CodebaseAnalysis = serde_yaml::from_str(&yaml_content).map_err(|e| {
        let error_msg = format!("Failed to parse analysis YAML: {}", e);
        log_agent_failed!(task_id, agent_name, &error_msg);

        // If duplicate key error, provide helpful context
        if error_msg.contains("duplicate") {
            eprintln!("\n❌ YAML PARSING ERROR: Duplicate keys detected");
            eprintln!("The analysis contains duplicate keys which is invalid YAML.");
            eprintln!("Common fix: Combine duplicate keys into a single key with an array.");
            eprintln!("\nGenerated YAML preview (first 500 chars):");
            eprintln!("{}", &yaml_content.chars().take(500).collect::<String>());
        }

        anyhow::anyhow!("{}", error_msg)
    })?;

    log_agent_complete!(task_id, agent_name, "Codebase analysis complete");
    Ok(analysis)
}

/// Extract YAML content from markdown code blocks
fn extract_yaml(text: &str) -> String {
    let yaml = if text.contains("```yaml") {
        let yaml_start = text.find("```yaml").unwrap() + 7;
        let yaml_end = text[yaml_start..]
            .rfind("```")
            .map(|pos| pos + yaml_start)
            .unwrap_or(text.len());
        text[yaml_start..yaml_end].trim().to_string()
    } else if text.contains("```") {
        let yaml_start = text.find("```").unwrap() + 3;
        let yaml_end = text[yaml_start..]
            .rfind("```")
            .map(|pos| pos + yaml_start)
            .unwrap_or(text.len());
        text[yaml_start..yaml_end].trim().to_string()
    } else {
        text.trim().to_string()
    };

    // Remove leading document separator (---) if present
    // serde_yaml::from_str doesn't support multi-document YAML
    yaml.trim_start_matches("---").trim().to_string()
}
