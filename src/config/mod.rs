//! ThunderCode configuration and settings hierarchy.
//!
//! This crate implements the full settings system ported from the
//! TypeScript reference (`ref/utils/settings/`). It includes:
//!
//! - **`settings`** -- the main `SettingsJson` struct with all configuration fields.
//! - **`hierarchy`** -- 5-source settings merge with proper precedence.
//! - **`paths`** -- platform-specific file path resolution.
//! - **`loader`** -- JSON file loading, validation, and caching.
//! - **`env`** -- `THUNDERCODE_*` environment variable helpers.
//! - **`theme`** -- theme system with 6 built-in themes and 88 color properties.
//! - **`permissions_config`** -- permission rule types for settings files.

pub mod settings;
pub mod hierarchy;
pub mod paths;
pub mod loader;
pub mod env;
pub mod theme;
pub mod permissions_config;

// Re-export the most commonly used items at the crate root.
pub use settings::SettingsJson;
pub use hierarchy::{
    load_settings, merge_settings, SettingSource, SETTING_SOURCES,
};
pub use paths::{
    config_home_dir, managed_settings_dir, settings_file_path_for_source,
    user_settings_path, project_settings_path, local_settings_path,
};
pub use env::{is_env_truthy, is_env_defined_falsy, is_bare_mode};
pub use theme::{Theme, ThemeName, ThemeSetting, get_theme};
