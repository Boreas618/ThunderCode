# ThunderCode

A minimal, terminal-based AI coding agent. ThunderCode provides an interactive REPL where you can chat with an LLM that has access to file system operations, shell commands, and intelligent patching capabilities.

## Features

- **Interactive REPL** — Conversational interface for coding assistance
- **Tool-Equipped Agent** — LLM can read/write files, execute shell commands, and apply patches
- **Session Management** — Maintains conversation history with tool call tracking
- **Flexible Model Support** — Works with any OpenAI-compatible API endpoint

## Tools

ThunderCode provides the following tools to the LLM:

| Tool | Description |
|------|-------------|
| `read_file` | Read files with line numbers, supports slice and indentation-aware modes |
| `list_dir` | List directory contents with depth traversal |
| `grep_files` | Search files by regex pattern with glob filtering |
| `apply_patch` | Apply patches to add, update, or delete files |
| `update_plan` | Track task progress with step-by-step planning |
| `shell` | Execute commands via `execvp()` with array arguments |
| `shell_command` | Execute shell commands as strings in the user's default shell |
| `exec_command` | Run commands in a PTY for interactive sessions |
| `write_stdin` | Write to existing PTY sessions |

## Installation

Requires Python 3.12+.

```bash
# Clone the repository
git clone https://github.com/yourusername/ThunderCode.git
cd ThunderCode

# Install dependencies with uv
uv sync

# Or with pip
pip install -e .
```

## Configuration

Create a `.env` file based on the example:

```bash
cp .env.example .env
```

Edit `.env` with your API credentials:

```env
OPENAI_API_KEY=your-api-key-here
OPENAI_BASE_URL=your-base-url-here
```

You can also set the `MODEL` environment variable to specify which model to use (defaults to `aws/anthropic/claude-opus-4-5`).

## Usage

```bash
# Run with uv
uv run python thunder_code.py

# Or directly
python thunder_code.py
```

Once running, type your prompts at the `>>>` prompt:

```
>>> List all Python files in this directory
[Tool] list_dir
  ✓ OK

>>> Create a hello world script
[Tool] apply_patch
  ✓ Added file: hello.py
```

Press `Ctrl+C` or `Ctrl+D` to exit.

## Project Structure

```
ThunderCode/
├── thunder_code.py      # Main entry point
├── session.py         # Conversation session management
├── system_prompt.md   # System prompt for the LLM
├── tools/
│   ├── __init__.py    # Tool registry and factory
│   ├── base.py        # Base Tool and ToolKit classes
│   ├── fs.py          # File system tools (read, list, grep)
│   ├── apply_patch.py # Patch application tool
│   ├── plan.py        # Task planning tool
│   └── shell.py       # Shell execution tools
├── pyproject.toml     # Project configuration
└── uv.lock            # Dependency lock file
```

## Architecture

- **Session** — Manages the conversation history and orchestrates tool execution loops
- **ToolKit** — Registry that holds tools and converts them to OpenAI function schemas
- **Tool** — Abstract base class defining the interface for all tools

The main loop:
1. User enters a prompt
2. Session sends messages to the LLM with available tools
3. LLM responds with text and/or tool calls
4. Tool calls are executed and results added to history
5. Loop continues until LLM responds without tool calls

## Extending

To add a custom tool:

```python
from tools.base import Tool

class MyTool(Tool):
    @property
    def name(self) -> str:
        return "my_tool"
    
    @property
    def description(self) -> str:
        return "Description of what my tool does"
    
    @property
    def parameters(self) -> dict:
        return {
            "type": "object",
            "properties": {
                "arg1": {"type": "string", "description": "First argument"}
            },
            "required": ["arg1"]
        }
    
    def execute(self, arg1: str) -> dict:
        # Tool implementation
        return {"success": True, "result": f"Processed: {arg1}"}
```

Register it in `tools/__init__.py`:

```python
def create_default_toolkit() -> ToolKit:
    toolkit = ToolKit()
    # ... existing tools ...
    toolkit.register(MyTool())
    return toolkit
```

## License

MIT
