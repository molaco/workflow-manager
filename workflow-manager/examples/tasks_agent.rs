/*
┌─────────────────────────────────────────────────────────────────────────────┐
│                         TASKS AGENT WORKFLOW                                 │
└─────────────────────────────────────────────────────────────────────────────┘

  Phase 0: GENERATE TASK OVERVIEW
    │
    ├─> Input: IMPL.md + tasks_overview_template.yaml
    ├─> Main orchestrator analyzes implementation requirements
    ├─> LLM generates high-level task breakdown
    └─> Output: tasks_overview_<timestamp>.yaml

         ↓

  Phase 1: EXPAND TASKS (concurrent batches)
    │
    ├─> Parse tasks_overview.yaml
    ├─> Generate execution plan (AI-based or simple batching)
    ├─> For each batch (parallel execution):
    │   ├─> Suborchestrator coordinates 4 sub-agents:
    │   │   ├─> @files - identify files to create/modify
    │   │   ├─> @functions - specify code items
    │   │   ├─> @formal - determine verification needs
    │   │   └─> @tests - design test strategy
    │   └─> Combine outputs into detailed task spec
    └─> Output: tasks_<timestamp>.yaml

         ↓

  Phase 2: REVIEW TASKS (concurrent batches)
    │
    ├─> Match overview tasks with detailed tasks
    ├─> For each batch (parallel execution):
    │   ├─> Suborchestrator coordinates @reviewer agents
    │   ├─> Validate completeness, consistency, correctness
    │   └─> Generate assessment report
    └─> Output: task_review_report.txt

┌─────────────────────────────────────────────────────────────────────────────┐
│ FEATURES:                                                                    │
│ • Resume from any phase (--overview-file, --tasks-file)                     │
│ • Concurrent execution (--batch-size N for parallel expansion/review)      │
│ • Phase selection (--phases 0,1,2)                                          │
│ • AI or simple batching (--simple-batching for fixed-size batches)         │
│ • Stream mode (--stream to write tasks incrementally)                      │
└─────────────────────────────────────────────────────────────────────────────┘

EXAMPLE COMMANDS:

  # Run all phases (full workflow)
  cargo run --example tasks_agent -- \
    --impl IMPL.md \
    --overview-template TEMPLATES/tasks_overview_template.yaml \
    --task-template TEMPLATES/task_template.yaml \
    --output tasks.yaml

  # Phase 0 only: Generate overview
  cargo run --example tasks_agent -- \
    --phases 0 \
    --impl IMPL.md \
    --overview-template TEMPLATES/tasks_overview_template.yaml

  # Phase 1 only: Expand tasks (AI batching, 3 concurrent)
  cargo run --example tasks_agent -- \
    --phases 1 \
    --overview-file tasks_overview_20250101_120000.yaml \
    --task-template TEMPLATES/task_template.yaml \
    --batch-size 3

  # Phase 1 only: Expand tasks (simple batching of 5)
  cargo run --example tasks_agent -- \
    --phases 1 \
    --overview-file tasks_overview_20250101_120000.yaml \
    --task-template TEMPLATES/task_template.yaml \
    --simple-batching \
    --batch-size 5

  # Phase 2 only: Review tasks
  cargo run --example tasks_agent -- \
    --phases 2 \
    --overview-file tasks_overview_20250101_120000.yaml \
    --tasks-file tasks_20250101_120000.yaml \
    --impl IMPL.md \
    --task-template TEMPLATES/task_template.yaml \
    --batch-size 3

  # Resume from Phase 1 onwards
  cargo run --example tasks_agent -- \
    --phases 1,2 \
    --overview-file tasks_overview_20250101_120000.yaml \
    --task-template TEMPLATES/task_template.yaml \
    --impl IMPL.md
*/

use clap::Parser;
use claude_agent_sdk::{query, AgentDefinition, ClaudeAgentOptions, ContentBlock, Message};
use futures::{stream::FuturesUnordered, StreamExt};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{
    fs,
    sync::{RwLock, Semaphore},
};

/// Shared state for live task display
#[derive(Clone)]
struct TaskLogger {
    task_id: u32,
    last_line: Arc<RwLock<String>>,
    sub_agents: Arc<RwLock<std::collections::HashMap<String, String>>>,
}

impl TaskLogger {
    fn new(task_id: u32) -> Self {
        Self {
            task_id,
            last_line: Arc::new(RwLock::new(String::from("Starting..."))),
            sub_agents: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    async fn update(&self, line: &str) {
        // Check if this line mentions a sub-agent
        let agent_names = ["@files", "@functions", "@formal", "@tests"];
        for agent_name in &agent_names {
            if line.contains(agent_name) {
                let mut agents = self.sub_agents.write().await;
                let clean_name = agent_name.trim_start_matches('@');
                agents.insert(clean_name.to_string(), line.to_string());
            }
        }

        let mut last = self.last_line.write().await;
        *last = line.to_string();
    }

    async fn update_sub_agent(&self, agent_name: &str, status: &str) {
        let mut agents = self.sub_agents.write().await;
        agents.insert(agent_name.to_string(), status.to_string());
    }

    async fn get_last_line(&self) -> String {
        self.last_line.read().await.clone()
    }

    async fn get_sub_agents(&self) -> std::collections::HashMap<String, String> {
        self.sub_agents.read().await.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskOverview {
    task: TaskInfo,
    dependencies: Option<Dependencies>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskInfo {
    id: u32,
    name: String,
    description: Option<String>,
    complexity: Option<String>,
    estimated_effort: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Dependencies {
    #[serde(default)]
    requires_completion_of: Vec<TaskDependency>,
    #[serde(default)]
    depends_on: Vec<TaskDependency>,
    #[serde(default)]
    depended_upon_by: Vec<TaskDependency>,
    #[serde(default)]
    external: Vec<serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum TaskIdValue {
    Number(u32),
    String(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskDependency {
    #[serde(default)]
    task_id: Option<TaskIdValue>,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskDetail {
    task: TaskInfo,
    context: Option<serde_yaml::Value>,
    files: Option<Vec<FileSpec>>,
    functions: Option<Vec<FunctionSpec>>,
    formal_verification: Option<serde_yaml::Value>,
    tests: Option<serde_yaml::Value>,
    dependencies: Option<Dependencies>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileSpec {
    path: String,
    description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FunctionSpec {
    file: String,
    items: Vec<serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReviewResult {
    task_id: u32,
    success: bool,
    issues: Vec<String>,
    summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BatchDefinition {
    batch_id: u32,
    description: String,
    tasks: Vec<BatchTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BatchTask {
    task_id: u32,
    task_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecutionPlan {
    total_tasks: u32,
    total_batches: u32,
    batches: Vec<BatchDefinition>,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Tasks Agent - Multi-agent task planning orchestrator", long_about = None)]
struct Args {
    /// Path to IMPL.md file(s) (can specify multiple)
    #[arg(long, num_args = 1..)]
    impl_files: Vec<String>,

    /// Path to tasks_overview_template.yaml
    #[arg(long)]
    overview_template: Option<String>,

    /// Path to task_template.yaml
    #[arg(long)]
    task_template: Option<String>,

    /// Output file path for tasks.yaml
    #[arg(short, long)]
    output: Option<String>,

    /// Number of tasks to process in parallel (if specified, enables simple batching)
    #[arg(long)]
    batch_size: Option<usize>,

    /// Comma-separated phases to execute (0=overview, 1=expand, 2=review)
    #[arg(long, default_value = "0,1,2")]
    phases: String,

    /// Path to saved tasks_overview.yaml (for resuming from Phase 1)
    #[arg(long)]
    overview_file: Option<String>,

    /// Path to tasks.yaml (required for Phase 2 if Phase 1 didn't run)
    #[arg(long)]
    tasks_file: Option<String>,

    /// Use simple fixed-size batching instead of AI dependency analysis
    #[arg(long)]
    simple_batching: bool,

    /// Stream tasks to file immediately (reduces memory usage)
    #[arg(long)]
    stream: bool,

    /// Enable debug output
    #[arg(long)]
    debug: bool,
}

/// Phase 0: Generate tasks_overview.yaml from IMPL.md
async fn generate_overview(impl_md: &str, overview_template: &str) -> anyhow::Result<String> {
    println!("{}", "=".repeat(80));
    println!("PHASE 0: Main Orchestrator - Generate Task Overview");
    println!("{}", "=".repeat(80));

    let system_prompt = r#"You are a task planning specialist focused on generating high-level task overviews.

Your goal is to analyze the implementation document and generate a tasks_overview.yaml file that breaks down the implementation into logical tasks.

Key instructions:
- Generate YAML that follows the tasks_overview_template.yaml structure exactly
- Create one task block per logical implementation objective
- Keep descriptions strategic and high-level (WHAT and WHY, not HOW)
- Assign sequential task IDs starting from 1
- Identify dependencies between tasks accurately
- Focus on business/architectural value and outcomes
- Estimate complexity and effort realistically

Output only valid YAML, no markdown code blocks or extra commentary."#;

    let prompt = format!(
        r#"Using the implementation document below, generate tasks_overview.yaml following the template structure.

# Implementation Document:
```
{}
```

# Template Structure (tasks_overview_template.yaml):
```yaml
{}
```

Generate a complete tasks_overview.yaml with all tasks identified from the implementation document. Use YAML multi-document format (separate tasks with ---) if there are multiple tasks.

Make sure to just give your response. You must not create or write any files just output the yaml and only that."#,
        impl_md, overview_template
    );

    let options = ClaudeAgentOptions::builder()
        .system_prompt(system_prompt.to_string())
        .allowed_tools(vec![
            "Read".to_string(),
            "Grep".to_string(),
            "Glob".to_string(),
        ])
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    let stream = query(&prompt, Some(options)).await?;
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
    Ok(clean_yaml(&response_text))
}

/// Phase 1: Expand a single task using suborchestrator with sub-agents
async fn expand_task(
    task_overview: &TaskOverview,
    task_template: &str,
    logger: Option<TaskLogger>,
    _debug: bool,
) -> anyhow::Result<String> {
    let task_id = task_overview.task.id;
    let task_name = &task_overview.task.name;

    if let Some(ref log) = logger {
        log.update(&format!("Expanding: {}", task_name)).await;
    }

    // Serialize task overview
    let task_overview_yaml = serde_yaml::to_string(task_overview)?;

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

Output valid YAML only, no markdown."#
            .to_string(),
        tools: Some(vec![
            "Read".to_string(),
            "Grep".to_string(),
            "Glob".to_string(),
        ]),
        model: Some("sonnet".to_string()),
    };

    let functions_agent = AgentDefinition {
        description: "Specialist that specifies functions, structs, traits, and other code items"
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

Output valid YAML only, no markdown."#
            .to_string(),
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

Output valid YAML only, no markdown."#
            .to_string(),
        tools: Some(vec!["Read".to_string(), "Grep".to_string()]),
        model: Some("sonnet".to_string()),
    };

    // Build agents map
    let mut agents_map = std::collections::HashMap::new();
    agents_map.insert("files".to_string(), files_agent);
    agents_map.insert("functions".to_string(), functions_agent);
    agents_map.insert("formal".to_string(), formal_agent);
    agents_map.insert("tests".to_string(), tests_agent);

    // Serialize agents to JSON and set other options via extra_args
    let agents_json = serde_json::to_string(&agents_map)?;
    let mut extra_args = std::collections::HashMap::new();
    extra_args.insert("agents".to_string(), Some(agents_json));
    extra_args.insert(
        "include-partial-messages".to_string(),
        Some("true".to_string()),
    );

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

    let mut options = ClaudeAgentOptions::builder()
        .system_prompt(system_prompt)
        .allowed_tools(vec![
            "Read".to_string(),
            "Grep".to_string(),
            "Glob".to_string(),
        ])
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    options.extra_args = extra_args;
    options.include_partial_messages = true;

    let stream = query(&query_prompt, Some(options)).await?;
    let mut stream = Box::pin(stream);

    let mut response_text = String::new();

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant { message, .. } => {
                for block in &message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            response_text.push_str(text);
                            // Update logger with last non-empty line and detect sub-agent mentions
                            if let Some(ref log) = logger {
                                let text_lower = text.to_lowercase();

                                // Parse each line to capture agent-specific messages
                                for line in text.lines() {
                                    let line_lower = line.to_lowercase();

                                    // Match agent-specific output patterns
                                    // Looking for lines mentioning agent names or delegation
                                    if line_lower.contains("files")
                                        && (line_lower.contains("agent")
                                            || line_lower.contains("specialist")
                                            || line_lower.contains("@files"))
                                    {
                                        let msg = if line.len() > 60 {
                                            format!("{}...", &line[..57])
                                        } else {
                                            line.to_string()
                                        };
                                        log.update_sub_agent("files", &msg).await;
                                    }
                                    if line_lower.contains("functions")
                                        && (line_lower.contains("agent")
                                            || line_lower.contains("specialist")
                                            || line_lower.contains("@functions"))
                                    {
                                        let msg = if line.len() > 60 {
                                            format!("{}...", &line[..57])
                                        } else {
                                            line.to_string()
                                        };
                                        log.update_sub_agent("functions", &msg).await;
                                    }
                                    if line_lower.contains("formal")
                                        && (line_lower.contains("verification")
                                            || line_lower.contains("agent")
                                            || line_lower.contains("@formal"))
                                    {
                                        let msg = if line.len() > 60 {
                                            format!("{}...", &line[..57])
                                        } else {
                                            line.to_string()
                                        };
                                        log.update_sub_agent("formal", &msg).await;
                                    }
                                    if line_lower.contains("test")
                                        && (line_lower.contains("agent")
                                            || line_lower.contains("specialist")
                                            || line_lower.contains("@tests"))
                                    {
                                        let msg = if line.len() > 60 {
                                            format!("{}...", &line[..57])
                                        } else {
                                            line.to_string()
                                        };
                                        log.update_sub_agent("tests", &msg).await;
                                    }
                                }

                                // Update main status with last line
                                if let Some(last_line) =
                                    text.lines().filter(|l| !l.trim().is_empty()).last()
                                {
                                    let truncated = if last_line.len() > 80 {
                                        format!("{}...", &last_line[..77])
                                    } else {
                                        last_line.to_string()
                                    };
                                    log.update(&truncated).await;
                                }
                            }
                        }
                        ContentBlock::ToolUse { name, input, .. } => {
                            // Detect Task tool calls to sub-agents
                            if name == "Task" {
                                if let Some(ref log) = logger {
                                    // Try to extract which sub-agent is being invoked
                                    if let Some(prompt_val) = input.get("prompt") {
                                        let prompt_str = prompt_val.as_str().unwrap_or("");

                                        // Detect which sub-agent based on prompt content
                                        let agent_name = if prompt_str
                                            .contains("files identification")
                                            || prompt_str.contains("@files")
                                        {
                                            Some("files")
                                        } else if prompt_str.contains("functions specification")
                                            || prompt_str.contains("@functions")
                                        {
                                            Some("functions")
                                        } else if prompt_str.contains("formal verification")
                                            || prompt_str.contains("@formal")
                                        {
                                            Some("formal")
                                        } else if prompt_str.contains("test")
                                            && (prompt_str.contains("specialist")
                                                || prompt_str.contains("@tests"))
                                        {
                                            Some("tests")
                                        } else {
                                            None
                                        };

                                        if let Some(agent) = agent_name {
                                            let desc = input
                                                .get("description")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("Working...");
                                            log.update_sub_agent(agent, &format!("⚙️ {}", desc))
                                                .await;
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Message::User { message, .. } => {
                // Tool results - extract which agent completed
                if let Some(ref log) = logger {
                    // UserContent is Option<UserContent>, which can be String or Blocks
                    if let Some(claude_agent_sdk::types::UserContent::Blocks(blocks)) =
                        &message.content
                    {
                        for block in blocks {
                            if let ContentBlock::ToolResult {
                                tool_use_id: _,
                                content,
                                ..
                            } = block
                            {
                                // Try to detect which agent from the result content
                                let content_str = format!("{:?}", content);
                                if content_str.contains("files:") && content_str.len() > 50 {
                                    log.update_sub_agent("files", "✓ Complete").await;
                                } else if content_str.contains("functions:")
                                    && content_str.len() > 50
                                {
                                    log.update_sub_agent("functions", "✓ Complete").await;
                                } else if content_str.contains("formal_verification:") {
                                    log.update_sub_agent("formal", "✓ Complete").await;
                                } else if content_str.contains("tests:") && content_str.len() > 50 {
                                    log.update_sub_agent("tests", "✓ Complete").await;
                                }
                            }
                        }
                    }
                }
            }
            Message::Result { .. } => break,
            _ => {}
        }
    }

    if let Some(ref log) = logger {
        log.update("✓ Complete").await;
    }
    Ok(clean_yaml(&response_text))
}

/// Display live task status (updates in place)
async fn display_live_status(loggers: &[TaskLogger]) {
    // Calculate total lines needed (1 main + 4 sub-agents per task)
    let lines_per_task = 5;
    let total_lines = loggers.len() * lines_per_task;

    // Move cursor up to overwrite previous output
    if total_lines > 0 {
        print!("\x1B[{}A", total_lines);
    }

    for logger in loggers {
        let last_line = logger.get_last_line().await;
        let sub_agents = logger.get_sub_agents().await;

        // Main task line
        print!("\r\x1B[K");
        println!("  [Task {}]: {}", logger.task_id, last_line);

        // Sub-agent lines
        for agent_name in ["files", "functions", "formal", "tests"] {
            print!("\r\x1B[K");
            if let Some(status) = sub_agents.get(agent_name) {
                let truncated = if status.len() > 70 {
                    format!("{}...", &status[..67])
                } else {
                    status.clone()
                };
                println!("    @{}: {}", agent_name, truncated);
            } else {
                println!("    @{}: -", agent_name);
            }
        }
    }
    io::stdout().flush().unwrap();
}

/// Generate simple execution plan (fixed-size batches)
fn generate_simple_batches(tasks: &[TaskOverview], batch_size: usize) -> Vec<Vec<TaskOverview>> {
    println!("\n{}", "=".repeat(80));
    println!("Batch Planning: Simple batching with size={}", batch_size);
    println!("{}", "=".repeat(80));

    let mut batches = Vec::new();
    for chunk in tasks.chunks(batch_size) {
        batches.push(chunk.to_vec());
    }

    println!("Created {} batch(es)", batches.len());
    batches
}

/// Generate AI-based execution plan (dependency analysis)
async fn generate_execution_plan(tasks_overview_yaml: &str) -> anyhow::Result<ExecutionPlan> {
    println!("\n{}", "=".repeat(80));
    println!("Batch Planning: Analyzing dependencies with AI agent");
    println!("{}", "=".repeat(80));

    let system_prompt = r#"You are an execution planning specialist focused on dependency analysis and batch optimization.

Your goal is to analyze tasks_overview.yaml and generate an optimal execution plan that maximizes parallelization while respecting dependencies.

Key instructions:
- Analyze requires_completion_of for each task
- Group tasks into batches where all tasks in a batch can run in parallel
- Tasks can only be in a batch if ALL their dependencies are in previous batches
- Maximize tasks per batch (more parallelization = faster execution)
- Batches execute sequentially, tasks within batch execute in parallel
- Identify the critical path (longest dependency chain)
- Detect any circular dependencies and warn about them

Output only valid YAML following the template structure, no markdown code blocks or extra commentary."#;

    let prompt = format!(
        r#"Analyze the tasks and their dependencies, then generate an execution plan.

# Tasks Overview:
```yaml
{}
```

Generate a complete execution_plan.yaml that:
1. Groups tasks into optimal batches for parallel execution
2. Respects all dependencies (requires_completion_of)
3. Maximizes parallelization potential
4. Includes rationale for each batch
5. Identifies critical path and parallelization potential

Output only the YAML, no markdown formatting."#,
        tasks_overview_yaml
    );

    let options = ClaudeAgentOptions::builder()
        .system_prompt(system_prompt.to_string())
        .allowed_tools(vec!["Read".to_string()])
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    let stream = query(&prompt, Some(options)).await?;
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

    let yaml_content = clean_yaml(&response_text);
    let plan_wrapper: serde_yaml::Value = serde_yaml::from_str(&yaml_content)?;
    let plan: ExecutionPlan = serde_yaml::from_value(
        plan_wrapper
            .get("execution_plan")
            .ok_or_else(|| anyhow::anyhow!("Missing execution_plan key"))?
            .clone(),
    )?;

    Ok(plan)
}

/// Parse execution plan and group tasks into batches
fn parse_execution_plan(plan: &ExecutionPlan, tasks: &[TaskOverview]) -> Vec<Vec<TaskOverview>> {
    let mut task_map: std::collections::HashMap<u32, TaskOverview> =
        tasks.iter().map(|t| (t.task.id, t.clone())).collect();

    let mut batches = Vec::new();
    for batch_def in &plan.batches {
        let mut batch = Vec::new();
        for task_ref in &batch_def.tasks {
            if let Some(task) = task_map.remove(&task_ref.task_id) {
                batch.push(task);
            }
        }
        if !batch.is_empty() {
            batches.push(batch);
        }
    }

    batches
}

/// Phase 2: Review tasks
async fn review_tasks(
    tasks_overview: &[TaskOverview],
    tasks_details: &[TaskDetail],
    impl_md: &str,
    task_template: &str,
    batch_size: usize,
) -> anyhow::Result<Vec<ReviewResult>> {
    println!("\n{}", "=".repeat(80));
    println!("PHASE 2: Batched Review - Validate Tasks");
    println!("{}", "=".repeat(80));

    // Match overview with details
    let mut task_map: std::collections::HashMap<u32, TaskDetail> = tasks_details
        .iter()
        .map(|t| (t.task.id, t.clone()))
        .collect();

    let mut task_pairs = Vec::new();
    for overview in tasks_overview {
        if let Some(detail) = task_map.remove(&overview.task.id) {
            task_pairs.push((overview.clone(), detail));
        }
    }

    // Create batches
    let batches: Vec<_> = task_pairs.chunks(batch_size).collect();
    println!(
        "Created {} batch(es) with batch_size={}\n",
        batches.len(),
        batch_size
    );

    let mut all_results = Vec::new();

    for (batch_num, batch) in batches.iter().enumerate() {
        println!(
            "\n→ Processing Review Batch {}/{}",
            batch_num + 1,
            batches.len()
        );

        let result = review_batch(batch, impl_md, task_template, batch_num + 1).await?;

        all_results.extend(result);
    }

    Ok(all_results)
}

/// Review a single batch of tasks
async fn review_batch(
    batch: &[(TaskOverview, TaskDetail)],
    impl_md: &str,
    task_template: &str,
    batch_num: usize,
) -> anyhow::Result<Vec<ReviewResult>> {
    println!("  Reviewing {} tasks...", batch.len());

    let reviewer_agent = AgentDefinition {
        description: "Specialist that validates individual task specifications against requirements".to_string(),
        prompt: r#"You are an implementation plan reviewer.

Your job is to validate that a detailed task specification (from tasks.yaml) matches its overview (from tasks_overview.yaml) and aligns with the IMPL.md requirements.

You will receive:
1. Implementation requirements (IMPL.md)
2. Task overview YAML (high-level strategic description)
3. Detailed task specification YAML (complete implementation spec)

Check for:
1. Completeness: All key components from overview are specified in detail
2. Consistency: Detailed spec aligns with overview purpose and scope
3. Correctness: Implementation approach makes sense for the requirements
4. Testability: Tests adequately cover the functionality
5. Dependencies: External dependencies are properly identified
6. Template adherence: Detailed spec follows the task_template structure

Report any issues found. If everything looks good, confirm that.

Format your response as:
ASSESSMENT: [APPROVED|NEEDS_REVISION]
ISSUES: [List any issues, or "None"]
SUMMARY: [Brief summary]"#.to_string(),
        tools: Some(vec!["Read".to_string()]),
        model: Some("sonnet".to_string()),
    };

    // Build agents map and serialize to JSON
    let mut agents_map = std::collections::HashMap::new();
    agents_map.insert("reviewer".to_string(), reviewer_agent);
    let agents_json = serde_json::to_string(&agents_map)?;
    let mut extra_args = std::collections::HashMap::new();
    extra_args.insert("agents".to_string(), Some(agents_json));
    extra_args.insert(
        "include-partial-messages".to_string(),
        Some("true".to_string()),
    );

    let task_list: Vec<_> = batch
        .iter()
        .map(|(overview, _)| format!("  - Task {}: {}", overview.task.id, overview.task.name))
        .collect();

    let system_prompt = format!(
        r#"You are a review suborchestrator coordinating Step 2: Review & Validation.

## YOUR ROLE
Coordinate the @reviewer agent to validate all {} tasks in your batch.

## AVAILABLE CONTEXT
- Implementation requirements (IMPL.md)
- Task overview structure (tasks_overview.yaml)
- Task template structure (task_template.yaml)
- Individual task details (provided when you invoke @reviewer)

## YOUR AGENT
**@reviewer** - Validates individual task specifications
- Input: Task overview + detailed spec + IMPL.md context
- Output: ASSESSMENT, ISSUES, SUMMARY

## WORKFLOW
1. For each task in your batch, invoke @reviewer agent with the task's overview and detailed spec
2. Run ALL @reviewer invocations in parallel for efficiency
3. Parse each reviewer's response to extract ASSESSMENT, ISSUES, and SUMMARY
4. Combine all results into a JSON array

## OUTPUT FORMAT
Output ONLY a valid JSON array with this exact structure:
[
  {{
    "task_id": <task_id_number>,
    "success": <true|false>,
    "issues": [<list of issue strings, or empty array>],
    "summary": "<brief summary string>"
  }},
  ...
]

IMPORTANT:
- Convert ASSESSMENT to success boolean (APPROVED=true, NEEDS_REVISION=false)
- Output ONLY the JSON array, no markdown code blocks, no extra commentary"#,
        batch.len()
    );

    let query_prompt = format!(
        r#"Coordinate review of all {} tasks in your batch.

## CONTEXT

### Implementation Requirements (IMPL.md):
```
{}
```

### Expected Task Template Structure (task_template.yaml):
```yaml
{}
```

## YOUR BATCH
Review these tasks:
{}

## INSTRUCTIONS
For EACH task above:
1. Extract the task's overview and detailed spec
2. Invoke @reviewer with both
3. Parse the reviewer's response

Run ALL @reviewer agents in PARALLEL, then combine results into JSON array."#,
        batch.len(),
        impl_md,
        task_template,
        task_list.join("\n")
    );

    let mut options = ClaudeAgentOptions::builder()
        .system_prompt(system_prompt)
        .allowed_tools(vec!["Read".to_string()])
        .permission_mode(claude_agent_sdk::PermissionMode::BypassPermissions)
        .build();

    options.extra_args = extra_args;
    options.include_partial_messages = true;

    let stream = query(&query_prompt, Some(options)).await?;
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

    // Parse JSON response
    let json_content = extract_json(&response_text);
    let results: Vec<ReviewResult> = serde_json::from_str(&json_content)
        .map_err(|e| anyhow::anyhow!("Failed to parse review results: {}", e))?;

    println!("  ✓ Batch {} review complete", batch_num);
    Ok(results)
}

/// Generate final review report
async fn generate_review_report(
    results: &[ReviewResult],
    output_path: &Path,
) -> anyhow::Result<()> {
    println!("\n{}", "=".repeat(80));
    println!("FINAL REPORT: Main Orchestrator Summary");
    println!("{}", "=".repeat(80));

    let approved = results.iter().filter(|r| r.success).count();
    let needs_revision = results.len() - approved;

    println!("Total tasks reviewed: {}", results.len());
    println!("✓ Approved: {}", approved);
    println!("✗ Needs revision: {}\n", needs_revision);

    if needs_revision > 0 {
        println!("Tasks requiring revision:\n");
        for result in results {
            if !result.success {
                println!("  Task {}:", result.task_id);
                for issue in &result.issues {
                    println!("    - {}", issue);
                }
                println!("    Summary: {}\n", result.summary);
            }
        }
    } else {
        println!("✓ All tasks approved! Ready for implementation.\n");
    }

    // Save report
    let mut report = String::new();
    report.push_str(&"=".repeat(80));
    report.push_str("\nTASK REVIEW REPORT\n");
    report.push_str(&"=".repeat(80));
    report.push_str(&format!("\n\nTotal tasks: {}\n", results.len()));
    report.push_str(&format!("Approved: {}\n", approved));
    report.push_str(&format!("Needs revision: {}\n\n", needs_revision));

    for result in results {
        report.push_str(&format!(
            "\nTask {}: {}\n",
            result.task_id,
            if result.success {
                "APPROVED"
            } else {
                "NEEDS REVISION"
            }
        ));
        report.push_str(&format!("Summary: {}\n", result.summary));
        if !result.issues.is_empty() {
            report.push_str("Issues:\n");
            for issue in &result.issues {
                report.push_str(&format!("  - {}\n", issue));
            }
        }
        report.push_str("\n");
    }

    fs::write(output_path, report).await?;
    println!("✓ Full report saved to: {}", output_path.display());

    Ok(())
}

/// Clean YAML response by removing markdown code blocks
fn clean_yaml(text: &str) -> String {
    if text.contains("```yaml") {
        let start = text.find("```yaml").unwrap() + 7;
        let end = text[start..].find("```").unwrap() + start;
        text[start..end].trim().to_string()
    } else if text.contains("```") {
        let start = text.find("```").unwrap() + 3;
        let end = text[start..].find("```").unwrap() + start;
        text[start..end].trim().to_string()
    } else {
        text.trim().to_string()
    }
}

/// Extract JSON from markdown code blocks
fn extract_json(text: &str) -> String {
    if text.contains("```json") {
        let start = text.find("```json").unwrap() + 7;
        let end = text[start..].find("```").unwrap() + start;
        text[start..end].trim().to_string()
    } else if text.contains("```") {
        let start = text.find("```").unwrap() + 3;
        let end = text[start..].find("```").unwrap() + start;
        text[start..end].trim().to_string()
    } else {
        text.trim().to_string()
    }
}

/// Parse multi-document YAML
fn parse_multi_doc_yaml<T: for<'de> Deserialize<'de>>(yaml: &str) -> anyhow::Result<Vec<T>> {
    let mut results = Vec::new();
    for doc in serde_yaml::Deserializer::from_str(yaml) {
        results.push(T::deserialize(doc)?);
    }
    Ok(results)
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

    let mut tasks_overview_yaml = String::new();
    let mut tasks_overview: Vec<TaskOverview> = Vec::new();
    let mut tasks_details: Vec<TaskDetail> = Vec::new();

    // Phase 0: Generate task overview
    if phases_to_run.contains(&0) {
        if args.impl_files.is_empty() {
            anyhow::bail!("--impl-files is required when running phase 0");
        }
        if args.overview_template.is_none() {
            anyhow::bail!("--overview-template is required when running phase 0");
        }

        // Load IMPL.md file(s)
        let mut impl_parts = Vec::new();
        for impl_file in &args.impl_files {
            let content = fs::read_to_string(impl_file).await?;
            if args.impl_files.len() > 1 {
                impl_parts.push(format!("# Source: {}\n\n{}", impl_file, content));
            } else {
                impl_parts.push(content);
            }
        }
        let impl_md = impl_parts.join("\n\n---\n\n");

        // Load overview template
        let overview_template = fs::read_to_string(args.overview_template.unwrap()).await?;

        // Generate overview
        tasks_overview_yaml = generate_overview(&impl_md, &overview_template).await?;

        // Save to file
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let overview_path = PathBuf::from(format!("tasks_overview_{}.yaml", timestamp));
        fs::write(&overview_path, &tasks_overview_yaml).await?;
        println!("[Phase 0] Saved: {}", overview_path.display());

        tasks_overview = parse_multi_doc_yaml(&tasks_overview_yaml)?;
    } else if let Some(overview_file) = &args.overview_file {
        tasks_overview_yaml = fs::read_to_string(overview_file).await?;
        tasks_overview = parse_multi_doc_yaml(&tasks_overview_yaml)?;
        println!("[Phase 0] Loaded overview from: {}", overview_file);
    }

    // Phase 1: Expand tasks
    if phases_to_run.contains(&1) {
        if tasks_overview.is_empty() {
            anyhow::bail!("Phase 0 must run before Phase 1, or provide --overview-file");
        }
        if args.task_template.is_none() {
            anyhow::bail!("--task-template is required when running phase 1");
        }

        let task_template = fs::read_to_string(args.task_template.as_ref().unwrap()).await?;

        println!("\n{}", "=".repeat(80));
        println!("PHASE 1: Suborchestrators - Expand Tasks");
        println!("{}", "=".repeat(80));
        println!("Found {} tasks to expand\n", tasks_overview.len());

        // Generate execution plan
        // If batch_size is specified, use simple batching (like Python version)
        let (batches, concurrency) = if let Some(size) = args.batch_size {
            (generate_simple_batches(&tasks_overview, size), size)
        } else if args.simple_batching {
            // Explicit simple batching without size defaults to 5
            (generate_simple_batches(&tasks_overview, 5), 5)
        } else {
            // AI-based dependency analysis
            let plan = generate_execution_plan(&tasks_overview_yaml).await?;
            (parse_execution_plan(&plan, &tasks_overview), 5)
        };

        println!("Execution plan: {} batch(es)", batches.len());
        for (i, batch) in batches.iter().enumerate() {
            let task_ids: Vec<_> = batch.iter().map(|t| t.task.id).collect();
            if batch.len() == 1 {
                println!("  Batch {}: Task {} (sequential)", i + 1, task_ids[0]);
            } else {
                println!("  Batch {}: Tasks {:?} (parallel)", i + 1, task_ids);
            }
        }
        println!();

        // Execute batches
        let sem = Arc::new(Semaphore::new(concurrency));
        let mut all_expanded = Vec::new();

        for (batch_num, batch) in batches.iter().enumerate() {
            println!("\n→ Executing Batch {}/{}", batch_num + 1, batches.len());
            println!("  Running {} task(s)...\n", batch.len());

            // Create loggers for each task
            let loggers: Vec<TaskLogger> =
                batch.iter().map(|t| TaskLogger::new(t.task.id)).collect();

            // Print initial status lines (placeholders) - 1 main + 4 sub-agents
            for logger in &loggers {
                println!("  [Task {}]: {}", logger.task_id, "Starting...");
                println!("    @files: -");
                println!("    @functions: -");
                println!("    @formal: -");
                println!("    @tests: -");
            }

            let mut tasks = FuturesUnordered::new();

            for (i, task) in batch.iter().enumerate() {
                let task = task.clone();
                let task_template = task_template.clone();
                let sem = sem.clone();
                let logger = loggers[i].clone();

                tasks.push(async move {
                    let _permit = sem
                        .acquire()
                        .await
                        .map_err(|_| anyhow::anyhow!("Semaphore closed"))?;
                    expand_task(&task, &task_template, Some(logger), false).await
                });
            }

            // Spawn display updater
            let display_loggers = loggers.clone();
            let display_task = tokio::spawn(async move {
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    display_live_status(&display_loggers).await;
                }
            });

            while let Some(result) = tasks.next().await {
                all_expanded.push(result?);
            }

            // Cancel display task and show final status
            display_task.abort();
            display_live_status(&loggers).await;
            println!(); // Add newline after final status
        }

        // Save tasks
        let tasks_yaml = all_expanded.join("\n---\n");
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let tasks_path = args
            .output
            .clone()
            .unwrap_or_else(|| format!("tasks_{}.yaml", timestamp));
        fs::write(&tasks_path, &tasks_yaml).await?;
        println!("\n[Phase 1] Saved: {}", tasks_path);

        tasks_details = parse_multi_doc_yaml(&tasks_yaml)?;
    }

    // Phase 2: Review tasks
    if phases_to_run.contains(&2) {
        if tasks_overview.is_empty() {
            anyhow::bail!("Phase 0 must run before Phase 2, or provide --overview-file");
        }

        // Load tasks if not already loaded from Phase 1
        if tasks_details.is_empty() {
            if let Some(tasks_file) = &args.tasks_file {
                let tasks_yaml = fs::read_to_string(tasks_file).await?;
                tasks_details = parse_multi_doc_yaml(&tasks_yaml)?;
                println!("[Phase 2] Loaded tasks from: {}", tasks_file);
            } else {
                anyhow::bail!("Phase 1 must run before Phase 2, or provide --tasks-file");
            }
        }

        if args.impl_files.is_empty() {
            anyhow::bail!("--impl-files is required when running phase 2");
        }
        if args.task_template.is_none() {
            anyhow::bail!("--task-template is required when running phase 2");
        }

        // Load IMPL.md
        let mut impl_parts = Vec::new();
        for impl_file in &args.impl_files {
            let content = fs::read_to_string(impl_file).await?;
            if args.impl_files.len() > 1 {
                impl_parts.push(format!("# Source: {}\n\n{}", impl_file, content));
            } else {
                impl_parts.push(content);
            }
        }
        let impl_md = impl_parts.join("\n\n---\n\n");

        let task_template = fs::read_to_string(args.task_template.as_ref().unwrap()).await?;

        let review_batch_size = args.batch_size.unwrap_or(5);
        let results = review_tasks(
            &tasks_overview,
            &tasks_details,
            &impl_md,
            &task_template,
            review_batch_size,
        )
        .await?;

        let report_path = PathBuf::from("task_review_report.txt");
        generate_review_report(&results, &report_path).await?;
    }

    println!("\n{}", "=".repeat(80));
    println!("Workflow complete!");
    println!("{}", "=".repeat(80));

    Ok(())
}
