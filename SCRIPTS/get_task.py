#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.8"
# dependencies = [
#     "pyyaml",
# ]
# ///
"""Get a task from tasks.yaml by ID."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any, Iterable, Iterator, Tuple

import yaml

PROJECT_ROOT = Path(__file__).resolve().parents[1]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("task_id", type=int, help="Numeric ID of the task to retrieve")
    parser.add_argument(
        "-f",
        "--file",
        dest="tasks_file",
        type=Path,
        default=Path("tasks.yaml"),
        help="Path to the tasks YAML file (default: tasks.yaml in the project root)",
    )
    return parser.parse_args()


def resolve_tasks_file(path: Path) -> Path:
    candidate = path if path.is_absolute() else PROJECT_ROOT / path
    if candidate.exists():
        return candidate
    return path


def iter_document_text(path: Path) -> Iterator[Tuple[str, bool]]:
    """Yield (document_text, has_trailing_separator) tuples preserving formatting."""
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines(keepends=True)
    current: list[str] = []
    for line in lines:
        if line.strip() == "---":
            yield "".join(current), True
            current = []
        else:
            current.append(line)
    # capture trailing doc (if any content remains or file ends with ---)
    yield "".join(current), False


def iter_task_documents(path: Path) -> Iterable[tuple[dict[str, Any], str, bool]]:
    for doc_text, has_separator in iter_document_text(path):
        if not doc_text.strip():
            continue
        try:
            doc = yaml.safe_load(doc_text)
        except yaml.YAMLError as err:
            raise yaml.YAMLError(f"Failed to parse document: {err}") from err
        if isinstance(doc, dict) and "task" in doc:
            yield doc, doc_text, has_separator


def find_task_by_id(task_id: int, path: Path) -> tuple[str, bool] | None:
    for task_doc, doc_text, has_separator in iter_task_documents(path):
        task_info = task_doc.get("task")
        if isinstance(task_info, dict) and task_info.get("id") == task_id:
            return doc_text, has_separator
    return None


def main() -> int:
    args = parse_args()
    tasks_path = resolve_tasks_file(args.tasks_file)

    if not tasks_path.exists():
        print(f"Tasks file not found: {tasks_path}", file=sys.stderr)
        return 1

    try:
        found = find_task_by_id(args.task_id, tasks_path)
    except yaml.YAMLError as err:
        print(f"Failed to parse {tasks_path}: {err}", file=sys.stderr)
        return 1

    if found is None:
        print(f"No task with id {args.task_id} found in {tasks_path}", file=sys.stderr)
        return 1

    doc_text, has_separator = found
    # Ensure we end with a newline if original doc didn't
    if doc_text and not doc_text.endswith("\n"):
        doc_text = f"{doc_text}\n"
    sys.stdout.write(doc_text)
    if has_separator:
        sys.stdout.write("---\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
