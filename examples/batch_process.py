"""Example: process every file under a folder with a prompt.

Usage:
    python examples/batch_process.py <folder> "<prompt>" [-g GLOB] [-t TOOLS...]

The prompt can reference the file path with {path}.  The agent has tool access
and can read, analyse, and modify files on its own — we only need to point it
at each file.

Use ``-t`` / ``--tools`` to control which tools are available to the agent.
Accepts individual tool names and/or set names:

    Sets:       readonly, filesystem, shell, patch, plan, all
    Tools:      read_file, list_dir, grep_files, apply_patch, update_plan,
                shell, shell_command, exec_command, write_stdin

Each file is processed in an independent session (history is reset between
files) so the agent treats them as unrelated tasks.

Examples:
    # Review (read-only, safe)
    python examples/batch_process.py ./src "Review {path} and suggest improvements." -g "*.py" -t readonly

    # Add docstrings (needs file-write access)
    python examples/batch_process.py ./lib "Add Google-style docstrings to every public function in {path}" -g "*.py" -t filesystem plan

    # Migrate code style (full access)
    python examples/batch_process.py ./src "Refactor {path} to use pathlib instead of os.path" -g "*.py"
"""

from __future__ import annotations

import argparse
import asyncio
import sys
from pathlib import Path
from typing import Any

# Allow running from the repo root: ``python examples/batch_process.py``
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from dotenv import load_dotenv
from nano_coder import NanoCoder


def _on_tool_start(name: str, arguments: dict[str, Any]) -> None:
    print(f"    [Tool] {name}")


def _on_tool_result(name: str, arguments: dict[str, Any], result: dict[str, Any]) -> None:
    if result.get("success"):
        print(f"      ✓ {result.get('message', 'OK')}")
    else:
        print(f"      ✗ {result.get('error', 'Failed')}")


def collect_files(folder: Path, glob_pattern: str) -> list[Path]:
    """Return sorted list of files matching *glob_pattern* under *folder*."""
    return sorted(p for p in folder.rglob(glob_pattern) if p.is_file())


async def run(
    folder: Path,
    prompt_template: str,
    glob_pattern: str = "*",
    tools: list[str] | None = None,
) -> None:
    load_dotenv()

    files = collect_files(folder, glob_pattern)
    if not files:
        print(f"No files matching '{glob_pattern}' found in {folder}")
        return

    print(f"Found {len(files)} file(s) in {folder}  (pattern: {glob_pattern})\n")

    async with NanoCoder(
        tools=tools,
        on_tool_start=_on_tool_start,
        on_tool_result=_on_tool_result,
    ) as coder:
        for idx, filepath in enumerate(files, 1):
            print(f"[{idx}/{len(files)}] {filepath}")

            # Build the prompt — the agent will use its tools to read/modify
            # the file as it sees fit.
            final_prompt = prompt_template.format(path=str(filepath))

            # Reset session so each file is processed independently
            coder.reset()
            response = await coder.prompt(final_prompt)

            print(f"\n  Response:\n  {response}\n")
            print("-" * 60)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Process each file under a folder with NanoCoder.",
    )
    parser.add_argument(
        "folder",
        type=Path,
        help="Root folder to scan for files.",
    )
    parser.add_argument(
        "prompt",
        help="Prompt template. Use {path} for the file path.",
    )
    parser.add_argument(
        "-g", "--glob",
        default="*",
        dest="glob_pattern",
        help="Glob pattern to filter files (default: '*' = all files).",
    )
    parser.add_argument(
        "-t", "--tools",
        nargs="+",
        default=None,
        help=(
            "Tool names and/or set names to enable "
            "(default: all). Sets: readonly, filesystem, shell, patch, plan, all."
        ),
    )
    args = parser.parse_args()

    if not args.folder.is_dir():
        sys.exit(f"Error: {args.folder} is not a directory.")

    asyncio.run(run(args.folder, args.prompt, args.glob_pattern, args.tools))


if __name__ == "__main__":
    main()
