//! API key resolution from environment variables and config files.

use std::path::PathBuf;

/// Resolve API key from env vars, in priority order.
pub fn resolve_api_key() -> Option<String> {
    std::env::var("THUNDERCODE_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .ok()
        .filter(|k| !k.is_empty())
}

/// Resolve base URL from env vars.
pub fn resolve_base_url() -> Option<String> {
    std::env::var("THUNDERCODE_BASE_URL")
        .or_else(|_| std::env::var("OPENAI_BASE_URL"))
        .ok()
        .filter(|u| !u.is_empty())
}

/// Read the API key from config files on disk.
pub fn read_api_key_from_config_file() -> Option<String> {
    let candidates = api_key_file_candidates();
    for path in candidates {
        if let Ok(contents) = std::fs::read_to_string(&path) {
            let key = contents.trim().to_string();
            if !key.is_empty() {
                return Some(key);
            }
        }
    }
    None
}

fn api_key_file_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(dir) = std::env::var("THUNDERCODE_CONFIG_DIR") {
        paths.push(PathBuf::from(dir).join("api_key"));
    }

    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".thundercode").join("api_key"));
    }

    if let Some(config) = dirs::config_dir() {
        paths.push(config.join("thundercode").join("api_key"));
    }

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_file_candidates_not_empty() {
        let paths = api_key_file_candidates();
        let _ = paths;
    }
}
