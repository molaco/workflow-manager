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
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{fs, sync::Semaphore};
use workflow_manager::research::{
    cli::Args, phase0_analyze::analyze_codebase, phase1_prompts::generate_prompts, types::*,
};
use workflow_manager_sdk::{
    log_phase_complete, log_phase_start, log_state_file, log_task_complete, log_task_failed,
    log_task_start, WorkflowDefinition,
};

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

/// Phase 4: Synthesize documentation from research results
async fn synthesize_documentation(results_file: &Path, output_path: &Path) -> anyhow::Result<()> {
    use claude_agent_sdk::AgentDefinition;
    use workflow_manager_sdk::{
        log_agent_complete, log_agent_message, log_agent_start, log_task_complete, log_task_start,
    };

    println!("\n{}", "=".repeat(80));
    println!("PHASE 4: Documentation Synthesis");
    println!("{}", "=".repeat(80));
    println!("results file path: {}", results_file.display());

    let task_id = "synthesize";
    let agent_name = "Documentation Agent";

    log_task_start!(4, task_id, "Synthesizing final documentation");
    log_agent_start!(
        task_id,
        agent_name,
        "Preparing synthesis with file-condenser subagent"
    );

    // Create unified synthesis prompt - agent decides strategy
    let synthesis_prompt = format!(
        r#"Read the research results overview file at {} and create comprehensive documentation.

The overview file contains a list of research findings with:
- title: The topic researched
- query: The research question
- response_file: Path to the YAML file containing the detailed findings
- focus: Key areas covered

# Your Task:

1. **Analyze the overview**: Read the overview file to understand all research findings

2. **Assess file sizes**: For each research finding, read the response_file and count the characters/lines

3. **Decide processing strategy**:
   - For files with **< 30,000 characters**: Read and process them directly
   - For files with **≥ 30,000 characters**: Use the Task tool to invoke the 'file-condenser' agent
     - Pass a message like "Condense this file: path/to/file.yaml"
     - The file-condenser will read the file and return a condensed summary (~5-10k chars)
     - Use the condensed version in your documentation

4. **Synthesize documentation**:
   - Combine all content (direct reads + condensed summaries) into cohesive, well-structured markdown
   - Include code examples, technical details, and actionable insights
   - Organize logically with proper headings and sections
   - Maintain technical accuracy while ensuring readability

5. **Write output**: Write the final documentation to {}

# Available Subagent:
- **file-condenser**: Condenses large research result files while preserving key technical details

# Strategy Tips:
- You can process multiple files with the file-condenser in parallel if needed
- Small files can be read directly for maximum detail
- Large files should be condensed to manage context efficiently
- Your goal is comprehensive yet manageable documentation"#,
        results_file.display(),
        output_path.display()
    );

    let preset = SystemPromptPreset {
        prompt_type: "preset".to_string(),
        preset: "claude_code".to_string(),
        append: Some(
            "You are a technical writer creating comprehensive documentation from research findings. You can intelligently decide when to condense large files versus read smaller files directly."
                .to_string(),
        ),
    };

    // Build options with file-condenser subagent
    let options = ClaudeAgentOptions::builder()
        .system_prompt(SystemPrompt::Preset(preset))
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .allowed_tools(vec![
            "Read".to_string(),
            "Write".to_string(),
            "Task".to_string(),
        ])
        .add_agent(
            "file-condenser",
            AgentDefinition {
                description: "Condenses a single research result file while preserving key technical details, code examples, and actionable insights".to_string(),
                prompt: "You are a technical documentation condenser. Read the provided research result YAML file and create a condensed summary that:\n\n1. Preserves all key technical details and insights\n2. Includes important code examples (condensed if very long)\n3. Maintains actionable recommendations\n4. Reduces verbosity and redundancy\n5. Target output: 5,000-10,000 characters\n\nReturn ONLY the condensed markdown content. Do not write to any files.".to_string(),
                tools: Some(vec!["Read".to_string()]),
                model: Some("sonnet".to_string()),
            },
        )
        .build();

    log_agent_message!(task_id, agent_name, "Querying Claude for synthesis");

    let stream = query(&synthesis_prompt, Some(options)).await?;
    let mut stream = Box::pin(stream);

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            println!("{}", text);
                            log_agent_message!(task_id, agent_name, text);
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

    println!("\n{}", "=".repeat(80));
    println!("✓ Documentation synthesis complete");
    println!("{}", "=".repeat(80));

    log_agent_complete!(task_id, agent_name, "Documentation synthesis complete");
    log_task_complete!(
        task_id,
        format!("Output written to {}", output_path.display())
    );

    Ok(())
}

/// Extract YAML from markdown code blocks and clean document separators
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
        let target_dir = PathBuf::from(dir)
            .canonicalize()
            .map_err(|e| anyhow::anyhow!("Invalid directory path '{}': {}", dir, e))?;
        std::env::set_current_dir(&target_dir).map_err(|e| {
            anyhow::anyhow!(
                "Failed to change directory to '{}': {}",
                target_dir.display(),
                e
            )
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
    let mut results_file_path: Option<PathBuf> = None;

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

        results_file_path = Some(results_path);
    } else if let Some(results_file) = &args.results_file {
        let content = fs::read_to_string(results_file).await?;
        research_results = serde_yaml::from_str(&content)?;
        println!("[Phase 2] Loaded results from: {}", results_file);
        results_file_path = Some(PathBuf::from(results_file));
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

        let results_file = results_file_path.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "No research results file available. Run Phase 2 first or provide --results-file"
            )
        })?;

        let output_path = if let Some(output) = &args.output {
            PathBuf::from(output)
        } else {
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
            PathBuf::from(format!("./OUTPUT/research_output_{}.md", timestamp))
        };

        synthesize_documentation(results_file, &output_path).await?;

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
