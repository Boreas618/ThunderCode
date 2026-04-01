//! Path utilities: tilde expansion, relative path conversion, project root detection.
//!
//! Ported from ref/utils/path.ts`.

use std::path::{Path, PathBuf};

/// Expand a path that may contain tilde notation (`~`) to an absolute path.
///
/// - `~` expands to the user's home directory
/// - `~/path` expands to a path within the home directory
/// - Absolute paths are returned as-is (normalized)
/// - Relative paths are resolved against `base_dir`
///
/// # Examples
/// ```
/// use crate::utils::paths::expand_path;
/// use std::path::PathBuf;
///
/// // Absolute paths pass through
/// let p = expand_path("/usr/bin", "/tmp");
/// assert_eq!(p, PathBuf::from("/usr/bin"));
///
/// // Relative paths resolve against base
/// let p = expand_path("src/main.rs", "/project");
/// assert_eq!(p, PathBuf::from("/project/src/main.rs"));
/// ```
pub fn expand_path(path: &str, base_dir: &str) -> PathBuf {
    let trimmed = path.trim();

    if trimmed.is_empty() {
        return PathBuf::from(base_dir);
    }

    // Home directory expansion
    if trimmed == "~" {
        return home_dir();
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        return home_dir().join(rest);
    }

    let p = Path::new(trimmed);

    // Absolute paths
    if p.is_absolute() {
        return p.to_path_buf();
    }

    // Relative paths resolve against base_dir
    Path::new(base_dir).join(p)
}

/// Convert an absolute path to a relative path from `cwd`.
///
/// If the path is outside `cwd` (the relative form would start with `..`),
/// the absolute path is returned unchanged to keep it unambiguous.
///
/// # Examples
/// ```
/// use crate::utils::paths::to_relative_path;
///
/// assert_eq!(to_relative_path("/project/src/main.rs", "/project"), "src/main.rs");
/// assert_eq!(to_relative_path("/other/file.rs", "/project"), "/other/file.rs");
/// ```
pub fn to_relative_path(absolute_path: &str, cwd: &str) -> String {
    let abs = Path::new(absolute_path);
    let base = Path::new(cwd);

    match pathdiff(abs, base) {
        Some(rel) if !rel.starts_with("..") => rel.to_string_lossy().to_string(),
        _ => absolute_path.to_string(),
    }
}

/// Simple path diff: compute a relative path from `base` to `path`.
///
/// Returns `None` if the paths have no common prefix (different roots on
/// Windows, or one is relative while the other is absolute).
fn pathdiff(path: &Path, base: &Path) -> Option<PathBuf> {
    // Normalize by collecting components
    let path_comps: Vec<_> = path.components().collect();
    let base_comps: Vec<_> = base.components().collect();

    // Find common prefix length
    let common = path_comps
        .iter()
        .zip(base_comps.iter())
        .take_while(|(a, b)| a == b)
        .count();

    if common == 0 && path.is_absolute() != base.is_absolute() {
        return None;
    }

    let mut result = PathBuf::new();
    for _ in common..base_comps.len() {
        result.push("..");
    }
    for comp in &path_comps[common..] {
        result.push(comp);
    }

    Some(result)
}

/// Returns the parent directory for a given path.
///
/// If the path is a directory, returns the path itself. If it looks like a file
/// (has an extension), returns its parent.
pub fn get_directory_for_path(path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.extension().is_none() {
        // Likely a directory
        p.to_path_buf()
    } else if let Some(parent) = p.parent() {
        parent.to_path_buf()
    } else {
        p.to_path_buf()
    }
}

/// Check if a path contains directory traversal patterns (`..`).
///
/// # Examples
/// ```
/// use crate::utils::paths::contains_path_traversal;
/// assert!(contains_path_traversal("../secret"));
/// assert!(contains_path_traversal("foo/../../bar"));
/// assert!(!contains_path_traversal("foo/bar"));
/// ```
pub fn contains_path_traversal(path: &str) -> bool {
    Path::new(path)
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
}

/// Detect the project root by walking up from `start_dir` looking for common
/// project markers (`.git`, `Cargo.toml`, `package.json`, etc.).
///
/// Returns `None` if no project root is found before reaching the filesystem
/// root.
pub fn detect_project_root(start_dir: &Path) -> Option<PathBuf> {
    static MARKERS: &[&str] = &[
        ".git",
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
        "Makefile",
        ".hg",
        "CMakeLists.txt",
        "build.gradle",
        "pom.xml",
    ];

    let mut current = start_dir.to_path_buf();
    loop {
        for marker in MARKERS {
            if current.join(marker).exists() {
                return Some(current);
            }
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Normalize a path for use as a config/JSON key.
///
/// On all platforms, converts backslashes to forward slashes for consistency.
pub fn normalize_path_for_config_key(path: &str) -> String {
    // Use std path normalization, then force forward slashes
    let normalized = Path::new(path)
        .components()
        .collect::<PathBuf>()
        .to_string_lossy()
        .to_string();
    normalized.replace('\\', "/")
}

/// Get the user's home directory, falling back to `/tmp` if unavailable.
fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_path_tilde() {
        let home = home_dir();
        assert_eq!(expand_path("~", "/base"), home);
        assert_eq!(expand_path("~/docs", "/base"), home.join("docs"));
    }

    #[test]
    fn test_expand_path_absolute() {
        assert_eq!(expand_path("/usr/bin", "/base"), PathBuf::from("/usr/bin"));
    }

    #[test]
    fn test_expand_path_relative() {
        assert_eq!(
            expand_path("src/main.rs", "/project"),
            PathBuf::from("/project/src/main.rs")
        );
    }

    #[test]
    fn test_expand_path_empty() {
        assert_eq!(expand_path("", "/base"), PathBuf::from("/base"));
        assert_eq!(expand_path("  ", "/base"), PathBuf::from("/base"));
    }

    #[test]
    fn test_to_relative_path_inside_cwd() {
        assert_eq!(
            to_relative_path("/project/src/main.rs", "/project"),
            "src/main.rs"
        );
    }

    #[test]
    fn test_to_relative_path_outside_cwd() {
        assert_eq!(
            to_relative_path("/other/file.rs", "/project"),
            "/other/file.rs"
        );
    }

    #[test]
    fn test_contains_path_traversal() {
        assert!(contains_path_traversal("../secret"));
        assert!(contains_path_traversal("foo/../../bar"));
        assert!(contains_path_traversal(".."));
        assert!(!contains_path_traversal("foo/bar"));
        assert!(!contains_path_traversal("foo/bar.txt"));
    }

    #[test]
    fn test_normalize_path_for_config_key() {
        assert_eq!(normalize_path_for_config_key("/a/b/c"), "/a/b/c");
        // On unix, backslashes are just characters in filenames, but we still normalize them
        // for cross-platform consistency
    }

    #[test]
    fn test_get_directory_for_path() {
        assert_eq!(
            get_directory_for_path("/project/src/main.rs"),
            PathBuf::from("/project/src")
        );
        assert_eq!(
            get_directory_for_path("/project/src"),
            PathBuf::from("/project/src")
        );
    }
}
