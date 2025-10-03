#!/usr/bin/env -S sh -c 'unset PYTHONPATH && uv run --script "$0" "$@"'
# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "claude_agent_sdk",
#     "rich",
#     "python-dotenv",
# ]
# ///

"""
Multi-Agent Orchestration Example for Code Development

This script demonstrates a pipeline of specialized agents working together:
1. Planner Agent - Creates implementation plan
2. Implementation Agent - Writes the code
3. Testing Agent - Creates tests
4. Review Agent - Reviews and provides feedback

Each agent has different tool permissions and focuses on a specific task.
"""

import asyncio
import json
from pathlib import Path
from dotenv import load_dotenv
from claude_agent_sdk import (
    ClaudeSDKClient,
    ClaudeAgentOptions,
    AssistantMessage,
    TextBlock,
    ResultMessage,
)
from rich.console import Console
from rich.panel import Panel
from rich.markdown import Markdown

load_dotenv()
console = Console()


class AgentOrchestrator:
    """Orchestrates multiple specialized agents in a workflow."""

    def __init__(self, workspace_dir: Path):
        self.workspace = workspace_dir
        self.workspace.mkdir(exist_ok=True)
        self.results = {}

    def save_result(self, agent_name: str, content: str):
        """Save agent output to workspace."""
        file_path = self.workspace / f"{agent_name}_output.md"
        file_path.write_text(content)
        self.results[agent_name] = content
        console.print(f"[dim]Saved {agent_name} output to {file_path}[/dim]")

    def load_context(self, *agent_names: str) -> str:
        """Load previous agent outputs as context."""
        context_parts = []
        for name in agent_names:
            if name in self.results:
                context_parts.append(f"## {name.title()} Output:\n\n{self.results[name]}")
        return "\n\n".join(context_parts)

    async def run_agent(
        self,
        name: str,
        system_prompt: str,
        user_prompt: str,
        allowed_tools: list[str],
        context_agents: list[str] = None
    ) -> str:
        """Run a specialized agent with specific configuration."""

        console.print(Panel(
            f"[bold]{name}[/bold]\n{system_prompt[:100]}...",
            title=f"🤖 Starting {name}",
            border_style="cyan"
        ))

        # Build full prompt with context from previous agents
        full_prompt = user_prompt
        if context_agents:
            context = self.load_context(*context_agents)
            if context:
                full_prompt = f"{context}\n\n---\n\n{user_prompt}"

        # Configure agent options
        options = ClaudeAgentOptions(
            system_prompt=system_prompt,
            allowed_tools=allowed_tools,
            permission_mode="bypassPermissions",
            max_turns=3,  # Limit conversation length
        )

        # Run agent
        response_parts = []
        async with ClaudeSDKClient(options=options) as client:
            await client.query(full_prompt)

            async for msg in client.receive_messages():
                if isinstance(msg, AssistantMessage):
                    for block in msg.content:
                        if isinstance(block, TextBlock):
                            response_parts.append(block.text)
                            # Show partial output
                            console.print(Markdown(block.text[:200] + "..." if len(block.text) > 200 else block.text))
                elif isinstance(msg, ResultMessage):
                    if msg.total_cost_usd:
                        console.print(f"[dim]Cost: ${msg.total_cost_usd:.4f}[/dim]")
                    break

        result = "\n\n".join(response_parts)
        self.save_result(name, result)

        console.print(Panel(
            f"✓ Completed {name}",
            border_style="green"
        ))
        console.print()

        return result


async def feature_development_workflow(feature_description: str):
    """
    Complete feature development workflow with multiple specialized agents.

    Workflow:
    1. Planner - Analyzes requirements and creates implementation plan
    2. Implementer - Writes the actual code based on plan
    3. Tester - Creates comprehensive tests
    4. Reviewer - Reviews everything and provides feedback
    """

    console.print(Panel(
        f"[bold magenta]Feature Development Workflow[/bold magenta]\n\n{feature_description}",
        title="🚀 Starting Multi-Agent Pipeline",
        border_style="magenta"
    ))
    console.print()

    workspace = Path("./workspace")
    orchestrator = AgentOrchestrator(workspace)

    # Agent 1: Planner
    await orchestrator.run_agent(
        name="planner",
        system_prompt=(
            "You are a technical planning specialist. Create detailed implementation plans "
            "with clear steps, file structure, and technical decisions. Focus on architecture "
            "and design patterns. Be concise but thorough."
        ),
        user_prompt=(
            f"Create an implementation plan for this feature:\n\n{feature_description}\n\n"
            "Include:\n"
            "- Architecture overview\n"
            "- File structure\n"
            "- Key functions/classes needed\n"
            "- Edge cases to handle\n"
            "- Testing considerations"
        ),
        allowed_tools=["Read"],  # Read-only access
    )

    # Agent 2: Implementer
    await orchestrator.run_agent(
        name="implementer",
        system_prompt=(
            "You are a senior software engineer. Write clean, well-documented code "
            "following the provided plan. Use best practices and modern Python patterns. "
            "Create actual files with working code."
        ),
        user_prompt=(
            "Implement the feature based on the plan above. "
            "Write the actual Python code with:\n"
            "- Clear docstrings\n"
            "- Type hints\n"
            "- Error handling\n"
            "- Example usage in comments"
        ),
        allowed_tools=["Read", "Write", "Edit"],
        context_agents=["planner"],
    )

    # Agent 3: Tester
    await orchestrator.run_agent(
        name="tester",
        system_prompt=(
            "You are a QA engineer specializing in test-driven development. "
            "Write comprehensive tests covering happy paths, edge cases, and error scenarios. "
            "Use pytest and follow testing best practices."
        ),
        user_prompt=(
            "Create comprehensive tests for the implemented code. Include:\n"
            "- Unit tests for all functions\n"
            "- Edge case tests\n"
            "- Error handling tests\n"
            "- Integration tests if needed\n"
            "Write actual test files that can be run with pytest."
        ),
        allowed_tools=["Read", "Write", "Edit", "Bash"],
        context_agents=["planner", "implementer"],
    )

    # Agent 4: Reviewer
    await orchestrator.run_agent(
        name="reviewer",
        system_prompt=(
            "You are a code review expert. Analyze code quality, security, performance, "
            "and adherence to best practices. Provide constructive feedback with specific "
            "suggestions for improvement."
        ),
        user_prompt=(
            "Review the implementation and tests. Provide feedback on:\n"
            "- Code quality and readability\n"
            "- Potential bugs or issues\n"
            "- Security concerns\n"
            "- Performance considerations\n"
            "- Test coverage and quality\n"
            "- Suggestions for improvement\n\n"
            "Rate each area (1-5) and provide specific examples."
        ),
        allowed_tools=["Read"],  # Read-only review
        context_agents=["planner", "implementer", "tester"],
    )

    # Summary
    console.print(Panel(
        "[bold green]✓ Workflow Complete![/bold green]\n\n"
        f"All outputs saved to: {workspace.absolute()}\n\n"
        "Files created:\n"
        f"- planner_output.md\n"
        f"- implementer_output.md\n"
        f"- tester_output.md\n"
        f"- reviewer_output.md",
        title="📦 Summary",
        border_style="green"
    ))


async def code_review_workflow(file_path: str):
    """
    Specialized code review workflow with multiple review agents.

    Workflow:
    1. Security Reviewer - Checks for security vulnerabilities
    2. Performance Reviewer - Analyzes performance issues
    3. Style Reviewer - Checks code style and conventions
    4. Consolidator - Merges all reviews into actionable report
    """

    console.print(Panel(
        f"[bold magenta]Code Review Pipeline[/bold magenta]\n\nReviewing: {file_path}",
        title="🔍 Starting Review Workflow",
        border_style="magenta"
    ))
    console.print()

    workspace = Path("./review_workspace")
    orchestrator = AgentOrchestrator(workspace)

    # Read the file to review
    code_content = Path(file_path).read_text()

    # Agent 1: Security Reviewer
    await orchestrator.run_agent(
        name="security_reviewer",
        system_prompt=(
            "You are a security specialist. Analyze code for security vulnerabilities, "
            "including SQL injection, XSS, authentication issues, sensitive data exposure, "
            "and insecure dependencies. Provide severity ratings."
        ),
        user_prompt=(
            f"Review this code for security issues:\n\n```python\n{code_content}\n```\n\n"
            "Provide:\n"
            "- List of security concerns with severity (Critical/High/Medium/Low)\n"
            "- Specific line numbers or code patterns\n"
            "- Recommended fixes"
        ),
        allowed_tools=["Read"],
    )

    # Agent 2: Performance Reviewer
    await orchestrator.run_agent(
        name="performance_reviewer",
        system_prompt=(
            "You are a performance optimization expert. Identify bottlenecks, "
            "inefficient algorithms, memory issues, and opportunities for optimization."
        ),
        user_prompt=(
            f"Analyze this code for performance issues:\n\n```python\n{code_content}\n```\n\n"
            "Identify:\n"
            "- Performance bottlenecks\n"
            "- Inefficient patterns\n"
            "- Memory concerns\n"
            "- Optimization opportunities with expected impact"
        ),
        allowed_tools=["Read"],
    )

    # Agent 3: Style Reviewer
    await orchestrator.run_agent(
        name="style_reviewer",
        system_prompt=(
            "You are a code quality expert focused on readability and maintainability. "
            "Check PEP 8 compliance, naming conventions, documentation, and code organization."
        ),
        user_prompt=(
            f"Review this code for style and maintainability:\n\n```python\n{code_content}\n```\n\n"
            "Evaluate:\n"
            "- PEP 8 compliance\n"
            "- Naming conventions\n"
            "- Documentation quality\n"
            "- Code organization\n"
            "- Readability improvements"
        ),
        allowed_tools=["Read"],
    )

    # Agent 4: Consolidator
    await orchestrator.run_agent(
        name="consolidator",
        system_prompt=(
            "You are a technical lead who consolidates code reviews. Synthesize feedback "
            "from multiple reviewers into a prioritized, actionable report with clear next steps."
        ),
        user_prompt=(
            "Consolidate the reviews above into a single actionable report.\n\n"
            "Create:\n"
            "1. Executive Summary (overall code quality rating)\n"
            "2. Critical Issues (must fix before merge)\n"
            "3. Important Issues (should fix soon)\n"
            "4. Suggestions (nice to have)\n"
            "5. Prioritized Action Items with effort estimates\n\n"
            "Remove duplicate issues and organize by priority."
        ),
        allowed_tools=[],  # No file access, just synthesis
        context_agents=["security_reviewer", "performance_reviewer", "style_reviewer"],
    )

    console.print(Panel(
        f"[bold green]✓ Review Complete![/bold green]\n\n"
        f"Results saved to: {workspace.absolute()}",
        title="📋 Review Summary",
        border_style="green"
    ))


async def bug_fix_workflow(bug_description: str, file_path: str = None):
    """
    Bug diagnosis and fixing workflow.

    Workflow:
    1. Diagnostic Agent - Analyzes bug and finds root cause
    2. Fix Agent - Implements the fix
    3. Verification Agent - Tests the fix
    4. Documentation Agent - Updates docs/changelog
    """

    console.print(Panel(
        f"[bold magenta]Bug Fix Workflow[/bold magenta]\n\n{bug_description}",
        title="🐛 Starting Bug Fix Pipeline",
        border_style="red"
    ))
    console.print()

    workspace = Path("./bugfix_workspace")
    orchestrator = AgentOrchestrator(workspace)

    # Agent 1: Diagnostic
    await orchestrator.run_agent(
        name="diagnostic",
        system_prompt=(
            "You are a debugging specialist. Analyze bug reports, trace root causes, "
            "and identify the exact source of issues. Use logs, stack traces, and code analysis."
        ),
        user_prompt=(
            f"Diagnose this bug:\n\n{bug_description}\n\n"
            + (f"Focus on file: {file_path}\n\n" if file_path else "") +
            "Provide:\n"
            "- Root cause analysis\n"
            "- Affected code sections\n"
            "- Why the bug occurs\n"
            "- Potential side effects of fixing it"
        ),
        allowed_tools=["Read", "Grep", "Bash"],
    )

    # Agent 2: Fix
    await orchestrator.run_agent(
        name="fix",
        system_prompt=(
            "You are a software engineer focused on bug fixes. Implement minimal, "
            "targeted fixes that resolve the issue without introducing new problems."
        ),
        user_prompt=(
            "Implement a fix for the bug based on the diagnostic above.\n\n"
            "Requirements:\n"
            "- Minimal changes\n"
            "- No refactoring (just fix the bug)\n"
            "- Add comments explaining the fix\n"
            "- Consider edge cases"
        ),
        allowed_tools=["Read", "Edit", "Write"],
        context_agents=["diagnostic"],
    )

    # Agent 3: Verification
    await orchestrator.run_agent(
        name="verification",
        system_prompt=(
            "You are a QA engineer. Verify bug fixes by creating tests that fail on the "
            "buggy code and pass on the fixed code. Also check for regressions."
        ),
        user_prompt=(
            "Verify the bug fix:\n\n"
            "1. Create a test that reproduces the original bug\n"
            "2. Confirm the test passes with the fix\n"
            "3. Check for potential regressions\n"
            "4. Run existing tests to ensure nothing broke"
        ),
        allowed_tools=["Read", "Write", "Bash"],
        context_agents=["diagnostic", "fix"],
    )

    # Agent 4: Documentation
    await orchestrator.run_agent(
        name="documentation",
        system_prompt=(
            "You are a technical writer. Document bug fixes clearly for changelog, "
            "commit messages, and any necessary code comments."
        ),
        user_prompt=(
            "Create documentation for this bug fix:\n\n"
            "Generate:\n"
            "1. Changelog entry (user-facing)\n"
            "2. Git commit message (following conventional commits)\n"
            "3. Any necessary inline code comments\n"
            "4. Update to README if needed"
        ),
        allowed_tools=["Read", "Write", "Edit"],
        context_agents=["diagnostic", "fix", "verification"],
    )

    console.print(Panel(
        f"[bold green]✓ Bug Fix Complete![/bold green]\n\n"
        f"Results saved to: {workspace.absolute()}",
        title="✓ Bug Fixed",
        border_style="green"
    ))


async def main():
    """Run example workflows."""

    console.print(Panel(
        "[bold cyan]Multi-Agent Orchestration Examples[/bold cyan]\n\n"
        "Available workflows:\n"
        "1. Feature Development (planner → implementer → tester → reviewer)\n"
        "2. Code Review (security → performance → style → consolidator)\n"
        "3. Bug Fix (diagnostic → fix → verification → documentation)",
        title="🎯 Agent Orchestration Demo",
        border_style="cyan"
    ))
    console.print()

    # Example 1: Feature Development
    console.print("[bold]Example 1: Feature Development Workflow[/bold]\n")
    await feature_development_workflow(
        "Create a simple URL shortener module with:\n"
        "- Function to generate short codes\n"
        "- Function to store URL mappings\n"
        "- Function to retrieve original URLs\n"
        "- Basic validation and error handling"
    )

    console.print("\n" + "="*60 + "\n")

    # Example 2: Code Review (on the script we just created)
    # console.print("[bold]Example 2: Code Review Workflow[/bold]\n")
    # await code_review_workflow("./workspace/url_shortener.py")

    # console.print("\n" + "="*60 + "\n")

    # Example 3: Bug Fix
    # console.print("[bold]Example 3: Bug Fix Workflow[/bold]\n")
    # await bug_fix_workflow(
    #     "Users report that short URLs with special characters cause 500 errors. "
    #     "The error occurs in the retrieve_url function when decoding the short code.",
    #     "./workspace/url_shortener.py"
    # )


if __name__ == "__main__":
    asyncio.run(main())
