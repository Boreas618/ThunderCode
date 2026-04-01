//! Model information helpers.
//!
//! Provider-neutral: the model ID is an opaque string passed through to the
//! API endpoint.  Helpers here provide sensible defaults and optional metadata.

/// Metadata for a model (all fields optional except id).
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub max_output_tokens: u32,
}

/// Resolve a user-facing model name.
///
/// Unknown names pass through unchanged -- the endpoint decides validity.
pub fn resolve_model_name(name: &str) -> String {
    name.to_owned()
}

/// Get the default model ID.
pub fn default_model() -> &'static str {
    "gpt-4o"
}

/// Get the small/fast model ID.
pub fn small_fast_model() -> &'static str {
    "gpt-4o-mini"
}

/// Default max output tokens for an unknown model.
pub fn max_output_tokens(_model_id: &str) -> u32 {
    16_384
}

/// Look up model metadata by ID.
///
/// Returns `None` for all models -- we don't maintain a built-in catalog.
/// Callers should handle the `None` case with sensible defaults.
pub fn get_model_info(_model_id: &str) -> Option<ModelInfo> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_passthrough() {
        assert_eq!(resolve_model_name("my-custom-model"), "my-custom-model");
    }

    #[test]
    fn unknown_model_returns_none() {
        assert!(get_model_info("anything").is_none());
    }
}
