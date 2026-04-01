//! QueryEngine -- the top-level owner of a conversation's query lifecycle.
//!
//! Ported from ref/QueryEngine.ts`. One `QueryEngine` per conversation.
//! Each `submit_message()` call starts a new turn; state (messages, usage,
//! file cache, etc.) persists across turns.

use futures::Stream;
use std::pin::Pin;

use crate::types::content::ContentBlockParam;
use crate::types::ids::SessionId;
use crate::types::messages::{Message, Usage, UserMessage};

use crate::query::config::{QueryConfig, QueryDeps, QueryGates};
use crate::query::query::{self, QueryEvent, QueryState};

// ---------------------------------------------------------------------------
// QueryEngine
// ---------------------------------------------------------------------------

/// Owns the query lifecycle and session state for a conversation.
///
/// Extracts the core logic from `ask()` into a standalone struct that can
/// be used by both the headless/SDK path and (in a future phase) the REPL.
pub struct QueryEngine {
    /// The conversation history (grows across turns).
    messages: Vec<Message>,

    /// Configuration for this engine (model, gates, etc.).
    config: QueryConfig,

    /// Dependency injection handles.
    deps: QueryDeps,

    /// How many turns have been submitted.
    turn_count: u32,

    /// Cumulative token usage across all turns.
    total_usage: Usage,
}

impl QueryEngine {
    /// Create a new engine with the given configuration.
    pub fn new(config: QueryConfig) -> Self {
        Self {
            messages: Vec::new(),
            config,
            deps: QueryDeps::production(),
            turn_count: 0,
            total_usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
        }
    }

    /// Create a new engine with the given configuration and initial messages.
    pub fn with_messages(config: QueryConfig, messages: Vec<Message>) -> Self {
        Self {
            messages,
            config,
            deps: QueryDeps::production(),
            turn_count: 0,
            total_usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
        }
    }

    /// Override the dependency injection handles (for testing).
    pub fn with_deps(mut self, deps: QueryDeps) -> Self {
        self.deps = deps;
        self
    }

    /// Submit a user message and get back a stream of events.
    ///
    /// The returned stream drives the inference loop: it calls the model,
    /// executes tools, handles compaction, and yields events until the turn
    /// completes or is aborted.
    ///
    /// # Arguments
    /// * `content` -- the user message content (text blocks, images, etc.).
    pub async fn submit_message(
        &mut self,
        content: Vec<ContentBlockParam>,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = QueryEvent> + Send>>> {
        self.turn_count += 1;

        // Build the user message.
        let user_message = UserMessage {
            role: "user".to_owned(),
            content: content_block_params_to_content_blocks(content),
            uuid: uuid::Uuid::new_v4(),
            is_bash_input: None,
            is_paste: None,
            is_queued: None,
            command_name: None,
            origin: None,
            is_meta: None,
        };
        self.messages.push(Message::User(user_message));

        // Snapshot messages for the query loop.
        let query_messages = self.messages.clone();
        let state = QueryState::new(query_messages);

        // Start the query loop.
        let stream = query::query(state, self.config.clone(), self.deps.clone());

        Ok(stream)
    }

    /// Submit a plain text message (convenience wrapper).
    pub async fn submit_text(
        &mut self,
        text: impl Into<String>,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = QueryEvent> + Send>>> {
        let content = vec![ContentBlockParam::Text {
            text: text.into(),
        }];
        self.submit_message(content).await
    }

    /// Get the full message history.
    pub fn get_messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get a mutable reference to the message history.
    pub fn get_messages_mut(&mut self) -> &mut Vec<Message> {
        &mut self.messages
    }

    /// Get the current turn count.
    pub fn get_turn_count(&self) -> u32 {
        self.turn_count
    }

    /// Get the cumulative token usage.
    pub fn get_total_usage(&self) -> &Usage {
        &self.total_usage
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &SessionId {
        &self.config.session_id
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &QueryConfig {
        &self.config
    }

    /// Accumulate usage from a turn into the running total.
    pub fn accumulate_usage(&mut self, usage: &Usage) {
        self.total_usage.input_tokens += usage.input_tokens;
        self.total_usage.output_tokens += usage.output_tokens;

        // Accumulate optional cache token fields.
        if let Some(cache_creation) = usage.cache_creation_input_tokens {
            *self
                .total_usage
                .cache_creation_input_tokens
                .get_or_insert(0) += cache_creation;
        }
        if let Some(cache_read) = usage.cache_read_input_tokens {
            *self.total_usage.cache_read_input_tokens.get_or_insert(0) += cache_read;
        }
    }

    /// Clear the conversation history, starting fresh.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.turn_count = 0;
    }
}

// ---------------------------------------------------------------------------
// Convenience builder
// ---------------------------------------------------------------------------

/// Builder for constructing a `QueryEngine` with non-default settings.
pub struct QueryEngineBuilder {
    config: QueryConfig,
    messages: Vec<Message>,
    deps: Option<QueryDeps>,
}

impl QueryEngineBuilder {
    /// Start building with default configuration.
    pub fn new() -> Self {
        Self {
            config: QueryConfig::default(),
            messages: Vec::new(),
            deps: None,
        }
    }

    /// Set the session ID.
    pub fn session_id(mut self, id: SessionId) -> Self {
        self.config.session_id = id;
        self
    }

    /// Set the model.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.config.model = model.into();
        self
    }

    /// Set the max output tokens.
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.config.max_tokens = tokens;
        self
    }

    /// Set the max turns limit.
    pub fn max_turns(mut self, turns: u32) -> Self {
        self.config.gates.max_turns = Some(turns);
        self
    }

    /// Set the thinking configuration.
    pub fn thinking_config(mut self, config: serde_json::Value) -> Self {
        self.config.thinking_config = Some(config);
        self
    }

    /// Set initial messages.
    pub fn messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
        self
    }

    /// Set custom query gates.
    pub fn gates(mut self, gates: QueryGates) -> Self {
        self.config.gates = gates;
        self
    }

    /// Override dependency injection (for testing).
    pub fn deps(mut self, deps: QueryDeps) -> Self {
        self.deps = Some(deps);
        self
    }

    /// Build the engine.
    pub fn build(self) -> QueryEngine {
        let mut engine = if self.messages.is_empty() {
            QueryEngine::new(self.config)
        } else {
            QueryEngine::with_messages(self.config, self.messages)
        };
        if let Some(deps) = self.deps {
            engine = engine.with_deps(deps);
        }
        engine
    }
}

impl Default for QueryEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert `ContentBlockParam`s (input-side) to `ContentBlock`s (message-side).
///
/// The two enums are structurally similar; this maps the common variants.
fn content_block_params_to_content_blocks(
    params: Vec<ContentBlockParam>,
) -> Vec<crate::types::content::ContentBlock> {
    params
        .into_iter()
        .map(|p| match p {
            ContentBlockParam::Text { text } => crate::types::content::ContentBlock::Text { text },
            ContentBlockParam::Image { source } => crate::types::content::ContentBlock::Image {
                source: crate::types::content::ImageSource {
                    source_type: source.source_type,
                    media_type: source.media_type,
                    data: source.data,
                },
            },
            ContentBlockParam::ToolUse { id, name, input } => {
                crate::types::content::ContentBlock::ToolUse { id, name, input }
            }
            ContentBlockParam::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => crate::types::content::ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            },
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_starts_empty() {
        let engine = QueryEngine::new(QueryConfig::default());
        assert_eq!(engine.get_turn_count(), 0);
        assert!(engine.get_messages().is_empty());
    }

    #[test]
    fn engine_accumulates_usage() {
        let mut engine = QueryEngine::new(QueryConfig::default());
        engine.accumulate_usage(&Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: Some(10),
            cache_read_input_tokens: Some(5),
        });
        engine.accumulate_usage(&Usage {
            input_tokens: 200,
            output_tokens: 100,
            cache_creation_input_tokens: Some(20),
            cache_read_input_tokens: None,
        });

        let total = engine.get_total_usage();
        assert_eq!(total.input_tokens, 300);
        assert_eq!(total.output_tokens, 150);
        assert_eq!(total.cache_creation_input_tokens, Some(30));
        assert_eq!(total.cache_read_input_tokens, Some(5));
    }

    #[test]
    fn engine_clear_resets() {
        let mut engine = QueryEngine::new(QueryConfig::default());
        engine.turn_count = 5;
        engine.messages.push(Message::System(
            crate::types::messages::SystemMessage::Informational {
                content: "test".to_owned(),
                level: crate::types::messages::SystemMessageLevel::Info,
            },
        ));
        engine.clear();
        assert_eq!(engine.get_turn_count(), 0);
        assert!(engine.get_messages().is_empty());
    }

    #[test]
    fn builder_produces_engine() {
        let engine = QueryEngineBuilder::new()
            .model("gpt-4o")
            .max_tokens(32768)
            .max_turns(50)
            .build();

        assert_eq!(engine.config().model, "gpt-4o");
        assert_eq!(engine.config().max_tokens, 32768);
        assert_eq!(engine.config().gates.max_turns, Some(50));
    }
}
