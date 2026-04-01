//! Subsystem initialization.
//!
//! ThunderCode is provider-neutral. Users configure:
//!   - THUNDERCODE_API_KEY (or OPENAI_API_KEY) — Bearer token
//!   - THUNDERCODE_BASE_URL (or OPENAI_BASE_URL) — OpenAI-compatible endpoint
//!   - THUNDERCODE_MODEL (or OPENAI_MODEL) — model name

use std::path::PathBuf;

use crate::api::client::{ApiClient, ClientConfig};
use crate::commands::CommandRegistry;
use crate::context::{SystemContext, UserContext};
use crate::query::{QueryEngine, QueryEngineBuilder};
use crate::state::BootstrapState;
use crate::telemetry::CostTracker;
use crate::tools::ToolRegistry;
use crate::types::ids::SessionId;

/// Everything the REPL loop needs.
#[allow(dead_code)]
pub struct InitResult {
    pub bootstrap: BootstrapState,
    pub client: Option<ApiClient>,
    pub engine: QueryEngine,
    pub tool_registry: ToolRegistry,
    pub command_registry: CommandRegistry,
    pub system_context: SystemContext,
    pub user_context: UserContext,
    pub cost_tracker: CostTracker,
    pub model: String,
    pub has_auth: bool,
}

/// Resolve API key from env vars (provider-neutral).
fn resolve_api_key() -> Option<String> {
    std::env::var("THUNDERCODE_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .ok()
        .filter(|k| !k.is_empty())
}

/// Resolve base URL from env vars (provider-neutral).
fn resolve_base_url() -> Option<String> {
    std::env::var("THUNDERCODE_BASE_URL")
        .or_else(|_| std::env::var("OPENAI_BASE_URL"))
        .ok()
        .filter(|u| !u.is_empty())
}

/// Resolve model from env vars (provider-neutral).
fn resolve_model() -> String {
    std::env::var("THUNDERCODE_MODEL")
        .or_else(|_| std::env::var("OPENAI_MODEL"))
        .unwrap_or_else(|_| "gpt-4o".to_owned())
}

/// Initialize all subsystems.
pub async fn initialize(resume_session: Option<&str>) -> anyhow::Result<InitResult> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let bootstrap = BootstrapState::new(cwd.clone());
    bootstrap.set_interactive(true);

    let model = resolve_model();
    let api_key = resolve_api_key();
    let base_url = resolve_base_url();
    let has_auth = api_key.is_some();

    if let Some(ref key) = api_key {
        bootstrap.set_auth_token(Some(key.clone()));
    }

    let client = api_key.map(|key| {
        ApiClient::new(ClientConfig {
            base_url,
            api_key: Some(key),
            ..Default::default()
        })
    });

    let session_id = if let Some(id) = resume_session {
        SessionId::from_str(id)
    } else {
        bootstrap.session_id()
    };
    let engine = QueryEngineBuilder::new()
        .model(&model)
        .max_tokens(16384)
        .session_id(session_id)
        .build();

    let tool_registry = ToolRegistry::with_all_base_tools();
    let command_registry = CommandRegistry::new();
    let system_context = crate::context::get_system_context(&cwd).await;
    let user_context = crate::context::get_user_context(&cwd).await;
    let cost_tracker = CostTracker::new();

    Ok(InitResult {
        bootstrap,
        client,
        engine,
        tool_registry,
        command_registry,
        system_context,
        user_context,
        cost_tracker,
        model,
        has_auth,
    })
}
