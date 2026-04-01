//! Environment variable utilities.
//!
//! Ported from ref/utils/env.ts` and `ref/utils/envUtils.ts`. Provides
//! helpers for reading, parsing, and classifying environment variables used
//! throughout ThunderCode (especially `THUNDERCODE_*` / `THUNDERCODE_*` vars).

use std::path::PathBuf;

/// Check if an environment variable value is "truthy".
///
/// Returns `true` for `"1"`, `"true"`, `"yes"`, `"on"` (case-insensitive).
///
/// # Examples
/// ```
/// use crate::utils::env::is_env_truthy;
/// assert!(is_env_truthy(Some("1")));
/// assert!(is_env_truthy(Some("true")));
/// assert!(is_env_truthy(Some("YES")));
/// assert!(!is_env_truthy(Some("0")));
/// assert!(!is_env_truthy(None));
/// ```
pub fn is_env_truthy(val: Option<&str>) -> bool {
    match val {
        None => false,
        Some(s) => {
            let normalized = s.trim().to_lowercase();
            matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
        }
    }
}

/// Check if an environment variable is explicitly set to a falsy value.
///
/// Returns `true` for `"0"`, `"false"`, `"no"`, `"off"` (case-insensitive).
/// Returns `false` if the var is unset (unlike `!is_env_truthy`).
///
/// # Examples
/// ```
/// use crate::utils::env::is_env_defined_falsy;
/// assert!(is_env_defined_falsy(Some("0")));
/// assert!(is_env_defined_falsy(Some("false")));
/// assert!(!is_env_defined_falsy(None)); // not defined at all
/// assert!(!is_env_defined_falsy(Some("1")));
/// ```
pub fn is_env_defined_falsy(val: Option<&str>) -> bool {
    match val {
        None => false,
        Some(s) => {
            if s.is_empty() {
                return false;
            }
            let normalized = s.trim().to_lowercase();
            matches!(normalized.as_str(), "0" | "false" | "no" | "off")
        }
    }
}

/// Read an env var by name, returning `None` if unset or empty.
pub fn get_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Read an env var as a `bool` (truthy check). Returns `false` if unset.
pub fn get_env_bool(name: &str) -> bool {
    is_env_truthy(std::env::var(name).ok().as_deref())
}

/// Read an env var as a `u64`. Returns `None` if unset, empty, or not a valid number.
pub fn get_env_u64(name: &str) -> Option<u64> {
    get_env(name).and_then(|v| v.parse().ok())
}

/// Read an env var as a `Duration` in milliseconds. Returns `None` if unset or invalid.
pub fn get_env_duration_ms(name: &str) -> Option<std::time::Duration> {
    get_env_u64(name).map(std::time::Duration::from_millis)
}

/// Get the ThunderCode config home directory.
///
/// Reads `THUNDERCODE_CONFIG_DIR` (for compatibility) or `THUNDERCODE_CONFIG_DIR`,
/// falling back to `~/.primary`.
pub fn get_config_home_dir() -> PathBuf {
    if let Some(dir) = get_env("THUNDERCODE_CONFIG_DIR").or_else(|| get_env("THUNDERCODE_CONFIG_DIR")) {
        PathBuf::from(dir)
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".thundercode")
    }
}

/// Check if we are running in a CI environment.
pub fn is_ci() -> bool {
    get_env_bool("CI")
}

/// Detect the current platform as a simple string.
pub fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "win32"
    } else {
        "linux"
    }
}

/// Check if running inside an SSH session.
pub fn is_ssh_session() -> bool {
    get_env("SSH_CONNECTION").is_some()
        || get_env("SSH_CLIENT").is_some()
        || get_env("SSH_TTY").is_some()
}

/// Parse an array of `KEY=VALUE` strings into a `Vec<(String, String)>`.
///
/// # Errors
/// Returns an error if any string does not contain `=`.
///
/// # Examples
/// ```
/// use crate::utils::env::parse_env_vars;
/// let vars = parse_env_vars(&["FOO=bar", "BAZ=qux=more"]).unwrap();
/// assert_eq!(vars, vec![("FOO".into(), "bar".into()), ("BAZ".into(), "qux=more".into())]);
/// ```
pub fn parse_env_vars(raw: &[&str]) -> Result<Vec<(String, String)>, String> {
    raw.iter()
        .map(|s| {
            let eq_pos = s.find('=').ok_or_else(|| {
                format!(
                    "Invalid environment variable format: {}, expected KEY=VALUE",
                    s
                )
            })?;
            let key = &s[..eq_pos];
            let value = &s[eq_pos + 1..];
            if key.is_empty() {
                return Err(format!(
                    "Invalid environment variable format: {}, key is empty",
                    s
                ));
            }
            Ok((key.to_string(), value.to_string()))
        })
        .collect()
}

/// Get the AWS region with fallback to `us-east-1`.
pub fn get_aws_region() -> String {
    get_env("AWS_REGION")
        .or_else(|| get_env("AWS_DEFAULT_REGION"))
        .unwrap_or_else(|| "us-east-1".to_string())
}

/// Detect the deployment environment from common env vars.
///
/// Returns a string like `"github-actions"`, `"docker"`, `"codespaces"`, etc.
/// Falls back to `"unknown"`.
pub fn detect_deployment_environment() -> &'static str {
    if get_env_bool("CODESPACES") {
        return "codespaces";
    }
    if get_env("GITPOD_WORKSPACE_ID").is_some() {
        return "gitpod";
    }
    if get_env("REPL_ID").is_some() || get_env("REPL_SLUG").is_some() {
        return "replit";
    }
    if get_env_bool("GITHUB_ACTIONS") {
        return "github-actions";
    }
    if get_env_bool("GITLAB_CI") {
        return "gitlab-ci";
    }
    if get_env_bool("CI") {
        return "ci";
    }
    if get_env("KUBERNETES_SERVICE_HOST").is_some() {
        return "kubernetes";
    }
    if std::path::Path::new("/.dockerenv").exists() {
        return "docker";
    }
    "unknown"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_env_truthy() {
        assert!(is_env_truthy(Some("1")));
        assert!(is_env_truthy(Some("true")));
        assert!(is_env_truthy(Some("TRUE")));
        assert!(is_env_truthy(Some("yes")));
        assert!(is_env_truthy(Some("YES")));
        assert!(is_env_truthy(Some("on")));
        assert!(is_env_truthy(Some("ON")));
        assert!(is_env_truthy(Some(" true ")));
        assert!(!is_env_truthy(Some("0")));
        assert!(!is_env_truthy(Some("false")));
        assert!(!is_env_truthy(Some("")));
        assert!(!is_env_truthy(None));
    }

    #[test]
    fn test_is_env_defined_falsy() {
        assert!(is_env_defined_falsy(Some("0")));
        assert!(is_env_defined_falsy(Some("false")));
        assert!(is_env_defined_falsy(Some("FALSE")));
        assert!(is_env_defined_falsy(Some("no")));
        assert!(is_env_defined_falsy(Some("off")));
        assert!(!is_env_defined_falsy(None));
        assert!(!is_env_defined_falsy(Some("")));
        assert!(!is_env_defined_falsy(Some("1")));
    }

    #[test]
    fn test_parse_env_vars() {
        let vars = parse_env_vars(&["FOO=bar", "BAZ=qux=more"]).unwrap();
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0], ("FOO".to_string(), "bar".to_string()));
        assert_eq!(vars[1], ("BAZ".to_string(), "qux=more".to_string()));
    }

    #[test]
    fn test_parse_env_vars_error() {
        let result = parse_env_vars(&["NO_EQUALS"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_env_vars_empty_key() {
        let result = parse_env_vars(&["=value"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_platform() {
        let p = platform();
        assert!(["darwin", "linux", "win32"].contains(&p));
    }

    #[test]
    fn test_get_config_home_dir() {
        // Just ensure it doesn't panic and returns a path
        let dir = get_config_home_dir();
        assert!(!dir.to_string_lossy().is_empty());
    }
}
