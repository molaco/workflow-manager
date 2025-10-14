#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.8"
# dependencies = [
#     "claude_code_sdk",
#     "anyio",
# ]
# ///

import anyio
from claude_code_sdk import query


async def main():
    async for message in query(prompt="What is 2 + 2?"):
        print(message)


anyio.run(main)
