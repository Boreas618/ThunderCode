# ThunderCode

A terminal-based AI coding agent written in Rust. ThunderCode provides a full TUI where you can chat with any LLM via an OpenAI-compatible API, with access to file operations, shell commands, code search, and more.

## Features

- **Full TUI** — Rich terminal interface with syntax highlighting, markdown rendering, and vim keybindings
- **Tool-Equipped Agent** — 30+ built-in tools for file I/O, shell, grep, git, MCP, tasks, and more
- **Provider-Neutral** — Works with any OpenAI-compatible endpoint (OpenAI, Ollama, vLLM, Together, OpenRouter, etc.)
- **Session Management** — Conversation history, session resume, cost tracking
- **Extensible** — MCP server support, skills, plugins, custom commands

## Installation

Requires Rust 1.75+.

```bash
git clone https://github.com/user/ThunderCode.git
cd ThunderCode
cargo build --release
```

The binary will be at `target/release/thundercode`.

## Configuration

Set environment variables:

```bash
export THUNDERCODE_API_KEY=your-api-key       # or OPENAI_API_KEY
export THUNDERCODE_BASE_URL=https://api.openai.com  # or OPENAI_BASE_URL
export THUNDERCODE_MODEL=gpt-4o               # or OPENAI_MODEL
```

## Usage

```bash
# Interactive TUI
thundercode

# One-shot prompt
thundercode "List all Rust files in this directory"

# Resume a previous session
thundercode --resume <session-id>
```

## Tools

| Tool | Description |
|------|-------------|
| `Bash` | Execute shell commands |
| `FileRead` | Read files with line ranges |
| `FileWrite` | Create new files |
| `FileEdit` | Apply targeted edits to existing files |
| `Glob` | Find files by pattern |
| `Grep` | Search file contents by regex |
| `Agent` | Spawn sub-agents for parallel work |
| `TaskCreate/Update/List` | Track progress with structured tasks |
| `WebFetch` / `WebSearch` | Fetch URLs and search the web |
| `LSP` | Language Server Protocol integration |
| `NotebookEdit` | Edit Jupyter notebooks |
| `MCP` | Model Context Protocol server tools |
| `Skill` | Invoke user-defined skills |
| `AskUser` | Request clarification from the user |

## Project Structure

```
ThunderCode/
├── src/
│   ├── main.rs          # Entry point, CLI, TUI setup
│   ├── repl.rs          # Main REPL loop
│   ├── display.rs       # TUI rendering
│   ├── init.rs          # Subsystem initialization
│   ├── input.rs         # Input handling
│   ├── api/             # OpenAI-compatible API client
│   ├── auth/            # API key resolution
│   ├── bridge/          # Environment bridge
│   ├── commands/        # Slash commands (/help, /model, etc.)
│   ├── config/          # Settings, paths, themes
│   ├── constants/       # Prompts, limits, product info
│   ├── context/         # System/user context, prompt building
│   ├── coordinator/     # Agent coordination
│   ├── git/             # Git operations (via libgit2)
│   ├── keybindings/     # Keyboard shortcut system
│   ├── mcp/             # Model Context Protocol client
│   ├── memory/          # RULES.md discovery, project memory
│   ├── permissions/     # Tool permission checking
│   ├── plugins/         # Plugin system
│   ├── query/           # Query engine, token budgets
│   ├── remote/          # WebSocket session management
│   ├── services/        # Analytics, compaction, diagnostics
│   ├── session/         # Session persistence and recovery
│   ├── skills/          # Skill discovery and invocation
│   ├── state/           # Application state
│   ├── tasks/           # Background task engine
│   ├── telemetry/       # Cost and usage tracking
│   ├── tools/           # 30+ built-in tools
│   ├── tui/             # Terminal UI components
│   ├── types/           # Shared types and traits
│   ├── utils/           # Formatting, paths, etc.
│   ├── vim/             # Vim mode engine
│   └── voice/           # Voice input support
├── Cargo.toml
└── build.rs
```

## License

MIT
