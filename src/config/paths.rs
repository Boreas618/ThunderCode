//! Settings file path resolution.
//!
//! Ported from ref/utils/envUtils.ts` (config home) and
//! `ref/utils/settings/settings.ts` (per-source paths) and
//! `ref/utils/settings/managedPath.ts` (managed settings).

use std::path::{Path, PathBuf};

use crate::types::settings::SettingSource;

// ============================================================================
// Config home
// ============================================================================

/// Return the ThunderCode configuration home directory.
///
/// Uses `$THUNDERCODE_CONFIG_DIR` if set, otherwise `~/.primary`.
pub fn config_home_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("THUNDERCODE_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".thundercode")
}

// ============================================================================
// User settings
// ============================================================================

/// Path to the user-level settings file.
///
/// `~/.thundercode/settings.json` (or `cowork_settings.json` when cowork mode).
pub fn user_settings_path() -> PathBuf {
    config_home_dir().join("settings.json")
}

// ============================================================================
// Project settings
// ============================================================================

/// Shared project settings (committed to version control).
///
/// `<project_dir>/.thundercode/settings.json`
pub fn project_settings_path(project_dir: &Path) -> PathBuf {
    project_dir.join(".thundercode").join("settings.json")
}

/// Local project settings (gitignored).
///
/// `<project_dir>/.thundercode/settings.local.json`
pub fn local_settings_path(project_dir: &Path) -> PathBuf {
    project_dir.join(".thundercode").join("settings.local.json")
}

/// Relative path to the shared project settings file from the project root.
pub fn relative_project_settings_path() -> PathBuf {
    PathBuf::from(".thundercode").join("settings.json")
}

/// Relative path to the local project settings file from the project root.
pub fn relative_local_settings_path() -> PathBuf {
    PathBuf::from(".thundercode").join("settings.local.json")
}

// ============================================================================
// Managed / policy settings
// ============================================================================

/// Platform-specific managed settings directory.
///
/// - macOS: `/Library/Application Support/ThunderCode`
/// - Linux: `/etc/thundercode`
/// - Windows: `C:\Program Files\ThunderCode`
pub fn managed_settings_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/Library/Application Support/ThunderCode")
    }
    #[cfg(target_os = "linux")]
    {
        PathBuf::from("/etc/thundercode")
    }
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(r"C:\Program Files\ThunderCode")
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        PathBuf::from("/etc/thundercode")
    }
}

/// Path to the managed settings file.
///
/// `<managed_dir>/managed-settings.json`
pub fn managed_settings_file_path() -> PathBuf {
    managed_settings_dir().join("managed-settings.json")
}

/// Path to the managed settings drop-in directory.
///
/// `<managed_dir>/managed-settings.d/`
/// Files in this directory are sorted alphabetically and merged
/// on top of `managed-settings.json` (drop-ins override base).
pub fn managed_settings_drop_in_dir() -> PathBuf {
    managed_settings_dir().join("managed-settings.d")
}

// ============================================================================
// Source-based path resolution
// ============================================================================

/// Get the absolute path to the settings file for a given source.
///
/// - `project_dir` is needed for `ProjectSettings` and `LocalSettings`.
/// - `flag_path` is the `--settings <path>` CLI argument.
///
/// Returns `None` when the necessary context is missing (e.g. no project
/// dir for project-scoped sources, or no flag path for FlagSettings).
pub fn settings_file_path_for_source(
    source: SettingSource,
    project_dir: Option<&Path>,
    flag_path: Option<&Path>,
) -> Option<PathBuf> {
    match source {
        SettingSource::UserSettings => Some(user_settings_path()),
        SettingSource::ProjectSettings => {
            project_dir.map(|d| project_settings_path(d))
        }
        SettingSource::LocalSettings => {
            project_dir.map(|d| local_settings_path(d))
        }
        SettingSource::FlagSettings => flag_path.map(PathBuf::from),
        SettingSource::PolicySettings => Some(managed_settings_file_path()),
    }
}

/// Get the root directory associated with a settings source.
pub fn settings_root_for_source(
    source: SettingSource,
    project_dir: Option<&Path>,
    flag_path: Option<&Path>,
) -> Option<PathBuf> {
    match source {
        SettingSource::UserSettings => Some(config_home_dir()),
        SettingSource::ProjectSettings | SettingSource::LocalSettings => {
            project_dir.map(PathBuf::from)
        }
        SettingSource::FlagSettings => {
            flag_path.and_then(|p| p.parent().map(PathBuf::from))
        }
        SettingSource::PolicySettings => Some(managed_settings_dir()),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_home_respects_env() {
        // Save and restore.
        let prev = std::env::var("THUNDERCODE_CONFIG_DIR").ok();
        std::env::set_var("THUNDERCODE_CONFIG_DIR", "/tmp/test-thundercode");
        assert_eq!(config_home_dir(), PathBuf::from("/tmp/test-thundercode"));
        match prev {
            Some(v) => std::env::set_var("THUNDERCODE_CONFIG_DIR", v),
            None => std::env::remove_var("THUNDERCODE_CONFIG_DIR"),
        }
    }

    #[test]
    fn project_paths() {
        let root = Path::new("/home/user/project");
        assert_eq!(
            project_settings_path(root),
            PathBuf::from("/home/user/project/.thundercode/settings.json")
        );
        assert_eq!(
            local_settings_path(root),
            PathBuf::from("/home/user/project/.thundercode/settings.local.json")
        );
    }

    #[test]
    fn managed_path_is_platform_specific() {
        let dir = managed_settings_dir();
        // Just verify it's not empty.
        assert!(!dir.as_os_str().is_empty());
    }

    #[test]
    fn source_path_returns_none_when_context_missing() {
        // No project dir -- project/local should be None.
        assert!(settings_file_path_for_source(
            SettingSource::ProjectSettings,
            None,
            None,
        ).is_none());
        assert!(settings_file_path_for_source(
            SettingSource::LocalSettings,
            None,
            None,
        ).is_none());
        // No flag path -- flag should be None.
        assert!(settings_file_path_for_source(
            SettingSource::FlagSettings,
            None,
            None,
        ).is_none());
        // User and policy always have paths.
        assert!(settings_file_path_for_source(
            SettingSource::UserSettings,
            None,
            None,
        ).is_some());
        assert!(settings_file_path_for_source(
            SettingSource::PolicySettings,
            None,
            None,
        ).is_some());
    }
}
