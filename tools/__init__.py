"""Tools module for NanoCoder."""

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
    # Factory
    "create_default_toolkit",
]


def create_default_toolkit() -> ToolKit:
    """Create a toolkit with all default tools registered."""
    toolkit = ToolKit()
    # File system tools
    toolkit.register(ReadFileTool())
    toolkit.register(ListDirTool())
    toolkit.register(GrepFilesTool())
    # Patch tool
    toolkit.register(ApplyPatchTool())
    # Plan tool
    toolkit.register(UpdatePlanTool())
    # Shell tools
    toolkit.register(ShellTool())
    toolkit.register(ShellCommandTool())
    toolkit.register(ExecCommandTool())
    toolkit.register(WriteStdinTool())
    return toolkit
