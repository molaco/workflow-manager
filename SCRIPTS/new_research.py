#!/usr/bin/env -S sh -c 'unset PYTHONPATH && uv run --script "$0" "$@"'
# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "claude_agent_sdk",
#     "python-dotenv",
#     "elevenlabs",
#     "rpds-py>=0.20.0",
#     "pydub",
#     "audioop-lts",
#     "pyyaml",
# ]
# ///

import asyncio
import argparse
import json
import os
import yaml
from pathlib import Path
from datetime import datetime
from dotenv import load_dotenv
from claude_agent_sdk import (
    ClaudeAgentOptions,
    AssistantMessage,
    TextBlock,
    ResultMessage,
    HookMatcher,
    HookContext,
    tool,
    create_sdk_mcp_server,
    query,
)
from typing import Any

# Load environment variables
load_dotenv()


# Helper function to play TTS
def play_tts(message: str, volume_reduction_db: float = 10.0) -> dict[str, Any]:
    """Generate and play TTS using ElevenLabs with volume control."""
    try:
        import io
        from pydub import AudioSegment

        api_key = os.getenv("ELEVENLABS_API_KEY")
        if not api_key:
            return {
                "success": False,
                "error": "ELEVENLABS_API_KEY not found in environment",
            }

        from elevenlabs.client import ElevenLabs
        from elevenlabs.play import play

        client = ElevenLabs(api_key=api_key)

        audio_iterator = client.text_to_speech.convert(
            text=message,
            voice_id="vGQNBgLaiM3EdZtxIiuY",
            model_id="eleven_flash_v2_5",
            output_format="mp3_44100_128",
        )

        audio_bytes = b"".join(audio_iterator)
        audio_segment = AudioSegment.from_file(io.BytesIO(audio_bytes), format="mp3")
        quieter_audio = audio_segment - volume_reduction_db
        output_io = io.BytesIO()
        quieter_audio.export(output_io, format="mp3")
        output_io.seek(0)
        play(output_io)

        return {"success": True, "message": f"TTS played: {message}"}
    except Exception as e:
        return {"success": False, "error": str(e)}


@tool("notify_tts", "Send TTS notification", {"message": str})
async def notify_tts(args: dict[str, Any]) -> dict[str, Any]:
    """Generate and play TTS using ElevenLabs."""
    message = args.get("message", "Notification")
    result = play_tts(message)

    if result["success"]:
        return {"content": [{"type": "text", "text": result["message"]}]}
    else:
        return {
            "content": [{"type": "text", "text": f"TTS Error: {result['error']}"}],
            "is_error": True,
        }


tts_server = create_sdk_mcp_server(
    name="elevenlabs_tts", version="1.0.0", tools=[notify_tts]
)


def load_mcp_config(selected_servers=None):
    """Read .mcp.json and return mcp_servers dict and allowed_tools list."""
    mcp_config_path = Path(__file__).parent.parent / ".mcp.json"

    with open(mcp_config_path, "r") as f:
        config = json.load(f)

    mcp_servers = {}
    allowed_tools = []

    for server_name, server_config in config.get("mcpServers", {}).items():
        if selected_servers is not None and server_name not in selected_servers:
            continue

        mcp_servers[server_name] = {
            "command": server_config["command"],
            "args": server_config.get("args", []),
            "env": server_config.get("env", {}),
        }

        allowed_tools.append(f"mcp__{server_name}__*")

    return mcp_servers, allowed_tools


def create_notify_with_tts_hook(volume_reduction_db: float):
    """Create a notify_with_tts hook with specified volume reduction."""

    async def notify_with_tts(
        input_data: dict[str, Any], tool_use_id: str | None, context: HookContext
    ) -> dict[str, Any]:
        """Trigger TTS notification for Stop/SubagentStop events."""
        tool_name = input_data.get("tool_name", "")
        message = f"Research step completed"
        result = play_tts(message, volume_reduction_db)
        if not result["success"]:
            print(f"TTS Error: {result['error']}")
        return {}

    return notify_with_tts


def load_prompt_file(file_path: str) -> str:
    """Load prompt content from file path or use as literal string."""
    prompt_path = Path(file_path)
    if prompt_path.exists() and prompt_path.is_file():
        try:
            with open(prompt_path, "r") as f:
                return f.read()
        except Exception as e:
            print(f"Error reading prompt file: {e}")
            return file_path
    else:
        return file_path


async def analyze_codebase(codebase_path: Path) -> dict:
    """Phase 0: Analyze codebase structure."""
    print("=" * 80)
    print("PHASE 0: Analyzing Codebase Structure")
    print("=" * 80)

    analysis_prompt = f"""Analyze the codebase at {codebase_path} and provide a structured overview.

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
Provide analysis as YAML with this structure:

```yaml
statistics:
  total_files: <number>
  by_extension:
    <ext>: <count>
  estimated_loc: <number>
  test_files: <count>

structure:
  root: <path>
  major_directories:
    - name: <dir_name>
      purpose: <brief description>
      file_count: <number>

entry_points:
  executables: [<list>]
  configs: [<list>]
  documentation: [<list>]

dependencies:
  external: [<list of key deps>]
  internal_modules: [<list>]
  frameworks: [<list>]

architecture:
  project_type: <library|application|monorepo|workspace>
  organization: <description>
  patterns: [<list>]

key_components:
  - module: <name>
    purpose: <description>
    location: <path>
```

Be concise but comprehensive. Focus on information useful for understanding the codebase structure."""

    options = ClaudeAgentOptions(
        system_prompt="You are a codebase analyst. Provide concise structural analysis.",
        allowed_tools=["Read", "Glob", "Grep", "Bash"],
        permission_mode="bypassPermissions",
    )

    response_text = []
    async for msg in query(prompt=analysis_prompt, options=options):
        if isinstance(msg, AssistantMessage):
            for block in msg.content:
                if isinstance(block, TextBlock):
                    response_text.append(block.text)
                    print(block.text)

    # Parse the YAML response
    full_response = "\n".join(response_text)

    # Extract YAML from markdown code blocks if present
    if "```yaml" in full_response:
        yaml_start = full_response.find("```yaml") + 7
        yaml_end = full_response.find("```", yaml_start)
        yaml_content = full_response[yaml_start:yaml_end].strip()
    elif "```" in full_response:
        yaml_start = full_response.find("```") + 3
        yaml_end = full_response.find("```", yaml_start)
        yaml_content = full_response[yaml_start:yaml_end].strip()
    else:
        yaml_content = full_response

    try:
        analysis_data = yaml.safe_load(yaml_content)
        return analysis_data
    except yaml.YAMLError as e:
        print(f"Error parsing YAML: {e}")
        print(f"Raw response: {yaml_content}")
        return {}


async def generate_prompts(
    objective: str,
    codebase_analysis: dict,
    prompt_writer: str,
    output_style: str,
    notify: bool,
    tts_volume: float,
) -> dict:
    """Phase 1: Generate research prompts based on objective."""
    print("=" * 80)
    print("PHASE 1: Generating Research Prompts")
    print("=" * 80)

    # Build system prompt with codebase analysis
    system_prompt = f"{prompt_writer}\n\n# Codebase Analysis\n{yaml.dump(codebase_analysis)}\n\n# Output Style\n{output_style}"

    # Setup options
    options_dict = {
        "system_prompt": system_prompt,
        "allowed_tools": ["Read", "Glob", "Grep"],
        "permission_mode": "bypassPermissions",
    }

    # Add TTS if enabled
    if notify:
        mcp_servers = {"tts": tts_server}
        allowed_tools = ["mcp__tts__notify_tts"]
        notify_hook = create_notify_with_tts_hook(tts_volume)
        hooks_config = {
            "Stop": [HookMatcher(hooks=[notify_hook])],
            "SubagentStop": [HookMatcher(hooks=[notify_hook])],
        }
        options_dict["mcp_servers"] = mcp_servers
        options_dict["allowed_tools"].extend(allowed_tools)
        options_dict["hooks"] = hooks_config

    options = ClaudeAgentOptions(**options_dict)

    # Execute prompt generation
    response_text = []
    async for msg in query(
        prompt=f"Generate research prompts for: {objective}", options=options
    ):
        if isinstance(msg, AssistantMessage):
            for block in msg.content:
                if isinstance(block, TextBlock):
                    response_text.append(block.text)
                    print(block.text)

    # Parse the YAML response
    full_response = "\n".join(response_text)

    # Extract YAML from markdown code blocks if present
    if "```yaml" in full_response:
        yaml_start = full_response.find("```yaml") + 7
        yaml_end = full_response.find("```", yaml_start)
        yaml_content = full_response[yaml_start:yaml_end].strip()
    elif "```" in full_response:
        yaml_start = full_response.find("```") + 3
        yaml_end = full_response.find("```", yaml_start)
        yaml_content = full_response[yaml_start:yaml_end].strip()
    else:
        yaml_content = full_response

    try:
        prompts_data = yaml.safe_load(yaml_content)
        return prompts_data
    except yaml.YAMLError as e:
        print(f"Error parsing YAML: {e}")
        print(f"Raw response: {yaml_content}")
        return {"objective": objective, "prompts": []}


async def execute_research_prompt(
    prompt: dict, notify: bool, tts_volume: float
) -> dict:
    """Phase 2: Execute a single research prompt with claude_code preset."""
    print("\n" + "-" * 80)
    print(f"EXECUTING: {prompt.get('title', 'Untitled')}")
    print("-" * 80)

    # Load MCP config
    mcp_servers, allowed_tools = load_mcp_config()

    # Setup options with claude_code preset
    options_dict = {
        "system_prompt": {
            "type": "preset",
            "preset": "claude_code",
            "append": "IMPORTANT: DO NOT create or write any files. Output all your research findings as yaml only. Your response will be collected and synthesized later into a final documentation file.",
        },
        "mcp_servers": mcp_servers,
        "allowed_tools": allowed_tools,
        "permission_mode": "bypassPermissions",
    }

    options = ClaudeAgentOptions(**options_dict)

    # Execute research
    query_text = prompt.get("query", "")
    response_text = []
    async for msg in query(prompt=query_text, options=options):
        if isinstance(msg, AssistantMessage):
            for block in msg.content:
                if isinstance(block, TextBlock):
                    response_text.append(block.text)
                    print(block.text)

    # Parse the YAML response
    full_response = "\n".join(response_text)

    # Extract YAML from markdown code blocks if present
    if "```yaml" in full_response:
        yaml_start = full_response.find("```yaml") + 7
        yaml_end = full_response.find("```", yaml_start)
        yaml_content = full_response[yaml_start:yaml_end].strip()
    elif "```" in full_response:
        yaml_start = full_response.find("```") + 3
        yaml_end = full_response.find("```", yaml_start)
        yaml_content = full_response[yaml_start:yaml_end].strip()
    else:
        yaml_content = full_response

    # Try to parse as YAML, fallback to raw text if it fails
    try:
        research_data = yaml.safe_load(yaml_content)
        # If successfully parsed as YAML, use it
        if isinstance(research_data, dict):
            response_content = research_data
        else:
            # If YAML parsed but not a dict, use raw text
            response_content = yaml_content
    except yaml.YAMLError:
        # If YAML parsing fails, use raw text
        response_content = yaml_content

    return {
        "title": prompt.get("title", "Untitled"),
        "query": query_text,
        "response": response_content,
        "focus": prompt.get("focus", []),
    }


async def synthesize_documentation(
    objective: str,
    research_results: list[dict],
    output_path: Path,
    notify: bool,
    tts_volume: float,
) -> str:
    """Phase 3: Synthesize all research into comprehensive documentation."""
    print("\n" + "=" * 80)
    print("PHASE 3: Synthesizing Documentation")
    print("=" * 80)

    # Build context from all research results
    research_context = f"# Research Objective\n{objective}\n\n"
    research_context += "# Research Findings\n\n"

    for i, result in enumerate(research_results, 1):
        research_context += f"## Finding {i}: {result['title']}\n\n"
        research_context += f"**Query:** {result['query']}\n\n"
        research_context += f"**Response:**\n{result['response']}\n\n"
        research_context += "---\n\n"

    # Setup synthesis prompt
    synthesis_prompt = f"""Based on the research findings below, create a comprehensive documentation that:

1. Synthesizes all findings into a cohesive narrative
2. Provides clear, actionable insights
3. Includes code examples and technical details where relevant
4. Organizes information logically with proper sections
5. Serves as both user documentation and agent context

{research_context}

Generate a well-structured markdown document and save it to {output_path}"""

    # Load MCP config
    mcp_servers, allowed_tools = load_mcp_config()

    # Setup options
    options_dict = {
        "system_prompt": {
            "type": "preset",
            "preset": "claude_code",
            "append": "You are a technical writer creating comprehensive documentation from research findings.",
        },
        "mcp_servers": mcp_servers,
        "allowed_tools": allowed_tools,
        "permission_mode": "bypassPermissions",
    }

    # Add TTS if enabled
    if notify:
        options_dict["mcp_servers"]["tts"] = tts_server
        options_dict["allowed_tools"].append("mcp__tts__notify_tts")
        notify_hook = create_notify_with_tts_hook(tts_volume)
        hooks_config = {
            "Stop": [HookMatcher(hooks=[notify_hook])],
        }
        options_dict["hooks"] = hooks_config

    options = ClaudeAgentOptions(**options_dict)

    # Execute synthesis
    async for msg in query(prompt=synthesis_prompt, options=options):
        if isinstance(msg, AssistantMessage):
            for block in msg.content:
                if isinstance(block, TextBlock):
                    print(block.text)

    return ""  # Agent saves file directly, no need to return content


async def main():
    parser = argparse.ArgumentParser(
        description="Research Agent - Analyze codebase, generate prompts, execute research, synthesize documentation"
    )
    parser.add_argument(
        "--input",
        "-i",
        type=str,
        help="Research objective/question (required for phase 1, optional for other phases)",
    )
    parser.add_argument(
        "--system-prompt",
        "-s",
        type=str,
        help="Prompt writer system prompt (file path or string) (required for phase 1)",
    )
    parser.add_argument(
        "--append",
        "-a",
        type=str,
        help="Output style format (file path or string) (required for phase 1)",
    )
    parser.add_argument(
        "--notify",
        action="store_true",
        help="Enable TTS notifications",
    )
    parser.add_argument(
        "--tts-volume",
        type=float,
        default=10.0,
        help="TTS volume reduction in decibels (default: 10.0)",
    )
    parser.add_argument(
        "--output",
        "-o",
        type=str,
        help="Output file path for final documentation (default: research_output_TIMESTAMP.md)",
    )
    parser.add_argument(
        "--batch-size",
        type=int,
        default=1,
        help="Number of research prompts to execute in parallel (default: 1 for sequential)",
    )
    parser.add_argument(
        "--phases",
        type=str,
        default="0,1,2,3",
        help="Comma-separated phases to execute (0=analyze, 1=prompts, 2=research, 3=synthesize). Default: 0,1,2,3",
    )
    parser.add_argument(
        "--analysis-file",
        type=str,
        help="Path to saved codebase analysis YAML (for resuming from Phase 1)",
    )
    parser.add_argument(
        "--prompts-file",
        type=str,
        help="Path to saved prompts YAML (for resuming from Phase 2)",
    )
    parser.add_argument(
        "--results-file",
        type=str,
        help="Path to saved research results YAML (for resuming from Phase 3)",
    )
    parser.add_argument(
        "--dir",
        type=str,
        help="Directory path to analyze for Phase 0 (default: current working directory)",
    )

    args = parser.parse_args()

    # Parse phases to execute
    phases_to_run = [int(p.strip()) for p in args.phases.split(",")]

    # Validate required arguments based on phases
    if 1 in phases_to_run:
        if not args.input:
            parser.error("--input is required when running phase 1")
        if not args.system_prompt:
            parser.error("--system-prompt is required when running phase 1")
        if not args.append:
            parser.error("--append is required when running phase 1")

    # Load prompts if needed
    prompt_writer = load_prompt_file(args.system_prompt) if args.system_prompt else ""
    output_style = load_prompt_file(args.append) if args.append else ""

    codebase_analysis = {}
    prompts_data = {}
    research_results = []

    # Phase 0: Analyze codebase
    if 0 in phases_to_run:
        codebase_path = Path(args.dir) if args.dir else Path.cwd()
        codebase_analysis = await analyze_codebase(codebase_path)

        # Save analysis to file
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        analysis_path = Path(f"codebase_analysis_{timestamp}.yaml")
        with open(analysis_path, "w") as f:
            yaml.dump(codebase_analysis, f)
        print(f"\n[Phase 0] Analysis saved to: {analysis_path}")
    elif args.analysis_file:
        # Load existing analysis
        with open(args.analysis_file, "r") as f:
            codebase_analysis = yaml.safe_load(f)
        print(f"\n[Phase 0] Loaded analysis from: {args.analysis_file}")

    # Phase 1: Generate prompts
    if 1 in phases_to_run:
        prompts_data = await generate_prompts(
            args.input,
            codebase_analysis,
            prompt_writer,
            output_style,
            args.notify,
            args.tts_volume,
        )

        if not prompts_data.get("prompts"):
            print("No prompts generated. Exiting.")
            return

        # Save prompts to file
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        prompts_path = Path(f"research_prompts_{timestamp}.yaml")
        with open(prompts_path, "w") as f:
            yaml.dump(prompts_data, f)
        print(f"\n[Phase 1] Prompts saved to: {prompts_path}")
        print(f"Generated {len(prompts_data['prompts'])} research prompts")
    elif args.prompts_file:
        # Load existing prompts
        with open(args.prompts_file, "r") as f:
            prompts_data = yaml.safe_load(f)
        print(f"\n[Phase 1] Loaded prompts from: {args.prompts_file}")
        print(f"Loaded {len(prompts_data.get('prompts', []))} research prompts")

    # Phase 2: Execute research prompts in batches
    if 2 in phases_to_run:
        prompts = prompts_data.get("prompts", [])
        if not prompts:
            print("No prompts to execute. Exiting.")
            return

        total_prompts = len(prompts)
        batch_size = args.batch_size

        if batch_size > 1:
            print(f"Executing in batches of {batch_size}")

        for batch_start in range(0, total_prompts, batch_size):
            batch_end = min(batch_start + batch_size, total_prompts)
            batch = prompts[batch_start:batch_end]

            if batch_size > 1:
                print(
                    f"\n[Batch {batch_start//batch_size + 1}] Executing prompts {batch_start + 1}-{batch_end} of {total_prompts}"
                )
            else:
                print(
                    f"\n[{batch_start + 1}/{total_prompts}] Executing research prompt..."
                )

            # Execute batch in parallel
            batch_tasks = [
                execute_research_prompt(prompt, args.notify, args.tts_volume)
                for prompt in batch
            ]
            batch_results = await asyncio.gather(*batch_tasks)
            research_results.extend(batch_results)

        # Save research results to file
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        results_path = Path(f"research_results_{timestamp}.yaml")
        with open(results_path, "w") as f:
            yaml.dump(research_results, f, default_flow_style=False, sort_keys=False)
        print(f"\n[Phase 2] Results saved to: {results_path}")
    elif args.results_file:
        # Load existing results
        with open(args.results_file, "r") as f:
            research_results = yaml.safe_load(f)
        print(f"\n[Phase 2] Loaded results from: {args.results_file}")

    # Phase 3: Synthesize documentation
    if 3 in phases_to_run:
        if not research_results:
            print("No research results to synthesize. Exiting.")
            return

        # Determine output path before Phase 3
        if args.output:
            output_path = Path(args.output)
        else:
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            output_path = Path(f"research_output_{timestamp}.md")

        # Use objective from args if provided, otherwise extract from prompts_data or use placeholder
        objective = (
            args.input
            if args.input
            else prompts_data.get("objective", "Research Objective")
        )

        await synthesize_documentation(
            objective, research_results, output_path, args.notify, args.tts_volume
        )

        print("\n" + "=" * 80)
        print(f"Research complete! Documentation saved to: {output_path}")
        print("=" * 80)
    else:
        print("\n" + "=" * 80)
        print("Selected phases completed!")
        print("=" * 80)


if __name__ == "__main__":
    asyncio.run(main())
