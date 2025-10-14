#!/usr/bin/env -S sh -c 'unset PYTHONPATH && uv run --script "$0" "$@"'
# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "claude_agent_sdk",
#     "python-dotenv",
#     "pyyaml",
#     "rich",
# ]
# ///

"""
Multi-Agent Task Planning Orchestrator with Rich Console Enhancements
=====================================================================

Enhanced version of tasks.py with:
- Live updating dashboard for parallel task execution (no interleaving!)
- Hierarchical agent visualization with proper indentation
- Styled panels and progress indicators
- Beautiful console output with Rich library

WORKFLOW ARCHITECTURE: Same as tasks.py (3 steps)
See original tasks.py for detailed architecture documentation.
"""

import asyncio
import argparse
import json
import yaml
import sys
import termios
import tty
from pathlib import Path
from typing import Any, Dict, List
from dotenv import load_dotenv
from contextlib import contextmanager

# Rich imports
from rich.console import Console, Group
from rich.panel import Panel
from rich.table import Table
from rich.live import Live
from rich.layout import Layout
from rich.text import Text
from rich.progress import Progress, SpinnerColumn, BarColumn, TextColumn, TaskProgressColumn
from io import StringIO

# Claude SDK imports
from claude_agent_sdk import (
    ClaudeSDKClient,
    ClaudeAgentOptions,
    AssistantMessage,
    UserMessage,
    TextBlock,
    ThinkingBlock,
    ToolUseBlock,
    ToolResultBlock,
    ResultMessage,
    AgentDefinition,
    query,
)

# Load environment variables
load_dotenv()

# Global console instance
console = Console()


# =============================================================================
# Rich Console Enhancement Classes
# =============================================================================


class KeyboardReader:
    """Non-blocking keyboard input reader for asyncio."""

    def __init__(self):
        self.fd = sys.stdin.fileno()
        self.old_settings = None

    def __enter__(self):
        """Set terminal to raw mode."""
        self.old_settings = termios.tcgetattr(self.fd)
        tty.setraw(self.fd)
        return self

    def __exit__(self, *args):
        """Restore terminal settings."""
        if self.old_settings:
            termios.tcsetattr(self.fd, termios.TCSADRAIN, self.old_settings)

    async def read_key(self):
        """Read a single keypress asynchronously."""
        loop = asyncio.get_event_loop()
        reader = asyncio.StreamReader(loop=loop)
        protocol = asyncio.StreamReaderProtocol(reader)
        await loop.connect_read_pipe(lambda: protocol, sys.stdin)

        key = await reader.read(1)
        return key.decode('utf-8') if key else None


class AgentLogger:
    """
    Logger that buffers output for a single task.
    Output is collected in memory and can be rendered together.
    Supports collapsed view (shows only latest line by default).
    Thread-safe for parallel execution.
    """

    def __init__(self, task_id: int = None, collapsed: bool = True):
        self.task_id = task_id
        self.indent = 0
        self.lock = asyncio.Lock()
        self.lines = []  # Buffer for output lines
        self.collapsed = collapsed
        self.scroll_offset = 0  # For scrolling through expanded view

    async def log(self, message: str, indent_override: int = None):
        """Log a message with indentation to buffer."""
        async with self.lock:
            indent = indent_override if indent_override is not None else self.indent
            self.lines.append(f"{'  ' * indent}{message}")

    async def agent_start(self, name: str, task_id: int = None):
        """Start an agent context."""
        prefix = f"[Task {task_id or self.task_id}] " if (task_id or self.task_id) else ""
        await self.log(f"{prefix}→ @{name}")
        async with self.lock:
            self.indent += 1

    async def agent_end(self):
        """End agent context."""
        async with self.lock:
            self.indent = max(0, self.indent - 1)

    async def success(self, message: str):
        """Log success."""
        await self.log(f"✓ {message}")

    async def error(self, message: str):
        """Log error."""
        await self.log(f"✗ {message}")

    async def info(self, message: str):
        """Log info."""
        await self.log(message)

    def toggle_collapsed(self):
        """Toggle between collapsed and expanded view."""
        self.collapsed = not self.collapsed
        # Reset scroll when collapsing
        if self.collapsed:
            self.scroll_offset = 0

    def scroll_up(self, lines: int = 5):
        """Scroll up in the expanded view."""
        if not self.collapsed:
            max_offset = max(0, len(self.lines) - 20)
            self.scroll_offset = min(self.scroll_offset + lines, max_offset)

    def scroll_down(self, lines: int = 5):
        """Scroll down in the expanded view."""
        if not self.collapsed:
            self.scroll_offset = max(0, self.scroll_offset - lines)

    def get_output(self, force_expanded: bool = False, max_lines: int = 20) -> str:
        """Get buffered output as a string (collapsed or expanded)."""
        if not self.lines:
            return "[dim]Waiting...[/dim]"

        if self.collapsed and not force_expanded:
            # Show only the last line (truncate if it has internal newlines)
            last_line = self.lines[-1]
            # Count actual display lines (in case message has newlines)
            display_lines = last_line.split('\n')
            if len(display_lines) > 1:
                return display_lines[-1]  # Just the very last line
            return last_line
        else:
            # Show a window of max_lines entries
            total_lines = len(self.lines)
            if total_lines <= max_lines:
                # All lines fit, show everything
                result = "\n".join(self.lines)
            else:
                # Show max_lines window based on scroll_offset
                # By default (scroll_offset=0), show most recent max_lines
                if self.scroll_offset == 0:
                    # Most recent lines
                    result = "\n".join(self.lines[-max_lines:])
                else:
                    # Scrolled up - show window from end minus offset
                    end_idx = total_lines - self.scroll_offset
                    start_idx = max(0, end_idx - max_lines)
                    result = "\n".join(self.lines[start_idx:end_idx])

            # Safety check: truncate to max_lines if result has more due to internal newlines
            result_lines = result.split('\n')
            if len(result_lines) > max_lines:
                # Keep the most recent max_lines of actual display lines
                result = '\n'.join(result_lines[-max_lines:])

            return result

    def get_renderable(self, is_selected: bool = False):
        """Get output as a Rich renderable with fixed height."""
        # Define content line counts (excluding borders and padding)
        COLLAPSED_LINES = 1   # Just one line of content
        EXPANDED_LINES = 20   # 20 lines of content

        # Get output content
        output = self.get_output()
        if not output:
            output = "[dim]Waiting...[/dim]"

        # Build display with indicators
        task_name = f"Task {self.task_id}" if self.task_id else "Task"
        selection = "[yellow]►[/yellow] " if is_selected else "  "
        collapse_indicator = "[dim][↕][/dim]"

        # Show scroll info if expanded and scrollable
        scroll_info = ""
        if not self.collapsed and len(self.lines) > 20:
            total_lines = len(self.lines)
            if self.scroll_offset == 0:
                scroll_info = f" [dim](latest 20/{total_lines})[/dim]"
            else:
                end_idx = total_lines - self.scroll_offset
                start_idx = max(0, end_idx - 20)
                scroll_info = f" [dim]({start_idx+1}-{end_idx}/{total_lines})[/dim]"

        # Combine header and content
        header = f"{selection}[cyan]{task_name}:[/cyan] {collapse_indicator}{scroll_info}"

        # Split output into lines and control exact count
        output_lines = output.split('\n')
        target_content_lines = COLLAPSED_LINES if self.collapsed else EXPANDED_LINES

        # Truncate or pad output to exact size
        if len(output_lines) > target_content_lines:
            output_lines = output_lines[:target_content_lines]
        else:
            while len(output_lines) < target_content_lines:
                output_lines.append("")

        # Build final display
        all_lines = [header] + output_lines
        padded_content = '\n'.join(all_lines)

        # Return Panel WITHOUT fixed height - let content control size
        return Panel(
            Text.from_markup(padded_content),
            border_style="yellow" if is_selected else "dim",
            padding=(0, 1)  # Horizontal padding for readability
        )


# =============================================================================
# Utility Functions (same as tasks.py)
# =============================================================================


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
    console.print(f"[green]✓[/green] Saved: {output_path}")


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
        console.print(f"\n[red]✗[/red] Error parsing YAML: {e}")
        console.print("Please fix the YAML syntax errors before proceeding.")
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
    console.print()
    console.print(Panel.fit(
        "[bold cyan]STEP 1: Main Orchestrator[/bold cyan]\n"
        "Generate tasks_overview.yaml from IMPL.md",
        border_style="cyan"
    ))
    console.print()

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

    with console.status("[cyan]Generating task overview...", spinner="dots"):
        async with ClaudeSDKClient(options=options) as client:
            await client.query(prompt)
            response, usage_stats = await extract_text_response(client)

    # Print usage stats in a nice box
    stats_text = f"[dim]Duration:[/dim] {usage_stats.get('duration_ms', 0)}ms\n"
    stats_text += f"[dim]Turns:[/dim] {usage_stats.get('num_turns', 0)}\n"
    if usage_stats.get('total_cost_usd'):
        stats_text += f"[dim]Cost:[/dim] ${usage_stats['total_cost_usd']:.4f}\n"
    if usage_stats.get('usage'):
        usage = usage_stats['usage']
        stats_text += f"[dim]Tokens:[/dim] Input: {usage.get('input_tokens', 0)}, Output: {usage.get('output_tokens', 0)}"

    console.print(Panel(stats_text, title="[cyan]Step 1 Statistics[/cyan]", border_style="cyan"))
    console.print()

    return clean_yaml_response(response), usage_stats


# =============================================================================
# STEP 2: Suborchestrators - Expand tasks into detailed specifications
# =============================================================================


async def suborchestrator_expand_task(
    task_overview: Dict[str, Any],
    task_template: str,
    logger: AgentLogger = None,
    debug: bool = False,
) -> tuple[str, dict]:
    """
    Suborchestrator uses Claude with defined sub-agents to expand task.
    Uses simple logger with indentation (Claude Code CLI style).
    Returns tuple of (yaml_content, usage_stats).
    """
    task_id = task_overview.get("task", {}).get("id", "?")
    task_name = task_overview.get("task", {}).get("name", "Unknown")

    # Create a dedicated logger for this task to keep output grouped
    task_logger = AgentLogger(task_id=task_id) if logger is None else logger

    # Log start
    await task_logger.agent_start("suborchestrator", task_id=task_id)
    await task_logger.info(f"Expanding: {task_name}")

    # Pre-serialize task_overview once for efficiency
    task_overview_yaml = yaml.dump(
        task_overview, default_flow_style=False, sort_keys=False
    )

    # Define specialized sub-agents (same as tasks.py)
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

    # System prompt for suborchestrator (same as tasks.py)
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

## YAML FORMATTING REQUIREMENTS (CRITICAL!)
When combining sub-agent outputs into the final YAML, you MUST follow these rules:

1. **All code blocks MUST use literal block syntax with pipe (|)**
2. **Multi-line strings MUST use literal block syntax (| or |-)**
3. **Preserve exact literal block format from sub-agent responses**

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
        include_partial_messages=True,
    )

    # Execute suborchestrator with sub-agents
    response_parts = []
    usage_stats = {}
    agents_invoked = set()

    async for msg in query(prompt=query_prompt, options=options):
        if isinstance(msg, AssistantMessage):
            for block in msg.content:
                if isinstance(block, TextBlock):
                    response_parts.append(block.text)

                    # Detect agent invocations
                    text = block.text
                    for agent_name in ["files", "functions", "formal", "tests"]:
                        if f"@{agent_name}" in text and agent_name not in agents_invoked:
                            agents_invoked.add(agent_name)
                            await task_logger.info(f"→ Delegating to @{agent_name}...")

                    if debug:
                        await task_logger.info(f"[DEBUG] {text[:100]}...")

                elif isinstance(block, ThinkingBlock):
                    if debug:
                        await task_logger.info(f"💭 Thinking: {block.thinking[:100]}...")

                elif isinstance(block, ToolUseBlock):
                    # Show tool use (sub-agent invocation)
                    tool_name = block.name
                    if tool_name.startswith("agent_"):
                        # It's a sub-agent invocation
                        agent_name = tool_name.replace("agent_", "")
                        await task_logger.agent_start(agent_name)
                    elif debug:
                        await task_logger.info(f"🔧 Tool: {tool_name}")

        elif isinstance(msg, UserMessage):
            # UserMessage contains tool results (including sub-agent outputs!)
            for block in msg.content:
                if isinstance(block, ToolResultBlock):
                    if debug:
                        # Show tool result (sub-agent output)
                        result_preview = str(block.content)[:200] if block.content else "None"
                        await task_logger.info(f"📤 Tool result: {result_preview}...")

                    # Check if this is end of a sub-agent
                    if block.tool_use_id:
                        await task_logger.agent_end()

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

    # Log completion
    await task_logger.success(f"Expansion complete ({usage_stats.get('duration_ms', 0)}ms)")
    await task_logger.agent_end()

    return combined_output, usage_stats


def generate_execution_plan_simple(
    tasks: List[Dict[str, Any]],
    batch_size: int = 5,
) -> str:
    """
    Generate a simple execution plan by chunking tasks into fixed-size batches.
    (Same as tasks.py)
    """
    console.print()
    console.print(Panel.fit(
        f"[bold yellow]Batch Planning[/bold yellow]\n"
        f"Simple batching with size={batch_size}",
        border_style="yellow"
    ))
    console.print()

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
    (Same as tasks.py but with rich styling)
    """
    console.print()
    console.print(Panel.fit(
        "[bold yellow]Batch Planning[/bold yellow]\n"
        "Analyzing dependencies with AI agent",
        border_style="yellow"
    ))
    console.print()

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

    with console.status("[yellow]Analyzing dependencies...", spinner="dots"):
        async with ClaudeSDKClient(options=options) as client:
            await client.query(prompt)
            response, _ = await extract_text_response(client)

    return clean_yaml_response(response)


def parse_execution_plan(
    execution_plan_yaml: str, tasks: List[Dict[str, Any]], debug: bool = False
) -> List[List[Dict[str, Any]]]:
    """
    Parse execution_plan.yaml and convert to batch structure.
    (Same as tasks.py)
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
            console.print(f"[dim]DEBUG: Parsing {len(plan_batches)} batches from execution plan[/dim]\n")

        for batch_def in plan_batches:
            batch_id = batch_def.get("batch_id", "?")
            batch_tasks = []
            task_refs = batch_def.get("tasks", [])

            if debug:
                console.print(f"[dim]  Batch {batch_id}: {len(task_refs)} tasks[/dim]")

            for task_ref in task_refs:
                task_id = task_ref.get("task_id")
                task_name = task_ref.get("task_name", "Unknown")
                if debug:
                    console.print(f"[dim]    - Task {task_id}: {task_name}[/dim]")

                if task_id in task_by_id:
                    batch_tasks.append(task_by_id[task_id])
                else:
                    console.print(
                        f"[yellow]⚠[/yellow] Warning: Task {task_id} not found in tasks_overview"
                    )

            if batch_tasks:
                batches.append(batch_tasks)
            if debug:
                console.print()

        return batches

    except Exception as e:
        console.print(f"[yellow]⚠[/yellow] Error parsing execution plan: {e}")
        console.print("Falling back to simple dependency analysis")
        return build_execution_batches_fallback(tasks)


def build_execution_batches_fallback(
    tasks: List[Dict[str, Any]],
) -> List[List[Dict[str, Any]]]:
    """
    Fallback: Simple dependency analysis if execution plan fails.
    (Same as tasks.py)
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
            console.print("[yellow]⚠[/yellow] Warning: Circular dependency detected or unresolved dependencies")
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
    Uses simple logger with indentation (Claude Code CLI style).
    """
    console.print("\n=== STEP 2: Suborchestrators - Expand Tasks ===\n")

    tasks = parse_tasks_overview(tasks_overview_yaml)

    if not tasks:
        console.print("✗ No valid tasks found. Aborting.")
        return ""

    console.print(f"Found {len(tasks)} tasks to expand\n")

    # Generate execution plan - either simple or AI-based
    if simple_batching:
        execution_plan_yaml = generate_execution_plan_simple(tasks, batch_size)
    else:
        execution_plan_yaml = await generate_execution_plan(tasks_overview_yaml)

    # Print execution plan only in debug mode
    if debug:
        console.print("\n--- Execution Plan ---")
        console.print(execution_plan_yaml)
        console.print()

    # Parse execution plan into batches
    batches = parse_execution_plan(execution_plan_yaml, tasks, debug)

    console.print(f"Execution plan: {len(batches)} batch(es)")
    if debug:
        for i, batch in enumerate(batches, 1):
            task_ids = [t.get("task", {}).get("id") for t in batch]
            if len(batch) == 1:
                console.print(f"  Batch {i}: Task {task_ids[0]} (sequential)")
            else:
                console.print(f"  Batch {i}: Tasks {task_ids} (parallel)")
    console.print()

    # Execute batches sequentially, tasks within batch in parallel
    all_expanded = []
    all_usage_stats = []
    tasks_path = project_root / "tasks.yaml"

    # Open file for streaming if requested
    file_handle = None
    if stream_to_file:
        console.print(f"Streaming mode: Writing tasks directly to {tasks_path}\n")
        file_handle = open(tasks_path, "w")

    try:
        for batch_num, batch in enumerate(batches, 1):
            console.print(f"\n→ Executing Batch {batch_num}/{len(batches)}")

            # Always use Live display for consistency (even single tasks)
            # This ensures Panel rendering works properly
            num_tasks = len(batch)
            task_label = "task" if num_tasks == 1 else "tasks"
            console.print(f"  Running {num_tasks} {task_label}...\n")

            # Create loggers for each task (collapsed by default)
            task_loggers = {
                task_doc.get("task", {}).get("id"): AgentLogger(
                    task_id=task_doc.get("task", {}).get("id"),
                    collapsed=True
                )
                for task_doc in batch
            }

            # Track currently selected task for scrolling
            selected_task_index = [0]  # Use list to allow modification in nested function

            # Start tasks with live display and keyboard input
            def create_display():
                """Create a Group of task renderables."""
                sel_idx = selected_task_index[0]
                help_text = f"[dim]Keys: 1-9=select/toggle | ↑/k=scroll up | ↓/j=scroll down | r=refresh | Selected: Task {batch[sel_idx].get('task', {}).get('id', '?')}[/dim]"
                renderables = [Text(help_text, style="dim")]
                for i, task_doc in enumerate(batch, 1):
                    task_id = task_doc.get("task", {}).get("id")
                    logger = task_loggers[task_id]
                    # Pass selection state to get_renderable
                    is_selected = (i - 1 == sel_idx)
                    renderables.append(logger.get_renderable(is_selected=is_selected))
                return Group(*renderables)

            with Live(create_display(), refresh_per_second=4, console=console) as live:
                # Create coroutines with loggers
                tasks_coros = [
                    suborchestrator_expand_task(
                        task_doc,
                        task_template,
                        task_loggers[task_doc.get("task", {}).get("id")],
                        debug
                    )
                    for task_doc in batch
                ]

                # Update display periodically
                async def update_display():
                    while True:
                        try:
                            live.update(create_display(), refresh=True)
                        except Exception:
                            pass
                        await asyncio.sleep(0.25)

                # Handle keyboard input (non-blocking)
                async def handle_keyboard():
                    try:
                        loop = asyncio.get_event_loop()
                        reader = asyncio.StreamReader(loop=loop)
                        protocol = asyncio.StreamReaderProtocol(reader)

                        # Set stdin to non-blocking
                        old_settings = termios.tcgetattr(sys.stdin.fileno())
                        tty.setcbreak(sys.stdin.fileno())

                        try:
                            await loop.connect_read_pipe(lambda: protocol, sys.stdin)

                            while True:
                                try:
                                    # Read one character with timeout
                                    char = await asyncio.wait_for(reader.read(1), timeout=0.5)
                                    if not char:
                                        continue

                                    key = char.decode('utf-8', errors='ignore')

                                    # Handle number keys (1-9) - select task and toggle expand
                                    if key.isdigit():
                                        task_index = int(key) - 1
                                        if 0 <= task_index < len(batch):
                                            selected_task_index[0] = task_index
                                            task_id = batch[task_index].get("task", {}).get("id")
                                            if task_id in task_loggers:
                                                task_loggers[task_id].toggle_collapsed()
                                                # Force immediate refresh to prevent ghost space
                                                live.update(create_display(), refresh=True)
                                                await asyncio.sleep(0.05)  # Small delay for update

                                    # Handle arrow keys
                                    elif key == '\x1b':  # ESC sequence (arrow keys)
                                        # Read next two chars for arrow key sequence
                                        try:
                                            next_chars = await asyncio.wait_for(reader.read(2), timeout=0.1)
                                            sel_idx = selected_task_index[0]
                                            if 0 <= sel_idx < len(batch):
                                                task_id = batch[sel_idx].get("task", {}).get("id")
                                                if task_id in task_loggers:
                                                    if next_chars == b'[A':  # Up arrow
                                                        task_loggers[task_id].scroll_up()
                                                        await asyncio.sleep(0.05)
                                                    elif next_chars == b'[B':  # Down arrow
                                                        task_loggers[task_id].scroll_down()
                                                        await asyncio.sleep(0.05)
                                        except asyncio.TimeoutError:
                                            pass

                                    # Handle vim-style keys (k=up, j=down)
                                    elif key in ('k', 'j'):
                                        sel_idx = selected_task_index[0]
                                        if 0 <= sel_idx < len(batch):
                                            task_id = batch[sel_idx].get("task", {}).get("id")
                                            if task_id in task_loggers:
                                                if key == 'k':
                                                    task_loggers[task_id].scroll_up()
                                                else:  # key == 'j'
                                                    task_loggers[task_id].scroll_down()
                                                await asyncio.sleep(0.05)

                                    # Handle refresh key (r or R)
                                    elif key in ('r', 'R'):
                                        # Clear console and force full redraw
                                        console.clear()
                                        live.update(create_display(), refresh=True)
                                        await asyncio.sleep(0.05)

                                except asyncio.TimeoutError:
                                    # Timeout is normal - just keep waiting for input
                                    continue
                                except asyncio.CancelledError:
                                    # Task was cancelled - exit gracefully
                                    break
                        finally:
                            termios.tcsetattr(sys.stdin.fileno(), termios.TCSADRAIN, old_settings)
                    except asyncio.CancelledError:
                        # Cancellation is expected when tasks complete
                        pass
                    except Exception:
                        # Suppress other errors during cleanup
                        pass

                # Run display updater, keyboard handler, and tasks concurrently
                display_task = asyncio.create_task(update_display())
                keyboard_task = asyncio.create_task(handle_keyboard())
                expanded_batch = await asyncio.gather(*tasks_coros)

                # Cancel background tasks
                display_task.cancel()
                keyboard_task.cancel()
                try:
                    await display_task
                except asyncio.CancelledError:
                    pass
                try:
                    await keyboard_task
                except asyncio.CancelledError:
                    pass

                # Final update
                live.update(create_display())

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

            console.print()

        # Print aggregate usage stats
        console.print("\n=== Step 2 Aggregate Statistics ===")
        total_duration = sum(s.get('duration_ms', 0) for s in all_usage_stats)
        total_turns = sum(s.get('num_turns', 0) for s in all_usage_stats)
        total_cost = sum(s.get('total_cost_usd', 0) or 0 for s in all_usage_stats)

        console.print(f"Total tasks expanded: {len(all_usage_stats)}")
        console.print(f"Total duration: {total_duration}ms ({total_duration/1000:.1f}s)")
        console.print(f"Total turns: {total_turns}")
        if total_cost > 0:
            console.print(f"Total cost: ${total_cost:.4f}")
        console.print()

        # Return combined YAML or empty string if streaming
        if stream_to_file:
            return ""  # Already written to file
        else:
            combined = "\n---\n".join(all_expanded)
            return combined

    finally:
        if file_handle:
            file_handle.close()
            console.print(f"✓ Tasks streamed to: {tasks_path}\n")


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
    (Same logic as tasks.py but with rich console output)
    """
    console.print(f"[cyan]→[/cyan] [Batch {batch_num}] Suborchestrator starting...")

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

    # System prompt for suborchestrator (same as tasks.py)
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

    # Build query prompt
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
                        console.print(f"[dim]{block.text}[/dim]")
                    # Show delegation progress
                    if "@reviewer" in block.text:
                        console.print(f"[dim]  [Batch {batch_num}] → Delegating to @reviewer agent...[/dim]")
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
        console.print(f"\n[dim][Batch {batch_num}] Raw suborchestrator output:[/dim]")
        console.print(f"[dim]{combined_output}[/dim]\n")

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

        console.print(f"[green]✓[/green] [Batch {batch_num}] Parsed {len(results)} review results")
        console.print(f"[dim]  Duration: {usage_stats.get('duration_ms', 0)}ms, Turns: {usage_stats.get('num_turns', 0)}[/dim]")
        if usage_stats.get('total_cost_usd'):
            console.print(f"[dim]  Cost: ${usage_stats['total_cost_usd']:.4f}[/dim]")

        # Attach usage stats to results for aggregation
        for result in results:
            result['_usage_stats'] = usage_stats

        return results

    except json.JSONDecodeError as e:
        console.print(f"[yellow]⚠[/yellow] Warning: Failed to parse JSON from suborchestrator: {e}")
        console.print(f"[dim]Attempted to parse: {json_str[:200]}...[/dim]\n")

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
    Coordinator function that orchestrates the review process.
    (Same logic as tasks.py but with rich console output)
    """
    console.print()
    console.print(Panel.fit(
        "[bold cyan]STEP 3: Batched Review[/bold cyan]\n"
        "Validate expanded tasks with @reviewer agents",
        border_style="cyan"
    ))
    console.print()

    overview_tasks = parse_tasks_overview(tasks_overview_yaml)
    detailed_tasks = parse_tasks_overview(tasks_yaml)

    console.print(
        f"[blue]ℹ[/blue] Matching {len(overview_tasks)} overview tasks with {len(detailed_tasks)} detailed tasks\n"
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
            console.print(f"[yellow]⚠[/yellow] Warning: No detailed task found for overview task {overview_id}")

    # Create batches
    batches = []
    for i in range(0, len(task_groups), batch_size):
        batches.append(task_groups[i : i + batch_size])

    console.print(f"[blue]ℹ[/blue] Created {len(batches)} batch(es) with batch_size={batch_size}\n")

    # Process each batch with a suborchestrator
    all_review_results = []

    for batch_num, batch in enumerate(batches, 1):
        console.print(f"\n[bold cyan]→ Processing Review Batch {batch_num}/{len(batches)}[/bold cyan]")
        task_ids = [g["overview"].get("task", {}).get("id") for g in batch]
        console.print(f"[dim]  Tasks in batch: {task_ids}[/dim]\n")

        batch_results = await review_suborchestrator(
            batch=batch,
            impl_md=impl_md,
            tasks_overview_yaml=tasks_overview_yaml,
            task_template=task_template,
            batch_num=batch_num,
            debug=debug,
        )

        all_review_results.extend(batch_results)
        console.print(f"[green]✓[/green] Batch {batch_num} review complete\n")

    return all_review_results


async def step3_main_orchestrator_report(review_results: List[Dict[str, Any]]):
    """
    Report generator function that produces final review summary.
    (Same logic as tasks.py but with rich console output)
    """
    console.print()
    console.print(Panel.fit(
        "[bold cyan]FINAL REPORT[/bold cyan]\n"
        "Main Orchestrator Summary",
        border_style="cyan"
    ))
    console.print()

    approved_count = sum(1 for r in review_results if r["success"])
    needs_revision_count = len(review_results) - approved_count

    console.print(f"Total tasks reviewed: {len(review_results)}")
    console.print(f"[green]✓[/green] Approved: {approved_count}")
    console.print(f"[red]✗[/red] Needs revision: {needs_revision_count}\n")

    if needs_revision_count > 0:
        console.print("[yellow]Tasks requiring revision:[/yellow]\n")
        for result in review_results:
            if not result["success"]:
                console.print(f"  [red]Task {result['task_id']}:[/red]")
                for issue in result["issues"]:
                    console.print(f"    [dim]- {issue}[/dim]")
                console.print(f"    [dim]Summary: {result['summary']}[/dim]\n")
    else:
        console.print("[green]✓ All tasks approved! Ready for implementation.[/green]\n")

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

    console.print(f"[green]✓[/green] Full report saved to: {report_path}")


# =============================================================================
# Main Workflow
# =============================================================================


async def main():
    parser = argparse.ArgumentParser(
        description="Multi-agent task planning orchestrator with Rich console enhancements"
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
        nargs='+',
        help="Path(s) to implementation file(s) - can specify multiple files (default: auto-detect IMPL.md)",
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

    # Print header
    console.print()
    console.print(Panel.fit(
        "[bold cyan]Multi-Agent Task Planning Orchestrator[/bold cyan]\n"
        "Powered by Claude Agent SDK + Rich Console",
        border_style="cyan",
        padding=(1, 2)
    ))
    console.print()

    # Load templates based on which step is running
    overview_template = None
    task_template = None

    # Load overview template if needed (step 1 or all)
    if args.step in ["1", "all"]:
        if not args.tasks_overview_template:
            console.print("[red]✗[/red] Error: --tasks-overview-template is required for step 1")
            return
        overview_template_path = Path(args.tasks_overview_template)
        if not overview_template_path.exists():
            console.print(
                f"[red]✗[/red] Error: tasks_overview_template.yaml not found at {overview_template_path}"
            )
            return
        console.print("[blue]ℹ[/blue] Loading tasks_overview_template...")
        overview_template = load_template(overview_template_path)

    # Load task template if needed (step 2, 3, or all)
    if args.step in ["2", "3", "all"]:
        if not args.task_template:
            console.print("[red]✗[/red] Error: --task-template is required for steps 2 and 3")
            return
        task_template_path = Path(args.task_template)
        if not task_template_path.exists():
            console.print(f"[red]✗[/red] Error: task_template.yaml not found at {task_template_path}")
            return
        console.print("[blue]ℹ[/blue] Loading task_template...")
        task_template = load_template(task_template_path)

    project_root = Path(__file__).parent.parent

    # Load IMPL.md only if needed (step 1 or step 3)
    impl_md = None
    if args.step in ["1", "3", "all"]:
        if args.impl:
            # Handle multiple implementation files
            impl_parts = []
            for impl_file in args.impl:
                impl_path = Path(impl_file)
                if not impl_path.exists():
                    console.print(f"[red]✗[/red] Error: Implementation file not found at {impl_path}")
                    return
                console.print(f"[blue]ℹ[/blue] Loading {impl_path.name}...")
                with open(impl_path, "r") as f:
                    content = f.read()
                    # Add separator with filename for clarity when multiple files
                    if len(args.impl) > 1:
                        impl_parts.append(f"# Source: {impl_path.name}\n\n{content}")
                    else:
                        impl_parts.append(content)

            impl_md = "\n\n---\n\n".join(impl_parts)
        else:
            try:
                impl_md = load_impl_md()
            except FileNotFoundError as e:
                console.print(f"[red]✗[/red] Error: {e}")
                console.print("Please create IMPL.md or specify path with --impl")
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
            console.print(
                f"[red]✗[/red] Error: tasks_overview.yaml not found at {overview_path}. Run step 1 first or specify with --tasks-overview."
            )
            return
        with open(overview_path, "r") as f:
            tasks_overview_yaml = f.read()

    if args.step in ["2", "all"]:
        # Step 2: Expand tasks
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
            console.print("\n[red]✗[/red] No tasks generated. Not saving tasks.yaml")
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
            console.print(
                f"[red]✗[/red] Error: tasks.yaml not found at {tasks_path}. Run step 2 first or specify with --tasks."
            )
            return
        with open(tasks_path, "r") as f:
            tasks_yaml = f.read()

    if args.step in ["3", "all"]:
        # Step 3: Review tasks
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
