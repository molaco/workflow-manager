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
        print(f"Error parsing YAML: {e}")
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

    # Clean response (remove markdown code blocks if present)
    if "```yaml" in response:
        response = response.split("```yaml")[1].split("```")[0].strip()
    elif "```" in response:
        response = response.split("```")[1].split("```")[0].strip()

    return response


# =============================================================================
# STEP 2: Suborchestrators - Expand tasks into detailed specifications
# =============================================================================


async def spawn_specialized_agent(
    task_overview: Dict[str, Any],
    task_template: str,
    agent_type: str,
    agent_prompt: str,
) -> str:
    """
    Spawn a specialized agent (files, functions, formal, tests).
    """
    print(f"  └─ Spawning {agent_type} agent...")

    task_yaml = yaml.dump(task_overview, default_flow_style=False, sort_keys=False)

    prompt = f"""You are a {agent_type} specialist working on task expansion.

Task Overview:
```yaml
{task_yaml}
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
) -> str:
    """
    Suborchestrator spawns 4 specialized agents and combines their outputs.
    """
    task_id = task_overview.get("task", {}).get("id", "?")
    task_name = task_overview.get("task", {}).get("name", "Unknown")

    print(f"\n→ Suborchestrator for Task {task_id}: {task_name}")

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

    # Spawn agents sequentially
    agent_outputs = {}
    for agent_type, agent_prompt in agent_specs.items():
        output = await spawn_specialized_agent(
            task_overview, task_template, agent_type, agent_prompt
        )
        agent_outputs[agent_type] = output

    # Combine outputs into complete task
    print(f"  └─ Combining agent outputs into complete task...")

    combined_prompt = f"""Combine the following agent outputs into a complete task specification.

Task Overview:
```yaml
{yaml.dump(task_overview, default_flow_style=False, sort_keys=False)}
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

    # Clean output
    if "```yaml" in combined_output:
        combined_output = combined_output.split("```yaml")[1].split("```")[0].strip()
    elif "```" in combined_output:
        combined_output = combined_output.split("```")[1].split("```")[0].strip()

    print(f"✓ Task {task_id} expansion complete\n")

    return combined_output


async def step2_expand_all_tasks(
    tasks_overview_yaml: str,
    task_template: str,
) -> str:
    """
    For each task in overview, spawn a suborchestrator to expand it.
    """
    print("\n" + "=" * 80)
    print("STEP 2: Suborchestrators - Expand Tasks")
    print("=" * 80 + "\n")

    tasks = parse_tasks_overview(tasks_overview_yaml)
    print(f"Found {len(tasks)} tasks to expand\n")

    expanded_tasks = []

    # Process tasks sequentially (can be parallelized if needed)
    for task_doc in tasks:
        expanded = await suborchestrator_expand_task(task_doc, task_template)
        expanded_tasks.append(expanded)

    # Combine into multi-document YAML
    combined = "\n---\n".join(expanded_tasks)

    return combined


# =============================================================================
# STEP 3: Reviewer Agents - Validate expanded tasks
# =============================================================================


async def spawn_reviewer_agent(
    task_group: Dict[str, Any],
    impl_md: str,
) -> Dict[str, Any]:
    """
    Spawn a reviewer agent to validate a task group.
    Returns: {"success": bool, "issues": list, "summary": str}
    """
    overview_task = task_group["overview"]
    detailed_task = task_group["detailed"]

    task_id = overview_task.get("task", {}).get("id", "?")
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

    overview_yaml = yaml.dump(overview_task, default_flow_style=False, sort_keys=False)
    detailed_yaml = yaml.dump(detailed_task, default_flow_style=False, sort_keys=False)

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

    # Spawn reviewer agents
    review_results = []
    for group in task_groups:
        result = await spawn_reviewer_agent(group, impl_md)
        review_results.append(result)

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

    args = parser.parse_args()

    # Load templates
    print("Loading templates...")
    overview_template, task_template = load_templates()

    # Load IMPL.md
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

    project_root = Path(__file__).parent.parent

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
        overview_path = project_root / "tasks_overview.yaml"
        if not overview_path.exists():
            print("Error: tasks_overview.yaml not found. Run step 1 first.")
            return
        with open(overview_path, "r") as f:
            tasks_overview_yaml = f.read()

    if args.step in ["2", "all"]:
        # Step 2: Expand tasks
        tasks_yaml = await step2_expand_all_tasks(tasks_overview_yaml, task_template)
        tasks_path = project_root / "tasks.yaml"
        save_yaml(tasks_yaml, tasks_path)

        if args.step == "2":
            return
    else:
        # Load existing detailed tasks
        tasks_path = project_root / "tasks.yaml"
        if not tasks_path.exists():
            print("Error: tasks.yaml not found. Run step 2 first.")
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
