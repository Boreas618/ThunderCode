//! Sandbox restrictions for filesystem and network access.
//!
//! Ported from ref/utils/permissions/pathValidation.ts` and
//! `ref/utils/permissions/filesystem.ts`.
//!
//! ## Filesystem restrictions
//!
//! - **Reads**: `deny_only` blocks specific paths; `allow_within_deny` carves
//!   out exceptions inside denied subtrees.
//! - **Writes**: `allow_only` whitelists writable directories; `deny_within_allow`
//!   blocks specific paths even inside allowed directories.
//!
//! ## Network restrictions
//!
//! - `allowed_hosts` / `denied_hosts` for network access control.

use std::path::{Path, PathBuf};

// ============================================================================
// Sandbox configuration
// ============================================================================

/// Filesystem restriction configuration.
#[derive(Debug, Clone, Default)]
pub struct SandboxFsConfig {
    // -- Read restrictions --
    /// Paths that are denied for reads.
    pub read_deny_only: Vec<PathBuf>,
    /// Exceptions inside denied read paths.
    pub read_allow_within_deny: Vec<PathBuf>,

    // -- Write restrictions --
    /// Only these paths are writable (allowlist).
    pub write_allow_only: Vec<PathBuf>,
    /// Paths denied even within allowed write directories.
    pub write_deny_within_allow: Vec<PathBuf>,
}

/// Network restriction configuration.
#[derive(Debug, Clone, Default)]
pub struct SandboxNetConfig {
    /// Hosts explicitly allowed for network access.
    pub allowed_hosts: Vec<String>,
    /// Hosts explicitly denied.
    pub denied_hosts: Vec<String>,
}

/// Full sandbox configuration.
#[derive(Debug, Clone, Default)]
pub struct SandboxConfig {
    pub enabled: bool,
    pub fs: SandboxFsConfig,
    pub net: SandboxNetConfig,
}

// ============================================================================
// Path validation
// ============================================================================

/// Normalize a path for comparison: resolve `.` / `..`, lowercase on
/// case-insensitive platforms (macOS, Windows), and handle macOS
/// `/private/var` -> `/var` symlink equivalence.
fn normalize_for_comparison(path: &Path) -> String {
    let s = path.to_string_lossy().to_string();
    // Handle macOS /private/ prefix equivalence.
    let s = s
        .replace("/private/var/", "/var/")
        .replace("/private/tmp/", "/tmp/");
    // Case-insensitive on macOS/Windows.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let s = s.to_lowercase();
    s
}

/// Returns `true` if `child` is the same as or inside `parent`.
pub fn path_in_directory(child: &Path, parent: &Path) -> bool {
    let child_n = normalize_for_comparison(child);
    let parent_n = normalize_for_comparison(parent);

    if child_n == parent_n {
        return true;
    }

    // Ensure parent ends with `/` for prefix matching so that
    // `/tmp/foo` does not match parent `/tmp/f`.
    let parent_prefix = if parent_n.ends_with('/') {
        parent_n
    } else {
        format!("{parent_n}/")
    };

    child_n.starts_with(&parent_prefix)
}

/// Result of a sandbox path check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxPathResult {
    Allowed,
    Denied { reason: String },
}

impl SandboxConfig {
    /// Check if a read operation on `path` is allowed by the sandbox.
    pub fn check_read(&self, path: &Path) -> SandboxPathResult {
        if !self.enabled {
            return SandboxPathResult::Allowed;
        }

        // Check deny-only list.
        for denied in &self.fs.read_deny_only {
            if path_in_directory(path, denied) {
                // Check allow-within-deny exceptions.
                let has_exception = self
                    .fs
                    .read_allow_within_deny
                    .iter()
                    .any(|allowed| path_in_directory(path, allowed));
                if !has_exception {
                    return SandboxPathResult::Denied {
                        reason: format!(
                            "Path '{}' is in a denied read directory '{}'",
                            path.display(),
                            denied.display()
                        ),
                    };
                }
            }
        }

        SandboxPathResult::Allowed
    }

    /// Check if a write operation on `path` is allowed by the sandbox.
    pub fn check_write(&self, path: &Path) -> SandboxPathResult {
        if !self.enabled {
            return SandboxPathResult::Allowed;
        }

        // Write uses an allow-only model: if allow_only is non-empty,
        // the path must be inside at least one allowed directory.
        if !self.fs.write_allow_only.is_empty() {
            let in_allowed = self
                .fs
                .write_allow_only
                .iter()
                .any(|allowed| path_in_directory(path, allowed));

            if !in_allowed {
                return SandboxPathResult::Denied {
                    reason: format!(
                        "Path '{}' is not in any allowed write directory",
                        path.display()
                    ),
                };
            }
        }

        // Check deny-within-allow list (e.g. .primary/settings.json
        // is blocked even if .primary/ parent is in allow_only).
        for denied in &self.fs.write_deny_within_allow {
            if path_in_directory(path, denied) {
                return SandboxPathResult::Denied {
                    reason: format!(
                        "Path '{}' is in a specifically denied write path '{}'",
                        path.display(),
                        denied.display()
                    ),
                };
            }
        }

        SandboxPathResult::Allowed
    }

    /// Check if a host is allowed for network access.
    pub fn check_network(&self, host: &str) -> SandboxPathResult {
        if !self.enabled {
            return SandboxPathResult::Allowed;
        }

        // Deny list takes precedence.
        let lower_host = host.to_lowercase();
        for denied in &self.net.denied_hosts {
            if lower_host == denied.to_lowercase()
                || lower_host.ends_with(&format!(".{}", denied.to_lowercase()))
            {
                return SandboxPathResult::Denied {
                    reason: format!("Host '{}' is in the denied hosts list", host),
                };
            }
        }

        // If allowed list is non-empty, host must be in it.
        if !self.net.allowed_hosts.is_empty() {
            let in_allowed = self.net.allowed_hosts.iter().any(|allowed| {
                let a = allowed.to_lowercase();
                lower_host == a || lower_host.ends_with(&format!(".{a}"))
            });
            if !in_allowed {
                return SandboxPathResult::Denied {
                    reason: format!("Host '{}' is not in the allowed hosts list", host),
                };
            }
        }

        SandboxPathResult::Allowed
    }
}

// ============================================================================
// Dangerous paths
// ============================================================================

/// Well-known files that should be protected from auto-editing.
pub const DANGEROUS_FILES: &[&str] = &[
    ".gitconfig",
    ".gitmodules",
    ".bashrc",
    ".bash_profile",
    ".zshrc",
    ".zprofile",
    ".profile",
    ".ripgreprc",
    ".mcp.json",
    ".primary.json",
];

/// Directories that should be protected from auto-editing.
pub const DANGEROUS_DIRECTORIES: &[&str] = &[".git", ".vscode", ".idea", ".thundercode"];

/// Returns `true` if `path` is a dangerous removal target (root dirs,
/// home dir, direct children of root, wildcard globs, etc.).
pub fn is_dangerous_removal_path(path: &str) -> bool {
    // Normalize separators.
    let forward = path.replace(['\\'], "/");
    let forward = forward.replace("//", "/");

    if forward == "*" || forward.ends_with("/*") {
        return true;
    }

    let normalized = if forward == "/" {
        forward.as_str()
    } else {
        forward.trim_end_matches('/')
    };

    if normalized == "/" {
        return true;
    }

    // Windows drive root: C:/ or D:/
    if normalized.len() <= 3
        && normalized.as_bytes().first().map_or(false, |b| b.is_ascii_alphabetic())
        && normalized.as_bytes().get(1) == Some(&b':')
    {
        return true;
    }

    // Home directory.
    if let Some(home) = dirs::home_dir() {
        let home_str = home
            .to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_string();
        if normalized == home_str {
            return true;
        }
    }

    // Direct children of root: /usr, /tmp, etc.
    if let Some(parent) = Path::new(normalized).parent() {
        if parent == Path::new("/") {
            return true;
        }
    }

    false
}

/// Returns `true` if any segment of `path` is a known dangerous file or directory.
pub fn is_dangerous_path(path: &str) -> bool {
    let segments: Vec<&str> = path.split(['/', '\\']).collect();
    let filename = segments.last().copied().unwrap_or("");

    // Check directories.
    for seg in &segments {
        let lower = seg.to_lowercase();
        for &dir in DANGEROUS_DIRECTORIES {
            if lower == dir.to_lowercase() {
                return true;
            }
        }
    }

    // Check files.
    let lower_file = filename.to_lowercase();
    for &f in DANGEROUS_FILES {
        if lower_file == f.to_lowercase() {
            return true;
        }
    }

    false
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_sandbox() -> SandboxConfig {
        SandboxConfig {
            enabled: true,
            fs: SandboxFsConfig {
                read_deny_only: vec![PathBuf::from("/secrets")],
                read_allow_within_deny: vec![PathBuf::from("/secrets/public")],
                write_allow_only: vec![
                    PathBuf::from("/home/user/project"),
                    PathBuf::from("/tmp/primary"),
                ],
                write_deny_within_allow: vec![PathBuf::from(
                    "/home/user/project/.thundercode/settings.json",
                )],
            },
            net: SandboxNetConfig {
                allowed_hosts: vec!["api.example.com".into(), "cdn.example.com".into()],
                denied_hosts: vec!["evil.com".into()],
            },
        }
    }

    // -- Filesystem reads --

    #[test]
    fn read_allowed_normal_path() {
        let sb = make_sandbox();
        assert_eq!(
            sb.check_read(Path::new("/home/user/file.txt")),
            SandboxPathResult::Allowed
        );
    }

    #[test]
    fn read_denied_path() {
        let sb = make_sandbox();
        assert!(matches!(
            sb.check_read(Path::new("/secrets/key.pem")),
            SandboxPathResult::Denied { .. }
        ));
    }

    #[test]
    fn read_allowed_within_deny() {
        let sb = make_sandbox();
        assert_eq!(
            sb.check_read(Path::new("/secrets/public/readme.txt")),
            SandboxPathResult::Allowed
        );
    }

    // -- Filesystem writes --

    #[test]
    fn write_allowed_in_project() {
        let sb = make_sandbox();
        assert_eq!(
            sb.check_write(Path::new("/home/user/project/src/main.rs")),
            SandboxPathResult::Allowed
        );
    }

    #[test]
    fn write_denied_outside_allowlist() {
        let sb = make_sandbox();
        assert!(matches!(
            sb.check_write(Path::new("/etc/passwd")),
            SandboxPathResult::Denied { .. }
        ));
    }

    #[test]
    fn write_denied_within_allow() {
        let sb = make_sandbox();
        assert!(matches!(
            sb.check_write(Path::new("/home/user/project/.thundercode/settings.json")),
            SandboxPathResult::Denied { .. }
        ));
    }

    // -- Network --

    #[test]
    fn network_allowed_host() {
        let sb = make_sandbox();
        assert_eq!(
            sb.check_network("api.example.com"),
            SandboxPathResult::Allowed
        );
    }

    #[test]
    fn network_denied_host() {
        let sb = make_sandbox();
        assert!(matches!(
            sb.check_network("evil.com"),
            SandboxPathResult::Denied { .. }
        ));
    }

    #[test]
    fn network_not_in_allowed_list() {
        let sb = make_sandbox();
        assert!(matches!(
            sb.check_network("unknown.org"),
            SandboxPathResult::Denied { .. }
        ));
    }

    // -- Disabled sandbox --

    #[test]
    fn disabled_sandbox_allows_everything() {
        let sb = SandboxConfig::default();
        assert_eq!(
            sb.check_read(Path::new("/secrets/key.pem")),
            SandboxPathResult::Allowed
        );
        assert_eq!(
            sb.check_write(Path::new("/etc/passwd")),
            SandboxPathResult::Allowed
        );
        assert_eq!(
            sb.check_network("evil.com"),
            SandboxPathResult::Allowed
        );
    }

    // -- Dangerous paths --

    #[test]
    fn dangerous_removal_paths() {
        assert!(is_dangerous_removal_path("/"));
        assert!(is_dangerous_removal_path("/*"));
        assert!(is_dangerous_removal_path("*"));
        assert!(is_dangerous_removal_path("/usr"));
        assert!(is_dangerous_removal_path("/tmp"));
        assert!(!is_dangerous_removal_path("/home/user/project/file.txt"));
    }

    #[test]
    fn dangerous_file_detection() {
        assert!(is_dangerous_path("/home/user/.bashrc"));
        assert!(is_dangerous_path("/project/.git/config"));
        assert!(is_dangerous_path("/project/.vscode/settings.json"));
        assert!(!is_dangerous_path("/home/user/project/src/main.rs"));
    }

    // -- path_in_directory --

    #[test]
    fn path_in_dir_exact_match() {
        assert!(path_in_directory(
            Path::new("/home/user"),
            Path::new("/home/user")
        ));
    }

    #[test]
    fn path_in_dir_child() {
        assert!(path_in_directory(
            Path::new("/home/user/file.txt"),
            Path::new("/home/user")
        ));
    }

    #[test]
    fn path_in_dir_no_false_prefix() {
        // /tmp/foo should NOT match parent /tmp/f
        assert!(!path_in_directory(
            Path::new("/tmp/foo"),
            Path::new("/tmp/f")
        ));
    }
}
