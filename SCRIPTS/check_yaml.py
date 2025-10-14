#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.8"
# dependencies = [
#     "pyyaml",
# ]
# ///
"""
Quick YAML validator for tasks.yaml files.
Usage: ./scripts/check_yaml.py [path/to/file.yaml]
"""

import sys
import yaml
from pathlib import Path


def check_yaml(file_path: str):
    """Parse and validate YAML file, print errors if found."""
    path = Path(file_path)

    if not path.exists():
        print(f"❌ File not found: {file_path}")
        return False

    print(f"📄 Checking: {file_path}")
    print("=" * 60)

    try:
        with open(path, "r") as f:
            content = f.read()

        # Try to parse as multi-document YAML
        docs = list(yaml.safe_load_all(content))

        # Count valid documents
        valid_docs = [doc for doc in docs if doc is not None]

        print(f"✅ YAML is valid!")
        print(f"📊 Total documents: {len(valid_docs)}")

        # Check for task structure
        task_docs = [
            doc for doc in valid_docs if isinstance(doc, dict) and "task" in doc
        ]
        print(f"📋 Task documents: {len(task_docs)}")

        if task_docs:
            print(f"\n📌 Task IDs found:")
            for doc in task_docs:
                task_id = doc.get("task", {}).get("id", "?")
                task_name = doc.get("task", {}).get("name", "Unknown")
                print(f"   - Task {task_id}: {task_name}")

        return True

    except yaml.YAMLError as e:
        print(f"❌ YAML parsing error:\n")
        print(str(e))

        # Try to show context around the error
        if hasattr(e, "problem_mark"):
            mark = e.problem_mark
            print(f"\n📍 Error location:")
            print(f"   Line: {mark.line + 1}")
            print(f"   Column: {mark.column + 1}")

            # Show lines around error
            lines = content.split("\n")
            start = max(0, mark.line - 2)
            end = min(len(lines), mark.line + 3)

            print(f"\n📝 Context (lines {start + 1}-{end}):")
            for i in range(start, end):
                prefix = ">>> " if i == mark.line else "    "
                line_num = f"{i + 1:4d}"
                print(f"{prefix}{line_num} | {lines[i]}")

            if mark.column > 0:
                print(f"         {' ' * mark.column}^")

        return False

    except Exception as e:
        print(f"❌ Unexpected error: {type(e).__name__}")
        print(str(e))
        return False


if __name__ == "__main__":
    # Default to tasks.yaml in current directory
    file_path = sys.argv[1] if len(sys.argv) > 1 else "tasks.yaml"

    success = check_yaml(file_path)
    sys.exit(0 if success else 1)
