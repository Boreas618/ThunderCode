//! TUI-based output rendering for the REPL.
//!
//! Uses the thundercode-tui rendering engine (App, Screen, Renderer, DOM)
//! instead of raw println! calls. All terminal output goes through the
//! App's render pipeline with double-buffered diffing.

use std::io::{self, Write};

use crate::tui::app::{App, PermissionDialogData, TranscriptEntry};
use crate::tui::components::status_line::StatusLineData;
use crate::tui::termio::{csi, dec};

// ---------------------------------------------------------------------------
// App initialization
// ---------------------------------------------------------------------------

/// Create the TUI app with terminal dimensions.
pub fn create_app() -> App {
    let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
    App::new(width, height)
}

// ---------------------------------------------------------------------------
// Welcome banner (rendered via the App)
// ---------------------------------------------------------------------------

/// Show the welcome banner by populating the app state and triggering a render.
pub fn show_welcome(app: &mut App, model: &str, tool_count: usize, command_count: usize) {
    app.set_welcome(model, tool_count, command_count);
    app.status.model = model.to_string();
    app.render();
}

// ---------------------------------------------------------------------------
// Prompt
// ---------------------------------------------------------------------------

/// Render the prompt. The app always shows the prompt; this triggers a redraw.
pub fn render_prompt(app: &mut App) {
    app.render_prompt_only();
}

// ---------------------------------------------------------------------------
// Streaming assistant text
// ---------------------------------------------------------------------------

/// Add a user message to the transcript.
pub fn add_user_message(app: &mut App, text: &str) {
    app.transcript
        .push(TranscriptEntry::User { text: text.into() });
    app.render();
}

/// Start streaming an assistant response (creates a new entry).
pub fn start_assistant_stream(app: &mut App) {
    app.transcript.push(TranscriptEntry::AssistantText {
        text: String::new(),
        is_streaming: true,
    });
}

/// Append text to the current streaming assistant response.
pub fn append_assistant_text(app: &mut App, text: &str) {
    if let Some(TranscriptEntry::AssistantText {
        text: ref mut t, ..
    }) = app.transcript.last_mut()
    {
        t.push_str(text);
    }
    // Partial render -- just update the message area
    app.render();
}

/// Finish the current streaming assistant response.
pub fn finish_assistant_stream(app: &mut App) {
    if let Some(TranscriptEntry::AssistantText {
        ref mut is_streaming,
        ..
    }) = app.transcript.last_mut()
    {
        *is_streaming = false;
    }
    app.render();
}

/// Add a thinking indicator.
pub fn add_thinking(app: &mut App) {
    app.transcript
        .push(TranscriptEntry::Thinking { text: None });
}

/// Add a tool use entry.
pub fn add_tool_use(
    app: &mut App,
    tool_name: &str,
    input: &serde_json::Value,
    in_progress: bool,
) {
    let input_summary = if let Some(obj) = input.as_object() {
        obj.iter()
            .map(|(k, v)| {
                let val_str = match v {
                    serde_json::Value::String(s) => {
                        if s.len() > 80 {
                            format!("{}...", &s[..77])
                        } else {
                            s.clone()
                        }
                    }
                    other => {
                        let s = other.to_string();
                        if s.len() > 80 {
                            format!("{}...", &s[..77])
                        } else {
                            s
                        }
                    }
                };
                format!("{k}: {val_str}")
            })
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        String::new()
    };

    app.transcript.push(TranscriptEntry::ToolUse {
        tool_name: tool_name.into(),
        display_name: tool_name.into(),
        input_summary,
        in_progress,
    });
    app.render();
}

/// Add a tool use with a pre-formatted input summary string.
pub fn add_tool_use_simple(
    app: &mut App,
    tool_name: &str,
    display_name: &str,
    input_summary: &str,
    in_progress: bool,
) {
    app.transcript.push(TranscriptEntry::ToolUse {
        tool_name: tool_name.into(),
        display_name: display_name.into(),
        input_summary: input_summary.into(),
        in_progress,
    });
    app.render();
}

/// Mark the last tool use as completed (no longer in progress).
pub fn complete_tool_use(app: &mut App) {
    for entry in app.transcript.iter_mut().rev() {
        if let TranscriptEntry::ToolUse { ref mut in_progress, .. } = entry {
            *in_progress = false;
            break;
        }
    }
    app.render();
}

/// Mark the last tool use as completed (no longer in progress).
pub fn finish_tool_use(app: &mut App) {
    // Find the last ToolUse entry and set in_progress = false
    for entry in app.transcript.iter_mut().rev() {
        if let TranscriptEntry::ToolUse {
            ref mut in_progress,
            ..
        } = entry
        {
            *in_progress = false;
            break;
        }
    }
}

/// Add a tool result.
pub fn add_tool_result(app: &mut App, tool_name: &str, content: &str, is_error: bool) {
    app.transcript.push(TranscriptEntry::ToolResult {
        tool_name: tool_name.into(),
        content: content.into(),
        is_error,
    });
    app.render();
}

// ---------------------------------------------------------------------------
// Spinner
// ---------------------------------------------------------------------------

/// Start the spinner.
pub fn start_spinner(app: &mut App) {
    app.start_spinner();
    app.render();
}

/// Stop the spinner.
pub fn stop_spinner(app: &mut App) {
    app.stop_spinner();
    app.render();
}

/// Tick the spinner animation.
pub fn tick_spinner(app: &mut App) {
    app.tick_spinner();
    app.render_spinner_only();
}

// ---------------------------------------------------------------------------
// Permission dialog
// ---------------------------------------------------------------------------

/// Show a permission dialog and return immediately.
/// The caller must poll for the response.
pub fn show_permission_dialog(app: &mut App, tool_name: &str, description: &str) {
    app.permission_dialog = Some(PermissionDialogData::new(tool_name, description));
    app.render();
}

/// Clear the permission dialog.
pub fn clear_permission_dialog(app: &mut App) {
    app.permission_dialog = None;
    app.render();
}

/// Prompt for permission using the TUI dialog. Blocks until y/n is pressed.
pub fn prompt_permission(app: &mut App, tool_name: &str, description: &str) -> bool {
    show_permission_dialog(app, tool_name, description);

    // Read key events until we get y or n
    loop {
        if let Ok(true) = crossterm::event::poll(std::time::Duration::from_millis(100)) {
            if let Ok(crossterm::event::Event::Key(key)) = crossterm::event::read() {
                match key.code {
                    crossterm::event::KeyCode::Char('y') | crossterm::event::KeyCode::Char('Y') => {
                        clear_permission_dialog(app);
                        return true;
                    }
                    crossterm::event::KeyCode::Char('n')
                    | crossterm::event::KeyCode::Char('N')
                    | crossterm::event::KeyCode::Enter => {
                        clear_permission_dialog(app);
                        return false;
                    }
                    crossterm::event::KeyCode::Char('c')
                        if key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        clear_permission_dialog(app);
                        return false;
                    }
                    _ => continue,
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Status line updates
// ---------------------------------------------------------------------------

/// Update the status line data.
pub fn update_status(app: &mut App, data: StatusLineData) {
    app.status = data;
}

// ---------------------------------------------------------------------------
// Command output / messages
// ---------------------------------------------------------------------------

/// Print a command result into the transcript.
pub fn print_command_result(app: &mut App, output: &str) {
    if !output.is_empty() {
        app.transcript.push(TranscriptEntry::AssistantText {
            text: output.into(),
            is_streaming: false,
        });
        app.render();
    }
}

/// Print help listing.
pub fn print_help(app: &mut App, commands: &[(String, String)]) {
    let mut text = String::new();
    text.push_str("\x1b[1mAvailable commands:\x1b[0m\n\n");
    let max_name_len = commands
        .iter()
        .map(|(name, _)| name.len())
        .max()
        .unwrap_or(10);
    for (name, desc) in commands {
        text.push_str(&format!(
            "  \x1b[32m/{name:<width$}\x1b[0m  \x1b[2m{desc}\x1b[0m\n",
            width = max_name_len
        ));
    }
    app.transcript.push(TranscriptEntry::AssistantText {
        text,
        is_streaming: false,
    });
    app.render();
}

/// Print an info message.
pub fn print_info(app: &mut App, msg: &str) {
    app.transcript.push(TranscriptEntry::AssistantText {
        text: format!("\x1b[2m{msg}\x1b[0m"),
        is_streaming: false,
    });
    app.render();
}

/// Print a warning message.
pub fn print_warning(app: &mut App, msg: &str) {
    app.transcript.push(TranscriptEntry::AssistantText {
        text: format!("\x1b[1;33mWarning:\x1b[0m {msg}"),
        is_streaming: false,
    });
    app.render();
}

/// Print an error message.
pub fn print_error(msg: &str) {
    // Errors go to stderr directly -- they can happen before the app is set up
    let mut out = io::stderr();
    let _ = writeln!(out, "\x1b[1;31mError:\x1b[0m {msg}");
    let _ = out.flush();
}

/// Print an error message into the app transcript.
pub fn print_error_in_app(app: &mut App, msg: &str) {
    app.transcript.push(TranscriptEntry::AssistantText {
        text: format!("\x1b[1;31mError:\x1b[0m {msg}"),
        is_streaming: false,
    });
    app.render();
}

// ---------------------------------------------------------------------------
// Cost summary
// ---------------------------------------------------------------------------

/// Print the session cost summary on exit (to regular stdout after leaving TUI).
pub fn print_cost_summary_on_exit(summary: &crate::telemetry::CostSummary) {
    let mut out = io::stdout();
    let _ = writeln!(out);
    let formatted = crate::telemetry::format_cost_summary(summary);
    for line in formatted.lines() {
        let _ = writeln!(out, "  \x1b[2m{line}\x1b[0m");
    }
    let _ = writeln!(out);
    let _ = out.flush();
}

// ---------------------------------------------------------------------------
// Terminal resize
// ---------------------------------------------------------------------------

/// Handle terminal resize.
pub fn handle_resize(app: &mut App, width: u16, height: u16) {
    app.resize(width, height);
    app.render();
}
