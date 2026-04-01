//! Bridge authentication and URL resolution.
//!
//! Consolidates bridge auth/URL overrides. Two layers:
//! - Override functions return env var overrides (or None).
//! - Non-override functions fall through to the real OAuth store/config.
//!
//! Ported from ref/bridge/bridgeConfig.ts`.

use std::env;

/// Environment variable for bridge OAuth token override.
const BRIDGE_OAUTH_TOKEN_ENV: &str = "THUNDERCODE_BRIDGE_OAUTH_TOKEN";

/// Environment variable for bridge base URL override.
const BRIDGE_BASE_URL_ENV: &str = "THUNDERCODE_BRIDGE_BASE_URL";

/// Default API base URL.
const DEFAULT_API_BASE_URL: &str = "https://api.openai.com";

/// Get the bridge OAuth token override from environment, if set.
pub fn get_bridge_token_override() -> Option<String> {
    env::var(BRIDGE_OAUTH_TOKEN_ENV).ok().filter(|s| !s.is_empty())
}

/// Get the bridge base URL override from environment, if set.
pub fn get_bridge_base_url_override() -> Option<String> {
    env::var(BRIDGE_BASE_URL_ENV).ok().filter(|s| !s.is_empty())
}

/// Get the access token for bridge API calls.
///
/// Checks the env override first, then falls back to the OAuth keychain.
/// Returns `None` if not logged in.
pub fn get_bridge_access_token() -> Option<String> {
    get_bridge_token_override()
    // In a full implementation, this would fall through to:
    // .or_else(|| get_oauth_tokens().map(|t| t.access_token))
}

/// Get the base URL for bridge API calls.
///
/// Checks the env override first, then falls back to the production config.
pub fn get_bridge_base_url() -> String {
    get_bridge_base_url_override().unwrap_or_else(|| DEFAULT_API_BASE_URL.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_base_url() {
        // Remove override if set
        env::remove_var(BRIDGE_BASE_URL_ENV);
        let url = get_bridge_base_url();
        assert_eq!(url, DEFAULT_API_BASE_URL);
    }

    #[test]
    fn test_base_url_override() {
        env::set_var(BRIDGE_BASE_URL_ENV, "https://custom.api.com");
        let url = get_bridge_base_url();
        assert_eq!(url, "https://custom.api.com");
        env::remove_var(BRIDGE_BASE_URL_ENV);
    }

    #[test]
    fn test_token_override_none_when_unset() {
        env::remove_var(BRIDGE_OAUTH_TOKEN_ENV);
        assert!(get_bridge_token_override().is_none());
    }

    #[test]
    fn test_token_override_empty_string() {
        env::set_var(BRIDGE_OAUTH_TOKEN_ENV, "");
        assert!(get_bridge_token_override().is_none());
        env::remove_var(BRIDGE_OAUTH_TOKEN_ENV);
    }
}
