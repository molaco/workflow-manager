/*
┌─────────────────────────────────────────────────────────────────────────────┐
│                         RESEARCH AGENT WORKFLOW                              │
└─────────────────────────────────────────────────────────────────────────────┘

  Phase 0: ANALYZE CODEBASE
    │
    ├─> Read files (Glob, Read, Grep, Bash)
    ├─> Count files by extension
    ├─> Map directory structure
    ├─> Identify entry points & configs
    ├─> Extract dependencies & frameworks
    ├─> Detect architecture patterns
    └─> Output: codebase_analysis_<timestamp>.yaml

         ↓

  Phase 1: GENERATE RESEARCH PROMPTS
    │
    ├─> Input: objective + codebase_analysis.yaml
    ├─> Use custom system prompt + output style
    ├─> LLM generates research questions
    └─> Output: research_prompts_<timestamp>.yaml

         ↓

  Phase 2: EXECUTE RESEARCH (concurrent)
    │
    ├─> For each prompt in research_prompts.yaml:
    │   ├─> Query Claude with prompt (concurrent execution)
    │   ├─> Collect YAML response
    │   └─> Store result in ./RESULTS/
    └─> Output: research_results_<timestamp>.yaml

         ↓

  Phase 3: VALIDATE & FIX YAML (loop until valid)
    │
    ├─> Validate all result files with check_yaml.py
    ├─> Identify files with errors
    └─> Loop:
        ├─> Fix broken files concurrently with Claude
        ├─> Re-validate fixed files
        └─> Continue until all valid

         ↓

  Phase 4: SYNTHESIZE DOCUMENTATION
    │
    ├─> Input: objective + research_results.yaml
    ├─> LLM synthesizes all findings
    ├─> Generate comprehensive markdown
    └─> Output: research_output_<timestamp>.md (or custom path)

┌─────────────────────────────────────────────────────────────────────────────┐
│ FEATURES:                                                                    │
│ • Resume from any phase (--analysis-file, --prompts-file, --results-file)  │
│ • Concurrent execution (--batch-size N for parallel prompts & fixes)       │
│ • Phase selection (--phases 0,1,2,3,4)                                      │
│ • Custom prompts (--system-prompt, --append for output style)              │
│ • YAML validation & repair (Phase 3 - can run standalone or after Phase 2) │
└─────────────────────────────────────────────────────────────────────────────┘

EXAMPLE COMMANDS:

  # Run all phases (full workflow)
  cargo run --example new_research_agent -- \
    --input "How does the authentication system work?" \
    --system-prompt prompts/writer.md \
    --append prompts/style.md \
    --output docs/auth_guide.md

  # Phase 0 only: Analyze codebase
  cargo run --example new_research_agent -- \
    --phases 0 \
    --dir /path/to/codebase

  # Phase 1 only: Generate prompts (requires analysis file)
  cargo run --example new_research_agent -- \
    --phases 1 \
    --input "Explain the database layer" \
    --system-prompt prompts/writer.md \
    --append prompts/style.md \
    --analysis-file codebase_analysis_20250101_120000.yaml

  # Phase 2 only: Execute research (sequential)
  cargo run --example new_research_agent -- \
    --phases 2 \
    --prompts-file research_prompts_20250101_120000.yaml

  # Phase 2 only: Execute research (parallel batch of 3)
  cargo run --example new_research_agent -- \
    --phases 2 \
    --prompts-file research_prompts_20250101_120000.yaml \
    --batch-size 3

  # Phase 3 only: Validate & fix YAML files (using directory)
  cargo run --example new_research_agent -- \
    --phases 3 \
    --results-dir ./RESULTS \
    --batch-size 2

  # Phase 3 only: Validate & fix YAML files (using results file)
  cargo run --example new_research_agent -- \
    --phases 3 \
    --results-file research_results_20250101_120000.yaml \
    --batch-size 2

  # Phase 4 only: Synthesize documentation (input optional)
  cargo run --example new_research_agent -- \
    --phases 4 \
    --results-file research_results_20250101_120000.yaml \
    --output docs/api_guide.md

  # Resume from Phase 2 onwards (includes validation, input optional for phase 4)
  cargo run --example new_research_agent -- \
    --phases 2,3,4 \
    --prompts-file research_prompts_20250101_120000.yaml \
    --output docs/testing.md

*/

use clap::Parser;
use claude_agent_sdk::{
    query, ClaudeAgentOptions, ContentBlock, Message, SystemPrompt, SystemPromptPreset,
};
use futures::{stream::FuturesUnordered, StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{fs, sync::Semaphore};
use workflow_manager_sdk::{
    log_phase_complete, log_phase_start, log_state_file, log_task_complete, log_task_failed,
    log_task_start, WorkflowDefinition,
};

// Use flexible YAML instead of rigid structs
type CodebaseAnalysis = serde_yaml::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResearchPrompt {
    title: String,
    query: String,
    focus: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PromptsData {
    objective: String,
    prompts: Vec<ResearchPrompt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResearchResult {
    title: String,
    query: String,
    response_file: String,
    focus: Vec<String>,
}

#[derive(Parser, Debug, Clone, WorkflowDefinition)]
#[workflow(
    id = "research_agent",
    name = "Research Agent Workflow",
    description = "Multi-phase research workflow: Analyze codebase → Generate prompts → Execute research → Validate YAML → Synthesize docs"
)]
struct Args {
    /// Research objective/question
    #[arg(short, long)]
    #[field(
        label = "Research Objective",
        description = "[TEXT] What do you want to research about the codebase?",
        type = "text",
        required_for_phases = "1"
    )]
    input: Option<String>,

    /// Prompt writer system prompt (file path or string)
    #[arg(short = 's', long)]
    #[field(
        label = "System Prompt",
        description = "[TEXT] Path to prompt writer system prompt file",
        type = "file_path",
        required_for_phases = "1"
    )]
    system_prompt: Option<String>,

    /// Output style format (file path or string)
    #[arg(short = 'a', long)]
    #[field(
        label = "Output Style",
        description = "[TEXT] Path to output style format file",
        type = "file_path",
        required_for_phases = "1"
    )]
    append: Option<String>,

    /// Output file path for final documentation
    #[arg(short, long)]
    #[field(
        label = "Output File",
        description = "[TEXT] Path for final documentation (e.g., docs/guide.md)",
        type = "file_path"
    )]
    output: Option<String>,

    /// Number of research prompts to execute in parallel (default: 1 for sequential)
    #[arg(long, default_value = "1")]
    #[field(
        label = "Batch Size",
        description = "[NUMBER] Parallel execution batch size (1-10)",
        type = "number",
        min = "1",
        max = "10"
    )]
    batch_size: usize,

    /// Comma-separated phases to execute (0=analyze, 1=prompts, 2=research, 3=validate, 4=synthesize)
    #[arg(long, default_value = "0,1,2,3,4")]
    #[field(
        label = "Phases to Run",
        description = "[PHASES] Select which phases to execute (0-4)",
        type = "phase_selector",
        total_phases = "5"
    )]
    phases: String,

    /// Path to saved codebase analysis YAML (for resuming from Phase 1)
    #[arg(long)]
    #[field(
        label = "Analysis File",
        description = "[STATE FILE] Resume with existing codebase analysis",
        type = "state_file",
        pattern = "codebase_analysis_*.yaml",
        required_for_phases = "1"
    )]
    analysis_file: Option<String>,

    /// Path to saved prompts YAML (for resuming from Phase 2)
    #[arg(long)]
    #[field(
        label = "Prompts File",
        description = "[STATE FILE] Resume with existing research prompts",
        type = "state_file",
        pattern = "research_prompts_*.yaml",
        required_for_phases = "2"
    )]
    prompts_file: Option<String>,

    /// Path to saved research results YAML (for resuming from Phase 3)
    #[arg(long)]
    #[field(
        label = "Results File",
        description = "[STATE FILE] Resume with existing research results",
        type = "state_file",
        pattern = "research_results_*.yaml",
        required_for_phases = "3,4"
    )]
    results_file: Option<String>,

    /// Directory path to analyze for Phase 0
    #[arg(long)]
    #[field(
        label = "Directory",
        description = "[TEXT] Directory to analyze (default: current directory)",
        type = "file_path"
    )]
    dir: Option<String>,

    /// Directory containing YAML files to validate (for Phase 3)
    #[arg(long)]
    #[field(
        label = "Results Directory",
        description = "[TEXT] Directory containing YAML files to validate",
        type = "file_path",
        required_for_phases = "3"
    )]
    results_dir: Option<String>,

    // Hidden metadata flag
    #[arg(long, hide = true)]
    workflow_metadata: bool,
}

/// Phase 0: Analyze codebase structure
async fn analyze_codebase(codebase_path: &Path) -> anyhow::Result<CodebaseAnalysis> {
    use workflow_manager_sdk::{
        log_agent_complete, log_agent_failed, log_agent_message, log_agent_start,
    };

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

/// Phase 1: Generate research prompts based on objective
async fn generate_prompts(
    objective: &str,
    codebase_analysis: &CodebaseAnalysis,
    prompt_writer: &str,
    output_style: &str,
) -> anyhow::Result<PromptsData> {
    use workflow_manager_sdk::{
        log_agent_complete, log_agent_failed, log_agent_message, log_agent_start,
    };

    let task_id = "generate";
    let agent_name = "Suborchestrator Agent";

    log_agent_start!(task_id, agent_name, "Generating research prompts");

    // Build system prompt with codebase analysis
    let analysis_yaml = serde_yaml::to_string(codebase_analysis)?;
    let system_prompt = format!(
        "{}\n\n# Codebase Analysis\n{}\n\n# Output Style\n{}",
        prompt_writer, analysis_yaml, output_style
    );

    let options = ClaudeAgentOptions::builder()
        .system_prompt(system_prompt)
        .allowed_tools(vec![
            "Read".to_string(),
            "Glob".to_string(),
            "Grep".to_string(),
        ])
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    let query_text = format!("Generate research prompts for: {}", objective);
    let stream = query(&query_text, Some(options)).await?;
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

    let yaml_content = extract_yaml(&response_text);

    // Parse YAML flexibly first (like Python's yaml.safe_load)
    let prompts_yaml: serde_yaml::Value = serde_yaml::from_str(&yaml_content).map_err(|e| {
        let error_msg = format!("Failed to parse prompts YAML: {}", e);
        log_agent_failed!(task_id, agent_name, &error_msg);

        // If duplicate key error, provide helpful context
        if error_msg.contains("duplicate") {
            eprintln!("\n❌ YAML PARSING ERROR: Duplicate keys detected in prompts");
            eprintln!("The prompts YAML contains duplicate keys which is invalid.");
            eprintln!("\nGenerated YAML preview (first 500 chars):");
            eprintln!("{}", &yaml_content.chars().take(500).collect::<String>());
        }

        anyhow::anyhow!("{}", error_msg)
    })?;

    // Extract fields with safe defaults
    let objective = prompts_yaml["objective"]
        .as_str()
        .unwrap_or("Unknown objective")
        .to_string();

    let prompts_array = prompts_yaml["prompts"]
        .as_sequence()
        .cloned()
        .unwrap_or_default();

    let prompts: Vec<ResearchPrompt> = prompts_array
        .iter()
        .filter_map(|p| {
            Some(ResearchPrompt {
                title: p["title"].as_str()?.to_string(),
                query: p["query"].as_str()?.to_string(),
                focus: p["focus"]
                    .as_sequence()
                    .map(|seq| {
                        seq.iter()
                            .filter_map(|f| f.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect();

    let prompts_data = PromptsData { objective, prompts };

    log_agent_complete!(
        task_id,
        agent_name,
        format!("Generated {} prompts", prompts_data.prompts.len())
    );
    Ok(prompts_data)
}

/// Phase 2: Execute a single research prompt
async fn execute_research_prompt(
    prompt: &ResearchPrompt,
    result_number: usize,
    timestamp: &str,
    prefix: Option<&str>,
) -> anyhow::Result<ResearchResult> {
    use workflow_manager_sdk::{
        log_agent_complete, log_agent_failed, log_agent_message, log_agent_start,
    };

    let prefix = prefix.unwrap_or("");
    let task_id = format!("research_{}", result_number);
    let agent_name = format!("Research Agent {}", result_number);

    log_agent_start!(
        &task_id,
        &agent_name,
        format!("Executing: {}", prompt.title)
    );

    println!("\n{}", "-".repeat(80));
    println!("{}EXECUTING: {}", prefix, prompt.title);
    println!("{}", "-".repeat(80));

    let preset = SystemPromptPreset {
        prompt_type: "preset".to_string(),
        preset: "claude_code".to_string(),
        append: Some("IMPORTANT: DO NOT create or write any files. Output all your research findings as yaml only.".to_string()),
    };

    let options = ClaudeAgentOptions::builder()
        .system_prompt(SystemPrompt::Preset(preset))
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    let stream = query(&prompt.query, Some(options)).await?;
    let mut stream = Box::pin(stream);

    let mut response_text = String::new();

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            if !prefix.is_empty() {
                                println!("{}{}", prefix, text);
                            } else {
                                println!("{}", text);
                            }
                            response_text.push_str(text);
                            log_agent_message!(&task_id, &agent_name, text);
                        }
                        ContentBlock::ToolUse { name, .. } => {
                            log_agent_message!(
                                &task_id,
                                &agent_name,
                                format!("🔧 Using tool: {}", name)
                            );
                        }
                        ContentBlock::ToolResult { tool_use_id, .. } => {
                            log_agent_message!(
                                &task_id,
                                &agent_name,
                                format!("✓ Tool result: {}", tool_use_id)
                            );
                        }
                        _ => {}
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

    println!("\n");

    // Write response directly to file (no serialization issues)
    let yaml_content = extract_yaml(&response_text);
    let response_filename = format!(
        "./RESULTS/research_result_{}_{}.yaml",
        result_number, timestamp
    );
    fs::write(&response_filename, &yaml_content)
        .await
        .map_err(|e| {
            log_agent_failed!(
                &task_id,
                &agent_name,
                format!("Failed to write file: {}", e)
            );
            e
        })?;

    println!("{}Response saved to: {}", prefix, response_filename);
    log_agent_complete!(
        &task_id,
        &agent_name,
        format!("Saved to {}", response_filename)
    );

    Ok(ResearchResult {
        title: prompt.title.clone(),
        query: prompt.query.clone(),
        response_file: response_filename,
        focus: prompt.focus.clone(),
    })
}

/// Validate YAML file using check_yaml.py script
async fn validate_yaml_file(file_path: &str) -> anyhow::Result<(String, bool, String)> {
    use tokio::process::Command;

    let output = Command::new("uv")
        .args([
            "run",
            "/home/molaco/Documents/japanese/SCRIPTS/check_yaml.py",
            file_path,
        ])
        .output()
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined_output = format!("{}{}", stdout, stderr);

    let is_valid = output.status.success()
        && !combined_output.contains("❌")
        && !combined_output.contains("Error");

    Ok((file_path.to_string(), is_valid, combined_output))
}

/// Fix invalid YAML file by querying Claude
async fn execute_fix_yaml(
    file_path: &str,
    error_message: &str,
    prefix: Option<&str>,
    fixer_number: usize,
) -> anyhow::Result<()> {
    use workflow_manager_sdk::{
        log_agent_complete, log_agent_failed, log_agent_message, log_agent_start,
    };

    let prefix = prefix.unwrap_or("");
    let task_id = format!("fix_yaml_{}", fixer_number);
    let agent_name = format!("YAML Fixer {}", fixer_number);

    log_agent_start!(&task_id, &agent_name, format!("Fixing: {}", file_path));

    println!("\n{}", "-".repeat(80));
    println!("{}FIXING: {}", prefix, file_path);
    println!("{}", "-".repeat(80));

    // Read the broken YAML
    let broken_yaml = fs::read_to_string(file_path).await.map_err(|e| {
        log_agent_failed!(&task_id, &agent_name, format!("Failed to read file: {}", e));
        e
    })?;

    log_agent_message!(
        &task_id,
        &agent_name,
        format!("Read {} bytes from file", broken_yaml.len())
    );
    log_agent_message!(
        &task_id,
        &agent_name,
        format!(
            "Error: {}",
            error_message.lines().next().unwrap_or("Unknown error")
        )
    );

    let fix_prompt = format!(
        r#"The following YAML file has validation errors. Please fix it and output ONLY the corrected YAML.

File: {}

Validation Error:
{}

Current Content:
```yaml
{}
```

Output the fixed YAML wrapped in ```yaml code blocks."#,
        file_path, error_message, broken_yaml
    );

    let preset = SystemPromptPreset {
        prompt_type: "preset".to_string(),
        preset: "claude_code".to_string(),
        append: Some("You are a YAML expert. Fix the YAML syntax errors and output ONLY valid YAML. Do not add explanations.".to_string()),
    };

    let options = ClaudeAgentOptions::builder()
        .system_prompt(SystemPrompt::Preset(preset))
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    let stream = query(&fix_prompt, Some(options)).await?;
    let mut stream = Box::pin(stream);

    let mut response_text = String::new();

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            if !prefix.is_empty() {
                                println!("{}{}", prefix, text);
                            } else {
                                println!("{}", text);
                            }
                            response_text.push_str(text);
                            log_agent_message!(&task_id, &agent_name, text);
                        }
                        ContentBlock::ToolUse { name, .. } => {
                            log_agent_message!(
                                &task_id,
                                &agent_name,
                                format!("🔧 Using tool: {}", name)
                            );
                        }
                        ContentBlock::ToolResult { tool_use_id, .. } => {
                            log_agent_message!(
                                &task_id,
                                &agent_name,
                                format!("✓ Tool result: {}", tool_use_id)
                            );
                        }
                        _ => {}
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

    // Extract and write fixed YAML
    let fixed_yaml = extract_yaml(&response_text);
    fs::write(file_path, &fixed_yaml).await.map_err(|e| {
        log_agent_failed!(
            &task_id,
            &agent_name,
            format!("Failed to write fixed YAML: {}", e)
        );
        e
    })?;

    println!("\n{}Fixed YAML written to: {}", prefix, file_path);
    log_agent_complete!(
        &task_id,
        &agent_name,
        format!("Fixed and saved {} bytes", fixed_yaml.len())
    );

    Ok(())
}

/// Phase 4 Map: Summarize a single research result
async fn summarize_research_result(
    result: &ResearchResult,
    result_number: usize,
    prefix: Option<&str>,
) -> anyhow::Result<String> {
    use workflow_manager_sdk::{
        log_agent_complete, log_agent_failed, log_agent_message, log_agent_start,
    };

    let prefix = prefix.unwrap_or("");
    let task_id = format!("summarize_{}", result_number);
    let agent_name = format!("Summarizer {}", result_number);

    log_agent_start!(
        &task_id,
        &agent_name,
        format!("Summarizing: {}", result.title)
    );

    println!("\n{}", "-".repeat(80));
    println!("{}SUMMARIZING: {}", prefix, result.title);
    println!("{}", "-".repeat(80));

    // Read the result file
    let response_content = fs::read_to_string(&result.response_file)
        .await
        .map_err(|e| {
            log_agent_failed!(&task_id, &agent_name, format!("Failed to read file: {}", e));
            anyhow::anyhow!("Failed to read {}: {}", result.response_file, e)
        })?;

    log_agent_message!(
        &task_id,
        &agent_name,
        format!("Read {} bytes from result file", response_content.len())
    );

    let summarize_prompt = format!(
        r#"Summarize the following research finding concisely. Extract only the key insights, facts, and actionable information.

# Finding: {}

**Query:** {}

**Research Data:**
```yaml
{}
```

# Instructions
- Extract 3-5 key points maximum
- Focus on actionable insights and important facts
- Be concise but preserve technical details
- Output as structured markdown"#,
        result.title, result.query, response_content
    );

    let preset = SystemPromptPreset {
        prompt_type: "preset".to_string(),
        preset: "claude_code".to_string(),
        append: Some(
            "You are a technical summarizer. Extract only essential information.".to_string(),
        ),
    };

    let options = ClaudeAgentOptions::builder()
        .system_prompt(SystemPrompt::Preset(preset))
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    let stream = query(&summarize_prompt, Some(options)).await?;
    let mut stream = Box::pin(stream);

    let mut response_text = String::new();

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            if !prefix.is_empty() {
                                println!("{}{}", prefix, text);
                            } else {
                                println!("{}", text);
                            }
                            response_text.push_str(text);
                            log_agent_message!(&task_id, &agent_name, text);
                        }
                        ContentBlock::ToolUse { name, .. } => {
                            log_agent_message!(
                                &task_id,
                                &agent_name,
                                format!("🔧 Using tool: {}", name)
                            );
                        }
                        ContentBlock::ToolResult { tool_use_id, .. } => {
                            log_agent_message!(
                                &task_id,
                                &agent_name,
                                format!("✓ Tool result: {}", tool_use_id)
                            );
                        }
                        _ => {}
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

    println!("\n");
    log_agent_complete!(
        &task_id,
        &agent_name,
        format!("Summary complete ({} chars)", response_text.len())
    );
    Ok(response_text)
}

/// Document with tracked label for reduce phase
#[derive(Debug, Clone)]
struct LabeledDoc {
    content: String,
    label: String,
}

/// Phase 4 Map: Parallel summarization of all research results
async fn map_phase_summarize(
    research_results: &[ResearchResult],
    batch_size: usize,
) -> anyhow::Result<Vec<LabeledDoc>> {
    println!("\n{}", "=".repeat(80));
    println!(
        "PHASE 4 MAP: Summarizing {} Research Results (concurrency: {})",
        research_results.len(),
        batch_size
    );
    println!("{}", "=".repeat(80));

    let sem = Arc::new(Semaphore::new(batch_size));
    let mut tasks = FuturesUnordered::new();

    for (i, result) in research_results.iter().enumerate() {
        let result = result.clone();
        let sem = sem.clone();
        let result_number = i + 1;
        let prefix = format!("[Summarizer {}]: ", result_number);

        tasks.push(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|_| anyhow::anyhow!("Semaphore closed"))?;

            let task_id = format!("summarize_{}", result_number);
            log_task_start!(4, &task_id, format!("Summarizing result {}", result_number));

            let content = summarize_research_result(&result, result_number, Some(&prefix)).await?;

            log_task_complete!(&task_id, format!("Summarized {}", result.title));

            Ok::<LabeledDoc, anyhow::Error>(LabeledDoc {
                content,
                label: format!("doc{}", result_number),
            })
        });
    }

    let mut summaries = Vec::new();
    while let Some(result) = tasks.next().await {
        summaries.push(result?);
    }

    println!("\n[Phase 4 Map] Generated {} summaries", summaries.len());
    Ok(summaries)
}

/// Combine two documents into one
async fn combine_two_docs(
    doc1: &LabeledDoc,
    doc2: Option<&LabeledDoc>,
    prefix: Option<&str>,
    combiner_number: usize,
) -> anyhow::Result<LabeledDoc> {
    use workflow_manager_sdk::{log_agent_complete, log_agent_message, log_agent_start};

    let prefix = prefix.unwrap_or("");
    let task_id = format!("combine_{}", combiner_number);
    let agent_name = format!("Combiner {}", combiner_number);

    let (combine_prompt, new_label) = if let Some(doc2) = doc2 {
        log_agent_start!(
            &task_id,
            &agent_name,
            format!("Combining: {} + {}", doc1.label, doc2.label)
        );
        println!(
            "{}Combining: {} + {} → {}{}",
            prefix,
            doc1.label,
            doc2.label,
            doc1.label,
            &doc2.label[3..]
        );
        let new_label = format!("{}{}", doc1.label, &doc2.label[3..]);
        let prompt = format!(
            r#"Combine these two documentation sections into a single cohesive section.

# Document 1
{}

# Document 2
{}

# Instructions
- Merge overlapping information
- Maintain all key points from both documents
- Organize logically
- Keep it concise
- Output as structured markdown"#,
            doc1.content, doc2.content
        );
        (prompt, new_label)
    } else {
        log_agent_start!(
            &task_id,
            &agent_name,
            format!("Processing single: {}", doc1.label)
        );
        println!("{}Processing single: {} (no pair)", prefix, doc1.label);
        let new_label = doc1.label.clone();
        let prompt = format!(
            r#"Refine and structure this documentation section for clarity.

# Document
{}

# Instructions
- Improve organization and flow
- Keep all key information
- Output as structured markdown"#,
            doc1.content
        );
        (prompt, new_label)
    };

    log_agent_message!(
        &task_id,
        &agent_name,
        format!(
            "Input: {} chars",
            doc1.content.len() + doc2.map_or(0, |d| d.content.len())
        )
    );

    let preset = SystemPromptPreset {
        prompt_type: "preset".to_string(),
        preset: "claude_code".to_string(),
        append: Some("You are a technical writer combining research findings.".to_string()),
    };

    let options = ClaudeAgentOptions::builder()
        .system_prompt(SystemPrompt::Preset(preset))
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    let stream = query(&combine_prompt, Some(options)).await?;
    let mut stream = Box::pin(stream);

    let mut response_text = String::new();

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            response_text.push_str(text);
                            // Only log summary for combining to reduce noise
                            if text.len() > 100 {
                                log_agent_message!(
                                    &task_id,
                                    &agent_name,
                                    format!("{}...", &text[..100])
                                );
                            } else {
                                log_agent_message!(&task_id, &agent_name, text);
                            }
                        }
                        ContentBlock::ToolUse { name, .. } => {
                            log_agent_message!(
                                &task_id,
                                &agent_name,
                                format!("🔧 Using tool: {}", name)
                            );
                        }
                        ContentBlock::ToolResult { tool_use_id, .. } => {
                            log_agent_message!(
                                &task_id,
                                &agent_name,
                                format!("✓ Tool result: {}", tool_use_id)
                            );
                        }
                        _ => {}
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

    log_agent_complete!(
        &task_id,
        &agent_name,
        format!(
            "Combined into {} ({} chars)",
            new_label,
            response_text.len()
        )
    );

    Ok(LabeledDoc {
        content: response_text,
        label: new_label,
    })
}

/// Phase 4 Reduce: Combine documents in parallel rounds
async fn parallel_reduce_round(
    docs: Vec<LabeledDoc>,
    batch_size: usize,
    round_number: usize,
) -> anyhow::Result<Vec<LabeledDoc>> {
    let num_pairs = docs.len().div_ceil(2);
    println!("\n{}", "-".repeat(80));
    println!(
        "REDUCE ROUND {}: {} docs → {} docs (concurrency: {})",
        round_number,
        docs.len(),
        num_pairs,
        batch_size
    );
    println!("{}", "-".repeat(80));

    let sem = Arc::new(Semaphore::new(batch_size));
    let mut tasks = FuturesUnordered::new();

    for (pair_idx, pair) in docs.chunks(2).enumerate() {
        let doc1 = pair[0].clone();
        let doc2 = pair.get(1).cloned();
        let sem = sem.clone();
        let combiner_number = pair_idx + 1;
        let prefix = format!("[Combiner {}]: ", combiner_number);

        tasks.push(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|_| anyhow::anyhow!("Semaphore closed"))?;

            let task_id = format!("combine_round{}_pair{}", round_number, combiner_number);
            let desc = if let Some(ref d2) = doc2 {
                format!("Combining {} + {}", doc1.label, d2.label)
            } else {
                format!("Processing single {}", doc1.label)
            };
            log_task_start!(4, &task_id, desc);

            let result =
                combine_two_docs(&doc1, doc2.as_ref(), Some(&prefix), combiner_number).await;

            if let Ok(ref combined) = result {
                log_task_complete!(&task_id, format!("Combined into {}", combined.label));
            } else if let Err(ref e) = result {
                log_task_failed!(&task_id, format!("Failed to combine: {}", e));
            }

            result
        });
    }

    let mut combined_docs = Vec::new();
    while let Some(result) = tasks.next().await {
        combined_docs.push(result?);
    }

    Ok(combined_docs)
}

/// Phase 4: Map-Reduce synthesis
async fn synthesize_documentation(
    research_results: &[ResearchResult],
    output_path: &Path,
    batch_size: usize,
) -> anyhow::Result<()> {
    use workflow_manager_sdk::{log_task_complete, log_task_start};

    println!("\n{}", "=".repeat(80));
    println!("PHASE 4: Map-Reduce Documentation Synthesis");
    println!("{}", "=".repeat(80));

    // Map phase: Summarize each result
    log_task_start!(
        4,
        "map_phase",
        format!("Summarizing {} research results", research_results.len())
    );
    let mut current_docs = map_phase_summarize(research_results, batch_size).await?;
    log_task_complete!(
        "map_phase",
        format!("Created {} summaries", current_docs.len())
    );

    // Reduce phase: Iteratively combine until one document
    let mut round_number = 1;
    while current_docs.len() > 1 {
        let task_id = format!("reduce_round_{}", round_number);
        log_task_start!(
            4,
            &task_id,
            format!(
                "Reduce round {}: {} → {} docs",
                round_number,
                current_docs.len(),
                current_docs.len().div_ceil(2)
            )
        );
        current_docs = parallel_reduce_round(current_docs, batch_size, round_number).await?;
        log_task_complete!(&task_id, format!("Round {} complete", round_number));
        round_number += 1;
    }

    let final_doc = current_docs
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No final document generated"))?;

    let final_content = final_doc.content;

    // Final polish
    log_task_start!(
        4,
        "final_polish",
        "Creating final comprehensive documentation"
    );

    use workflow_manager_sdk::{log_agent_complete, log_agent_message, log_agent_start};

    let task_id = "final_synthesis";
    let agent_name = "Final Synthesizer";

    log_agent_start!(task_id, agent_name, "Creating comprehensive documentation");

    println!("\n{}", "=".repeat(80));
    println!("FINAL SYNTHESIS: Creating comprehensive documentation");
    println!("{}", "=".repeat(80));

    log_agent_message!(
        task_id,
        agent_name,
        format!(
            "Input: {} chars of synthesized research",
            final_content.len()
        )
    );

    let final_prompt = format!(
        r#"Create the final comprehensive documentation based on the synthesized research below.

# Synthesized Research
{}

# Instructions
- Add introduction and overview sections
- Ensure logical flow and structure
- Add table of contents if appropriate
- Include actionable recommendations
- Polish for clarity and completeness

Save the final documentation to: {}"#,
        final_content,
        output_path.display()
    );

    let preset = SystemPromptPreset {
        prompt_type: "preset".to_string(),
        preset: "claude_code".to_string(),
        append: Some(
            "You are a technical writer creating final comprehensive documentation.".to_string(),
        ),
    };

    let options = ClaudeAgentOptions::builder()
        .system_prompt(SystemPrompt::Preset(preset))
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    let stream = query(&final_prompt, Some(options)).await?;
    let mut stream = Box::pin(stream);

    let mut doc_length = 0;

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            print!("{}", text);
                            doc_length += text.len();
                            // Log periodic progress
                            if doc_length % 1000 < text.len() {
                                log_agent_message!(
                                    task_id,
                                    agent_name,
                                    format!("Writing... ({} chars)", doc_length)
                                );
                            }
                        }
                        ContentBlock::ToolUse { name, .. } => {
                            log_agent_message!(
                                task_id,
                                agent_name,
                                format!("🔧 Using tool: {}", name)
                            );
                        }
                        ContentBlock::ToolResult { tool_use_id, .. } => {
                            log_agent_message!(
                                task_id,
                                agent_name,
                                format!("✓ Tool result: {}", tool_use_id)
                            );
                        }
                        _ => {}
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

    log_agent_complete!(
        task_id,
        agent_name,
        format!("Final documentation complete ({} chars)", doc_length)
    );
    log_task_complete!("final_polish", "Documentation complete");
    println!("\n");
    Ok(())
}

/// Extract YAML from markdown code blocks and clean document separators
fn extract_yaml(text: &str) -> String {
    let yaml = if text.contains("```yaml") {
        let yaml_start = text.find("```yaml").unwrap() + 7;
        let yaml_end = text[yaml_start..].rfind("```")
            .map(|pos| pos + yaml_start)
            .unwrap_or(text.len());
        text[yaml_start..yaml_end].trim().to_string()
    } else if text.contains("```") {
        let yaml_start = text.find("```").unwrap() + 3;
        let yaml_end = text[yaml_start..].rfind("```")
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

/// Load file content or return literal string
async fn load_prompt_file(file_path: &str) -> anyhow::Result<String> {
    let path = Path::new(file_path);
    if path.exists() && path.is_file() {
        Ok(fs::read_to_string(path).await?)
    } else {
        Ok(file_path.to_string())
    }
}

/// Find all YAML files in a directory
async fn find_yaml_files(dir_path: &str) -> anyhow::Result<Vec<String>> {
    let mut yaml_files = Vec::new();
    let mut entries = fs::read_dir(dir_path).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "yaml" || ext == "yml" {
                    if let Some(path_str) = path.to_str() {
                        yaml_files.push(path_str.to_string());
                    }
                }
            }
        }
    }

    yaml_files.sort();
    Ok(yaml_files)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Handle metadata request from TUI
    if args.workflow_metadata {
        args.print_metadata();
        return Ok(());
    }

    // Parse phases to execute
    let phases_to_run: Vec<usize> = args
        .phases
        .split(',')
        .filter_map(|p| p.trim().parse().ok())
        .collect();

    // Validate required arguments based on phases
    if phases_to_run.contains(&1) {
        if args.input.is_none() {
            anyhow::bail!("--input is required when running phase 1");
        }
        if args.system_prompt.is_none() {
            anyhow::bail!("--system-prompt is required when running phase 1");
        }
        if args.append.is_none() {
            anyhow::bail!("--append is required when running phase 1");
        }
    }

    // Change working directory to target directory if specified
    if let Some(dir) = &args.dir {
        let target_dir = PathBuf::from(dir).canonicalize().map_err(|e| {
            anyhow::anyhow!("Invalid directory path '{}': {}", dir, e)
        })?;
        std::env::set_current_dir(&target_dir).map_err(|e| {
            anyhow::anyhow!("Failed to change directory to '{}': {}", target_dir.display(), e)
        })?;
        println!("📁 Working directory: {}", target_dir.display());
        println!();
    }

    // Create directory structure for workflow artifacts
    fs::create_dir_all("./RESULTS").await?;
    fs::create_dir_all("./OUTPUT").await?;

    let mut codebase_analysis: Option<CodebaseAnalysis> = None;
    let mut prompts_data: Option<PromptsData> = None;
    let mut research_results: Vec<ResearchResult> = Vec::new();

    // Phase 0: Analyze codebase
    if phases_to_run.contains(&0) {
        log_phase_start!(0, "Analyze Codebase", 5);
        log_task_start!(
            0,
            "analyze",
            "Analyzing codebase structure and dependencies"
        );

        let codebase_path = args
            .dir
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap());

        let analysis = analyze_codebase(&codebase_path).await?;

        // Save analysis to file
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let analysis_path = PathBuf::from(format!("./OUTPUT/codebase_analysis_{}.yaml", timestamp));
        let analysis_yaml = serde_yaml::to_string(&analysis)?;
        fs::write(&analysis_path, analysis_yaml).await?;
        println!("[Phase 0] Analysis saved to: {}", analysis_path.display());

        log_task_complete!("analyze", format!("Saved to {}", analysis_path.display()));
        log_state_file!(0, analysis_path.display().to_string(), "Codebase analysis");
        log_phase_complete!(0, "Analyze Codebase");

        codebase_analysis = Some(analysis);
    } else if let Some(analysis_file) = &args.analysis_file {
        let content = fs::read_to_string(analysis_file).await?;
        codebase_analysis = Some(serde_yaml::from_str(&content)?);
        println!("[Phase 0] Loaded analysis from: {}", analysis_file);
    }

    // Phase 1: Generate prompts
    if phases_to_run.contains(&1) {
        log_phase_start!(1, "Generate Prompts", 5);
        log_task_start!(1, "generate", "Generating research prompts from objective");

        let analysis = codebase_analysis.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Phase 0 must run before Phase 1, or provide --analysis-file")
        })?;

        let prompt_writer = load_prompt_file(args.system_prompt.as_ref().unwrap()).await?;
        let output_style = load_prompt_file(args.append.as_ref().unwrap()).await?;

        let prompts = generate_prompts(
            args.input.as_ref().unwrap(),
            analysis,
            &prompt_writer,
            &output_style,
        )
        .await?;

        // Save prompts to file
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let prompts_path = PathBuf::from(format!("./OUTPUT/research_prompts_{}.yaml", timestamp));
        let prompts_yaml = serde_yaml::to_string(&prompts)?;
        fs::write(&prompts_path, prompts_yaml).await?;
        println!("[Phase 1] Prompts saved to: {}", prompts_path.display());
        println!("Generated {} research prompts", prompts.prompts.len());

        log_task_complete!(
            "generate",
            format!("Generated {} prompts", prompts.prompts.len())
        );
        log_state_file!(
            1,
            prompts_path.display().to_string(),
            "Research prompts for Phase 2"
        );
        log_phase_complete!(1, "Generate Prompts");

        prompts_data = Some(prompts);
    } else if let Some(prompts_file) = &args.prompts_file {
        let content = fs::read_to_string(prompts_file).await?;
        prompts_data = Some(serde_yaml::from_str(&content)?);
        println!("[Phase 1] Loaded prompts from: {}", prompts_file);
    }

    // Phase 2: Execute research prompts concurrently
    if phases_to_run.contains(&2) {
        log_phase_start!(2, "Execute Research", 5);

        let prompts = prompts_data.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Phase 1 must run before Phase 2, or provide --prompts-file")
        })?;

        // Create RESULTS directory if it doesn't exist
        fs::create_dir_all("./RESULTS").await?;

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let total_prompts = prompts.prompts.len();
        let sem = Arc::new(Semaphore::new(args.batch_size));

        println!("{}", "=".repeat(80));
        println!(
            "PHASE 2: Executing {} Research Prompts (concurrency: {})",
            total_prompts, args.batch_size
        );
        println!("{}", "=".repeat(80));

        // Push all tasks to FuturesUnordered
        let mut tasks = FuturesUnordered::new();
        for (i, prompt) in prompts.prompts.iter().enumerate() {
            let prompt = prompt.clone();
            let timestamp = timestamp.clone();
            let sem = sem.clone();
            let result_number = i + 1;
            let prefix = format!("[Suborchestrator {}]: ", result_number);

            tasks.push(async move {
                let _permit = sem
                    .acquire()
                    .await
                    .map_err(|_| anyhow::anyhow!("Semaphore closed"))?;

                let task_id = format!("research_{}", result_number);
                log_task_start!(
                    2,
                    &task_id,
                    format!("Research task {}/{}", result_number, total_prompts),
                    total_prompts
                );

                let result =
                    execute_research_prompt(&prompt, result_number, &timestamp, Some(&prefix))
                        .await?;
                log_task_complete!(&task_id, format!("Completed research {}", result_number));
                Ok::<_, anyhow::Error>(result)
            });
        }

        // Collect results as they complete (fail-fast on first error)
        while let Some(result) = tasks.next().await {
            let research_result = result?;
            research_results.push(research_result);
        }

        // Save research results to file
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let results_path = PathBuf::from(format!("./RESULTS/research_results_{}.yaml", timestamp));
        let results_yaml = serde_yaml::to_string(&research_results)?;
        fs::write(&results_path, results_yaml).await?;
        println!("\n[Phase 2] Results saved to: {}", results_path.display());

        log_state_file!(
            2,
            results_path.display().to_string(),
            "Research results for Phase 3 validation"
        );
        log_phase_complete!(2, "Execute Research");
    } else if let Some(results_file) = &args.results_file {
        let content = fs::read_to_string(results_file).await?;
        research_results = serde_yaml::from_str(&content)?;
        println!("[Phase 2] Loaded results from: {}", results_file);
    }

    // Phase 3: Validate and fix YAML files
    if phases_to_run.contains(&3) {
        log_phase_start!(3, "Validate YAML", 5);
        log_task_start!(3, "validate_initial", "Initial YAML validation scan");

        println!("\n{}", "=".repeat(80));
        println!("PHASE 3: Validating YAML Results");
        println!("{}", "=".repeat(80));

        // Determine which files to validate
        let result_files: Vec<String> = if let Some(results_dir) = &args.results_dir {
            // Use directory path to find all YAML files
            println!("Scanning directory for YAML files: {}", results_dir);
            let files = find_yaml_files(results_dir).await?;
            println!("Found {} YAML files", files.len());
            files
        } else if !research_results.is_empty() {
            // Use results from Phase 2
            research_results
                .iter()
                .map(|r| r.response_file.clone())
                .collect()
        } else {
            anyhow::bail!("No YAML files to validate. Run Phase 2 first, provide --results-file, or specify --results-dir");
        };

        let mut validation_tasks = FuturesUnordered::new();
        for file in &result_files {
            let file = file.clone();
            validation_tasks.push(async move { validate_yaml_file(&file).await });
        }

        let mut files_with_errors = Vec::new();
        while let Some(result) = validation_tasks.next().await {
            let (file, is_valid, error) = result?;
            if !is_valid {
                files_with_errors.push((file, error));
            }
        }

        log_task_complete!(
            "validate_initial",
            format!("Found {} files with errors", files_with_errors.len())
        );

        // Loop to fix and re-validate until all are valid
        let mut fix_iteration = 0;
        loop {
            if files_with_errors.is_empty() {
                println!("\n✓ All files validated successfully!");
                break;
            }

            fix_iteration += 1;
            let task_id = format!("fix_iteration_{}", fix_iteration);
            log_task_start!(
                3,
                &task_id,
                format!(
                    "Fixing {} YAML files (iteration {})",
                    files_with_errors.len(),
                    fix_iteration
                )
            );

            println!(
                "\n⚠ Found {} files with errors. Fixing...",
                files_with_errors.len()
            );

            let current_batch = std::mem::take(&mut files_with_errors);

            // Fix all broken files in parallel
            let sem = Arc::new(Semaphore::new(args.batch_size));
            let mut fix_tasks = FuturesUnordered::new();

            for (i, (file, error)) in current_batch.iter().enumerate() {
                let file = file.clone();
                let error = error.clone();
                let sem = sem.clone();
                let fixer_number = i + 1;
                let prefix = format!("[YAML Fixer {}]: ", fixer_number);

                fix_tasks.push(async move {
                    let _permit = sem
                        .acquire()
                        .await
                        .map_err(|_| anyhow::anyhow!("Semaphore closed"))?;

                    let fix_task_id = format!("fix_yaml_{}", fixer_number);
                    log_task_start!(
                        3,
                        &fix_task_id,
                        format!("Fixing YAML file {}", fixer_number)
                    );

                    let result = execute_fix_yaml(&file, &error, Some(&prefix), fixer_number).await;

                    if result.is_ok() {
                        log_task_complete!(&fix_task_id, format!("Fixed {}", file));
                    } else if let Err(ref e) = result {
                        log_task_failed!(&fix_task_id, format!("Failed to fix: {}", e));
                    }

                    result
                });
            }

            // Wait for all fixes to complete (fail-fast on error)
            while let Some(result) = fix_tasks.next().await {
                result?;
            }

            // Re-validate the files we just fixed and repopulate files_with_errors
            for (file, _) in current_batch {
                let (path, is_valid, error_msg) = validate_yaml_file(&file).await?;
                if !is_valid {
                    files_with_errors.push((path, error_msg));
                }
            }

            log_task_complete!(
                &task_id,
                format!("{} files still invalid", files_with_errors.len())
            );
        }

        log_phase_complete!(3, "Validate YAML");
    }

    // Phase 4: Synthesize documentation
    if phases_to_run.contains(&4) {
        log_phase_start!(4, "Synthesize Docs", 5);

        if research_results.is_empty() {
            anyhow::bail!("No research results to synthesize");
        }

        let output_path = if let Some(output) = &args.output {
            PathBuf::from(output)
        } else {
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
            PathBuf::from(format!("./OUTPUT/research_output_{}.md", timestamp))
        };

        synthesize_documentation(&research_results, &output_path, args.batch_size).await?;

        log_state_file!(
            4,
            output_path.display().to_string(),
            "Final synthesized documentation"
        );
        log_phase_complete!(4, "Synthesize Docs");

        println!("\n{}", "=".repeat(80));
        println!(
            "Research complete! Documentation saved to: {}",
            output_path.display()
        );
        println!("{}", "=".repeat(80));
    } else {
        println!("\n{}", "=".repeat(80));
        println!("Selected phases completed!");
        println!("{}", "=".repeat(80));
    }

    Ok(())
}
