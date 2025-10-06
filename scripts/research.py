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
    ClaudeSDKClient,
    ClaudeAgentOptions,
    AssistantMessage,
    TextBlock,
    ResultMessage,
    HookMatcher,
    HookContext,
    tool,
    create_sdk_mcp_server,
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


async def generate_prompts(objective: str, prompt_writer: str, output_style: str, notify: bool, tts_volume: float) -> dict:
    """Phase 1: Generate research prompts based on objective."""
    print("=" * 80)
    print("PHASE 1: Generating Research Prompts")
    print("=" * 80)

    # Build system prompt
    system_prompt = f"{prompt_writer}\n\n# Output Style\n{output_style}"

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
    async with ClaudeSDKClient(options=options) as client:
        await client.query(f"Generate research prompts for: {objective}")

        response_text = []
        async for msg in client.receive_messages():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        response_text.append(block.text)
                        print(block.text)
            elif isinstance(msg, ResultMessage):
                break

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


async def execute_research_prompt(prompt: dict, notify: bool, tts_volume: float) -> dict:
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
            "append": "IMPORTANT: DO NOT create or write any files. Output all your research findings as text only. Your response will be collected and synthesized later into a final documentation file.",
        },
        "mcp_servers": mcp_servers,
        "allowed_tools": allowed_tools,
        "permission_mode": "bypassPermissions",
    }

    options = ClaudeAgentOptions(**options_dict)

    # Execute research
    async with ClaudeSDKClient(options=options) as client:
        query_text = prompt.get("query", "")
        await client.query(query_text)

        response_text = []
        async for msg in client.receive_messages():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        response_text.append(block.text)
                        print(block.text)
            elif isinstance(msg, ResultMessage):
                break

        return {
            "title": prompt.get("title", "Untitled"),
            "query": query_text,
            "response": "\n".join(response_text),
            "focus": prompt.get("focus", []),
        }


async def synthesize_documentation(objective: str, research_results: list[dict], output_path: Path, notify: bool, tts_volume: float) -> str:
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
    async with ClaudeSDKClient(options=options) as client:
        await client.query(synthesis_prompt)

        async for msg in client.receive_messages():
            if isinstance(msg, AssistantMessage):
                for block in msg.content:
                    if isinstance(block, TextBlock):
                        print(block.text)
            elif isinstance(msg, ResultMessage):
                break

        return ""  # Agent saves file directly, no need to return content


async def main():
    parser = argparse.ArgumentParser(description="Research Agent - Generate prompts, execute research, synthesize documentation")
    parser.add_argument(
        "--input", "-i",
        type=str,
        required=True,
        help="Research objective/question",
    )
    parser.add_argument(
        "--system-prompt", "-s",
        type=str,
        required=True,
        help="Prompt writer system prompt (file path or string)",
    )
    parser.add_argument(
        "--append", "-a",
        type=str,
        required=True,
        help="Output style format (file path or string)",
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
        "--output", "-o",
        type=str,
        help="Output file path for final documentation (default: research_output_TIMESTAMP.md)",
    )
    parser.add_argument(
        "--batch-size",
        type=int,
        default=1,
        help="Number of research prompts to execute in parallel (default: 1 for sequential)",
    )

    args = parser.parse_args()

    # Load prompts
    prompt_writer = load_prompt_file(args.system_prompt)
    output_style = load_prompt_file(args.append)

    # Phase 1: Generate prompts
    prompts_data = await generate_prompts(
        args.input,
        prompt_writer,
        output_style,
        args.notify,
        args.tts_volume
    )

    if not prompts_data.get("prompts"):
        print("No prompts generated. Exiting.")
        return

    print(f"\nGenerated {len(prompts_data['prompts'])} research prompts")

    # Phase 2: Execute research prompts in batches
    research_results = []
    prompts = prompts_data["prompts"]
    total_prompts = len(prompts)
    batch_size = args.batch_size

    if batch_size > 1:
        print(f"Executing in batches of {batch_size}")

    for batch_start in range(0, total_prompts, batch_size):
        batch_end = min(batch_start + batch_size, total_prompts)
        batch = prompts[batch_start:batch_end]

        if batch_size > 1:
            print(f"\n[Batch {batch_start//batch_size + 1}] Executing prompts {batch_start + 1}-{batch_end} of {total_prompts}")
        else:
            print(f"\n[{batch_start + 1}/{total_prompts}] Executing research prompt...")

        # Execute batch in parallel
        batch_tasks = [
            execute_research_prompt(prompt, args.notify, args.tts_volume)
            for prompt in batch
        ]
        batch_results = await asyncio.gather(*batch_tasks)
        research_results.extend(batch_results)

    # Determine output path before Phase 3
    if args.output:
        output_path = Path(args.output)
    else:
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        output_path = Path(f"research_output_{timestamp}.md")

    # Phase 3: Synthesize documentation (agent will save the file directly)
    await synthesize_documentation(
        args.input,
        research_results,
        output_path,
        args.notify,
        args.tts_volume
    )

    print("\n" + "=" * 80)
    print(f"Research complete! Documentation saved to: {output_path}")
    print("=" * 80)


if __name__ == "__main__":
    asyncio.run(main())
