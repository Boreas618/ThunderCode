"""Tools module for NanoCoder."""

from __future__ import annotations

from collections.abc import Sequence

from tools.base import Tool, ToolKit
from tools.apply_patch import ApplyPatchTool
from tools.fs import ReadFileTool, ListDirTool, GrepFilesTool
from tools.plan import UpdatePlanTool
from tools.shell import ShellTool, ShellCommandTool, ExecCommandTool, WriteStdinTool

__all__ = [
    # Base
    "Tool",
    "ToolKit",
    # File system tools
    "ReadFileTool",
    "ListDirTool",
    "GrepFilesTool",
    # Patch tool
    "ApplyPatchTool",
    # Plan tool
    "UpdatePlanTool",
    # Shell tools
    "ShellTool",
    "ShellCommandTool",
    "ExecCommandTool",
    "WriteStdinTool",
    # Registry & sets
    "TOOL_REGISTRY",
    "TOOL_SETS",
    "resolve_tool_names",
    # Factory
    "create_default_toolkit",
]

# ---------------------------------------------------------------------------
# Tool registry: maps each tool name to its class.
# ---------------------------------------------------------------------------

TOOL_REGISTRY: dict[str, type[Tool]] = {
    "read_file": ReadFileTool,
    "list_dir": ListDirTool,
    "grep_files": GrepFilesTool,
    "apply_patch": ApplyPatchTool,
    "update_plan": UpdatePlanTool,
    "shell": ShellTool,
    "shell_command": ShellCommandTool,
    "exec_command": ExecCommandTool,
    "write_stdin": WriteStdinTool,
}

# ---------------------------------------------------------------------------
# Named tool sets: logical groupings that can be referenced by name.
# ---------------------------------------------------------------------------

TOOL_SETS: dict[str, list[str]] = {
    "readonly": ["read_file", "list_dir", "grep_files"],
    "filesystem": ["read_file", "list_dir", "grep_files", "apply_patch"],
    "shell": ["shell", "shell_command", "exec_command", "write_stdin"],
    "patch": ["apply_patch"],
    "plan": ["update_plan"],
    "all": list(TOOL_REGISTRY.keys()),
}


def resolve_tool_names(tools: Sequence[str]) -> list[str]:
    """Expand a mix of tool set names and individual tool names into a flat,
    deduplicated list of tool names.

    Raises ``ValueError`` for unknown entries.

    Examples::

        resolve_tool_names(["readonly"])
        # -> ["read_file", "list_dir", "grep_files"]

        resolve_tool_names(["readonly", "apply_patch", "plan"])
        # -> ["read_file", "list_dir", "grep_files", "apply_patch", "update_plan"]
    """
    result: list[str] = []
    for entry in tools:
        if entry in TOOL_SETS:
            result.extend(TOOL_SETS[entry])
        elif entry in TOOL_REGISTRY:
            result.append(entry)
        else:
            raise ValueError(
                f"Unknown tool or tool set: {entry!r}. "
                f"Available tools: {sorted(TOOL_REGISTRY)}. "
                f"Available sets: {sorted(TOOL_SETS)}."
            )
    # Deduplicate while preserving order
    seen: set[str] = set()
    deduped: list[str] = []
    for name in result:
        if name not in seen:
            seen.add(name)
            deduped.append(name)
    return deduped


def create_default_toolkit(tools: Sequence[str] | None = None) -> ToolKit:
    """Create a toolkit with the requested tools registered.

    Parameters
    ----------
    tools : sequence of str, optional
        Tool names and/or tool-set names to include.  Accepts any mix of
        individual names (e.g. ``"read_file"``, ``"apply_patch"``) and set
        names (e.g. ``"readonly"``, ``"shell"``, ``"all"``).
        If ``None``, all available tools are registered.

    Examples::

        create_default_toolkit()                           # everything
        create_default_toolkit(["readonly"])                # safe, read-only
        create_default_toolkit(["readonly", "plan"])        # read + planning
        create_default_toolkit(["filesystem", "plan"])      # read/write files + planning
        create_default_toolkit(["read_file", "list_dir"])   # pick individual tools
    """
    if tools is None:
        tool_names = list(TOOL_REGISTRY.keys())
    else:
        tool_names = resolve_tool_names(tools)

    toolkit = ToolKit()
    for name in tool_names:
        toolkit.register(TOOL_REGISTRY[name]())
    return toolkit
