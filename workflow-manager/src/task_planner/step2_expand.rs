//! Step 2: Expand tasks into detailed specifications
//!
//! This module implements the suborchestrator pattern where each high-level task
//! is expanded into a detailed specification using specialized sub-agents for:
//! - Files identification
//! - Functions/code items specification
//! - Formal verification requirements
//! - Test strategy and implementation

use anyhow::{Context, Result};
use futures::future::join_all;
use futures::StreamExt;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

use crate::task_planner::execution_plan::{
    build_execution_batches_fallback, generate_execution_plan_ai, generate_execution_plan_simple,
    parse_execution_plan,
};
use crate::task_planner::types::{TaskOverview, UsageStats};
use crate::task_planner::utils::{clean_yaml_response, parse_tasks_overview};
use workflow_manager_sdk::{
    log_agent_complete, log_agent_message, log_agent_start, log_phase_complete, log_phase_start,
    log_state_file, log_task_complete, log_task_start,
};

/// Create sub-agent definitions for task expansion
fn create_sub_agents() -> HashMap<String, claude_agent_sdk::AgentDefinition> {
    let mut agents = HashMap::new();

    agents.insert(
        "files".to_string(),
        claude_agent_sdk::AgentDefinition {
            description: "Specialist that identifies all files to be created or modified"
                .to_string(),
            prompt: r#"You are a files identification specialist.

Identify all files that will be created or modified for the task.
For each file, provide:
- path: Full path to the file
- description: Brief description of the file's role

IMPORTANT: Use literal block syntax (|) for multi-line descriptions!

Output format:
files:
  - path: "path/to/file.rs"
    description: "Brief single-line description"
  - path: "path/to/complex_file.rs"
    description: |
      Multi-line description
      with more details.

Output valid YAML only, no markdown."#
                .to_string(),
            tools: Some(vec!["Read".to_string(), "Grep".to_string(), "Glob".to_string()]),
            model: Some("sonnet".to_string()),
        },
    );

    agents.insert(
        "functions".to_string(),
        claude_agent_sdk::AgentDefinition {
            description:
                "Specialist that specifies functions, structs, traits, and other code items"
                    .to_string(),
            prompt: r#"You are a functions specification specialist.

Identify all functions, structs, enums, traits, and other items to be implemented.
For each item, provide:
- type: enum_variant|struct|trait_impl|method|constant|function|module_declaration
- name: Full qualified name or signature
- description: Brief description of purpose and behavior
- preconditions: What must be true before execution (optional)
- postconditions: What will be true after execution (optional)
- invariants: Properties that remain constant (optional)

Group items by file.

IMPORTANT: Use literal block syntax (|) for multi-line strings!

Output format:
functions:
  - file: "path/to/file.rs"
    items:
      - type: "function"
        name: "function_name"
        description: |
          Brief description here.
          Can span multiple lines.
        preconditions: |
          - Condition 1
          - Condition 2
        postconditions: |
          - Outcome 1

Output valid YAML only, no markdown."#
                .to_string(),
            tools: Some(vec!["Read".to_string(), "Grep".to_string(), "Glob".to_string()]),
            model: Some("sonnet".to_string()),
        },
    );

    agents.insert(
        "formal".to_string(),
        claude_agent_sdk::AgentDefinition {
            description: "Specialist that determines formal verification requirements".to_string(),
            prompt: r#"You are a formal verification specialist.

Determine if formal verification is needed for the task.
Provide:
- needed: true or false
- level: None|Basic|Critical
- explanation: Why verification is/isn't needed
- properties: List formal properties to verify (if needed)
- strategy: Verification approach (if needed)

Output format:
formal_verification:
  needed: false
  level: "None"
  explanation: |
    Explanation here

Output valid YAML only, no markdown."#
                .to_string(),
            tools: Some(vec!["Read".to_string()]),
            model: Some("sonnet".to_string()),
        },
    );

    agents.insert(
        "tests".to_string(),
        claude_agent_sdk::AgentDefinition {
            description: "Specialist that designs test strategy and implements test code"
                .to_string(),
            prompt: r#"You are a testing specialist.

Design comprehensive tests for the task.
Provide:
- strategy: approach and rationale
- implementation: Complete test code in Rust
- coverage: List of behaviors tested

CRITICAL: ALL code blocks MUST use literal block syntax (|) - this is mandatory!

Output format:
tests:
  strategy:
    approach: "unit tests"
    rationale:
      - "Reason 1"
  implementation:
    file: "tests/test_file.rs"
    location: "create new"
    code: |
      #[cfg(test)]
      mod tests {
          // Test code here
      }
  coverage:
    - "Behavior 1"

Output valid YAML only, no markdown."#
                .to_string(),
            tools: Some(vec!["Read".to_string(), "Grep".to_string()]),
            model: Some("sonnet".to_string()),
        },
    );

    agents
}

/// Suborchestrator expands a single task using sub-agents
///
/// This function coordinates multiple specialized sub-agents to expand a high-level
/// task overview into a complete detailed specification.
pub async fn suborchestrator_expand_task(
    task_overview: &TaskOverview,
    task_template: &str,
    debug: bool,
) -> Result<(String, UsageStats)> {
    let task_id = task_overview.task.id;
    let task_name = &task_overview.task.name;

    if debug {
        println!("  Debug: Starting expansion for Task {}: {}", task_id, task_name);
    }

    // Serialize task overview for the prompt
    let task_overview_yaml =
        serde_yaml::to_string(task_overview).context("Failed to serialize task overview")?;

    // Define specialized sub-agents
    let agents = create_sub_agents();

    // System prompt for suborchestrator
    let system_prompt = format!(
        r#"Your task is to expand Task {} ("{}") from a high-level overview into a complete, detailed specification.

## OBJECTIVE
Transform the task overview below into a complete task specification that matches the task_template structure by delegating to specialized agents.

IMPORTANT: You are in the PLANNING phase. DO NOT create, write, or modify any files. Your sole purpose is to OUTPUT a YAML specification that describes what should be implemented.

## INPUT: TASK OVERVIEW (High-level)
This is the current state of Task {} - a strategic description of WHAT needs to be done and WHY:
```yaml
{}
```

## OUTPUT TARGET: TASK TEMPLATE (Detailed structure)
Your goal is to produce a complete YAML document following this template structure:
```yaml
{}
```

## YOUR SPECIALIZED AGENTS
You have 4 sub-agents available to help you fill out different sections of the task_template:

1. **@files agent** → Fills the `files:` section
   - Identifies all files to create/modify
   - Provides paths and descriptions

2. **@functions agent** → Fills the `functions:` section
   - Specifies all code items to implement (functions, structs, traits, etc.)
   - Groups by file with detailed specifications

3. **@formal agent** → Fills the `formal_verification:` section
   - Determines if formal verification is needed
   - Specifies verification strategy if applicable

4. **@tests agent** → Fills the `tests:` section
   - Designs test strategy and rationale
   - Provides complete test implementation code

## WORKFLOW
1. Delegate to @files, @functions, @formal, and @tests agents (you can call them in parallel or sequentially)
2. Review each agent's output for completeness
3. Ask follow-up questions to any agent if their output is unclear or incomplete
4. Combine all agent outputs into the final task specification
5. Ensure the output follows the task_template structure exactly

## YAML FORMATTING REQUIREMENTS (CRITICAL!)
When combining sub-agent outputs into the final YAML, you MUST follow these rules:

1. **All code blocks MUST use literal block syntax with pipe (|)**
2. **Multi-line strings MUST use literal block syntax (| or |-)**
3. **Preserve exact literal block format from sub-agent responses**

## IMPORTANT REQUIREMENTS
- Preserve task id ({}) and name ("{}") from the overview
- Expand the context section based on the overview's description
- Include the dependencies section from the overview
- All sections must be complete and valid YAML
- Output ONLY the final YAML, no markdown code blocks or commentary
- DO NOT create, write, or modify any files - this is a planning phase only
- Your job is to OUTPUT the specification, not to implement it"#,
        task_id, task_name, task_id, task_overview_yaml, task_template, task_id, task_name
    );

    let query_prompt = format!(
        r#"Expand Task {} ("{}") by coordinating with your specialized agents.

IMPORTANT: Run all agents in parallel for maximum efficiency:
- Invoke @files, @functions, @formal, and @tests agents simultaneously
- Wait for all agents to complete
- Then combine their outputs into the complete task specification in YAML format."#,
        task_id, task_name
    );

    let options = claude_agent_sdk::ClaudeAgentOptions {
        allowed_tools: vec!["Read".to_string().into(), "Grep".to_string().into(), "Glob".to_string().into()],
        system_prompt: Some(claude_agent_sdk::SystemPrompt::String(system_prompt)),
        agents: Some(agents),
        permission_mode: Some(claude_agent_sdk::PermissionMode::BypassPermissions),
        ..Default::default()
    };

    // Track which agents have been invoked
    let mut agents_invoked = std::collections::HashSet::new();

    let agent_name = "suborchestrator";
    let task_agent_id = format!("task_{}_suborchestrator", task_id);
    log_agent_start!(&task_agent_id, agent_name, format!("Expanding task {} with sub-agents", task_id));

    let stream = claude_agent_sdk::query(&query_prompt, Some(options))
        .await
        .context("Failed to query Claude agent")?;

    let mut response_parts = Vec::new();
    let mut usage_stats = None;

    futures::pin_mut!(stream);

    while let Some(result) = stream.next().await {
        let message = result.context("Failed to receive message from stream")?;

        match message {
            claude_agent_sdk::Message::Assistant { message, .. } => {
                for block in message.content {
                    if let claude_agent_sdk::ContentBlock::Text { text } = block {
                        response_parts.push(text.clone());

                        // Print streaming text to console
                        print!("{}", text);
                        use std::io::Write;
                        let _ = std::io::stdout().flush();

                        // Emit streaming text for live TUI updates
                        log_agent_message!(&task_agent_id, agent_name, &text);

                        // Detect agent invocations for logging
                        for sub_agent in &["files", "functions", "formal", "tests"] {
                            if text.contains(&format!("@{}", sub_agent))
                                && !agents_invoked.contains(*sub_agent)
                            {
                                agents_invoked.insert(sub_agent.to_string());
                                // Sub-agent delegation is already visible in the streaming text
                            }
                        }

                        if debug && !text.is_empty() {
                            let preview_len = text.len().min(100);
                            println!("    Debug: Response: {}...", &text[..preview_len]);
                        }
                    }
                }
            }
            claude_agent_sdk::Message::Result {
                duration_ms,
                duration_api_ms,
                num_turns,
                total_cost_usd,
                usage,
                session_id,
                ..
            } => {
                // Parse usage from JSON value
                let token_usage = if let Some(usage_value) = usage {
                    let input_tokens = usage_value
                        .get("input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    let output_tokens = usage_value
                        .get("output_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;

                    crate::task_planner::types::TokenUsage {
                        input_tokens,
                        output_tokens,
                    }
                } else {
                    crate::task_planner::types::TokenUsage {
                        input_tokens: 0,
                        output_tokens: 0,
                    }
                };

                usage_stats = Some(UsageStats {
                    duration_ms,
                    duration_api_ms: Some(duration_api_ms),
                    num_turns,
                    total_cost_usd,
                    usage: token_usage,
                    session_id: Some(session_id.as_str().to_string()),
                });
            }
            _ => {
                // Ignore other message types
            }
        }
    }

    let combined_output = response_parts.join("\n");
    let cleaned = clean_yaml_response(&combined_output);
    let stats = usage_stats.ok_or_else(|| anyhow::anyhow!("No usage stats received"))?;

    log_agent_complete!(&task_agent_id, agent_name, format!("Task {} expanded", task_id));

    if debug {
        println!(
            "  Debug: Task {}: {:.2}s, {} turns, ${:.4}, {} in, {} out",
            task_id,
            stats.duration_ms as f64 / 1000.0,
            stats.num_turns,
            stats.total_cost_usd.unwrap_or(0.0),
            stats.usage.input_tokens,
            stats.usage.output_tokens
        );
    }

    Ok((cleaned, stats))
}

/// Expand all tasks in batches with parallel execution
///
/// This function orchestrates the expansion of all tasks from the overview,
/// using execution planning to determine batch groupings and parallelization.
pub async fn step2_expand_all_tasks(
    tasks_overview_yaml: &str,
    task_template: &str,
    project_root: &Path,
    stream_to_file: bool,
    debug: bool,
    use_ai_planning: bool,
    batch_size: usize,
) -> Result<String> {
    println!("\n{}", "=".repeat(80));
    println!("STEP 2: Suborchestrators");
    println!("{}", "=".repeat(80));
    println!("Expand Tasks\n");

    log_phase_start!(2, "Task Expansion", 3);

    // Parse tasks from overview
    let tasks = parse_tasks_overview(tasks_overview_yaml)
        .context("Failed to parse tasks_overview.yaml")?;

    println!("Found {} tasks", tasks.len());

    if tasks.is_empty() {
        anyhow::bail!("No valid tasks found in tasks_overview.yaml");
    }

    // Generate execution plan
    let execution_plan_yaml = if use_ai_planning {
        generate_execution_plan_ai(tasks_overview_yaml)
            .await
            .unwrap_or_else(|e| {
                println!("AI planning failed ({}), using simple batching", e);
                generate_execution_plan_simple(&tasks, batch_size)
            })
    } else {
        generate_execution_plan_simple(&tasks, batch_size)
    };

    if debug {
        println!("Execution Plan:\n{}", execution_plan_yaml);
    }

    // Parse execution plan into batches
    let batches = parse_execution_plan(&execution_plan_yaml, &tasks, debug).unwrap_or_else(|e| {
        println!("Plan parsing failed ({}), using fallback", e);
        build_execution_batches_fallback(&tasks)
    });

    println!("Execution plan: {} batch(es)", batches.len());

    // Execute batches
    let mut all_expanded = Vec::new();
    let mut all_usage_stats = Vec::new();
    let tasks_path = project_root.join("tasks.yaml");

    let mut file_handle = if stream_to_file {
        println!("Streaming to: {}", tasks_path.display());
        Some(std::fs::File::create(&tasks_path).context("Failed to create tasks.yaml")?)
    } else {
        None
    };

    for (batch_num, batch) in batches.iter().enumerate() {
        let batch_id = batch_num + 1;
        let batch_task_id = format!("batch_{}", batch_id);

        log_task_start!(
            2,
            &batch_task_id,
            format!("Batch {}/{} ({} tasks)", batch_id, batches.len(), batch.len())
        );

        println!("→ Executing Batch {}/{} ({} tasks)", batch_id, batches.len(), batch.len());

        // Execute tasks in parallel
        let tasks_futures: Vec<_> = batch
            .iter()
            .map(|task| suborchestrator_expand_task(task, task_template, debug))
            .collect();

        let expanded_batch = join_all(tasks_futures).await;

        // Handle results
        for result in expanded_batch {
            let (expanded, usage_stats) = result.context("Task expansion failed")?;

            if let Some(ref mut file) = file_handle {
                if !all_expanded.is_empty() {
                    file.write_all(b"\n---\n")
                        .context("Failed to write separator")?;
                }
                file.write_all(expanded.as_bytes())
                    .context("Failed to write task YAML")?;
                file.flush().context("Failed to flush file")?;
            } else {
                all_expanded.push(expanded);
            }

            all_usage_stats.push(usage_stats);
        }

        log_task_complete!(&batch_task_id, format!("Batch {} complete", batch_id));
    }

    // Aggregate stats
    let total_duration: u64 = all_usage_stats.iter().map(|s| s.duration_ms).sum();
    let total_turns: u32 = all_usage_stats.iter().map(|s| s.num_turns).sum();
    let total_cost: f64 = all_usage_stats
        .iter()
        .filter_map(|s| s.total_cost_usd)
        .sum();

    println!(
        "\nAggregate: {} tasks, {:.2}s, {} turns, ${:.4}",
        all_usage_stats.len(),
        total_duration as f64 / 1000.0,
        total_turns,
        total_cost
    );

    log_phase_complete!(2, "Task Expansion");

    if file_handle.is_some() {
        println!("✓ Streamed to: {}", tasks_path.display());
        log_state_file!(2, tasks_path.display().to_string(), "Expanded tasks");
        Ok(String::new())
    } else {
        let result = all_expanded.join("\n---\n");
        println!("✓ Saved: {}", tasks_path.display());
        log_state_file!(2, tasks_path.display().to_string(), "Expanded tasks");
        Ok(result)
    }
}
