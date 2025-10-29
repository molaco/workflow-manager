//! Phase 1: Expand tasks into detailed specifications
//!
//! This phase:
//! - Generates execution plan (AI dependency analysis or simple batching)
//! - Executes suborchestrators in parallel batches
//! - Each suborchestrator coordinates 4 specialized sub-agents:
//!   - @files: Identifies files to create/modify
//!   - @functions: Specifies code items (functions, structs, traits)
//!   - @formal: Determines formal verification needs
//!   - @tests: Designs test strategy and implementation
//! - Outputs detailed task specifications (tasks.yaml)

use crate::task_planner::utils::{
    build_execution_batches_fallback, generate_ai_execution_plan, generate_simple_execution_plan,
    get_task_id, get_task_name, parse_execution_plan,
};
use crate::workflow_utils::{execute_agent, execute_batch, execute_task, extract_yaml, parse_yaml_multi, AgentConfig};
use anyhow::{Context, Result};
use claude_agent_sdk::{AgentDefinition, ClaudeAgentOptions};
use serde_yaml::Value;

/// Execute a single suborchestrator to expand a task
async fn expand_single_task(
    task: &Value,
    task_template: &str,
) -> Result<String> {
    let task_id = get_task_id(task)
        .ok_or_else(|| anyhow::anyhow!("Task missing id field"))?;
    let task_name = get_task_name(task)
        .ok_or_else(|| anyhow::anyhow!("Task missing name field"))?;

    // Serialize task overview to YAML
    let task_overview_yaml = serde_yaml::to_string(task)?;

    // Define specialized sub-agents
    let files_agent = AgentDefinition {
        description: "Specialist that identifies all files to be created or modified".to_string(),
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

Output valid YAML only, no markdown."#.to_string(),
        tools: Some(vec![
            "Read".to_string(),
            "Grep".to_string(),
            "Glob".to_string(),
        ]),
        model: Some("sonnet".to_string()),
    };

    let functions_agent = AgentDefinition {
        description: "Specialist that specifies functions, structs, traits, and other code items".to_string(),
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

Output valid YAML only, no markdown."#.to_string(),
        tools: Some(vec![
            "Read".to_string(),
            "Grep".to_string(),
            "Glob".to_string(),
        ]),
        model: Some("sonnet".to_string()),
    };

    let formal_agent = AgentDefinition {
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

Output valid YAML only, no markdown."#.to_string(),
        tools: Some(vec!["Read".to_string()]),
        model: Some("sonnet".to_string()),
    };

    let tests_agent = AgentDefinition {
        description: "Specialist that designs test strategy and implements test code".to_string(),
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

Output valid YAML only, no markdown."#.to_string(),
        tools: Some(vec!["Read".to_string(), "Grep".to_string()]),
        model: Some("sonnet".to_string()),
    };

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

    let options = ClaudeAgentOptions::builder()
        .system_prompt(system_prompt)
        .allowed_tools(vec![
            "Read".to_string(),
            "Grep".to_string(),
            "Glob".to_string(),
            "Task".to_string(),  // Required for sub-agent delegation
        ])
        .add_agent("files", files_agent)
        .add_agent("functions", functions_agent)
        .add_agent("formal", formal_agent)
        .add_agent("tests", tests_agent)
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    let config = AgentConfig::new(
        format!("expand_{}", task_id),  // Match parent task ID for proper TUI nesting
        format!("Task {} Suborchestrator", task_id),
        format!("Expanding task {} with sub-agents", task_id),
        query_prompt,
        options,
    );

    let response = execute_agent(config).await?;

    // Extract YAML from response
    let yaml_content = extract_yaml(&response);
    Ok(yaml_content)
}

/// Phase 1: Expand all tasks using suborchestrators
pub async fn expand_tasks(
    tasks_overview_yaml: &str,
    task_template: &str,
    simple_batching: bool,
    batch_size: usize,
) -> Result<String> {
    println!("\n{}", "=".repeat(80));
    println!("PHASE 1: Suborchestrators - Expand Tasks");
    println!("{}", "=".repeat(80));

    // Parse tasks overview
    let tasks: Vec<Value> = parse_yaml_multi(tasks_overview_yaml)
        .context("Failed to parse tasks_overview.yaml")?;

    println!("Found {} tasks to expand\n", tasks.len());

    // Generate execution plan
    let batches = if simple_batching {
        generate_simple_execution_plan(&tasks, batch_size)?
    } else {
        // AI-based dependency analysis
        let execution_plan_yaml = generate_ai_execution_plan(tasks_overview_yaml).await?;

        // Try to parse execution plan, fallback to simple analysis if it fails
        parse_execution_plan(&execution_plan_yaml, &tasks)
            .unwrap_or_else(|e| {
                println!("Failed to parse execution plan: {}", e);
                build_execution_batches_fallback(&tasks)
            })
    };

    println!("Execution plan: {} batch(es)", batches.len());
    for (i, batch) in batches.iter().enumerate() {
        let task_ids: Vec<u32> = batch.iter().filter_map(get_task_id).collect();
        if batch.len() == 1 {
            println!("  Batch {}: Task {} (sequential)", i + 1, task_ids[0]);
        } else {
            println!("  Batch {}: Tasks {:?} (parallel)", i + 1, task_ids);
        }
    }
    println!();

    // Execute batches
    let mut all_expanded = Vec::new();

    for (batch_num, batch) in batches.iter().enumerate() {
        println!("\n→ Executing Batch {}/{}", batch_num + 1, batches.len());
        println!("  Running {} task(s)...\n", batch.len());

        // Execute batch in parallel using execute_batch
        let task_template_clone = task_template.to_string();
        let expanded_batch = execute_batch(
            1, // phase number
            batch.clone(),
            batch.len(), // concurrency = batch size (all parallel)
            move |task, ctx| {
                let task_template = task_template_clone.clone();
                async move {
                    let task_id = get_task_id(&task).unwrap_or(0);
                    let task_name = get_task_name(&task).unwrap_or("Unknown");

                    let yaml = execute_task(
                        format!("expand_{}", task_id),
                        format!("Expanding: {}", task_name),
                        ctx,
                        || async move {
                            let result = expand_single_task(&task, &task_template).await?;
                            Ok((result, format!("Expanded task {}", task_id)))
                        }
                    ).await?;

                    // Return tuple for execute_batch
                    Ok((yaml, format!("Task {} complete", task_id)))
                }
            },
        )
        .await?;

        all_expanded.extend(expanded_batch);
    }

    // Combine all expanded tasks (extract just the YAML, not the summary message)
    let yaml_parts: Vec<String> = all_expanded.into_iter().map(|(yaml, _)| yaml).collect();
    let combined_yaml = yaml_parts.join("\n---\n");

    println!("\n{}", "=".repeat(80));
    println!("✓ All tasks expanded successfully");
    println!("{}", "=".repeat(80));

    Ok(combined_yaml)
}
