#!/usr/bin/env -S sh -c 'unset PYTHONPATH && uv run --script "$0" "$@"'
# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "claude_agent_sdk",
#     "python-dotenv",
#     "pyyaml",
# ]
# ///

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
    ResultMessage,
)

# Load environment variables
load_dotenv()


def load_templates() -> tuple[str, str]:
    """Load YAML templates from DOCS/TEMPLATES/"""
    base_path = Path(__file__).parent.parent / "DOCS" / "TEMPLATES"

    with open(base_path / "tasks_overview_template.yaml", "r") as f:
        overview_template = f.read()

    with open(base_path / "task_template.yaml", "r") as f:
        task_template = f.read()

    return overview_template, task_template


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


async def extract_text_response(client: ClaudeSDKClient) -> str:
    """Extract text from agent response."""
    response_parts = []

    async for msg in client.receive_response():
        if isinstance(msg, AssistantMessage):
            for block in msg.content:
                if isinstance(block, TextBlock):
                    response_parts.append(block.text)

    return "\n".join(response_parts)


# =============================================================================
# STEP 1: Main Orchestrator - Generate tasks_overview.yaml
# =============================================================================


async def step1_generate_overview(impl_md: str, overview_template: str) -> str:
    """
    Main orchestrator generates tasks_overview.yaml from IMPL.md.
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
        response = await extract_text_response(client)

    return clean_yaml_response(response)


# =============================================================================
# STEP 2: Suborchestrators - Expand tasks into detailed specifications
# =============================================================================


async def spawn_specialized_agent(
    task_overview_yaml: str,
    task_template: str,
    agent_type: str,
    agent_prompt: str,
    task_id: Any,
) -> str:
    """
    Spawn a specialized agent (files, functions, formal, tests).
    task_overview_yaml: Pre-serialized YAML string for efficiency.
    """
    print(f"\n[Task {task_id}] Spawning {agent_type} agent...")

    prompt = f"""You are a {agent_type} specialist working on task expansion.

Task Overview:
```yaml
{task_overview_yaml}
```

Task Template Section to Fill:
```yaml
{task_template}
```

{agent_prompt}

Output only the YAML section you're responsible for, no markdown or extra text."""

    options = ClaudeAgentOptions(
        system_prompt=f"You are a {agent_type} specification expert.",
        allowed_tools=["Read", "Grep", "Glob"],
        permission_mode="bypassPermissions",
    )

    async with ClaudeSDKClient(options=options) as client:
        await client.query(prompt)
        response = await extract_text_response(client)

    return response


async def suborchestrator_expand_task(
    task_overview: Dict[str, Any],
    task_template: str,
    debug: bool = False,
) -> str:
    """
    Suborchestrator spawns 4 specialized agents and combines their outputs.
    """
    task_id = task_overview.get("task", {}).get("id", "?")
    task_name = task_overview.get("task", {}).get("name", "Unknown")

    print(f"\n[Task {task_id}] Suborchestrator: {task_name}")

    # Pre-serialize task_overview once for efficiency
    task_overview_yaml = yaml.dump(task_overview, default_flow_style=False, sort_keys=False)

    # Define specialized agent prompts
    agent_specs = {
        "files": """Identify all files that will be created or modified for this task.
For each file, provide:
- path: Full path to the file
- description: Brief description of the file's role

Output format:
files:
  - path: "path/to/file.rs"
    description: "Description here"
""",
        "functions": """Identify all functions, structs, enums, traits, and other items to be implemented.
For each item, provide:
- type: enum_variant|struct|trait_impl|method|constant|function|module_declaration
- name: Full qualified name or signature
- description: Brief description of purpose and behavior
- preconditions: What must be true before execution (optional)
- postconditions: What will be true after execution (optional)
- invariants: Properties that remain constant (optional)

Group items by file.

Output format:
functions:
  - file: "path/to/file.rs"
    items:
      - type: "function"
        name: "function_name"
        description: "Description"
""",
        "formal": """Determine if formal verification is needed for this task.
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
""",
        "tests": """Design comprehensive tests for this task.
Provide:
- strategy: approach and rationale
- implementation: Complete test code in Rust
- coverage: List of behaviors tested

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
""",
    }

    # Spawn agents in parallel (they work independently)
    agent_coros = [
        spawn_specialized_agent(task_overview_yaml, task_template, agent_type, agent_prompt, task_id)
        for agent_type, agent_prompt in agent_specs.items()
    ]
    agent_outputs_list = await asyncio.gather(*agent_coros)

    # Map outputs back to agent types
    agent_outputs = dict(zip(agent_specs.keys(), agent_outputs_list))

    # Combine outputs into complete task
    print(f"\n[Task {task_id}] Combining agent outputs into complete task...")

    combined_prompt = f"""Combine the following agent outputs into a complete task specification.

Task Overview:
```yaml
{task_overview_yaml}
```

Files Agent Output:
```yaml
{agent_outputs['files']}
```

Functions Agent Output:
```yaml
{agent_outputs['functions']}
```

Formal Verification Agent Output:
```yaml
{agent_outputs['formal']}
```

Tests Agent Output:
```yaml
{agent_outputs['tests']}
```

Task Template Structure:
```yaml
{task_template}
```

Combine all outputs into a single, complete task YAML following the task_template structure. Include:
- task id and name from overview
- context section (expand from overview description)
- files section (from files agent)
- functions section (from functions agent)
- formal_verification section (from formal agent)
- tests section (from tests agent)
- dependencies section (from overview)

Output valid YAML only, no markdown."""

    options = ClaudeAgentOptions(
        system_prompt="You are a task integration specialist. Combine agent outputs into valid YAML.",
        allowed_tools=["Read"],
        permission_mode="bypassPermissions",
    )

    async with ClaudeSDKClient(options=options) as client:
        await client.query(combined_prompt)
        combined_output = await extract_text_response(client)

    combined_output = clean_yaml_response(combined_output)

    print(f"\n[Task {task_id}] Expansion complete")

    # Print the task YAML only in debug mode
    if debug:
        print(f"\n{'='*80}")
        print(f"TASK {task_id}: {task_name}")
        print(f"{'='*80}\n")
        print(combined_output)
        print(f"\n{'='*80}\n")

    return combined_output


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
                    print(f"      ⚠ Warning: Task {task_id} not found in tasks_overview")

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
) -> str:
    """
    For each task in overview, spawn a suborchestrator to expand it.
    Uses AI agent for intelligent batch planning and parallelization.

    Args:
        stream_to_file: If True, write tasks to file immediately to reduce memory usage.
                       Useful for large projects with many tasks.
        debug: If True, print detailed debug information including batches and task YAML.
    """
    print("\n" + "=" * 80)
    print("STEP 2: Suborchestrators - Expand Tasks")
    print("=" * 80 + "\n")

    tasks = parse_tasks_overview(tasks_overview_yaml)

    if not tasks:
        print("✗ No valid tasks found. Aborting.")
        return ""

    print(f"Found {len(tasks)} tasks to expand\n")

    # Generate execution plan using AI agent
    execution_plan_yaml = await generate_execution_plan(tasks_overview_yaml)

    # Print execution plan only in debug mode
    if debug:
        print("\n" + "="*80)
        print("EXECUTION PLAN")
        print("="*80 + "\n")
        print(execution_plan_yaml)
        print("\n" + "="*80 + "\n")

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
    tasks_path = project_root / "tasks.yaml"

    # Open file for streaming if requested
    file_handle = None
    if stream_to_file:
        print(f"Streaming mode: Writing tasks directly to {tasks_path}\n")
        file_handle = open(tasks_path, "w")

    try:
        for batch_num, batch in enumerate(batches, 1):
            print(f"→ Executing Batch {batch_num}/{len(batches)}")

            if len(batch) == 1:
                # Single task - run directly (YAML printed inside suborchestrator)
                expanded = await suborchestrator_expand_task(batch[0], task_template, debug)
                if stream_to_file:
                    if batch_num > 1:
                        file_handle.write("\n---\n")
                    file_handle.write(expanded)
                    file_handle.flush()  # Ensure written immediately
                else:
                    all_expanded.append(expanded)
            else:
                # Multiple tasks - run in parallel (YAML printed inside each suborchestrator as they complete)
                print(f"  Running {len(batch)} tasks in parallel...")
                tasks_coros = [
                    suborchestrator_expand_task(task_doc, task_template, debug)
                    for task_doc in batch
                ]
                expanded_batch = await asyncio.gather(*tasks_coros)
                if stream_to_file:
                    for expanded in expanded_batch:
                        if batch_num > 1 or expanded_batch.index(expanded) > 0:
                            file_handle.write("\n---\n")
                        file_handle.write(expanded)
                    file_handle.flush()
                else:
                    all_expanded.extend(expanded_batch)

            print()

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


async def spawn_reviewer_agent(
    overview_yaml: str,
    detailed_yaml: str,
    impl_md: str,
    task_id: Any,
) -> Dict[str, Any]:
    """
    Spawn a reviewer agent to validate a task group.
    Pre-serialized YAML strings passed for efficiency.
    Returns: {"success": bool, "issues": list, "summary": str}
    """
    print(f"  → Reviewing task {task_id}...")

    system_prompt = """You are an implementation plan reviewer.

Your job is to validate that the detailed task specification matches the overview and aligns with the IMPL.md requirements.

Check for:
1. Completeness: All key components from overview are specified in detail
2. Consistency: Detailed spec aligns with overview purpose and scope
3. Correctness: Implementation approach makes sense for the requirements
4. Testability: Tests adequately cover the functionality
5. Dependencies: External dependencies are properly identified

Report any issues found. If everything looks good, confirm that."""

    prompt = f"""Review this task implementation plan.

# Implementation Requirements (IMPL.md):
```
{impl_md}
```

# Task Overview:
```yaml
{overview_yaml}
```

# Detailed Task Specification:
```yaml
{detailed_yaml}
```

Review the detailed specification against the overview and requirements. Report:
1. Is the detailed spec complete?
2. Does it match the overview's intent?
3. Are there any issues or concerns?
4. Overall assessment: APPROVED or NEEDS_REVISION

Format your response as:
ASSESSMENT: [APPROVED|NEEDS_REVISION]
ISSUES: [List any issues, or "None"]
SUMMARY: [Brief summary]"""

    options = ClaudeAgentOptions(
        system_prompt=system_prompt,
        allowed_tools=["Read"],
        permission_mode="bypassPermissions",
    )

    async with ClaudeSDKClient(options=options) as client:
        await client.query(prompt)
        response = await extract_text_response(client)

    # Parse reviewer response
    approved = "APPROVED" in response and "NEEDS_REVISION" not in response

    # Extract issues (simple parsing)
    issues = []
    if "ISSUES:" in response:
        issues_text = response.split("ISSUES:")[1].split("SUMMARY:")[0].strip()
        if issues_text and issues_text.lower() != "none":
            issues = [issues_text]

    # Extract summary
    summary = response
    if "SUMMARY:" in response:
        summary = response.split("SUMMARY:")[1].strip()

    return {
        "task_id": task_id,
        "success": approved,
        "issues": issues,
        "summary": summary,
    }


async def step3_review_tasks(
    tasks_overview_yaml: str,
    tasks_yaml: str,
    impl_md: str,
) -> List[Dict[str, Any]]:
    """
    Parse both YAMLs, group corresponding tasks, and spawn reviewer agents.
    """
    print("\n" + "=" * 80)
    print("STEP 3: Reviewer Agents - Validate Tasks")
    print("=" * 80 + "\n")

    overview_tasks = parse_tasks_overview(tasks_overview_yaml)
    detailed_tasks = parse_tasks_overview(tasks_yaml)

    print(
        f"Matching {len(overview_tasks)} overview tasks with {len(detailed_tasks)} detailed tasks\n"
    )

    # Group tasks by ID
    task_groups = []
    for overview in overview_tasks:
        overview_id = overview.get("task", {}).get("id")

        # Find matching detailed task
        detailed = None
        for det in detailed_tasks:
            if det.get("task", {}).get("id") == overview_id:
                detailed = det
                break

        if detailed:
            task_groups.append(
                {
                    "overview": overview,
                    "detailed": detailed,
                }
            )
        else:
            print(f"⚠ Warning: No detailed task found for overview task {overview_id}")

    # Spawn reviewer agents in parallel (critical performance optimization)
    print(f"Spawning {len(task_groups)} reviewer agents in parallel...\n")

    review_coros = [
        spawn_reviewer_agent(
            overview_yaml=yaml.dump(group["overview"], default_flow_style=False, sort_keys=False),
            detailed_yaml=yaml.dump(group["detailed"], default_flow_style=False, sort_keys=False),
            impl_md=impl_md,
            task_id=group["overview"].get("task", {}).get("id", "?")
        )
        for group in task_groups
    ]
    review_results = await asyncio.gather(*review_coros)

    return review_results


async def step3_main_orchestrator_report(review_results: List[Dict[str, Any]]):
    """
    Main orchestrator collects all reviewer results and generates final report.
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

    args = parser.parse_args()

    # Load templates
    print("Loading templates...")
    overview_template, task_template = load_templates()

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
        tasks_overview_yaml = await step1_generate_overview(impl_md, overview_template)
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
        tasks_yaml = await step2_expand_all_tasks(
            tasks_overview_yaml, task_template, project_root,
            stream_to_file=args.stream, debug=args.debug
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
        review_results = await step3_review_tasks(
            tasks_overview_yaml, tasks_yaml, impl_md
        )
        await step3_main_orchestrator_report(review_results)


if __name__ == "__main__":
    asyncio.run(main())
