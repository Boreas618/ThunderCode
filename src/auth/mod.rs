//! Authentication helpers.
//!
//! Provider-neutral: just resolves an API key from environment or config.

pub mod api_key;

pub use api_key::resolve_api_key;
