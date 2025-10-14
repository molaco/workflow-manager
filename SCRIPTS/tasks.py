#!/usr/bin/env -S sh -c 'unset PYTHONPATH && uv run --script "$0" "$@"'
# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "claude_agent_sdk",
#     "python-dotenv",
#     "pyyaml",
# ]
# ///

"""
Multi-Agent Task Planning Orchestrator
======================================

WORKFLOW ARCHITECTURE:
┌─────────────────────────────────────────────────────────────────────────────┐
│                            STEP 1: MAIN ORCHESTRATOR                        │
│                         (Generate tasks_overview.yaml)                      │
│                                                                             │
│  Input: IMPL.md + tasks_overview_template.yaml                             │
│  Output: tasks_overview.yaml (high-level task breakdown)                   │
│  Description: Analyzes implementation doc and creates strategic task plan  │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                     STEP 2: BATCH PLANNING & EXECUTION                      │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────┐    │
│  │  Planning Agent (Optional)                                        │    │
│  │  - Analyzes task dependencies                                     │    │
│  │  - Creates optimal execution batches                              │    │
│  │  - Maximizes parallelization                                      │    │
│  │  Alternative: Simple fixed-size batching (--batch-size N)         │    │
│  └───────────────────────────────────────────────────────────────────┘    │
│                                      │                                      │
│                                      ▼                                      │
│  ┌───────────────────────────────────────────────────────────────────┐    │
│  │                    SUBORCHESTRATORS                               │    │
│  │              (One per task, runs in batches)                      │    │
│  │                                                                   │    │
│  │  For each task, spawns 4 specialized agents in parallel:         │    │
│  │                                                                   │    │
│  │    ┌──────────────┐  ┌──────────────┐  ┌──────────────┐         │    │
│  │    │ Files Agent  │  │Formal Agent  │  │Functions Agt │         │    │
│  │    │              │  │              │  │              │         │    │
│  │    │ Identifies   │  │ Determines   │  │ Specifies    │         │    │
│  │    │ all files    │  │ if formal    │  │ functions,   │         │    │
│  │    │ to create/   │  │ verification │  │ structs,     │         │    │
│  │    │ modify       │  │ is needed    │  │ traits, etc  │         │    │
│  │    └──────────────┘  └──────────────┘  └──────────────┘         │    │
│  │                                                                   │    │
│  │    ┌──────────────┐                                              │    │
│  │    │ Tests Agent  │                                              │    │
│  │    │              │                                              │    │
│  │    │ Designs test │                                              │    │
│  │    │ strategy &   │                                              │    │
│  │    │ implements   │                                              │    │
│  │    └──────────────┘                                              │    │
│  │           │                                                       │    │
│  │           ▼                                                       │    │
│  │    ┌─────────────────────────────────────────────────┐          │    │
│  │    │     Integration Agent                           │          │    │
│  │    │     - Combines all agent outputs                │          │    │
│  │    │     - Produces complete task specification      │          │    │
│  │    └─────────────────────────────────────────────────┘          │    │
│  └───────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  Output: tasks.yaml (detailed specifications for all tasks)                │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                      STEP 3: REVIEW & VALIDATION                            │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────┐    │
│  │          COORDINATOR FUNCTION (step3_review_tasks)                │    │
│  │          - Parses tasks_overview.yaml and tasks.yaml              │    │
│  │          - Matches overview tasks with detailed tasks             │    │
│  │          - Creates batches (configurable size)                    │    │
│  │          - Spawns suborchestrators sequentially                   │    │
│  └───────────────────────────────────────────────────────────────────┘    │
│                                      │                                      │
│                                      ▼                                      │
│  ┌───────────────────────────────────────────────────────────────────┐    │
│  │            REVIEW SUBORCHESTRATORS (one per batch)                │    │
│  │            Each suborchestrator receives:                         │    │
│  │              - IMPL.md (implementation requirements)              │    │
│  │              - tasks_overview.yaml (full content)                 │    │
│  │              - task_template.yaml (template structure)            │    │
│  │              - Task ID/name list for its batch                    │    │
│  │                                                                   │    │
│  │            For each task in batch:                                │    │
│  │              1. Extracts task overview from tasks_overview.yaml   │    │
│  │              2. Invokes @reviewer agent with:                     │    │
│  │                 - Task overview YAML                              │    │
│  │                 - Instructions to read detailed spec from         │    │
│  │                   tasks.yaml (using Read tool)                    │    │
│  │                 - IMPL.md context                                 │    │
│  │              3. Coordinates ALL @reviewer agents IN PARALLEL      │    │
│  │                                                                   │    │
│  │            Each @reviewer validates:                              │    │
│  │              - Completeness                                       │    │
│  │              - Consistency with overview                          │    │
│  │              - Correctness of approach                            │    │
│  │              - Test coverage                                      │    │
│  │              - Template adherence                                 │    │
│  │                                                                   │    │
│  │            Returns JSON array of review results                   │    │
│  └───────────────────────────────────────────────────────────────────┘    │
│                                      │                                      │
│                                      ▼                                      │
│  ┌───────────────────────────────────────────────────────────────────┐    │
│  │        REPORT GENERATOR (step3_main_orchestrator_report)          │    │
│  │        - Collects all review results from batches                 │    │
│  │        - Generates approval/revision summary                      │    │
│  │        - Saves task_review_report.txt                             │    │
│  └───────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  Output: task_review_report.txt (validation results)                       │
└─────────────────────────────────────────────────────────────────────────────┘

PARALLELIZATION STRATEGY:
- Step 1: Single agent (sequential)
- Step 2: Batches run sequentially, tasks within batch run in parallel
  - Each task spawns a suborchestrator with 4 sub-agents (files, functions, formal, tests)
  - Sub-agents run in parallel, suborchestrator combines outputs
- Step 3: Batches run sequentially, reviewers within batch run in parallel
  - Coordinator function spawns suborchestrators sequentially (one per batch)
  - Each suborchestrator coordinates @reviewer agents in parallel for its batch
  - Report generator collects all results
  - Batch size configurable with --batch-size (default: 5)

AGENT DESCRIPTIONS:
┌──────────────────────────┬────────────────────────────────────────────────┐
│ Component                │ Description                                    │
├──────────────────────────┼────────────────────────────────────────────────┤
│ STEP 1                   │                                                │
│   Overview Agent         │ Top-level planner that breaks IMPL.md into    │
│                          │ strategic tasks (generates tasks_overview.yaml)│
├──────────────────────────┼────────────────────────────────────────────────┤
│ STEP 2                   │                                                │
│   Planning Agent         │ (Optional) Analyzes dependencies & creates    │
│   (optional)             │ execution batches based on task dependencies  │
│                          │                                                │
│   Task Suborchestrator   │ Coordinates 4 sub-agents to expand one task   │
│   (one per task)         │ from overview to detailed specification       │
│                          │                                                │
│   @files sub-agent       │ Identifies files to create/modify with paths  │
│   @functions sub-agent   │ Specifies functions, structs, traits to impl  │
│   @formal sub-agent      │ Determines if formal verification is needed   │
│   @tests sub-agent       │ Designs test strategy and implements tests    │
├──────────────────────────┼────────────────────────────────────────────────┤
│ STEP 3                   │                                                │
│   Coordinator Function   │ Parses files, creates batches, collects results│
│                          │                                                │
│   Review Suborchestrator │ Coordinates @reviewer agents for one batch    │
│   (one per batch)        │ Receives high-level context + task list       │
│                          │                                                │
│   @reviewer sub-agent    │ Validates one task's detailed spec against    │
│   (one per task)         │ its overview and IMPL.md requirements         │
│                          │                                                │
│   Report Generator       │ Collects all results and generates final report│
└──────────────────────────┴────────────────────────────────────────────────┘
"""

import asyncio
import argparse
import json
import yaml
from pathlib import Path
from typing import Any, Dict, List
from dotenv import load_dotenv
from claude_agent_sdk import (
    ClaudeSDKClient,
    ClaudeAgentOptions,
    AssistantMessage,
    TextBlock,
    ThinkingBlock,
    ResultMessage,
    AgentDefinition,
    query,
)

# Load environment variables
load_dotenv()


def load_template(template_path: Path) -> str:
    """Load a YAML template from the given path."""
    with open(template_path, "r") as f:
        return f.read()


def load_impl_md() -> str:
    """Load IMPL.md from project root or DOCS/"""
    possible_paths = [
        Path(__file__).parent.parent / "IMPL.md",
        Path(__file__).parent.parent / "DOCS" / "IMPL.md",
    ]

    for path in possible_paths:
        if path.exists():
            with open(path, "r") as f:
                return f.read()

    raise FileNotFoundError("IMPL.md not found in project root or DOCS/")


def save_yaml(data: str, output_path: Path):
    """Save YAML data to file."""
    with open(output_path, "w") as f:
        f.write(data)
    print(f"✓ Saved: {output_path}")


def clean_yaml_response(response: str) -> str:
    """Clean YAML response by removing markdown code blocks if present."""
    if "```yaml" in response:
        return response.split("```yaml")[1].split("```")[0].strip()
    elif "```" in response:
        return response.split("```")[1].split("```")[0].strip()
    return response


def parse_tasks_overview(yaml_content: str) -> List[Dict[str, Any]]:
    """Parse tasks_overview.yaml and extract task list."""
    try:
        # Handle both single task and multi-document YAML
        docs = list(yaml.safe_load_all(yaml_content))

        # If single document, wrap in list
        if len(docs) == 1 and isinstance(docs[0], dict) and "task" in docs[0]:
            return [docs[0]]

        # Filter out None and non-task documents
        tasks = [doc for doc in docs if doc and isinstance(doc, dict) and "task" in doc]
        return tasks
    except yaml.YAMLError as e:
        print(f"\n✗ Error parsing YAML: {e}")
        print("Please fix the YAML syntax errors before proceeding.")
        return []


async def extract_text_response(client: ClaudeSDKClient) -> tuple[str, dict]:
    """Extract text from agent response with usage stats."""
    response_parts = []
    usage_stats = {}

    async for msg in client.receive_response():
        if isinstance(msg, AssistantMessage):
            for block in msg.content:
                if isinstance(block, TextBlock):
                    response_parts.append(block.text)
        elif isinstance(msg, ResultMessage):
            usage_stats = {
                'duration_ms': msg.duration_ms,
                'duration_api_ms': msg.duration_api_ms,
                'num_turns': msg.num_turns,
                'total_cost_usd': msg.total_cost_usd,
                'usage': msg.usage,
                'session_id': msg.session_id,
            }

    return "\n".join(response_parts), usage_stats


async def extract_text_from_query(prompt: str, options: ClaudeAgentOptions) -> tuple[str, dict]:
    """Extract text from query() response with usage stats."""
    response_parts = []
    usage_stats = {}

    async for msg in query(prompt=prompt, options=options):
        if isinstance(msg, AssistantMessage):
            for block in msg.content:
                if isinstance(block, TextBlock):
                    response_parts.append(block.text)
        elif isinstance(msg, ResultMessage):
            usage_stats = {
                'duration_ms': msg.duration_ms,
                'duration_api_ms': msg.duration_api_ms,
                'num_turns': msg.num_turns,
                'total_cost_usd': msg.total_cost_usd,
                'usage': msg.usage,
                'session_id': msg.session_id,
            }

    return "\n".join(response_parts), usage_stats


# =============================================================================
# STEP 1: Main Orchestrator - Generate tasks_overview.yaml
# =============================================================================


async def step1_generate_overview(impl_md: str, overview_template: str) -> tuple[str, dict]:
    """
    Main orchestrator generates tasks_overview.yaml from IMPL.md.
    Returns tuple of (yaml_content, usage_stats).
    """
    print("\n" + "=" * 80)
    print("STEP 1: Main Orchestrator - Generate tasks_overview.yaml")
    print("=" * 80 + "\n")

    system_prompt = """You are a task planning specialist focused on generating high-level task overviews.

Your goal is to analyze the implementation document and generate a tasks_overview.yaml file that breaks down the implementation into logical tasks.

Key instructions:
- Generate YAML that follows the tasks_overview_template.yaml structure exactly
- Create one task block per logical implementation objective
- Keep descriptions strategic and high-level (WHAT and WHY, not HOW)
- Assign sequential task IDs starting from 1
- Identify dependencies between tasks accurately
- Focus on business/architectural value and outcomes
- Estimate complexity and effort realistically

Output only valid YAML, no markdown code blocks or extra commentary."""

    prompt = f"""Using the implementation document below, generate tasks_overview.yaml following the template structure.

# Implementation Document:
```
{impl_md}
```

# Template Structure (tasks_overview_template.yaml):
```yaml
{overview_template}
```

Generate a complete tasks_overview.yaml with all tasks identified from the implementation document. Use YAML multi-document format (separate tasks with ---) if there are multiple tasks.

Make sure to just give your response. You must not create or write any files just output the yaml and only that.

"""

    options = ClaudeAgentOptions(
        system_prompt=system_prompt,
        allowed_tools=["Read", "Grep", "Glob"],
        permission_mode="bypassPermissions",
    )

    async with ClaudeSDKClient(options=options) as client:
        await client.query(prompt)
        response, usage_stats = await extract_text_response(client)

    # Print usage stats
    print(f"\n[Step 1] Usage Statistics:")
    print(f"  Duration: {usage_stats.get('duration_ms', 0)}ms")
    print(f"  Turns: {usage_stats.get('num_turns', 0)}")
    if usage_stats.get('total_cost_usd'):
        print(f"  Cost: ${usage_stats['total_cost_usd']:.4f}")
    if usage_stats.get('usage'):
        print(f"  Tokens: {usage_stats['usage']}")
    print()

    return clean_yaml_response(response), usage_stats


# =============================================================================
# STEP 2: Suborchestrators - Expand tasks into detailed specifications
# =============================================================================


async def suborchestrator_expand_task(
    task_overview: Dict[str, Any],
    task_template: str,
    debug: bool = False,
) -> tuple[str, dict]:
    """
    Suborchestrator uses Claude with defined sub-agents to expand task.
    Claude intelligently delegates to specialized agents as needed.
    Returns tuple of (yaml_content, usage_stats).
    """
    task_id = task_overview.get("task", {}).get("id", "?")
    task_name = task_overview.get("task", {}).get("name", "Unknown")

    print(f"\n[Task {task_id}] Suborchestrator: {task_name}")

    # Pre-serialize task_overview once for efficiency
    task_overview_yaml = yaml.dump(
        task_overview, default_flow_style=False, sort_keys=False
    )

    # Define specialized sub-agents
    agents = {
        "files": AgentDefinition(
            description="Specialist that identifies all files to be created or modified",
            prompt="""You are a files identification specialist.

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

Output valid YAML only, no markdown.""",
            tools=["Read", "Grep", "Glob"],
            model="sonnet",
        ),
        "functions": AgentDefinition(
            description="Specialist that specifies functions, structs, traits, and other code items",
            prompt="""You are a functions specification specialist.

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

Output valid YAML only, no markdown.""",
            tools=["Read", "Grep", "Glob"],
            model="sonnet",
        ),
        "formal": AgentDefinition(
            description="Specialist that determines formal verification requirements",
            prompt="""You are a formal verification specialist.

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

Output valid YAML only, no markdown.""",
            tools=["Read"],
            model="sonnet",
        ),
        "tests": AgentDefinition(
            description="Specialist that designs test strategy and implements test code",
            prompt="""You are a testing specialist.

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

Output valid YAML only, no markdown.""",
            tools=["Read", "Grep"],
            model="sonnet",
        ),
    }

    # System prompt for suborchestrator (main instructions)
    system_prompt = f"""Your task is to expand Task {task_id} ("{task_name}") from a high-level overview into a complete, detailed specification.

## OBJECTIVE
Transform the task overview below into a complete task specification that matches the task_template structure by delegating to specialized agents.

IMPORTANT: You are in the PLANNING phase. DO NOT create, write, or modify any files. Your sole purpose is to OUTPUT a YAML specification that describes what should be implemented.

## INPUT: TASK OVERVIEW (High-level)
This is the current state of Task {task_id} - a strategic description of WHAT needs to be done and WHY:
```yaml
{task_overview_yaml}
```

## OUTPUT TARGET: TASK TEMPLATE (Detailed structure)
Your goal is to produce a complete YAML document following this template structure:
```yaml
{task_template}
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

## QUALITY STANDARDS
- All file paths must be complete and valid
- Function specifications must include clear descriptions
- Test coverage must be comprehensive
- Dependencies must be clearly identified
- YAML output must be valid and follow the template structure exactly

## YAML FORMATTING REQUIREMENTS (CRITICAL!)
When combining sub-agent outputs into the final YAML, you MUST follow these rules:

1. **All code blocks MUST use literal block syntax with pipe (|)**:
   ✓ CORRECT:
   code: |
     fn example() {{
       // code here
     }}

   ✗ WRONG (breaks YAML parsing):
   code: fn example() {{ ... }}
   code: "fn example() {{ ... }}"

2. **Multi-line strings MUST use literal block syntax (| or |-)**:
   ✓ CORRECT:
   description: |
     This is a multi-line
     description with details.

   ✗ WRONG:
   description: This is a multi-line\ndescription

3. **Preserve exact literal block format from sub-agent responses**:
   - When @tests agent outputs `code: |`, keep the `|`
   - When @functions agent outputs multi-line descriptions with `|`, keep the `|`
   - NEVER convert literal blocks to inline strings

4. **Special characters require literal blocks**:
   - Code containing: {{ }} : " ' # [ ]
   - SVG paths: "M 0 0 L 1 1"
   - Test code with #[test] attributes
   - Any Rust code, especially with macros

5. **Example of CORRECT final output**:
   tests:
     implementation:
       code: |
         #[cfg(test)]
         mod tests {{
           #[test]
           fn test_something() {{
             assert_eq!(2 + 2, 4);
           }}
         }}

## IMPORTANT REQUIREMENTS
- Preserve task id ({task_id}) and name ("{task_name}") from the overview
- Expand the context section based on the overview's description
- Include the dependencies section from the overview
- All sections must be complete and valid YAML
- Output ONLY the final YAML, no markdown code blocks or commentary
- DO NOT create, write, or modify any files - this is a planning phase only
- Your job is to OUTPUT the specification, not to implement it"""

    # Short query prompt
    query_prompt = f"""Expand Task {task_id} ("{task_name}") by coordinating with your specialized agents.

IMPORTANT: Run all agents in parallel for maximum efficiency:
- Invoke @files, @functions, @formal, and @tests agents simultaneously
- Wait for all agents to complete
- Then combine their outputs into the complete task specification in YAML format."""

    options = ClaudeAgentOptions(
        allowed_tools=["Read", "Grep", "Glob"],
        system_prompt=system_prompt,
        agents=agents,
        permission_mode="bypassPermissions",
        include_partial_messages=True,  # Enable streaming of partial messages for better visibility
    )

    # Execute suborchestrator with sub-agents
    response_parts = []
    usage_stats = {}
    async for msg in query(prompt=query_prompt, options=options):
        if isinstance(msg, AssistantMessage):
            for block in msg.content:
                if isinstance(block, TextBlock):
                    response_parts.append(block.text)
                    # Print streaming output in debug mode
                    if debug:
                        print(block.text)
                    # Print progress indicator for agent delegation (always shown)
                    # Detect agent invocations by looking for @agent_name syntax
                    text_lower = block.text.lower()
                    if "@files" in block.text:  # Use original text to preserve @ symbol
                        print(f"[Task {task_id}] → Delegating to @files agent...")
                    elif "@functions" in block.text:
                        print(f"[Task {task_id}] → Delegating to @functions agent...")
                    elif "@formal" in block.text:
                        print(f"[Task {task_id}] → Delegating to @formal agent...")
                    elif "@tests" in block.text:
                        print(f"[Task {task_id}] → Delegating to @tests agent...")
                elif isinstance(block, ThinkingBlock):
                    # Print thinking blocks in debug mode
                    if debug:
                        print(f"\n[Task {task_id}] 💭 Thinking:")
                        print(block.thinking)
                        print()
        elif isinstance(msg, ResultMessage):
            usage_stats = {
                'duration_ms': msg.duration_ms,
                'duration_api_ms': msg.duration_api_ms,
                'num_turns': msg.num_turns,
                'total_cost_usd': msg.total_cost_usd,
                'usage': msg.usage,
                'session_id': msg.session_id,
            }

    combined_output = "\n".join(response_parts)
    combined_output = clean_yaml_response(combined_output)

    print(f"\n[Task {task_id}] Expansion complete")
    print(f"[Task {task_id}] Duration: {usage_stats.get('duration_ms', 0)}ms, Turns: {usage_stats.get('num_turns', 0)}")
    if usage_stats.get('total_cost_usd'):
        print(f"[Task {task_id}] Cost: ${usage_stats['total_cost_usd']:.4f}")

    # Print the final suborchestrator response only in debug mode
    if debug:
        print(f"\n{'='*80}")
        print(f"[Task {task_id}] FINAL SPECIFICATION: {task_name}")
        print(f"{'='*80}")
        print(combined_output)
        print(f"{'='*80}\n")

    return combined_output, usage_stats


def generate_execution_plan_simple(
    tasks: List[Dict[str, Any]],
    batch_size: int = 5,
) -> str:
    """
    Generate a simple execution plan by chunking tasks into fixed-size batches.
    Ignores dependencies - just splits tasks into batches of specified size.

    Args:
        tasks: List of task documents
        batch_size: Maximum number of tasks per batch (default: 5)

    Returns:
        execution_plan.yaml as a string
    """
    print("\n" + "=" * 80)
    print(f"BATCH PLANNING: Simple batching with size={batch_size}")
    print("=" * 80 + "\n")

    # Create batches
    batches = []
    for i in range(0, len(tasks), batch_size):
        batch_tasks = tasks[i : i + batch_size]

        batch_def = {
            "batch_id": len(batches) + 1,
            "description": f"Batch {len(batches) + 1} - Tasks {i + 1} to {min(i + batch_size, len(tasks))}",
            "strategy": "sequential",
            "tasks": [
                {
                    "task_id": task.get("task", {}).get("id"),
                    "task_name": task.get("task", {}).get("name", "Unknown"),
                    "reason": f"Part of batch {len(batches) + 1}",
                }
                for task in batch_tasks
            ],
            "parallelization_rationale": f"Fixed batch size of {batch_size} tasks running in parallel",
        }
        batches.append(batch_def)

    plan = {
        "execution_plan": {
            "total_tasks": len(tasks),
            "total_batches": len(batches),
            "batches": batches,
            "dependencies_summary": {
                "critical_path": [],
                "parallelization_potential": "high" if len(batches) > 1 else "low",
                "parallelization_explanation": f"Tasks split into {len(batches)} fixed-size batches of up to {batch_size} tasks each",
            },
        }
    }

    return yaml.dump(plan, default_flow_style=False, sort_keys=False)


async def generate_execution_plan(
    tasks_overview_yaml: str,
) -> str:
    """
    Use an AI agent to analyze tasks_overview.yaml and generate an execution plan.
    Returns execution_plan.yaml as a string.
    """
    print("\n" + "=" * 80)
    print("BATCH PLANNING: Analyzing dependencies and generating execution plan")
    print("=" * 80 + "\n")

    system_prompt = """You are an execution planning specialist focused on dependency analysis and batch optimization.

Your goal is to analyze tasks_overview.yaml and generate an optimal execution plan that maximizes parallelization while respecting dependencies.

Key instructions:
- Analyze requires_completion_of for each task
- Group tasks into batches where all tasks in a batch can run in parallel
- Tasks can only be in a batch if ALL their dependencies are in previous batches
- Maximize tasks per batch (more parallelization = faster execution)
- Batches execute sequentially, tasks within batch execute in parallel
- Identify the critical path (longest dependency chain)
- Detect any circular dependencies and warn about them

Output only valid YAML following the template structure, no markdown code blocks or extra commentary."""

    execution_plan_template = """execution_plan:
  total_tasks: [NUMBER]
  total_batches: [NUMBER]

  batches:
    - batch_id: 1
      description: "[Brief description of what this batch accomplishes]"
      strategy: "sequential"  # All batches execute sequentially
      tasks:
        - task_id: [NUMBER]
          task_name: "[TASK_NAME]"
          reason: "[Why this task is in this batch - e.g., 'No dependencies' or 'Depends on batch 1']"

      # Tasks within this batch can run in parallel because:
      parallelization_rationale: |
        [Explain why these tasks can run in parallel.
        E.g., "All tasks have no dependencies" or
        "All dependencies from previous batches are satisfied"]

  dependencies_summary:
    critical_path:
      # Longest dependency chain
      - task_id: [NUMBER]
      - task_id: [NUMBER]

    parallelization_potential: "[low|medium|high]"
    parallelization_explanation: |
      [Explain the overall parallelization potential.
      E.g., "High - 10 out of 14 tasks can run in parallel across 3 batches"]"""

    prompt = f"""Analyze the tasks and their dependencies, then generate an execution plan.

# Tasks Overview:
```yaml
{tasks_overview_yaml}
```

# Execution Plan Template:
```yaml
{execution_plan_template}
```

Generate a complete execution_plan.yaml that:
1. Groups tasks into optimal batches for parallel execution
2. Respects all dependencies (requires_completion_of)
3. Maximizes parallelization potential
4. Includes rationale for each batch
5. Identifies critical path and parallelization potential

Output only the YAML, no markdown formatting."""

    options = ClaudeAgentOptions(
        system_prompt=system_prompt,
        allowed_tools=["Read"],
        permission_mode="bypassPermissions",
    )

    async with ClaudeSDKClient(options=options) as client:
        await client.query(prompt)
        response = await extract_text_response(client)

    return clean_yaml_response(response)


def parse_execution_plan(
    execution_plan_yaml: str, tasks: List[Dict[str, Any]], debug: bool = False
) -> List[List[Dict[str, Any]]]:
    """
    Parse execution_plan.yaml and convert to batch structure.
    Returns: List of batches, where each batch is a list of task documents.
    """
    try:
        plan = yaml.safe_load(execution_plan_yaml)

        # Build task lookup by ID
        task_by_id = {}
        for task_doc in tasks:
            task_id = task_doc.get("task", {}).get("id")
            if task_id:
                task_by_id[task_id] = task_doc

        # Extract batches from plan
        batches = []
        plan_batches = plan.get("execution_plan", {}).get("batches", [])

        if debug:
            print(f"DEBUG: Parsing {len(plan_batches)} batches from execution plan\n")

        for batch_def in plan_batches:
            batch_id = batch_def.get("batch_id", "?")
            batch_tasks = []
            task_refs = batch_def.get("tasks", [])

            if debug:
                print(f"  Batch {batch_id}: {len(task_refs)} tasks")

            for task_ref in task_refs:
                task_id = task_ref.get("task_id")
                task_name = task_ref.get("task_name", "Unknown")
                if debug:
                    print(f"    - Task {task_id}: {task_name}")

                if task_id in task_by_id:
                    batch_tasks.append(task_by_id[task_id])
                else:
                    print(
                        f"      ⚠ Warning: Task {task_id} not found in tasks_overview"
                    )

            if batch_tasks:
                batches.append(batch_tasks)
            if debug:
                print()

        return batches

    except Exception as e:
        print(f"⚠ Error parsing execution plan: {e}")
        print("Falling back to simple dependency analysis")
        return build_execution_batches_fallback(tasks)


def build_execution_batches_fallback(
    tasks: List[Dict[str, Any]],
) -> List[List[Dict[str, Any]]]:
    """
    Fallback: Simple dependency analysis if execution plan fails.
    """
    # Build task lookup by ID
    task_by_id = {}
    for task_doc in tasks:
        task_id = task_doc.get("task", {}).get("id")
        if task_id:
            task_by_id[task_id] = task_doc

    # Track which tasks have been scheduled
    scheduled = set()
    batches = []

    while len(scheduled) < len(tasks):
        # Find tasks that can run now (all dependencies satisfied)
        current_batch = []

        for task_doc in tasks:
            task = task_doc.get("task", {})
            task_id = task.get("id")

            if task_id in scheduled:
                continue

            # Check if all dependencies are satisfied
            dependencies = task.get("dependencies", {}).get(
                "requires_completion_of", []
            )

            # Handle empty array (use len check instead of truthiness)
            if len(dependencies) == 0:
                can_run = True
            else:
                can_run = all(
                    dep.get("task_id") in scheduled
                    for dep in dependencies
                    if isinstance(dep, dict) and dep.get("task_id")
                )

            if can_run:
                current_batch.append(task_doc)
                scheduled.add(task_id)

        if not current_batch:
            # Circular dependency or error - add remaining tasks to avoid infinite loop
            print("⚠ Warning: Circular dependency detected or unresolved dependencies")
            remaining = [
                t for t in tasks if t.get("task", {}).get("id") not in scheduled
            ]
            if remaining:
                batches.append(remaining)
            break

        batches.append(current_batch)

    return batches


async def step2_expand_all_tasks(
    tasks_overview_yaml: str,
    task_template: str,
    project_root: Path,
    stream_to_file: bool = False,
    debug: bool = False,
    simple_batching: bool = False,
    batch_size: int = 5,
) -> str:
    """
    For each task in overview, spawn a suborchestrator to expand it.
    Uses AI agent for intelligent batch planning and parallelization.

    Args:
        stream_to_file: If True, write tasks to file immediately to reduce memory usage.
                       Useful for large projects with many tasks.
        debug: If True, print detailed debug information including batches and task YAML.
        simple_batching: If True, use simple fixed-size batching instead of AI dependency analysis.
        batch_size: Size of batches when using simple_batching (default: 5).
    """
    print("\n" + "=" * 80)
    print("STEP 2: Suborchestrators - Expand Tasks")
    print("=" * 80 + "\n")

    tasks = parse_tasks_overview(tasks_overview_yaml)

    if not tasks:
        print("✗ No valid tasks found. Aborting.")
        return ""

    print(f"Found {len(tasks)} tasks to expand\n")

    # Generate execution plan - either simple or AI-based
    if simple_batching:
        execution_plan_yaml = generate_execution_plan_simple(tasks, batch_size)
    else:
        execution_plan_yaml = await generate_execution_plan(tasks_overview_yaml)

    # Print execution plan only in debug mode
    if debug:
        print("\n" + "=" * 80)
        print("EXECUTION PLAN")
        print("=" * 80 + "\n")
        print(execution_plan_yaml)
        print("\n" + "=" * 80 + "\n")

    # Parse execution plan into batches
    batches = parse_execution_plan(execution_plan_yaml, tasks, debug)

    print(f"Execution plan: {len(batches)} batch(es)")
    if debug:
        print()
        for i, batch in enumerate(batches, 1):
            task_ids = [t.get("task", {}).get("id") for t in batch]
            if len(batch) == 1:
                print(f"  Batch {i}: Task {task_ids[0]} (sequential)")
            else:
                print(f"  Batch {i}: Tasks {task_ids} (parallel)")
    print()

    # Execute batches sequentially, tasks within batch in parallel
    all_expanded = []
    all_usage_stats = []
    tasks_path = project_root / "tasks.yaml"

    # Open file for streaming if requested
    file_handle = None
    if stream_to_file:
        print(f"Streaming mode: Writing tasks directly to {tasks_path}\n")
        file_handle = open(tasks_path, "w")

    try:
        for batch_num, batch in enumerate(batches, 1):
            print(f"\n→ Executing Batch {batch_num}/{len(batches)}")

            if len(batch) == 1:
                # Single task - run directly (YAML printed inside suborchestrator)
                expanded, usage_stats = await suborchestrator_expand_task(
                    batch[0], task_template, debug
                )
                all_usage_stats.append(usage_stats)
                if stream_to_file:
                    if batch_num > 1:
                        file_handle.write("\n---\n")
                    file_handle.write(expanded)
                    file_handle.flush()  # Ensure written immediately
                else:
                    all_expanded.append(expanded)
            else:
                # Multiple tasks - run in parallel (YAML printed inside each suborchestrator as they complete)
                print(f"  Running {len(batch)} tasks in parallel...\n")
                tasks_coros = [
                    suborchestrator_expand_task(task_doc, task_template, debug)
                    for task_doc in batch
                ]
                expanded_batch = await asyncio.gather(*tasks_coros)
                if stream_to_file:
                    for i, (expanded, usage_stats) in enumerate(expanded_batch):
                        all_usage_stats.append(usage_stats)
                        if batch_num > 1 or i > 0:
                            file_handle.write("\n---\n")
                        file_handle.write(expanded)
                    file_handle.flush()
                else:
                    for expanded, usage_stats in expanded_batch:
                        all_expanded.append(expanded)
                        all_usage_stats.append(usage_stats)

            print()

        # Print aggregate usage stats
        print(f"\n{'='*80}")
        print(f"STEP 2: Aggregate Usage Statistics")
        print(f"{'='*80}")
        total_duration = sum(s.get('duration_ms', 0) for s in all_usage_stats)
        total_turns = sum(s.get('num_turns', 0) for s in all_usage_stats)
        total_cost = sum(s.get('total_cost_usd', 0) or 0 for s in all_usage_stats)
        print(f"  Total tasks expanded: {len(all_usage_stats)}")
        print(f"  Total duration: {total_duration}ms ({total_duration/1000:.1f}s)")
        print(f"  Total turns: {total_turns}")
        if total_cost > 0:
            print(f"  Total cost: ${total_cost:.4f}")
        print(f"{'='*80}\n")

        # Return combined YAML or empty string if streaming
        if stream_to_file:
            return ""  # Already written to file
        else:
            combined = "\n---\n".join(all_expanded)
            return combined

    finally:
        if file_handle:
            file_handle.close()
            print(f"✓ Tasks streamed to: {tasks_path}\n")


# =============================================================================
# STEP 3: Reviewer Agents - Validate expanded tasks
# =============================================================================


async def review_suborchestrator(
    batch: List[Dict[str, Any]],
    impl_md: str,
    tasks_overview_yaml: str,
    task_template: str,
    batch_num: int,
    debug: bool = False,
) -> List[Dict[str, Any]]:
    """
    Suborchestrator agent that coordinates @reviewer sub-agents for a batch of tasks.

    This is an AI agent (not just a function) that receives high-level context about
    the review workflow and delegates detailed validation to @reviewer sub-agents.

    Args:
        batch: List of task groups (each with 'overview' and 'detailed') - used for task IDs only
        impl_md: Implementation document content (full context)
        tasks_overview_yaml: Full tasks_overview.yaml content (full context)
        task_template: task_template.yaml structure (full context)
        batch_num: Batch number for logging
        debug: Enable debug output

    Returns:
        List of review results as dicts (JSON array format)

    Flow:
        1. Receives high-level Step 3 workflow context
        2. For each task in batch, invokes @reviewer with task-specific details
        3. @reviewer agents run IN PARALLEL
        4. Collects and synthesizes all results into JSON array
    """
    print(f"[Batch {batch_num}] Suborchestrator starting...")

    # Define the reviewer agent
    reviewer_agent = AgentDefinition(
        description="Specialist that validates individual task specifications against requirements",
        prompt="""You are an implementation plan reviewer.

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
SUMMARY: [Brief summary]""",
        tools=["Read"],
        model="sonnet",
    )

    # Build task list for suborchestrator
    task_list = []
    for group in batch:
        task_id = group["overview"].get("task", {}).get("id")
        task_name = group["overview"].get("task", {}).get("name", "Unknown")
        task_list.append(
            {
                "task_id": task_id,
                "task_name": task_name,
            }
        )

    # System prompt for suborchestrator
    system_prompt = f"""You are a review suborchestrator coordinating Step 3: Review & Validation.

## YOUR ROLE
Coordinate the @reviewer agent to validate all {len(task_list)} tasks in your batch.

## STEP 3 WORKFLOW (Review & Validation)
This is the final validation step in the multi-agent task planning workflow:
1. Each task has both an overview (tasks_overview.yaml) and detailed spec (tasks.yaml)
2. Your job is to validate that detailed specs match their overviews and align with IMPL.md
3. You coordinate @reviewer agents in parallel for efficiency
4. You collect and synthesize all review results into a JSON report

## AVAILABLE CONTEXT
You have access to:
- Implementation requirements (IMPL.md)
- Task overview structure (tasks_overview.yaml)
- Task template structure (task_template.yaml)
- Individual task details (provided when you invoke @reviewer)

## YOUR AGENT
**@reviewer** - Validates individual task specifications
- Input: Task overview + detailed spec + IMPL.md context
- Output: ASSESSMENT, ISSUES, SUMMARY

## WORKFLOW
1. For each task in your batch, invoke @reviewer agent with:
   - The task's overview YAML (from tasks_overview.yaml)
   - The task's detailed specification YAML (from tasks.yaml)
   - Reference to IMPL.md for requirements context
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
- Output ONLY the JSON array, no markdown code blocks, no extra commentary
"""

    # Build query prompt - suborchestrator gets high-level context, not all task details
    task_summary = "\n".join(
        [f"  - Task {t['task_id']}: {t['task_name']}" for t in task_list]
    )

    query_prompt = f"""Coordinate review of all {len(task_list)} tasks in your batch.

## CONTEXT FOR STEP 3 WORKFLOW

### Implementation Requirements (IMPL.md):
```
{impl_md}
```

### Tasks Overview Structure (tasks_overview.yaml):
```yaml
{tasks_overview_yaml}
```

### Expected Task Template Structure (task_template.yaml):
```yaml
{task_template}
```

## YOUR BATCH
Review these tasks:
{task_summary}

## INSTRUCTIONS
For EACH task above:
1. Extract the task's overview from tasks_overview.yaml (you have it above)
2. Extract the task's detailed spec from tasks.yaml (use Read tool if needed)
3. Invoke @reviewer with both the overview and detailed spec
4. Parse the reviewer's response

Run ALL @reviewer agents in PARALLEL, then combine results into JSON array.

IMPORTANT: Each @reviewer needs the specific task's overview and detailed YAML - delegate the task details to them, don't try to process everything yourself."""

    options = ClaudeAgentOptions(
        allowed_tools=["Read"],
        system_prompt=system_prompt,
        agents={"reviewer": reviewer_agent},
        permission_mode="bypassPermissions",
        include_partial_messages=True,
    )

    # Execute suborchestrator
    response_parts = []
    usage_stats = {}
    async for msg in query(prompt=query_prompt, options=options):
        if isinstance(msg, AssistantMessage):
            for block in msg.content:
                if isinstance(block, TextBlock):
                    response_parts.append(block.text)
                    if debug:
                        print(block.text)
                    # Show delegation progress
                    if "@reviewer" in block.text:
                        print(f"[Batch {batch_num}] → Delegating to @reviewer agent...")
        elif isinstance(msg, ResultMessage):
            usage_stats = {
                'duration_ms': msg.duration_ms,
                'duration_api_ms': msg.duration_api_ms,
                'num_turns': msg.num_turns,
                'total_cost_usd': msg.total_cost_usd,
                'usage': msg.usage,
                'session_id': msg.session_id,
            }

    combined_output = "\n".join(response_parts)

    if debug:
        print(f"\n[Batch {batch_num}] Raw suborchestrator output:")
        print(combined_output)
        print()

    # Parse JSON response
    try:
        # Clean potential markdown code blocks
        if "```json" in combined_output:
            json_str = combined_output.split("```json")[1].split("```")[0].strip()
        elif "```" in combined_output:
            json_str = combined_output.split("```")[1].split("```")[0].strip()
        else:
            json_str = combined_output.strip()

        results = json.loads(json_str)

        print(f"[Batch {batch_num}] ✓ Parsed {len(results)} review results")
        print(f"[Batch {batch_num}] Duration: {usage_stats.get('duration_ms', 0)}ms, Turns: {usage_stats.get('num_turns', 0)}")
        if usage_stats.get('total_cost_usd'):
            print(f"[Batch {batch_num}] Cost: ${usage_stats['total_cost_usd']:.4f}")

        # Attach usage stats to results for aggregation
        for result in results:
            result['_usage_stats'] = usage_stats

        return results

    except json.JSONDecodeError as e:
        print(f"⚠ Warning: Failed to parse JSON from suborchestrator: {e}")
        print(f"Attempted to parse: {json_str[:200]}...\n")

        # Fallback: return failed results for this batch
        return [
            {
                "task_id": t["task_id"],
                "success": False,
                "issues": ["Failed to parse suborchestrator response"],
                "summary": "Review failed due to JSON parsing error",
            }
            for t in task_list
        ]


async def step3_review_tasks(
    tasks_overview_yaml: str,
    tasks_yaml: str,
    impl_md: str,
    task_template: str,
    batch_size: int = 5,
    debug: bool = False,
) -> List[Dict[str, Any]]:
    """
    Coordinator function (not an AI agent) that orchestrates the review process.

    This function handles file parsing, batch creation, and sequential execution
    of review suborchestrators. It is not an AI agent itself - it's a standard
    Python function that spawns AI agents (suborchestrators).

    Args:
        tasks_overview_yaml: Full tasks_overview.yaml content
        tasks_yaml: Full tasks.yaml content
        impl_md: Implementation document content
        task_template: task_template.yaml structure for validation
        batch_size: Number of tasks to review per batch (default: 5)
        debug: Enable debug output

    Returns:
        List of all review results from all batches

    Flow:
        1. Parse and match overview tasks with detailed tasks
        2. Create batches of N tasks
        3. For each batch sequentially:
           - Spawn review_suborchestrator (AI agent)
           - Suborchestrator coordinates @reviewer agents in parallel
           - Collect batch results
        4. Return all results combined
    """
    print("\n" + "=" * 80)
    print("STEP 3: Batched Review with Suborchestrators")
    print("=" * 80 + "\n")

    overview_tasks = parse_tasks_overview(tasks_overview_yaml)
    detailed_tasks = parse_tasks_overview(tasks_yaml)

    print(
        f"Matching {len(overview_tasks)} overview tasks with {len(detailed_tasks)} detailed tasks\n"
    )

    # Build lookup dict for O(1) access
    detailed_map = {
        det.get("task", {}).get("id"): det
        for det in detailed_tasks
        if det.get("task", {}).get("id")
    }

    # Group tasks by ID (pair overview with detailed)
    task_groups = []
    for overview in overview_tasks:
        overview_id = overview.get("task", {}).get("id")
        detailed = detailed_map.get(overview_id)

        if detailed:
            task_groups.append(
                {
                    "overview": overview,
                    "detailed": detailed,
                }
            )
        else:
            print(f"⚠ Warning: No detailed task found for overview task {overview_id}")

    # Create batches
    batches = []
    for i in range(0, len(task_groups), batch_size):
        batches.append(task_groups[i : i + batch_size])

    print(f"Created {len(batches)} batch(es) with batch_size={batch_size}\n")

    # Process each batch with a suborchestrator
    all_review_results = []

    for batch_num, batch in enumerate(batches, 1):
        print(f"\n→ Processing Review Batch {batch_num}/{len(batches)}")
        task_ids = [g["overview"].get("task", {}).get("id") for g in batch]
        print(f"  Tasks in batch: {task_ids}\n")

        batch_results = await review_suborchestrator(
            batch=batch,
            impl_md=impl_md,
            tasks_overview_yaml=tasks_overview_yaml,
            task_template=task_template,
            batch_num=batch_num,
            debug=debug,
        )

        all_review_results.extend(batch_results)
        print(f"✓ Batch {batch_num} review complete\n")

    return all_review_results


async def step3_main_orchestrator_report(review_results: List[Dict[str, Any]]):
    """
    Report generator function (not an AI agent) that produces final review summary.

    This is a standard Python function that takes all review results and generates
    a human-readable report file. It does not use AI - just data processing.
    """
    print("\n" + "=" * 80)
    print("FINAL REPORT: Main Orchestrator Summary")
    print("=" * 80 + "\n")

    approved_count = sum(1 for r in review_results if r["success"])
    needs_revision_count = len(review_results) - approved_count

    print(f"Total tasks reviewed: {len(review_results)}")
    print(f"✓ Approved: {approved_count}")
    print(f"✗ Needs revision: {needs_revision_count}\n")

    if needs_revision_count > 0:
        print("Tasks requiring revision:\n")
        for result in review_results:
            if not result["success"]:
                print(f"  Task {result['task_id']}:")
                for issue in result["issues"]:
                    print(f"    - {issue}")
                print(f"    Summary: {result['summary']}\n")
    else:
        print("✓ All tasks approved! Ready for implementation.\n")

    # Save report
    report_path = Path(__file__).parent.parent / "task_review_report.txt"
    with open(report_path, "w") as f:
        f.write("=" * 80 + "\n")
        f.write("TASK REVIEW REPORT\n")
        f.write("=" * 80 + "\n\n")
        f.write(f"Total tasks: {len(review_results)}\n")
        f.write(f"Approved: {approved_count}\n")
        f.write(f"Needs revision: {needs_revision_count}\n\n")

        for result in review_results:
            f.write(
                f"\nTask {result['task_id']}: {'APPROVED' if result['success'] else 'NEEDS REVISION'}\n"
            )
            f.write(f"Summary: {result['summary']}\n")
            if result["issues"]:
                f.write("Issues:\n")
                for issue in result["issues"]:
                    f.write(f"  - {issue}\n")
            f.write("\n")

    print(f"✓ Full report saved to: {report_path}")


# =============================================================================
# Main Workflow
# =============================================================================


async def main():
    parser = argparse.ArgumentParser(
        description="Multi-agent task planning orchestrator"
    )
    parser.add_argument(
        "--step",
        type=str,
        choices=["1", "2", "3", "all"],
        default="all",
        help="Which step to run (1=overview, 2=expand, 3=review, all=complete workflow)",
    )
    parser.add_argument(
        "--impl",
        type=str,
        help="Path to IMPL.md (default: auto-detect)",
    )
    parser.add_argument(
        "--tasks-overview",
        type=str,
        help="Path to tasks_overview.yaml (default: ./tasks_overview.yaml)",
    )
    parser.add_argument(
        "--tasks",
        type=str,
        help="Path to tasks.yaml (default: ./tasks.yaml)",
    )
    parser.add_argument(
        "--stream",
        action="store_true",
        help="Stream tasks to file immediately (reduces memory usage for large projects)",
    )
    parser.add_argument(
        "--debug",
        action="store_true",
        help="Enable debug output (prints batches, task YAML, etc.)",
    )
    parser.add_argument(
        "--batch-size",
        type=int,
        default=None,
        help="Use simple fixed-size batching with specified size (e.g., --batch-size 5). If not specified, uses AI dependency analysis.",
    )
    parser.add_argument(
        "--tasks-overview-template",
        type=str,
        help="Path to tasks_overview_template.yaml (required for step 1)",
    )
    parser.add_argument(
        "--task-template",
        type=str,
        help="Path to task_template.yaml (required for steps 2 and 3)",
    )

    args = parser.parse_args()

    # Load templates based on which step is running
    overview_template = None
    task_template = None

    # Load overview template if needed (step 1 or all)
    if args.step in ["1", "all"]:
        if not args.tasks_overview_template:
            print("Error: --tasks-overview-template is required for step 1")
            return
        overview_template_path = Path(args.tasks_overview_template)
        if not overview_template_path.exists():
            print(
                f"Error: tasks_overview_template.yaml not found at {overview_template_path}"
            )
            return
        print("Loading tasks_overview_template...")
        overview_template = load_template(overview_template_path)

    # Load task template if needed (step 2, 3, or all)
    if args.step in ["2", "3", "all"]:
        if not args.task_template:
            print("Error: --task-template is required for steps 2 and 3")
            return
        task_template_path = Path(args.task_template)
        if not task_template_path.exists():
            print(f"Error: task_template.yaml not found at {task_template_path}")
            return
        print("Loading task_template...")
        task_template = load_template(task_template_path)

    project_root = Path(__file__).parent.parent

    # Load IMPL.md only if needed (step 1 or step 3)
    impl_md = None
    if args.step in ["1", "3", "all"]:
        if args.impl:
            impl_path = Path(args.impl)
            if not impl_path.exists():
                print(f"Error: IMPL.md not found at {impl_path}")
                return
            with open(impl_path, "r") as f:
                impl_md = f.read()
        else:
            try:
                impl_md = load_impl_md()
            except FileNotFoundError as e:
                print(f"Error: {e}")
                print("Please create IMPL.md or specify path with --impl")
                return

    # Execute workflow steps
    if args.step in ["1", "all"]:
        # Step 1: Generate overview
        tasks_overview_yaml, step1_usage = await step1_generate_overview(impl_md, overview_template)
        overview_path = project_root / "tasks_overview.yaml"
        save_yaml(tasks_overview_yaml, overview_path)

        if args.step == "1":
            return
    else:
        # Load existing overview
        if args.tasks_overview:
            overview_path = Path(args.tasks_overview)
        else:
            overview_path = project_root / "tasks_overview.yaml"

        if not overview_path.exists():
            print(
                f"Error: tasks_overview.yaml not found at {overview_path}. Run step 1 first or specify with --tasks-overview."
            )
            return
        with open(overview_path, "r") as f:
            tasks_overview_yaml = f.read()

    if args.step in ["2", "all"]:
        # Step 2: Expand tasks
        # Use streaming mode for large projects (reduces memory usage)
        # If --batch-size is specified, use simple batching; otherwise use AI dependency analysis
        simple_batching = args.batch_size is not None
        batch_size = args.batch_size if simple_batching else 5

        tasks_yaml = await step2_expand_all_tasks(
            tasks_overview_yaml,
            task_template,
            project_root,
            stream_to_file=args.stream,
            debug=args.debug,
            simple_batching=simple_batching,
            batch_size=batch_size,
        )

        # Only save if we actually generated tasks and not streaming
        if tasks_yaml and tasks_yaml.strip():
            tasks_path = project_root / "tasks.yaml"
            save_yaml(tasks_yaml, tasks_path)
        elif not tasks_yaml:
            # Empty means streaming mode was used, already saved
            pass
        else:
            print("\n✗ No tasks generated. Not saving tasks.yaml")
            return

        if args.step == "2":
            return
    else:
        # Load existing detailed tasks
        if args.tasks:
            tasks_path = Path(args.tasks)
        else:
            tasks_path = project_root / "tasks.yaml"

        if not tasks_path.exists():
            print(
                f"Error: tasks.yaml not found at {tasks_path}. Run step 2 first or specify with --tasks."
            )
            return
        with open(tasks_path, "r") as f:
            tasks_yaml = f.read()

    if args.step in ["3", "all"]:
        # Step 3: Review tasks
        # Use same batch_size as step2 if specified, otherwise default to 5
        review_batch_size = args.batch_size if args.batch_size is not None else 5

        review_results = await step3_review_tasks(
            tasks_overview_yaml,
            tasks_yaml,
            impl_md,
            task_template,
            batch_size=review_batch_size,
            debug=args.debug,
        )
        await step3_main_orchestrator_report(review_results)


if __name__ == "__main__":
    asyncio.run(main())
