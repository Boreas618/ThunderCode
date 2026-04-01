//! Environment variable helpers.
//!
//! Ported from ref/utils/envUtils.ts`.
//! Provides `THUNDERCODE_*` env var checks and utility functions.

// ============================================================================
// Truthy / falsy checks
// ============================================================================

/// Check if an environment variable value is truthy.
///
/// Returns `true` for `"1"`, `"true"`, `"yes"`, `"on"` (case-insensitive).
/// Returns `false` for `None`, empty string, or any other value.
///
/// Ported from TypeScript `isEnvTruthy`.
pub fn is_env_truthy(value: Option<&str>) -> bool {
    match value {
        None | Some("") => false,
        Some(v) => {
            let normalized = v.to_lowercase().trim().to_string();
            matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
        }
    }
}

/// Check if an environment variable is defined but falsy.
///
/// Returns `true` for `"0"`, `"false"`, `"no"`, `"off"` (case-insensitive).
/// Returns `false` for `None`, empty string, or truthy values.
///
/// Ported from TypeScript `isEnvDefinedFalsy`.
pub fn is_env_defined_falsy(value: Option<&str>) -> bool {
    match value {
        None | Some("") => false,
        Some(v) => {
            let normalized = v.to_lowercase().trim().to_string();
            matches!(normalized.as_str(), "0" | "false" | "no" | "off")
        }
    }
}

// ============================================================================
// Env-var-backed feature gates
// ============================================================================

/// Check if `THUNDERCODE_SIMPLE` is truthy or `--bare` was passed.
///
/// Bare mode skips hooks, LSP, plugin sync, skill dir-walk,
/// attribution, background prefetches, and ALL keychain/credential reads.
pub fn is_bare_mode() -> bool {
    is_env_truthy(std::env::var("THUNDERCODE_SIMPLE").ok().as_deref())
        || std::env::args().any(|a| a == "--bare")
}

/// Whether the coordinator mode is active.
pub fn is_coordinator_mode() -> bool {
    is_env_truthy(
        std::env::var("THUNDERCODE_COORDINATOR_MODE")
            .ok()
            .as_deref(),
    )
}

/// Whether background tasks are disabled.
pub fn is_background_tasks_disabled() -> bool {
    is_env_truthy(
        std::env::var("THUNDERCODE_DISABLE_BACKGROUND_TASKS")
            .ok()
            .as_deref(),
    )
}

/// Whether CCR v2 is in use.
pub fn use_ccr_v2() -> bool {
    is_env_truthy(
        std::env::var("THUNDERCODE_USE_CCR_V2")
            .ok()
            .as_deref(),
    )
}

/// Whether plan verification is enabled.
pub fn verify_plan() -> bool {
    is_env_truthy(
        std::env::var("THUNDERCODE_VERIFY_PLAN")
            .ok()
            .as_deref(),
    )
}

/// Override date for testing (ISO 8601 string or empty).
pub fn override_date() -> Option<String> {
    std::env::var("THUNDERCODE_OVERRIDE_DATE").ok().filter(|s| !s.is_empty())
}

/// Custom entrypoint override.
pub fn entrypoint() -> Option<String> {
    std::env::var("THUNDERCODE_ENTRYPOINT").ok().filter(|s| !s.is_empty())
}

/// Custom config directory override.
pub fn config_dir_override() -> Option<String> {
    std::env::var("THUNDERCODE_CONFIG_DIR").ok().filter(|s| !s.is_empty())
}

/// Whether bash commands should maintain project working directory.
pub fn should_maintain_project_working_dir() -> bool {
    is_env_truthy(
        std::env::var("THUNDERCODE_BASH_MAINTAIN_PROJECT_WORKING_DIR")
            .ok()
            .as_deref(),
    )
}

// ============================================================================
// Env var parsing helpers
// ============================================================================

/// Parse an array of `KEY=VALUE` strings into a HashMap.
pub fn parse_env_vars(raw: &[String]) -> Result<std::collections::HashMap<String, String>, String> {
    let mut map = std::collections::HashMap::new();
    for entry in raw {
        let eq_idx = entry.find('=').ok_or_else(|| {
            format!(
                "Invalid environment variable format: {entry}, \
                 environment variables should be added as: -e KEY1=value1 -e KEY2=value2"
            )
        })?;
        let key = &entry[..eq_idx];
        let value = &entry[eq_idx + 1..];
        if key.is_empty() {
            return Err(format!("Invalid environment variable format: {entry}"));
        }
        map.insert(key.to_string(), value.to_string());
    }
    Ok(map)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truthy_values() {
        assert!(is_env_truthy(Some("1")));
        assert!(is_env_truthy(Some("true")));
        assert!(is_env_truthy(Some("TRUE")));
        assert!(is_env_truthy(Some("True")));
        assert!(is_env_truthy(Some("yes")));
        assert!(is_env_truthy(Some("YES")));
        assert!(is_env_truthy(Some("on")));
        assert!(is_env_truthy(Some("ON")));
    }

    #[test]
    fn non_truthy_values() {
        assert!(!is_env_truthy(None));
        assert!(!is_env_truthy(Some("")));
        assert!(!is_env_truthy(Some("0")));
        assert!(!is_env_truthy(Some("false")));
        assert!(!is_env_truthy(Some("no")));
        assert!(!is_env_truthy(Some("off")));
        assert!(!is_env_truthy(Some("random")));
    }

    #[test]
    fn defined_falsy_values() {
        assert!(is_env_defined_falsy(Some("0")));
        assert!(is_env_defined_falsy(Some("false")));
        assert!(is_env_defined_falsy(Some("FALSE")));
        assert!(is_env_defined_falsy(Some("no")));
        assert!(is_env_defined_falsy(Some("off")));
    }

    #[test]
    fn not_defined_falsy() {
        assert!(!is_env_defined_falsy(None));
        assert!(!is_env_defined_falsy(Some("")));
        assert!(!is_env_defined_falsy(Some("1")));
        assert!(!is_env_defined_falsy(Some("true")));
        assert!(!is_env_defined_falsy(Some("random")));
    }

    #[test]
    fn parse_env_vars_ok() {
        let input = vec![
            "KEY1=value1".to_string(),
            "KEY2=value2=extra".to_string(),
        ];
        let result = parse_env_vars(&input).unwrap();
        assert_eq!(result.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(result.get("KEY2"), Some(&"value2=extra".to_string()));
    }

    #[test]
    fn parse_env_vars_no_equals() {
        let input = vec!["NOEQUALS".to_string()];
        assert!(parse_env_vars(&input).is_err());
    }

    #[test]
    fn parse_env_vars_empty_key() {
        let input = vec!["=value".to_string()];
        assert!(parse_env_vars(&input).is_err());
    }
}
