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

#[derive(Parser, Debug)]
#[command(author, version, about = "Research Agent - Analyze codebase, generate prompts, execute research, validate YAML, synthesize documentation", long_about = None)]
struct Args {
    /// Research objective/question
    #[arg(short, long)]
    input: Option<String>,

    /// Prompt writer system prompt (file path or string)
    #[arg(short = 's', long)]
    system_prompt: Option<String>,

    /// Output style format (file path or string)
    #[arg(short = 'a', long)]
    append: Option<String>,

    /// Output file path for final documentation
    #[arg(short, long)]
    output: Option<String>,

    /// Number of research prompts to execute in parallel (default: 1 for sequential)
    #[arg(long, default_value = "1")]
    batch_size: usize,

    /// Comma-separated phases to execute (0=analyze, 1=prompts, 2=research, 3=validate, 4=synthesize)
    #[arg(long, default_value = "0,1,2,3,4")]
    phases: String,

    /// Path to saved codebase analysis YAML (for resuming from Phase 1)
    #[arg(long)]
    analysis_file: Option<String>,

    /// Path to saved prompts YAML (for resuming from Phase 2)
    #[arg(long)]
    prompts_file: Option<String>,

    /// Path to saved research results YAML (for resuming from Phase 3)
    #[arg(long)]
    results_file: Option<String>,

    /// Directory path to analyze for Phase 0
    #[arg(long)]
    dir: Option<String>,

    /// Directory containing YAML files to validate (for Phase 3)
    #[arg(long)]
    results_dir: Option<String>,
}

/// Phase 0: Analyze codebase structure
async fn analyze_codebase(codebase_path: &Path) -> anyhow::Result<CodebaseAnalysis> {
    println!("{}", "=".repeat(80));
    println!("PHASE 0: Analyzing Codebase Structure");
    println!("{}", "=".repeat(80));

    let analysis_prompt = format!(
        r#"Analyze the codebase at {} and provide a structured overview.

# Required Analysis

## 1. File Statistics
- Count files by extension (.rs, .py, .js, .md, etc.)
- Total lines of code estimate
- Identify test files vs source files

## 2. Directory Structure
- Map top 3 directory levels
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
Provide analysis as YAML with proper structure. Be concise but comprehensive."#,
        codebase_path.display()
    );

    let options = ClaudeAgentOptions::builder()
        .system_prompt(
            "You are a codebase analyst. Provide concise structural analysis.".to_string(),
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
                        println!("{}", text);
                        response_text.push_str(text);
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

    println!("\n");

    // Extract and parse YAML
    let yaml_content = extract_yaml(&response_text);
    let analysis: CodebaseAnalysis = serde_yaml::from_str(&yaml_content)
        .map_err(|e| anyhow::anyhow!("Failed to parse analysis YAML: {}", e))?;

    Ok(analysis)
}

/// Phase 1: Generate research prompts based on objective
async fn generate_prompts(
    objective: &str,
    codebase_analysis: &CodebaseAnalysis,
    prompt_writer: &str,
    output_style: &str,
) -> anyhow::Result<PromptsData> {
    println!("{}", "=".repeat(80));
    println!("PHASE 1: Generating Research Prompts");
    println!("{}", "=".repeat(80));

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
                        println!("{}", text);
                        response_text.push_str(text);
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

    println!("\n");

    let yaml_content = extract_yaml(&response_text);
    let prompts_data: PromptsData = serde_yaml::from_str(&yaml_content)
        .map_err(|e| anyhow::anyhow!("Failed to parse prompts YAML: {}", e))?;

    Ok(prompts_data)
}

/// Phase 2: Execute a single research prompt
async fn execute_research_prompt(
    prompt: &ResearchPrompt,
    result_number: usize,
    timestamp: &str,
    prefix: Option<&str>,
) -> anyhow::Result<ResearchResult> {
    let prefix = prefix.unwrap_or("");
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
                    if let ContentBlock::Text { text } = block {
                        if !prefix.is_empty() {
                            println!("{}{}", prefix, text);
                        } else {
                            println!("{}", text);
                        }
                        response_text.push_str(text);
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
    fs::write(&response_filename, &yaml_content).await?;

    println!("{}Response saved to: {}", prefix, response_filename);

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
        .args(&[
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
) -> anyhow::Result<()> {
    let prefix = prefix.unwrap_or("");

    println!("\n{}", "-".repeat(80));
    println!("{}FIXING: {}", prefix, file_path);
    println!("{}", "-".repeat(80));

    // Read the broken YAML
    let broken_yaml = fs::read_to_string(file_path).await?;

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
                    if let ContentBlock::Text { text } = block {
                        if !prefix.is_empty() {
                            println!("{}{}", prefix, text);
                        } else {
                            println!("{}", text);
                        }
                        response_text.push_str(text);
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

    // Extract and write fixed YAML
    let fixed_yaml = extract_yaml(&response_text);
    fs::write(file_path, &fixed_yaml).await?;

    println!("\n{}Fixed YAML written to: {}", prefix, file_path);

    Ok(())
}

/// Phase 4 Map: Summarize a single research result
async fn summarize_research_result(
    result: &ResearchResult,
    _result_number: usize,
    prefix: Option<&str>,
) -> anyhow::Result<String> {
    let prefix = prefix.unwrap_or("");
    println!("\n{}", "-".repeat(80));
    println!("{}SUMMARIZING: {}", prefix, result.title);
    println!("{}", "-".repeat(80));

    // Read the result file
    let response_content = fs::read_to_string(&result.response_file)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", result.response_file, e))?;

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
                    if let ContentBlock::Text { text } = block {
                        if !prefix.is_empty() {
                            println!("{}{}", prefix, text);
                        } else {
                            println!("{}", text);
                        }
                        response_text.push_str(text);
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

    println!("\n");
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
            let content = summarize_research_result(&result, result_number, Some(&prefix)).await?;
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
) -> anyhow::Result<LabeledDoc> {
    let prefix = prefix.unwrap_or("");

    let (combine_prompt, new_label) = if let Some(doc2) = doc2 {
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
                    if let ContentBlock::Text { text } = block {
                        response_text.push_str(text);
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

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
    let num_pairs = (docs.len() + 1) / 2;
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
        let prefix = format!("[Combiner {}]: ", pair_idx + 1);

        tasks.push(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|_| anyhow::anyhow!("Semaphore closed"))?;
            combine_two_docs(&doc1, doc2.as_ref(), Some(&prefix)).await
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
    println!("\n{}", "=".repeat(80));
    println!("PHASE 4: Map-Reduce Documentation Synthesis");
    println!("{}", "=".repeat(80));

    // Map phase: Summarize each result
    let mut current_docs = map_phase_summarize(research_results, batch_size).await?;

    // Reduce phase: Iteratively combine until one document
    let mut round_number = 1;
    while current_docs.len() > 1 {
        current_docs = parallel_reduce_round(current_docs, batch_size, round_number).await?;
        round_number += 1;
    }

    let final_doc = current_docs
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No final document generated"))?;

    let final_content = final_doc.content;

    // Final polish
    println!("\n{}", "=".repeat(80));
    println!("FINAL SYNTHESIS: Creating comprehensive documentation");
    println!("{}", "=".repeat(80));

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

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    if let ContentBlock::Text { text } = block {
                        print!("{}", text);
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

    println!("\n");
    Ok(())
}

/// Extract YAML from markdown code blocks
fn extract_yaml(text: &str) -> String {
    if text.contains("```yaml") {
        let yaml_start = text.find("```yaml").unwrap() + 7;
        let yaml_end = text[yaml_start..].find("```").unwrap() + yaml_start;
        text[yaml_start..yaml_end].trim().to_string()
    } else if text.contains("```") {
        let yaml_start = text.find("```").unwrap() + 3;
        let yaml_end = text[yaml_start..].find("```").unwrap() + yaml_start;
        text[yaml_start..yaml_end].trim().to_string()
    } else {
        text.trim().to_string()
    }
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

    let mut codebase_analysis: Option<CodebaseAnalysis> = None;
    let mut prompts_data: Option<PromptsData> = None;
    let mut research_results: Vec<ResearchResult> = Vec::new();

    // Phase 0: Analyze codebase
    if phases_to_run.contains(&0) {
        let codebase_path = args
            .dir
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap());

        let analysis = analyze_codebase(&codebase_path).await?;

        // Save analysis to file
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let analysis_path = PathBuf::from(format!("codebase_analysis_{}.yaml", timestamp));
        let analysis_yaml = serde_yaml::to_string(&analysis)?;
        fs::write(&analysis_path, analysis_yaml).await?;
        println!("[Phase 0] Analysis saved to: {}", analysis_path.display());

        codebase_analysis = Some(analysis);
    } else if let Some(analysis_file) = &args.analysis_file {
        let content = fs::read_to_string(analysis_file).await?;
        codebase_analysis = Some(serde_yaml::from_str(&content)?);
        println!("[Phase 0] Loaded analysis from: {}", analysis_file);
    }

    // Phase 1: Generate prompts
    if phases_to_run.contains(&1) {
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
        let prompts_path = PathBuf::from(format!("research_prompts_{}.yaml", timestamp));
        let prompts_yaml = serde_yaml::to_string(&prompts)?;
        fs::write(&prompts_path, prompts_yaml).await?;
        println!("[Phase 1] Prompts saved to: {}", prompts_path.display());
        println!("Generated {} research prompts", prompts.prompts.len());

        prompts_data = Some(prompts);
    } else if let Some(prompts_file) = &args.prompts_file {
        let content = fs::read_to_string(prompts_file).await?;
        prompts_data = Some(serde_yaml::from_str(&content)?);
        println!("[Phase 1] Loaded prompts from: {}", prompts_file);
    }

    // Phase 2: Execute research prompts concurrently
    if phases_to_run.contains(&2) {
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
                execute_research_prompt(&prompt, result_number, &timestamp, Some(&prefix)).await
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
    } else if let Some(results_file) = &args.results_file {
        let content = fs::read_to_string(results_file).await?;
        research_results = serde_yaml::from_str(&content)?;
        println!("[Phase 2] Loaded results from: {}", results_file);
    }

    // Phase 3: Validate and fix YAML files
    if phases_to_run.contains(&3) {
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

        // Loop to fix and re-validate until all are valid
        loop {
            if files_with_errors.is_empty() {
                println!("\n✓ All files validated successfully!");
                break;
            }

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
                let prefix = format!("[YAML Fixer {}]: ", i + 1);

                fix_tasks.push(async move {
                    let _permit = sem
                        .acquire()
                        .await
                        .map_err(|_| anyhow::anyhow!("Semaphore closed"))?;
                    execute_fix_yaml(&file, &error, Some(&prefix)).await
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
        }
    }

    // Phase 4: Synthesize documentation
    if phases_to_run.contains(&4) {
        if research_results.is_empty() {
            anyhow::bail!("No research results to synthesize");
        }

        let output_path = args.output.as_ref().map(PathBuf::from).unwrap_or_else(|| {
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
            PathBuf::from(format!("research_output_{}.md", timestamp))
        });

        synthesize_documentation(&research_results, &output_path, args.batch_size).await?;

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
