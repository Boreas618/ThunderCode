//! ThunderCode -- a Rust port of a terminal AI coding assistant.
//!
//! The TUI starts unconditionally. Auth is only needed for API calls.
//! Local features (slash commands, config, etc.) work without auth.

// Core types (must come first -- everything depends on it)
pub mod types;

// Foundation modules (no inter-module deps beyond types)
pub mod constants;
pub mod git;
pub mod keybindings;
pub mod mcp;
pub mod remote;
pub mod tui;
pub mod utils;
pub mod vim;
pub mod voice;

// Config & state
pub mod config;
pub mod state;
pub mod telemetry;

// Auth & API
pub mod auth;
pub mod api;

// Mid-level modules
pub mod context;
pub mod memory;
pub mod permissions;
pub mod query;
pub mod session;
pub mod skills;

// Higher-level modules
pub mod commands;
pub mod coordinator;
pub mod plugins;
pub mod services;
pub mod tasks;
pub mod tools;
pub mod bridge;

// Binary-specific modules
mod display;
mod init;
#[allow(dead_code)]
mod input;
mod repl;

use std::io::{self, Write};

use clap::Parser;
use crossterm::terminal;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "thundercode", version, about = "AI coding assistant")]
struct Cli {
    /// Resume a previous session by ID
    #[arg(long)]
    resume: Option<String>,

    /// Print the full system prompt and exit
    #[arg(long)]
    dump_system_prompt: bool,

    /// Initial prompt to send (non-interactive one-shot)
    prompt: Option<String>,
}

// ---------------------------------------------------------------------------
// Terminal management
// ---------------------------------------------------------------------------

fn enter_tui() -> anyhow::Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(
        stdout,
        terminal::EnterAlternateScreen,
        terminal::Clear(terminal::ClearType::All),
        crossterm::cursor::MoveTo(0, 0),
        crossterm::event::EnableBracketedPaste,
        crossterm::event::EnableMouseCapture,
        crossterm::cursor::Hide
    )?;
    Ok(())
}

fn leave_tui() {
    let mut stdout = io::stdout();
    let _ = crossterm::execute!(
        stdout,
        crossterm::event::DisableMouseCapture,
        crossterm::event::DisableBracketedPaste,
        terminal::LeaveAlternateScreen,
        crossterm::cursor::Show
    );
    let _ = terminal::disable_raw_mode();
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    // Fast path: --dump-system-prompt
    if cli.dump_system_prompt {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let sys_ctx = crate::context::get_system_context(&cwd).await;
        let usr_ctx = crate::context::get_user_context(&cwd).await;
        let tools: Vec<&dyn crate::types::tool::Tool> = vec![];
        let blocks = repl::ReplState::build_system_prompt(&sys_ctx, &usr_ctx, &tools, "gpt-4o-mini");
        for block in blocks {
            match block {
                crate::api::request::SystemBlock::Text { text, .. } => println!("{text}"),
            }
        }
        return Ok(());
    }

    // Initialize -- auth is best-effort, TUI starts regardless.
    let init_result = match init::initialize(cli.resume.as_deref()).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    let model = init_result.model.clone();
    let tool_count = init_result.tool_registry.len();
    let command_count = init_result.command_registry.all().len();
    let bootstrap_for_exit = init_result.bootstrap.clone();

    // Non-interactive one-shot: needs auth.
    if let Some(ref prompt_text) = cli.prompt {
        if init_result.client.is_none() {
            eprintln!(
                "No authentication found. Set THUNDERCODE_API_KEY or run `thundercode` to log in."
            );
            std::process::exit(1);
        }
        return run_oneshot(prompt_text, init_result).await;
    }

    // Enter TUI -- unconditionally.
    enter_tui()?;

    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        leave_tui();
        default_panic(info);
    }));

    // Create the TUI app.
    let mut app = display::create_app();

    // Welcome banner via TUI engine.
    display::show_welcome(&mut app, &init_result.model, tool_count, command_count);

    if !init_result.has_auth {
        display::print_warning(
            &mut app,
            "No API key set. Export THUNDERCODE_API_KEY (or OPENAI_API_KEY) and THUNDERCODE_BASE_URL. Local commands still work.",
        );
    }

    // REPL loop.
    let (abort_tx, abort_rx) = tokio::sync::watch::channel(false);

    let repl_state = repl::ReplState {
        bootstrap: init_result.bootstrap.clone(),
        client: init_result.client,
        tool_registry: init_result.tool_registry,
        command_registry: init_result.command_registry,
        system_context: init_result.system_context,
        user_context: init_result.user_context,
        cost_tracker: init_result.cost_tracker,
        model,
        messages: Vec::new(),
        system_prompt: Vec::new(),
        abort_tx,
        abort_rx,
    };

    let run_result = repl::run_repl(repl_state, &mut app).await;

    // Cleanup.
    leave_tui();

    let summary = {
        let snap = bootstrap_for_exit.snapshot();
        let mut model_breakdown = std::collections::HashMap::new();
        for (m, usage) in &snap.model_usage {
            model_breakdown.insert(
                m.clone(),
                crate::telemetry::ModelCosts {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                    cache_read_tokens: usage.cache_read_tokens,
                    cache_write_tokens: usage.cache_write_tokens,
                    cost_usd: usage.cost_usd,
                    call_count: 0,
                },
            );
        }
        crate::telemetry::CostSummary {
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
    };

    if summary.total_cost_usd > 0.0 || summary.total_input_tokens > 0 {
        display::print_cost_summary_on_exit(&summary);
    }

    run_result
}

async fn run_oneshot(prompt: &str, init: init::InitResult) -> anyhow::Result<()> {
    use futures::StreamExt;

    let client = init.client.as_ref().expect("oneshot requires auth");
    let request = crate::api::request::CreateMessageRequest::new(
        &init.model,
        crate::api::models::get_model_info(&init.model)
            .map(|m| m.max_output_tokens)
            .unwrap_or(16384),
        vec![crate::api::request::ApiMessage::user(prompt)],
    )
    .with_streaming();

    let raw_stream = client.create_message_stream(request).await?;
    let mut stream = std::pin::pin!(raw_stream);

    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(event) => match &event {
                crate::api::streaming::StreamEvent::ContentBlockDelta { delta, .. } => {
                    if let crate::api::streaming::ContentDelta::TextDelta { text } = delta {
                        print!("{text}");
                        let _ = io::stdout().flush();
                    }
                }
                crate::api::streaming::StreamEvent::MessageStop => println!(),
                crate::api::streaming::StreamEvent::Error { error } => {
                    eprintln!("API error: {error}");
                    std::process::exit(1);
                }
                _ => {}
            },
            Err(e) => {
                eprintln!("Stream error: {e}");
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
