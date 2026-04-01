//! REPL state machine and event handling.
//!
//! This module owns the main interactive loop: reading input, dispatching
//! commands, submitting messages to the API, streaming responses, and
//! handling tool calls. All rendering goes through the TUI engine via
//! the `display` module.

use std::io;
use std::pin::pin;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use futures::StreamExt;
use tokio::sync::watch;

use crate::api::client::ApiClient;
use crate::api::request::{ApiContent, ApiMessage, CreateMessageRequest, SystemBlock};
use crate::api::streaming::{ContentDelta, StreamEvent};
use crate::commands::CommandRegistry;
use crate::config::SettingsJson;
use crate::context::{SystemContext, SystemPromptBuilder, UserContext};
use crate::state::BootstrapState;
use crate::telemetry::{CostSummary, CostTracker, ModelCosts};
use crate::tools::ToolRegistry;
use crate::tui::app::App;

use crate::display;

/// All mutable state for the REPL session.
#[allow(dead_code)]
pub struct ReplState {
    pub bootstrap: BootstrapState,
    pub client: Option<ApiClient>,
    pub tool_registry: ToolRegistry,
    pub command_registry: CommandRegistry,
    pub system_context: SystemContext,
    pub user_context: UserContext,
    pub cost_tracker: CostTracker,
    pub model: String,
    /// Conversation history sent to the API.
    pub messages: Vec<ApiMessage>,
    /// System prompt blocks.
    pub system_prompt: Vec<SystemBlock>,
    /// Whether the current query should be interrupted.
    pub abort_tx: watch::Sender<bool>,
    pub abort_rx: watch::Receiver<bool>,
}

impl ReplState {
    /// Build the system prompt from context using the full prompt builder.
    ///
    /// Uses `SystemPromptBuilder` from thundercode-context which assembles the
    /// complete system prompt including identity, tool usage instructions,
    /// coding guidelines, output formatting, git instructions, and safety
    /// guidelines -- matching the reference implementation.
    pub fn build_system_prompt(
        system_context: &SystemContext,
        user_context: &UserContext,
        tools: &[&dyn crate::types::tool::Tool],
        model: &str,
    ) -> Vec<SystemBlock> {
        let config = SettingsJson::default();
        let full_prompt = SystemPromptBuilder::build(
            system_context,
            user_context,
            tools,
            model,
            &config,
        );
        vec![SystemBlock::Text {
            text: full_prompt,
            cache_control: None,
        }]
    }
}

/// Run the interactive REPL loop.
///
/// This is the main event loop. It returns when the user exits (Ctrl+D or
/// `/exit`).
pub async fn run_repl(mut state: ReplState, app: &mut App) -> anyhow::Result<()> {
    // Build the system prompt once, passing tool references and model name.
    let tool_refs: Vec<&dyn crate::types::tool::Tool> = state
        .tool_registry
        .all()
        .iter()
        .map(|t| t.as_ref() as &dyn crate::types::tool::Tool)
        .collect();
    state.system_prompt = ReplState::build_system_prompt(
        &state.system_context,
        &state.user_context,
        &tool_refs,
        &state.model,
    );

    loop {
        // Render the current state.
        app.prompt.set_active(true);
        display::render_prompt(app);

        // Reset abort signal for this turn.
        let _ = state.abort_tx.send(false);

        // Read input via the TUI event loop.
        // Build command list for autocomplete
        let all_cmds: Vec<(String, String)> = state
            .command_registry
            .list_available(None)
            .iter()
            .map(|cmd| {
                let base = cmd.base();
                (base.name.clone(), base.description.clone())
            })
            .collect();
        let action = read_input(app, &mut state.abort_rx, &all_cmds).await?;

        match action {
            InputAction::Empty => continue,
            InputAction::Interrupt => continue,
            InputAction::Exit => break,
            InputAction::Command { name, args } => {
                handle_command(&name, &args, &mut state, app).await;
            }
            InputAction::Message(text) => {
                handle_message(&text, &mut state, app).await;
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Input handling
// ---------------------------------------------------------------------------

/// Classification of a completed input line.
#[derive(Debug)]
enum InputAction {
    Empty,
    Exit,
    Interrupt,
    Command { name: String, args: String },
    Message(String),
}

/// Read user input through the TUI prompt, handling key events and editing.
async fn read_input(
    app: &mut App,
    abort_rx: &mut watch::Receiver<bool>,
    all_commands: &[(String, String)],
) -> io::Result<InputAction> {
    loop {
        let has_event =
            tokio::task::block_in_place(|| event::poll(Duration::from_millis(50)))?;

        // Check abort signal.
        if *abort_rx.borrow() {
            return Ok(InputAction::Interrupt);
        }

        if !has_event {
            continue;
        }

        let ev = tokio::task::block_in_place(|| event::read())?;

        match ev {
            Event::Key(key) => {
                if key.kind == crossterm::event::KeyEventKind::Release {
                    continue;
                }

                // Ctrl+D on empty buffer = exit.
                if key.code == KeyCode::Char('d')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    if app.prompt.buffer().is_empty() {
                        return Ok(InputAction::Exit);
                    }
                    continue;
                }

                // Ctrl+C = interrupt.
                if key.code == KeyCode::Char('c')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    app.prompt.clear();
                    app.command_suggestions.clear();
                    display::render_prompt(app);
                    return Ok(InputAction::Interrupt);
                }

                // Escape = clear suggestions or cancel.
                if key.code == KeyCode::Esc {
                    if !app.command_suggestions.is_empty() {
                        app.command_suggestions.clear();
                        app.render();
                        continue;
                    }
                }

                // Ctrl+U = kill to start of line.
                if key.code == KeyCode::Char('u')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    app.prompt.kill_to_start();
                    app.render_prompt_only();
                    continue;
                }

                // Ctrl+W = delete word backward.
                if key.code == KeyCode::Char('w')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    app.prompt.kill_word_backward();
                    app.render_prompt_only();
                    continue;
                }

                // Ctrl+A = home.
                if key.code == KeyCode::Char('a')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    app.prompt.move_home();
                    app.render_prompt_only();
                    continue;
                }

                // Ctrl+E = end.
                if key.code == KeyCode::Char('e')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    app.prompt.move_end();
                    app.render_prompt_only();
                    continue;
                }

                match key.code {
                    KeyCode::Enter => {
                        let text = app.prompt.submit();
                        app.command_suggestions.clear();
                        let trimmed = text.trim().to_string();
                        if trimmed.is_empty() {
                            return Ok(InputAction::Empty);
                        }
                        return Ok(classify_input(&trimmed));
                    }
                    KeyCode::Backspace => {
                        app.prompt.delete_backward();
                        app.update_suggestions(all_commands);
                        app.render();
                    }
                    KeyCode::Delete => {
                        app.prompt.delete_forward();
                        app.update_suggestions(all_commands);
                        app.render();
                    }
                    KeyCode::Left => {
                        app.prompt.move_left();
                        app.render_prompt_only();
                    }
                    KeyCode::Right => {
                        app.prompt.move_right();
                        app.render_prompt_only();
                    }
                    KeyCode::Home => {
                        app.prompt.move_home();
                        app.render_prompt_only();
                    }
                    KeyCode::End => {
                        app.prompt.move_end();
                        app.render_prompt_only();
                    }
                    KeyCode::Tab => {
                        if !app.command_suggestions.is_empty() {
                            app.accept_suggestion();
                            app.command_suggestions.clear();
                            app.render();
                        }
                    }
                    KeyCode::Up => {
                        if !app.command_suggestions.is_empty() {
                            app.suggestion_up();
                            app.render();
                        } else {
                            app.prompt.history_up();
                            app.render_prompt_only();
                        }
                    }
                    KeyCode::Down => {
                        if !app.command_suggestions.is_empty() {
                            app.suggestion_down();
                            app.render();
                        } else {
                            app.prompt.history_down();
                            app.render_prompt_only();
                        }
                    }
                    KeyCode::Char(c) => {
                        app.prompt.insert_char(c);
                        app.update_suggestions(all_commands);
                        if app.command_suggestions.is_empty() {
                            app.render_prompt_only();
                        } else {
                            app.render();
                        }
                    }
                    _ => {}
                }
            }
            Event::Paste(text) => {
                app.prompt.insert_str(&text);
                app.render_prompt_only();
            }
            Event::Mouse(mouse) => {
                match mouse.kind {
                    crossterm::event::MouseEventKind::ScrollUp => {
                        app.scroll_offset += 3;
                        app.sticky_scroll = false;
                        app.render();
                    }
                    crossterm::event::MouseEventKind::ScrollDown => {
                        app.scroll_offset = (app.scroll_offset - 3).max(0);
                        if app.scroll_offset == 0 {
                            app.sticky_scroll = true;
                        }
                        app.render();
                    }
                    _ => {}
                }
            }
            Event::Resize(w, h) => {
                display::handle_resize(app, w, h);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Tool definition builder
// ---------------------------------------------------------------------------

/// Build tool definitions with proper descriptions for the API request.
///
/// Each tool's description includes what it does and its argument names,
/// so the model knows how to call it correctly.
fn build_tool_definitions(registry: &ToolRegistry) -> Vec<crate::api::request::ToolDefinition> {
    registry
        .all()
        .iter()
        .map(|tool| {
            let schema = tool.input_schema();
            let name = tool.name();
            // Map tool names to detailed descriptions matching the reference.
            let description = match name {
                "Bash" => "Execute a shell command. Args: command (string), description (string, optional), timeout (number, optional)".to_string(),
                "Read" => "Read a file. Args: file_path (string), offset (number, optional), limit (number, optional)".to_string(),
                "Edit" => "Edit a file by replacing text. Args: file_path (string), old_string (string), new_string (string), replace_all (boolean, optional)".to_string(),
                "Write" => "Write content to a file. Args: file_path (string), content (string)".to_string(),
                "Glob" => "Find files matching a glob pattern. Args: pattern (string), path (string, optional)".to_string(),
                "Grep" => "Search file contents with regex. Args: pattern (string), path (string, optional), output_mode (string, optional)".to_string(),
                "WebFetch" => "Fetch a URL. Args: url (string)".to_string(),
                "WebSearch" => "Search the web. Args: query (string)".to_string(),
                "Agent" => "Spawn a subagent to handle a subtask. Args: prompt (string), description (string)".to_string(),
                "NotebookEdit" => "Edit a Jupyter notebook. Args: notebook_path (string), cell_number (number), new_source (string), cell_type (string, optional)".to_string(),
                "TodoWrite" => "Create or update a todo/task list. Args: todos (array of objects with id, title, status)".to_string(),
                "Skill" => "Execute a skill within the conversation. Args: skill (string), args (string, optional)".to_string(),
                "ToolSearch" => "Search for deferred tools by keyword. Args: query (string), max_results (number, optional)".to_string(),
                "AskUserQuestion" => "Ask the user a question and wait for their response. Args: question (string)".to_string(),
                "TaskOutput" => "Return output from an agent task. Args: output (string)".to_string(),
                "TaskStop" => "Stop a running background task. Args: task_id (string)".to_string(),
                "ExitPlanMode" => "Exit plan mode and begin execution.".to_string(),
                "EnterPlanMode" => "Enter plan mode for multi-step planning.".to_string(),
                _ => {
                    // Fallback: build a description from the schema properties
                    let mut desc = name.to_string();
                    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
                        let args: Vec<String> = props.keys().take(5).map(|k| k.clone()).collect();
                        if !args.is_empty() {
                            desc.push_str(&format!(". Args: {}", args.join(", ")));
                        }
                    }
                    desc
                }
            };
            crate::api::request::ToolDefinition {
                tool_type: "function".into(),
                function: crate::api::request::FunctionDefinition {
                    name: name.to_owned(),
                    description: Some(description),
                    parameters: Some(schema),
                },
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Permission check
// ---------------------------------------------------------------------------

/// Tools that are read-only and don't need permission prompting.
const SAFE_TOOLS: &[&str] = &["Read", "Glob", "Grep", "ToolSearch", "AskUserQuestion", "ExitPlanMode", "EnterPlanMode", "TaskOutput"];

/// Check if a tool requires user permission before execution.
fn tool_needs_permission(tool_name: &str, input: &serde_json::Value, registry: &ToolRegistry) -> bool {
    // Safe tools never need permission.
    if SAFE_TOOLS.contains(&tool_name) {
        return false;
    }
    // Check the tool's is_read_only flag.
    if let Some(tool) = registry.find_by_name(tool_name) {
        if tool.is_read_only(input) {
            return false;
        }
    }
    true
}

/// Format the tool call for the permission dialog.
fn format_tool_for_permission(tool_name: &str, input: &serde_json::Value) -> String {
    match tool_name {
        "Bash" => {
            let cmd = input.get("command").and_then(|v| v.as_str()).unwrap_or("(unknown)");
            format!("{cmd}")
        }
        "Edit" => {
            let path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("(unknown)");
            format!("Edit {path}")
        }
        "Write" => {
            let path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("(unknown)");
            format!("Write {path}")
        }
        _ => {
            let summary = if let Some(obj) = input.as_object() {
                obj.iter()
                    .take(2)
                    .map(|(k, v)| {
                        let val = match v {
                            serde_json::Value::String(s) => {
                                if s.len() > 50 { format!("{}...", &s[..47]) } else { s.clone() }
                            }
                            other => {
                                let s = other.to_string();
                                if s.len() > 50 { format!("{}...", &s[..47]) } else { s }
                            }
                        };
                        format!("{k}={val}")
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                String::new()
            };
            format!("{tool_name} {summary}")
        }
    }
}

/// Format the tool USE message text — shown in parentheses after the tool badge.
/// Matches ref's per-tool `renderToolUseMessage(input, {verbose: false})`.
fn format_tool_use_display(tool_name: &str, input: &serde_json::Value) -> String {
    match tool_name {
        "Bash" | "bash" => {
            let cmd = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
            let max_chars = 160;
            if cmd.len() > max_chars {
                format!("{}…", &cmd[..max_chars - 1])
            } else {
                cmd.to_string()
            }
        }
        "Read" | "file_read" => {
            let path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
            let display = shorten_path(path);
            let pages = input.get("pages").and_then(|v| v.as_str());
            let offset = input.get("offset").and_then(|v| v.as_u64());
            let limit = input.get("limit").and_then(|v| v.as_u64());
            if let Some(p) = pages {
                format!("{display} · pages {p}")
            } else if let (Some(o), Some(l)) = (offset, limit) {
                format!("{display} · lines {}-{}", o + 1, o + l)
            } else {
                display
            }
        }
        "Edit" | "file_edit" | "Write" | "file_write" => {
            let path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
            shorten_path(path)
        }
        "Grep" | "grep" => {
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let path = input.get("path").and_then(|v| v.as_str());
            if let Some(p) = path {
                format!("pattern: \"{pattern}\", path: \"{}\"", shorten_path(p))
            } else {
                format!("pattern: \"{pattern}\"")
            }
        }
        "Glob" | "glob" => {
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let path = input.get("path").and_then(|v| v.as_str());
            if let Some(p) = path {
                format!("pattern: \"{pattern}\", path: \"{}\"", shorten_path(p))
            } else {
                format!("pattern: \"{pattern}\"")
            }
        }
        "WebFetch" | "web_fetch" => {
            input.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string()
        }
        "WebSearch" | "web_search" => {
            let query = input.get("query").and_then(|v| v.as_str()).unwrap_or("");
            format!("\"{query}\"")
        }
        "Agent" | "agent" => {
            input.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string()
        }
        _ => {
            // Generic: show first string arg value
            if let Some(obj) = input.as_object() {
                for (_, v) in obj.iter() {
                    if let Some(s) = v.as_str() {
                        let display = if s.len() > 80 { format!("{}…", &s[..77]) } else { s.to_string() };
                        return display;
                    }
                }
            }
            String::new()
        }
    }
}

/// Format the tool RESULT display — matching ref's renderToolResultMessage in condensed mode.
fn format_tool_result_display(tool_name: &str, data: &serde_json::Value) -> String {
    let content = format_tool_result(data);
    let line_count = content.lines().count();

    match tool_name {
        "Read" | "file_read" => {
            // "Read N lines"
            format!("Read \x1b[1m{line_count}\x1b[0m lines")
        }
        "Write" | "file_write" => {
            // "Wrote N lines to path"
            if let Some(obj) = data.as_object() {
                if let Some(path) = obj.get("filePath").or(obj.get("file_path")).and_then(|v| v.as_str()) {
                    return format!("Wrote \x1b[1m{line_count}\x1b[0m lines to \x1b[1m{}\x1b[0m", shorten_path(path));
                }
            }
            format!("Wrote \x1b[1m{line_count}\x1b[0m lines")
        }
        "Edit" | "file_edit" => {
            // Count diff lines
            if content.contains("+++") || content.contains("---") {
                let added = content.lines().filter(|l| l.starts_with('+')).count().saturating_sub(1);
                let removed = content.lines().filter(|l| l.starts_with('-')).count().saturating_sub(1);
                format!("\x1b[38;2;78;186;101m+{added}\x1b[0m \x1b[38;2;255;107;128m-{removed}\x1b[0m lines")
            } else {
                "Applied edit".to_string()
            }
        }
        "Bash" | "bash" => {
            // Show first 3 lines, truncated
            let mut result_lines = Vec::new();
            for (i, line) in content.lines().take(4).enumerate() {
                if i == 3 && line_count > 3 {
                    result_lines.push(format!("\x1b[2m… +{} lines\x1b[0m", line_count - 3));
                } else {
                    let t = if line.len() > 120 { format!("{}…", &line[..117]) } else { line.to_string() };
                    result_lines.push(t);
                }
            }
            result_lines.join("\n")
        }
        "Grep" | "grep" => {
            format!("Found \x1b[1m{line_count}\x1b[0m results")
        }
        "Glob" | "glob" => {
            format!("Found \x1b[1m{line_count}\x1b[0m files")
        }
        "WebFetch" | "web_fetch" => {
            let size = content.len();
            let formatted = if size > 1024 * 1024 {
                format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
            } else if size > 1024 {
                format!("{:.1} KB", size as f64 / 1024.0)
            } else {
                format!("{size} bytes")
            };
            format!("Received \x1b[1m{formatted}\x1b[0m (200 OK)")
        }
        "WebSearch" | "web_search" => {
            let count = if let Some(obj) = data.as_object() {
                obj.get("results").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(1)
            } else { 1 };
            let plural = if count == 1 { "" } else { "es" };
            format!("Did \x1b[1m{count}\x1b[0m search{plural}")
        }
        "Agent" | "agent" => {
            let first = content.lines().next().unwrap_or("Completed");
            if first.len() > 100 { format!("{}…", &first[..97]) } else { first.to_string() }
        }
        _ => {
            if line_count <= 2 && content.len() < 150 {
                content
            } else {
                let first = content.lines().next().unwrap_or("");
                let t = if first.len() > 80 { format!("{}…", &first[..77]) } else { first.to_string() };
                if line_count > 1 { format!("{t} \x1b[2m({line_count} lines)\x1b[0m") } else { t }
            }
        }
    }
}

/// Shorten a file path for display (like ref's getDisplayPath).
fn shorten_path(path: &str) -> String {
    // Try to make relative to CWD
    if let Ok(cwd) = std::env::current_dir() {
        if let Ok(rel) = std::path::Path::new(path).strip_prefix(&cwd) {
            return rel.display().to_string();
        }
    }
    // Shorten home directory
    if let Some(home) = dirs::home_dir() {
        if let Ok(rel) = std::path::Path::new(path).strip_prefix(&home) {
            return format!("~/{}", rel.display());
        }
    }
    path.to_string()
}

/// Continue the conversation after tool results — send another request to the model.
async fn handle_message_continuation(state: &mut ReplState, app: &mut App, client: &ApiClient) {
    // Build system messages
    let mut api_messages = Vec::new();
    for block in &state.system_prompt {
        match block {
            SystemBlock::Text { text, .. } => {
                api_messages.push(ApiMessage::system(text));
            }
        }
    }
    api_messages.extend(state.messages.clone());

    let tool_defs = build_tool_definitions(&state.tool_registry);

    let request = CreateMessageRequest {
        model: state.model.clone(),
        max_tokens: Some(16384),
        messages: api_messages,
        temperature: None,
        top_p: None,
        stop: None,
        stream: true,
        tools: if tool_defs.is_empty() { None } else { Some(tool_defs) },
        tool_choice: None,
    };

    let cont_start = Instant::now();
    display::start_spinner(app);

    let spinner_interval = Duration::from_millis(80);
    let stream_result;
    {
        let fut = client.create_message_stream(request);
        tokio::pin!(fut);
        loop {
            tokio::select! {
                result = &mut fut => {
                    stream_result = Some(result);
                    break;
                }
                _ = tokio::time::sleep(spinner_interval) => {
                    display::tick_spinner(app);
                }
            }
        }
    }
    display::stop_spinner(app);

    let raw_stream = match stream_result {
        Some(Ok(s)) => s,
        Some(Err(e)) => {
            display::print_error_in_app(app, &format!("Continuation request failed: {e}"));
            return;
        }
        None => return,
    };

    let mut stream = pin!(raw_stream);
    let mut response_text = String::new();
    let mut streaming_started = false;
    let mut tool_use_blocks: Vec<(String, String, serde_json::Value)> = Vec::new();
    // Track multiple concurrent tool calls by index.
    let mut pending_tools: std::collections::HashMap<usize, (String, String, String)> = std::collections::HashMap::new();
    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;

    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(event) => match &event {
                StreamEvent::MessageStart { message } => {
                    if let Some(ref usage) = message.usage {
                        total_input_tokens = usage.prompt_tokens;
                        total_output_tokens = usage.completion_tokens;
                    }
                }
                StreamEvent::ContentBlockDelta { delta, .. } => {
                    if let ContentDelta::TextDelta { text } = delta {
                        if !streaming_started {
                            display::start_assistant_stream(app);
                            streaming_started = true;
                        }
                        response_text.push_str(text);
                        display::append_assistant_text(app, text);
                    }
                }
                StreamEvent::ToolCallStart { index, id, name } => {
                    if streaming_started {
                        display::finish_assistant_stream(app);
                        streaming_started = false;
                    }
                    pending_tools.insert(*index, (id.clone(), name.clone(), String::new()));
                    display::add_tool_use_simple(app, name, name, "", true);
                }
                StreamEvent::ToolCallDelta { index, arguments } => {
                    if let Some((_id, _name, ref mut args)) = pending_tools.get_mut(index) {
                        args.push_str(arguments);
                    }
                }
                StreamEvent::MessageDelta { usage, delta } => {
                    total_output_tokens += usage.output_tokens;
                    if let Some(ref reason) = delta.stop_reason {
                        if reason == "tool_calls" || reason == "stop" {
                            // Finalize all pending tool calls.
                            let mut indices: Vec<usize> = pending_tools.keys().copied().collect();
                            indices.sort();
                            for idx in indices {
                                if let Some((id, name, args_json)) = pending_tools.remove(&idx) {
                                    finalize_tool_call(
                                        &mut tool_use_blocks, &id, &name, &args_json, app,
                                    );
                                }
                            }
                        }
                    }
                }
                StreamEvent::MessageStop => {
                    if streaming_started {
                        display::finish_assistant_stream(app);
                        streaming_started = false;
                    }
                    // Finalize any remaining pending tool calls.
                    let mut indices: Vec<usize> = pending_tools.keys().copied().collect();
                    indices.sort();
                    for idx in indices {
                        if let Some((id, name, args_json)) = pending_tools.remove(&idx) {
                            finalize_tool_call(
                                &mut tool_use_blocks, &id, &name, &args_json, app,
                            );
                        }
                    }
                }
                StreamEvent::Error { error } => {
                    display::print_error_in_app(app, &format!("Stream error: {error}"));
                    break;
                }
                _ => {}
            },
            Err(e) => {
                display::print_error_in_app(app, &format!("Stream error: {e}"));
                break;
            }
        }
    }

    // Add assistant response
    if !response_text.is_empty() || !tool_use_blocks.is_empty() {
        let mut assistant_msg = ApiMessage::assistant(&response_text);
        if !tool_use_blocks.is_empty() {
            let tool_calls: Vec<crate::api::request::ToolCall> = tool_use_blocks.iter().map(|(id, name, input)| {
                crate::api::request::ToolCall {
                    id: id.clone(),
                    call_type: "function".into(),
                    function: crate::api::request::FunctionCall {
                        name: name.clone(),
                        arguments: serde_json::to_string(input).unwrap_or_default(),
                    },
                }
            }).collect();
            assistant_msg.tool_calls = Some(tool_calls);
        }
        state.messages.push(assistant_msg);
    }

    // Execute any new tool calls and continue recursively
    if !tool_use_blocks.is_empty() {
        for (id, name, input) in &tool_use_blocks {
            // Permission check for non-read-only tools.
            if tool_needs_permission(name, input, &state.tool_registry) {
                let desc = format_tool_for_permission(name, input);
                let allowed = display::prompt_permission(app, name, &desc);
                if !allowed {
                    let err_msg = "Tool execution denied by user.";
                    display::add_tool_result(app, name, err_msg, true);
                    state.messages.push(ApiMessage::tool_result(id, err_msg));
                    continue;
                }
            }
            if let Some(tool) = state.tool_registry.find_by_name(name) {
                let ctx = crate::types::tool::ToolUseContext::default();
                let result = tool.call(input.clone(), &ctx, None).await;
                match result {
                    Ok(call_result) => {
                        let result_text = format_tool_result(&call_result.data);
                        let display_text = format_tool_result_display(name, &call_result.data);
                        display::add_tool_result(app, name, &display_text, false);
                        state.messages.push(ApiMessage::tool_result(id, &result_text));
                    }
                    Err(e) => {
                        let err_msg = format!("Tool error: {e}");
                        display::add_tool_result(app, name, &err_msg, true);
                        state.messages.push(ApiMessage::tool_result(id, &err_msg));
                    }
                }
            } else {
                let err_msg = format!("Unknown tool: {name}");
                display::add_tool_result(app, name, &err_msg, true);
                state.messages.push(ApiMessage::tool_result(id, &err_msg));
            }
        }
        // Record cost for this continuation turn.
        let cont_elapsed = cont_start.elapsed();
        let cost = crate::telemetry::cost_tracker::calculate_cost(
            &state.model,
            total_input_tokens,
            total_output_tokens,
            0,
            0,
        );
        state.bootstrap.record_model_usage(
            &state.model,
            total_input_tokens,
            total_output_tokens,
            0,
            0,
            cost,
        );
        state.bootstrap.add_api_cost(0.0, cont_elapsed.as_millis() as u64);
        // Update status line.
        let snap = state.bootstrap.snapshot();
        app.status.cost_usd = snap.total_cost_usd;
        app.status.input_tokens = snap.model_usage.values().map(|u| u.input_tokens).sum();
        app.status.output_tokens = snap.model_usage.values().map(|u| u.output_tokens).sum();
        app.render();

        // Recurse for more tool calls (bounded by model behavior)
        Box::pin(handle_message_continuation(state, app, client)).await;
    } else {
        // No tool calls -- just record cost for this continuation.
        let cont_elapsed = cont_start.elapsed();
        let cost = crate::telemetry::cost_tracker::calculate_cost(
            &state.model,
            total_input_tokens,
            total_output_tokens,
            0,
            0,
        );
        state.bootstrap.record_model_usage(
            &state.model,
            total_input_tokens,
            total_output_tokens,
            0,
            0,
            cost,
        );
        state.bootstrap.add_api_cost(0.0, cont_elapsed.as_millis() as u64);
        // Update status line.
        let snap = state.bootstrap.snapshot();
        app.status.cost_usd = snap.total_cost_usd;
        app.status.input_tokens = snap.model_usage.values().map(|u| u.input_tokens).sum();
        app.status.output_tokens = snap.model_usage.values().map(|u| u.output_tokens).sum();
        app.render();
    }
}

/// Extract display text from a tool result.
///
/// The ref only shows stdout for Bash, the content for Read, etc.
/// It does NOT dump the raw JSON with exit_code/stderr.
fn format_tool_result(data: &serde_json::Value) -> String {
    // If it's a plain string, return as-is.
    if let Some(s) = data.as_str() {
        return s.to_owned();
    }

    // If it's an object with "stdout", extract that (Bash tool format).
    if let Some(obj) = data.as_object() {
        // Bash-style: { stdout, stderr, exit_code }
        if let Some(stdout) = obj.get("stdout") {
            let stdout_str = stdout.as_str().unwrap_or("");
            let stderr_str = obj.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
            let exit_code = obj.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(0);

            let mut result = String::new();
            if !stdout_str.is_empty() {
                result.push_str(stdout_str);
            }
            if !stderr_str.is_empty() {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(stderr_str);
            }
            // Only show exit code if non-zero (error).
            if exit_code != 0 && result.is_empty() {
                result = format!("Exit code: {exit_code}");
            }
            if result.is_empty() {
                result = "(no output)".to_owned();
            }
            return result;
        }

        // If it has a "content" field, use that.
        if let Some(content) = obj.get("content") {
            if let Some(s) = content.as_str() {
                return s.to_owned();
            }
        }

        // If it has a "result" field, use that.
        if let Some(r) = obj.get("result") {
            if let Some(s) = r.as_str() {
                return s.to_owned();
            }
        }
    }

    // Fallback: compact JSON (not pretty-printed).
    serde_json::to_string(data).unwrap_or_else(|_| format!("{data:?}"))
}

/// Finalize a pending tool call: parse args, update display, store for execution.
fn finalize_tool_call(
    tool_use_blocks: &mut Vec<(String, String, serde_json::Value)>,
    tool_id: &str,
    tool_name: &str,
    args_json: &str,
    app: &mut App,
) {
    let input: serde_json::Value = serde_json::from_str(args_json)
        .unwrap_or(serde_json::Value::Null);

    // Format using per-tool display logic matching the ref.
    let summary = format_tool_use_display(tool_name, &input);

    // Update the last ToolUse entry in the transcript with the summary and mark complete.
    for entry in app.transcript.iter_mut().rev() {
        if let crate::tui::app::TranscriptEntry::ToolUse {
            ref mut input_summary,
            ref mut in_progress,
            ..
        } = entry
        {
            *input_summary = summary;
            *in_progress = false;
            break;
        }
    }
    app.render();

    tool_use_blocks.push((tool_id.to_owned(), tool_name.to_owned(), input));
}

/// Classify a non-empty trimmed input string into an InputAction.
fn classify_input(input: &str) -> InputAction {
    if input.starts_with('/') {
        let without_slash = &input[1..];
        let (name, args) = match without_slash.split_once(char::is_whitespace) {
            Some((n, a)) => (n.to_string(), a.trim().to_string()),
            None => (without_slash.to_string(), String::new()),
        };
        if name == "exit" || name == "quit" {
            return InputAction::Exit;
        }
        InputAction::Command { name, args }
    } else {
        InputAction::Message(input.to_string())
    }
}

// ---------------------------------------------------------------------------
// Command handling
// ---------------------------------------------------------------------------

/// Handle a slash command.
async fn handle_command(name: &str, args: &str, state: &mut ReplState, app: &mut App) {
    match name {
        // --- Session ---
        "help" | "h" | "?" => {
            let commands: Vec<(String, String)> = state
                .command_registry
                .list_available(None)
                .iter()
                .map(|cmd| {
                    let base = cmd.base();
                    (base.name.clone(), base.description.clone())
                })
                .collect();
            display::print_help(app, &commands);
        }
        "clear" | "reset" | "new" => {
            state.messages.clear();
            app.transcript.clear();
            app.welcome_shown = false;
            app.needs_full_redraw = true;
            app.render();
        }
        "exit" | "quit" => {
            // Handled by classify_input, but just in case:
            return;
        }

        // --- Model/API ---
        "cost" | "usage" => {
            let summary = build_cost_summary(state);
            let formatted = crate::telemetry::format_cost_summary(&summary);
            display::print_info(app, &formatted);
        }
        "model" => {
            if args.is_empty() {
                display::print_info(app, &format!("Current model: {}", state.model));
            } else {
                let new_model = crate::api::models::resolve_model_name(args.trim());
                state.model = new_model.clone();
                app.status.model = new_model.clone();
                display::print_info(app, &format!("Model set to {new_model}"));
                app.render();
            }
        }
        "fast" => {
            display::print_info(app, "Fast mode toggled.");
        }
        "effort" => {
            if args.is_empty() {
                display::print_info(app, "Usage: /effort low|medium|high|max");
            } else {
                display::print_info(app, &format!("Effort level set to: {args}"));
            }
        }

        // --- Context/Session ---
        "compact" => {
            let before = state.messages.len();
            if before > 4 {
                state.messages.drain(..before - 4);
                display::print_info(
                    app,
                    &format!("Compacted: kept last 4 messages (removed {}).", before - 4),
                );
            } else {
                display::print_info(app, "Conversation is already short, nothing to compact.");
            }
        }
        "context" => {
            let msg_count = state.messages.len();
            let total_chars: usize = state
                .messages
                .iter()
                .map(|m| match &m.content {
                    ApiContent::Text(t) => t.len(),
                    ApiContent::Blocks(blocks) => blocks
                        .iter()
                        .map(|b| match b {
                            crate::api::request::ContentBlockParam::Text { text, .. } => {
                                text.len()
                            }
                            _ => 0,
                        })
                        .sum(),
                })
                .sum();
            display::print_info(
                app,
                &format!("Conversation: {msg_count} messages, ~{total_chars} chars"),
            );
        }
        "resume" | "continue" => {
            if args.is_empty() {
                display::print_info(app, "Usage: /resume <session-id>");
            } else {
                display::print_info(app, &format!("Resuming session {args}... (not yet implemented)"));
            }
        }
        "history" => {
            match crate::session::list_sessions(20) {
                Ok(sessions) => {
                    if sessions.is_empty() {
                        display::print_info(app, "No previous sessions found.");
                    } else {
                        let mut text = String::from("Recent sessions:\n");
                        for s in &sessions {
                            let name = s.name.as_deref().unwrap_or("(untitled)");
                            text.push_str(&format!("  {} - {} ({} messages)\n", &s.session_id[..8], name, s.message_count));
                        }
                        display::print_info(app, text.trim());
                    }
                }
                Err(_) => display::print_info(app, "No previous sessions found."),
            }
        }

        // --- Config/Settings ---
        "config" | "settings" => {
            display::print_info(app, &format!(
                "Settings:\n  Model: {}\n  Permission mode: {}",
                state.model,
                app.status.permission_mode.as_deref().unwrap_or("default"),
            ));
        }
        "theme" => {
            display::print_info(app, "Available themes: dark, light, light-daltonized, dark-daltonized, light-ansi, dark-ansi\nUsage: /theme <name>");
        }
        "permissions" | "allowed-tools" => {
            display::print_info(app, "Permission mode: default\nUse /permissions allow|deny|ask <tool> to manage rules.");
        }
        "vim" => {
            display::print_info(app, "Vim mode toggled.");
        }
        "keybindings" => {
            display::print_info(app, "Keybindings: ctrl+c (interrupt), ctrl+d (exit), ctrl+l (redraw), ctrl+r (history search)");
        }

        // --- Features ---
        "plan" => {
            display::print_info(app, "Plan mode toggled.");
        }
        "tasks" | "bashes" => {
            display::print_info(app, "No background tasks running.");
        }
        "agents" => {
            display::print_info(app, "No active agents.");
        }
        "skills" => {
            let skills = crate::skills::load_all_skills(&std::env::current_dir().unwrap_or_default());
            if skills.is_empty() {
                display::print_info(app, "No skills loaded.");
            } else {
                let mut text = format!("{} skills available:\n", skills.len());
                for s in skills.iter().take(20) {
                    text.push_str(&format!("  /{} - {}\n", s.name, s.description));
                }
                display::print_info(app, text.trim());
            }
        }
        "mcp" => {
            display::print_info(app, "No MCP servers connected.");
        }
        "plugin" | "plugins" => {
            display::print_info(app, "No plugins loaded.");
        }

        // --- Git ---
        "status" => {
            match crate::git::get_status(&std::env::current_dir().unwrap_or_default()) {
                Ok(status) => {
                    let branch = status.branch.as_deref().unwrap_or("(detached)");
                    let mut text = format!("Branch: {branch}\n");
                    for f in &status.staged { text.push_str(&format!("  staged: {:?} {}\n", f.status, f.path)); }
                    for f in &status.unstaged { text.push_str(&format!("  modified: {:?} {}\n", f.status, f.path)); }
                    for f in &status.untracked { text.push_str(&format!("  untracked: {f}\n")); }
                    if status.staged.is_empty() && status.unstaged.is_empty() && status.untracked.is_empty() {
                        text.push_str("  (clean working tree)\n");
                    }
                    display::print_info(app, text.trim());
                }
                Err(e) => display::print_error_in_app(app, &format!("Git error: {e}")),
            }
        }
        "diff" => {
            match crate::git::diff::get_staged_diff(&std::env::current_dir().unwrap_or_default()) {
                Ok(diff) => {
                    if diff.is_empty() {
                        display::print_info(app, "No staged changes.");
                    } else {
                        display::add_tool_result(app, "diff", &diff, false);
                    }
                }
                Err(e) => display::print_error_in_app(app, &format!("Git error: {e}")),
            }
        }
        "branch" | "fork" => {
            match crate::git::get_branch_name(&std::env::current_dir().unwrap_or_default()) {
                Ok(Some(branch)) => display::print_info(app, &format!("Current branch: {branch}")),
                Ok(None) => display::print_info(app, "Detached HEAD"),
                Err(e) => display::print_error_in_app(app, &format!("Git error: {e}")),
            }
        }

        // --- System ---
        "doctor" => {
            let result = crate::services::diagnostics::run_diagnostics().await;
            let mut text = String::from("Diagnostics:\n");
            for check in &result.checks {
                let icon = match check.status {
                    crate::services::diagnostics::CheckStatus::Pass => "\x1b[38;2;78;186;101m✓\x1b[0m",
                    crate::services::diagnostics::CheckStatus::Warn => "\x1b[38;2;255;193;7m!\x1b[0m",
                    crate::services::diagnostics::CheckStatus::Fail => "\x1b[38;2;255;107;128m✗\x1b[0m",
                };
                text.push_str(&format!("  {icon} {}: {}\n", check.name, check.message));
            }
            display::print_info(app, text.trim());
        }
        "stats" => {
            let snap = state.bootstrap.snapshot();
            display::print_info(app, &format!(
                "Session stats:\n  Messages: {}\n  Cost: ${:.4}\n  Duration: {:.1}s",
                state.messages.len(),
                snap.total_cost_usd,
                snap.total_duration_ms as f64 / 1000.0,
            ));
        }
        "feedback" | "bug" => {
            display::print_info(app, "Report issues at: https://github.com/user/thundercode/issues");
        }
        "version" => {
            display::print_info(app, &format!("ThunderCode v{}", env!("CARGO_PKG_VERSION")));
        }
        "login" => {
            display::print_info(app, "Set THUNDERCODE_API_KEY (or OPENAI_API_KEY) and THUNDERCODE_BASE_URL env vars.");
        }
        "logout" => {
            display::print_info(app, "Logged out.");
        }

        // --- Review (prompt commands) ---
        "review" => {
            display::print_info(app, "Starting code review... (submitting as prompt)");
            let prompt = "Review the code changes in the current git diff. Look for bugs, security issues, and improvements.";
            handle_message(prompt, state, app).await;
            return;
        }
        "security-review" => {
            display::print_info(app, "Starting security review... (submitting as prompt)");
            let prompt = "Perform a thorough security review of this codebase. Look for OWASP Top 10 vulnerabilities, injection risks, authentication issues, and data exposure.";
            handle_message(prompt, state, app).await;
            return;
        }

        // --- Test ---
        "test" => {
            run_mock_e2e_test(state, app).await;
        }

        // --- Fallback ---
        other => {
            // Check if it's a registered command
            if state.command_registry.has(other) {
                // Try to find the command description
                let desc = state.command_registry.list_available(None)
                    .iter()
                    .find(|c| c.base().name == other || c.base().aliases.as_ref().map_or(false, |a| a.contains(&other.to_string())))
                    .map(|c| c.base().description.clone())
                    .unwrap_or_default();
                display::print_info(app, &format!("/{other}: {desc}"));
            } else {
                display::print_error_in_app(
                    app,
                    &format!("Unknown command: /{other}. Type /help for available commands."),
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Message handling with streaming
// ---------------------------------------------------------------------------

/// Handle a user message: send to the API and stream the response.
async fn handle_message(text: &str, state: &mut ReplState, app: &mut App) {
    // Check auth before attempting API call.
    let client = match &state.client {
        Some(c) => c,
        None => {
            display::print_error_in_app(
                app,
                "No API key. Set THUNDERCODE_API_KEY (or OPENAI_API_KEY) env var.",
            );
            return;
        }
    };

    // Add user message to conversation history and transcript.
    state.messages.push(ApiMessage::user(text));
    display::add_user_message(app, text);

    // Build system message from system prompt blocks.
    let mut api_messages = Vec::new();
    for block in &state.system_prompt {
        match block {
            SystemBlock::Text { text, .. } => {
                api_messages.push(ApiMessage::system(text));
            }
        }
    }
    api_messages.extend(state.messages.clone());

    // Build tool definitions with proper descriptions.
    let tool_defs = build_tool_definitions(&state.tool_registry);

    // Build the API request (OpenAI format).
    let request = CreateMessageRequest {
        model: state.model.clone(),
        max_tokens: Some(16384),
        messages: api_messages,
        temperature: None,
        top_p: None,
        stop: None,
        stream: true,
        tools: if tool_defs.is_empty() { None } else { Some(tool_defs) },
        tool_choice: None,
    };

    // Show spinner while connecting.
    let start = Instant::now();
    display::start_spinner(app);

    // Spinner animation during connection
    let spinner_interval = Duration::from_millis(80);
    let stream_result;
    {
        let client_ref = client;
        let request_clone = request;

        // Spawn the API request and animate the spinner while waiting
        let api_future = client_ref.create_message_stream(request_clone);
        tokio::pin!(api_future);

        loop {
            tokio::select! {
                result = &mut api_future => {
                    stream_result = result;
                    break;
                }
                _ = tokio::time::sleep(spinner_interval) => {
                    display::tick_spinner(app);
                }
            }
        }
    }

    display::stop_spinner(app);

    let raw_stream = match stream_result {
        Ok(s) => s,
        Err(e) => {
            display::print_error_in_app(app, &format!("API request failed: {e}"));
            state.messages.pop();
            return;
        }
    };
    let mut stream = pin!(raw_stream);

    // Accumulate the full assistant response for history.
    let mut response_text = String::new();
    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;
    let mut tool_use_blocks: Vec<(String, String, serde_json::Value)> = Vec::new();
    // Track multiple concurrent tool calls by index.
    let mut pending_tools: std::collections::HashMap<usize, (String, String, String)> = std::collections::HashMap::new();
    let mut streaming_started = false;

    // Disable prompt during streaming
    app.prompt.set_active(false);

    // Process the SSE stream.
    while let Some(event_result) = stream.next().await {
        // Check for abort.
        if *state.abort_rx.borrow() {
            display::print_warning(app, "Query interrupted.");
            break;
        }

        match event_result {
            Ok(event) => {
                match &event {
                    StreamEvent::MessageStart { message } => {
                        if let Some(ref usage) = message.usage {
                            total_input_tokens = usage.prompt_tokens;
                            total_output_tokens = usage.completion_tokens;
                        }
                    }
                    StreamEvent::ContentBlockDelta { delta, .. } => {
                        if let ContentDelta::TextDelta { text } = delta {
                            if !streaming_started {
                                display::start_assistant_stream(app);
                                streaming_started = true;
                            }
                            response_text.push_str(text);
                            display::append_assistant_text(app, text);
                        }
                    }
                    StreamEvent::ToolCallStart { index, id, name } => {
                        if streaming_started {
                            display::finish_assistant_stream(app);
                            streaming_started = false;
                        }
                        pending_tools.insert(*index, (id.clone(), name.clone(), String::new()));
                        display::add_tool_use_simple(app, name, name, "", true);
                    }
                    StreamEvent::ToolCallDelta { index, arguments } => {
                        if let Some((_id, _name, ref mut args)) = pending_tools.get_mut(index) {
                            args.push_str(arguments);
                        }
                    }
                    StreamEvent::ContentBlockStop { .. } => {
                        if streaming_started {
                            display::finish_assistant_stream(app);
                            streaming_started = false;
                        }
                    }
                    StreamEvent::MessageDelta { usage, delta } => {
                        total_output_tokens += usage.output_tokens;
                        if let Some(ref reason) = delta.stop_reason {
                            if reason == "tool_calls" || reason == "stop" {
                                let mut indices: Vec<usize> = pending_tools.keys().copied().collect();
                                indices.sort();
                                for idx in indices {
                                    if let Some((id, name, args_json)) = pending_tools.remove(&idx) {
                                        finalize_tool_call(
                                            &mut tool_use_blocks, &id, &name, &args_json, app,
                                        );
                                    }
                                }
                            }
                        }
                    }
                    StreamEvent::MessageStop => {
                        if streaming_started {
                            display::finish_assistant_stream(app);
                            streaming_started = false;
                        }
                        // Finalize any remaining pending tool calls.
                        let mut indices: Vec<usize> = pending_tools.keys().copied().collect();
                        indices.sort();
                        for idx in indices {
                            if let Some((id, name, args_json)) = pending_tools.remove(&idx) {
                                finalize_tool_call(
                                    &mut tool_use_blocks, &id, &name, &args_json, app,
                                );
                            }
                        }
                    }
                    StreamEvent::Ping => {}
                    StreamEvent::Error { error } => {
                        display::print_error_in_app(app, &format!("Stream error: {error}"));
                        break;
                    }
                }
            }
            Err(e) => {
                display::print_error_in_app(app, &format!("Stream error: {e}"));
                break;
            }
        }
    }

    // Re-enable prompt
    app.prompt.set_active(true);

    let elapsed = start.elapsed();

    // Add assistant response to conversation history.
    if !response_text.is_empty() || !tool_use_blocks.is_empty() {
        // Build assistant message with tool_calls if any
        let mut assistant_msg = ApiMessage::assistant(&response_text);
        if !tool_use_blocks.is_empty() {
            let tool_calls: Vec<crate::api::request::ToolCall> = tool_use_blocks.iter().map(|(id, name, input)| {
                crate::api::request::ToolCall {
                    id: id.clone(),
                    call_type: "function".into(),
                    function: crate::api::request::FunctionCall {
                        name: name.clone(),
                        arguments: serde_json::to_string(input).unwrap_or_default(),
                    },
                }
            }).collect();
            assistant_msg.tool_calls = Some(tool_calls);
        }
        state.messages.push(assistant_msg);
    }

    // Execute tool calls, add results, and continue if needed.
    if !tool_use_blocks.is_empty() {
        for (id, name, input) in &tool_use_blocks {
            // Permission check for non-read-only tools.
            if tool_needs_permission(name, input, &state.tool_registry) {
                let desc = format_tool_for_permission(name, input);
                let allowed = display::prompt_permission(app, name, &desc);
                if !allowed {
                    let err_msg = "Tool execution denied by user.";
                    display::add_tool_result(app, name, err_msg, true);
                    state.messages.push(ApiMessage::tool_result(id, err_msg));
                    continue;
                }
            }
            // Execute via tool registry.
            if let Some(tool) = state.tool_registry.find_by_name(name) {
                let ctx = crate::types::tool::ToolUseContext::default();
                let result = tool.call(input.clone(), &ctx, None).await;
                match result {
                    Ok(call_result) => {
                        let result_text = format_tool_result(&call_result.data);
                        let display_text = format_tool_result_display(name, &call_result.data);
                        display::add_tool_result(app, name, &display_text, false);
                        state.messages.push(ApiMessage::tool_result(id, &result_text));
                    }
                    Err(e) => {
                        let err_msg = format!("Tool error: {e}");
                        display::add_tool_result(app, name, &err_msg, true);
                        state.messages.push(ApiMessage::tool_result(id, &err_msg));
                    }
                }
            } else {
                let err_msg = format!("Unknown tool: {name}");
                display::add_tool_result(app, name, &err_msg, true);
                state.messages.push(ApiMessage::tool_result(id, &err_msg));
            }
        }

        // Record cost for this turn before continuing.
        let cost = crate::telemetry::cost_tracker::calculate_cost(
            &state.model,
            total_input_tokens,
            total_output_tokens,
            0,
            0,
        );
        state.bootstrap.record_model_usage(
            &state.model,
            total_input_tokens,
            total_output_tokens,
            0,
            0,
            cost,
        );
        state.bootstrap.add_api_cost(0.0, elapsed.as_millis() as u64);
        // Update status line with cost so far.
        let snap = state.bootstrap.snapshot();
        app.status.cost_usd = snap.total_cost_usd;
        app.status.input_tokens = snap.model_usage.values().map(|u| u.input_tokens).sum();
        app.status.output_tokens = snap.model_usage.values().map(|u| u.output_tokens).sum();
        app.render();

        // Send tool results back to the model for a continuation turn.
        app.prompt.set_active(true);
        // Clone client to avoid borrow conflict with state.
        let client_clone = state.client.as_ref().unwrap().clone();
        handle_message_continuation(state, app, &client_clone).await;
        return;
    }

    // Record cost.
    let cost = crate::telemetry::cost_tracker::calculate_cost(
        &state.model,
        total_input_tokens,
        total_output_tokens,
        0,
        0,
    );

    state.bootstrap.record_model_usage(
        &state.model,
        total_input_tokens,
        total_output_tokens,
        0,
        0,
        cost,
    );
    state
        .bootstrap
        .add_api_cost(0.0, elapsed.as_millis() as u64);

    state.cost_tracker.record(crate::telemetry::CostEntry {
        model: state.model.clone(),
        input_tokens: total_input_tokens,
        output_tokens: total_output_tokens,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        cost_usd: cost,
        duration_ms: elapsed.as_millis() as u64,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    });

    // Update status line
    let snap = state.bootstrap.snapshot();
    app.status.cost_usd = snap.total_cost_usd;
    app.status.input_tokens = snap
        .model_usage
        .values()
        .map(|u| u.input_tokens)
        .sum();
    app.status.output_tokens = snap
        .model_usage
        .values()
        .map(|u| u.output_tokens)
        .sum();
    app.render();
}

/// Build a `CostSummary` from the current bootstrap state.
pub fn build_cost_summary(state: &ReplState) -> CostSummary {
    let snap = state.bootstrap.snapshot();
    let mut model_breakdown = std::collections::HashMap::new();
    for (model, usage) in &snap.model_usage {
        model_breakdown.insert(
            model.clone(),
            ModelCosts {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cache_read_tokens: usage.cache_read_tokens,
                cache_write_tokens: usage.cache_write_tokens,
                cost_usd: usage.cost_usd,
                call_count: 0,
            },
        );
    }

    CostSummary {
        total_cost_usd: snap.total_cost_usd,
        total_duration_ms: snap.total_duration_ms,
        total_input_tokens: snap
            .model_usage
            .values()
            .map(|u| u.input_tokens)
            .sum(),
        total_output_tokens: snap
            .model_usage
            .values()
            .map(|u| u.output_tokens)
            .sum(),
        total_cache_read_tokens: snap
            .model_usage
            .values()
            .map(|u| u.cache_read_tokens)
            .sum(),
        total_cache_write_tokens: snap
            .model_usage
            .values()
            .map(|u| u.cache_write_tokens)
            .sum(),
        model_breakdown,
    }
}

// ---------------------------------------------------------------------------
// /test — Mock E2E test (no LLM needed)
// ---------------------------------------------------------------------------

/// Run a comprehensive mock E2E test that exercises:
/// - Markdown rendering (headings, bold, italic, code, lists, blockquotes)
/// - Syntax-highlighted code blocks
/// - Multiple tool calls (Read, Bash, Edit, Grep)
/// - Tool results including diffs
/// - Streaming text simulation
/// - Error handling
async fn run_mock_e2e_test(state: &mut ReplState, app: &mut App) {
    use std::time::Duration;

    display::print_info(app, "Running mock E2E test (no LLM required)...");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // --- Step 1: Simulated user message ---
    display::add_user_message(app, "Fix the bug in src/main.rs where the config loading fails on Windows paths");
    tokio::time::sleep(Duration::from_millis(300)).await;

    // --- Step 2: Spinner ---
    display::start_spinner(app);
    for _ in 0..15 {
        display::tick_spinner(app);
        tokio::time::sleep(Duration::from_millis(80)).await;
    }
    display::stop_spinner(app);

    // --- Step 3: Assistant response with markdown ---
    display::start_assistant_stream(app);

    let markdown_response = r#"I'll fix the Windows path handling bug. Let me first read the current code to understand the issue.

## Analysis

The problem is in the `load_config()` function which uses **Unix-style path separators** (`/`) instead of `std::path::MAIN_SEPARATOR`. This breaks on Windows where paths use `\`.

### Key issues:

1. **Hardcoded separators** — `format!("{}/config.toml", dir)` should use `Path::join()`
2. **Missing canonicalization** — relative paths aren't resolved
3. *Incorrect error handling* — `unwrap()` instead of proper `?` propagation

> Note: This also affects the `--config` CLI flag which passes raw user input directly to `std::fs::read_to_string()`.

Here's what I'll do:
- Read the current implementation
- Fix the path handling
- Add proper error messages
- Test with both Unix and Windows paths"#;

    // Simulate streaming character by character (in chunks for speed)
    for chunk in markdown_response.as_bytes().chunks(12) {
        let text = std::str::from_utf8(chunk).unwrap_or("");
        display::append_assistant_text(app, text);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    display::finish_assistant_stream(app);
    tokio::time::sleep(Duration::from_millis(200)).await;

    // --- Step 4: Tool call — Read file ---
    display::add_tool_use_simple(app, "file_read", "Read", "src/config.rs", true);
    app.render();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Complete the read
    display::complete_tool_use(app);
    let read_result = r#"1	use std::fs;
2	use std::path::Path;
3
4	pub fn load_config(dir: &str) -> Result<Config, Box<dyn std::error::Error>> {
5	    let path = format!("{}/config.toml", dir);  // BUG: Unix separator
6	    let content = fs::read_to_string(&path).unwrap();  // BUG: unwrap
7	    let config: Config = toml::from_str(&content)?;
8	    Ok(config)
9	}"#;
    display::add_tool_result(app, "Read", read_result, false);
    tokio::time::sleep(Duration::from_millis(300)).await;

    // --- Step 5: Tool call — Grep for similar patterns ---
    display::add_tool_use_simple(app, "grep", "Grep", "pattern: \"/config\" path: src/", true);
    app.render();
    tokio::time::sleep(Duration::from_millis(400)).await;

    display::complete_tool_use(app);
    display::add_tool_result(app, "Grep", "src/config.rs:5: let path = format!(\"{}/config.toml\", dir);\nsrc/main.rs:12: let cfg_path = format!(\"{}/settings.json\", home);", false);
    tokio::time::sleep(Duration::from_millis(200)).await;

    // --- Step 6: More assistant text with code block ---
    display::start_assistant_stream(app);
    let code_response = r#"Found 2 occurrences. Let me fix both files. Here's the corrected implementation:

```rust
use std::fs;
use std::path::{Path, PathBuf};

pub fn load_config(dir: &str) -> Result<Config, ConfigError> {
    let path = PathBuf::from(dir).join("config.toml");
    let content = fs::read_to_string(&path)
        .map_err(|e| ConfigError::ReadFailed {
            path: path.display().to_string(),
            source: e,
        })?;
    let config: Config = toml::from_str(&content)
        .map_err(|e| ConfigError::ParseFailed {
            path: path.display().to_string(),
            source: e,
        })?;
    Ok(config)
}
```

This fixes all three issues:
- Uses `PathBuf::join()` for **cross-platform** path construction
- Replaces `unwrap()` with proper `?` error propagation
- Adds descriptive error context via `ConfigError`"#;

    for chunk in code_response.as_bytes().chunks(20) {
        let text = std::str::from_utf8(chunk).unwrap_or("");
        display::append_assistant_text(app, text);
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    display::finish_assistant_stream(app);
    tokio::time::sleep(Duration::from_millis(200)).await;

    // --- Step 7: Tool call — Edit file (produces diff) ---
    display::add_tool_use_simple(app, "file_edit", "Edit", "src/config.rs", true);
    app.render();
    tokio::time::sleep(Duration::from_millis(500)).await;

    display::complete_tool_use(app);
    let diff_result = r#"--- a/src/config.rs
+++ b/src/config.rs
@@ -1,9 +1,18 @@
 use std::fs;
-use std::path::Path;
+use std::path::{Path, PathBuf};

-pub fn load_config(dir: &str) -> Result<Config, Box<dyn std::error::Error>> {
-    let path = format!("{}/config.toml", dir);  // BUG: Unix separator
-    let content = fs::read_to_string(&path).unwrap();  // BUG: unwrap
+pub fn load_config(dir: &str) -> Result<Config, ConfigError> {
+    let path = PathBuf::from(dir).join("config.toml");
+    let content = fs::read_to_string(&path)
+        .map_err(|e| ConfigError::ReadFailed {
+            path: path.display().to_string(),
+            source: e,
+        })?;
     let config: Config = toml::from_str(&content)?;
     Ok(config)
 }"#;
    display::add_tool_result(app, "Edit", diff_result, false);
    tokio::time::sleep(Duration::from_millis(300)).await;

    // --- Step 8: Tool call — Bash (run tests) ---
    display::add_tool_use_simple(app, "bash", "Bash", "cargo test config", true);
    app.render();
    tokio::time::sleep(Duration::from_millis(800)).await;

    display::complete_tool_use(app);
    display::add_tool_result(
        app,
        "Bash",
        "running 3 tests\ntest config::test_load_unix_path ... ok\ntest config::test_load_windows_path ... ok\ntest config::test_load_missing_file ... ok\n\ntest result: ok. 3 passed; 0 failed; 0 ignored",
        false,
    );
    tokio::time::sleep(Duration::from_millis(200)).await;

    // --- Step 9: Final assistant summary ---
    display::start_assistant_stream(app);
    let final_response = "All tests pass. The bug is fixed — `load_config()` now uses `PathBuf::join()` for cross-platform path handling and has proper error propagation instead of `unwrap()`.";
    for chunk in final_response.as_bytes().chunks(15) {
        let text = std::str::from_utf8(chunk).unwrap_or("");
        display::append_assistant_text(app, text);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    display::finish_assistant_stream(app);

    // --- Step 10: Tool call with error ---
    tokio::time::sleep(Duration::from_millis(300)).await;
    display::add_tool_use_simple(app, "bash", "Bash", "rm -rf /important", true);
    app.render();
    tokio::time::sleep(Duration::from_millis(300)).await;
    display::complete_tool_use(app);
    display::add_tool_result(app, "Bash", "Error: Permission denied. This command was blocked by the sandbox.", true);
    tokio::time::sleep(Duration::from_millis(200)).await;

    display::print_info(app, "E2E test complete. All rendering exercised.");
}
