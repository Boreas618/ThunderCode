//! Settings hierarchy -- 5-source precedence and merging.
//!
//! Ported from ref/utils/settings/constants.ts` (sources) and
//! `ref/utils/settings/settings.ts` (merge logic).
//!
//! Sources are merged lowest-to-highest priority:
//!
//! 1. **UserSettings** -- `~/.thundercode/settings.json`
//! 2. **ProjectSettings** -- `.primary/settings.json` (shared, committed)
//! 3. **LocalSettings** -- `.primary/settings.local.json` (gitignored)
//! 4. **FlagSettings** -- `--settings <path>` CLI flag
//! 5. **PolicySettings** -- managed-settings.json or remote managed settings

use crate::config::settings::SettingsJson;

// Re-export the SettingSource enum so consumers can use it from either
// crate::config::hierarchy or thundercode_config (via lib.rs re-export).
pub use crate::types::settings::SettingSource;

// ============================================================================
// Source ordering
// ============================================================================

/// All possible sources in merge-priority order (lowest first).
pub const SETTING_SOURCES: &[SettingSource] = &[
    SettingSource::UserSettings,
    SettingSource::ProjectSettings,
    SettingSource::LocalSettings,
    SettingSource::FlagSettings,
    SettingSource::PolicySettings,
];

/// Editable sources (excludes policy and flag which are read-only).
pub const EDITABLE_SOURCES: &[SettingSource] = &[
    SettingSource::LocalSettings,
    SettingSource::ProjectSettings,
    SettingSource::UserSettings,
];

// ============================================================================
// Display names
// ============================================================================

/// Short lowercase display name for a setting source.
pub fn source_display_name(source: SettingSource) -> &'static str {
    match source {
        SettingSource::UserSettings => "user",
        SettingSource::ProjectSettings => "project",
        SettingSource::LocalSettings => "project, gitignored",
        SettingSource::FlagSettings => "cli flag",
        SettingSource::PolicySettings => "managed",
    }
}

/// Capitalized display name for a setting source.
pub fn source_display_name_capitalized(source: SettingSource) -> &'static str {
    match source {
        SettingSource::UserSettings => "User",
        SettingSource::ProjectSettings => "Project",
        SettingSource::LocalSettings => "Local",
        SettingSource::FlagSettings => "Flag",
        SettingSource::PolicySettings => "Managed",
    }
}

/// Long descriptive display name (lowercase, for inline use).
pub fn source_display_name_long(source: SettingSource) -> &'static str {
    match source {
        SettingSource::UserSettings => "user settings",
        SettingSource::ProjectSettings => "shared project settings",
        SettingSource::LocalSettings => "project local settings",
        SettingSource::FlagSettings => "command line arguments",
        SettingSource::PolicySettings => "enterprise managed settings",
    }
}

// ============================================================================
// Source flag parsing
// ============================================================================

/// Parse the `--setting-sources` CLI flag into `SettingSource` array.
pub fn parse_setting_sources_flag(flag: &str) -> Result<Vec<SettingSource>, String> {
    if flag.is_empty() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for name in flag.split(',').map(str::trim) {
        match name {
            "user" => result.push(SettingSource::UserSettings),
            "project" => result.push(SettingSource::ProjectSettings),
            "local" => result.push(SettingSource::LocalSettings),
            other => {
                return Err(format!(
                    "Invalid setting source: {other}. Valid options are: user, project, local"
                ));
            }
        }
    }
    Ok(result)
}

// ============================================================================
// Settings merging
// ============================================================================

/// Merge `overlay` into `base`, returning a new `SettingsJson`.
///
/// Arrays are concatenated and deduplicated (union merge),
/// matching the lodash `mergeWith` + `settingsMergeCustomizer`
/// behaviour in the TypeScript reference. Scalar/object fields
/// from `overlay` override `base`.
pub fn merge_settings(base: SettingsJson, overlay: SettingsJson) -> SettingsJson {
    // Serialize both to serde_json::Value, deep-merge, then deserialize back.
    let base_val = serde_json::to_value(&base).unwrap_or_default();
    let overlay_val = serde_json::to_value(&overlay).unwrap_or_default();
    let merged = deep_merge_values(base_val, overlay_val);
    serde_json::from_value(merged).unwrap_or_default()
}

/// Deep-merge two JSON values.
///
/// - Objects: recursively merged (overlay keys win).
/// - Arrays: concatenated and deduplicated.
/// - Scalars: overlay wins.
fn deep_merge_values(base: serde_json::Value, overlay: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match (base, overlay) {
        (Value::Object(mut base_map), Value::Object(overlay_map)) => {
            for (key, overlay_v) in overlay_map {
                let merged_v = if let Some(base_v) = base_map.remove(&key) {
                    deep_merge_values(base_v, overlay_v)
                } else {
                    overlay_v
                };
                base_map.insert(key, merged_v);
            }
            Value::Object(base_map)
        }
        (Value::Array(mut base_arr), Value::Array(overlay_arr)) => {
            // Concatenate and deduplicate (preserves order, first occurrence wins).
            for item in overlay_arr {
                if !base_arr.contains(&item) {
                    base_arr.push(item);
                }
            }
            Value::Array(base_arr)
        }
        // Scalar: overlay wins.
        (_, overlay) => overlay,
    }
}

/// Load merged settings from the given sources in priority order.
///
/// This is the main entry point. It reads each source file, merges them
/// lowest-to-highest priority, and returns the effective settings.
///
/// The `project_dir` is needed to resolve project/local settings paths.
/// The `flag_settings_path` is the optional `--settings <path>` argument.
pub fn load_settings(
    sources: &[SettingSource],
    project_dir: Option<&std::path::Path>,
    flag_settings_path: Option<&std::path::Path>,
) -> anyhow::Result<SettingsJson> {
    let mut merged = SettingsJson::default();

    for &source in sources {
        let path = crate::config::paths::settings_file_path_for_source(
            source,
            project_dir,
            flag_settings_path,
        );

        if let Some(ref p) = path {
            match crate::config::loader::load_settings_file(p) {
                Ok(settings) => {
                    merged = merge_settings(merged, settings);
                }
                Err(e) => {
                    // Log but don't fail -- missing files are expected.
                    tracing::debug!(
                        source = ?source,
                        path = %p.display(),
                        error = %e,
                        "Failed to load settings file"
                    );
                }
            }
        }
    }

    Ok(merged)
}

/// Get enabled setting sources with policy/flag always included.
pub fn get_enabled_sources(allowed: &[SettingSource]) -> Vec<SettingSource> {
    let mut set: Vec<SettingSource> = allowed.to_vec();
    if !set.contains(&SettingSource::PolicySettings) {
        set.push(SettingSource::PolicySettings);
    }
    if !set.contains(&SettingSource::FlagSettings) {
        set.push(SettingSource::FlagSettings);
    }
    set
}

/// Check if a specific source is enabled.
pub fn is_source_enabled(source: SettingSource, enabled: &[SettingSource]) -> bool {
    enabled.contains(&source)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_scalar_overlay_wins() {
        let base = SettingsJson {
            model: Some("base-model".into()),
            ..Default::default()
        };
        let overlay = SettingsJson {
            model: Some("overlay-model".into()),
            ..Default::default()
        };
        let merged = merge_settings(base, overlay);
        assert_eq!(merged.model.as_deref(), Some("overlay-model"));
    }

    #[test]
    fn merge_arrays_concatenated_and_deduped() {
        let mut base = SettingsJson::default();
        base.permissions = Some(crate::config::permissions_config::PermissionsSettings {
            allow: Some(vec!["Bash(npm:*)".into(), "Read".into()]),
            ..Default::default()
        });

        let mut overlay = SettingsJson::default();
        overlay.permissions = Some(crate::config::permissions_config::PermissionsSettings {
            allow: Some(vec!["Read".into(), "Write".into()]),
            ..Default::default()
        });

        let merged = merge_settings(base, overlay);
        let allow = merged.permissions.unwrap().allow.unwrap();
        assert!(allow.contains(&"Bash(npm:*)".to_string()));
        assert!(allow.contains(&"Read".to_string()));
        assert!(allow.contains(&"Write".to_string()));
        // "Read" should appear only once (deduplicated).
        assert_eq!(allow.iter().filter(|r| *r == "Read").count(), 1);
    }

    #[test]
    fn merge_none_fields_preserved() {
        let base = SettingsJson {
            model: Some("base-model".into()),
            fast_mode: Some(true),
            ..Default::default()
        };
        let overlay = SettingsJson {
            effort_level: Some(crate::config::settings::EffortLevel::High),
            ..Default::default()
        };
        let merged = merge_settings(base, overlay);
        // base fields survive if overlay has None.
        assert_eq!(merged.model.as_deref(), Some("base-model"));
        assert_eq!(merged.fast_mode, Some(true));
        assert_eq!(merged.effort_level, Some(crate::config::settings::EffortLevel::High));
    }

    #[test]
    fn merge_extra_fields() {
        let mut base = SettingsJson::default();
        base.extra
            .insert("customA".into(), serde_json::json!("hello"));

        let mut overlay = SettingsJson::default();
        overlay
            .extra
            .insert("customB".into(), serde_json::json!(42));

        let merged = merge_settings(base, overlay);
        assert_eq!(merged.extra.get("customA"), Some(&serde_json::json!("hello")));
        assert_eq!(merged.extra.get("customB"), Some(&serde_json::json!(42)));
    }

    #[test]
    fn parse_sources_flag() {
        let result = parse_setting_sources_flag("user,project,local").unwrap();
        assert_eq!(
            result,
            vec![
                SettingSource::UserSettings,
                SettingSource::ProjectSettings,
                SettingSource::LocalSettings,
            ]
        );
    }

    #[test]
    fn parse_sources_flag_empty() {
        let result = parse_setting_sources_flag("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_sources_flag_invalid() {
        let result = parse_setting_sources_flag("user,invalid");
        assert!(result.is_err());
    }

    #[test]
    fn enabled_sources_always_includes_policy_and_flag() {
        let allowed = vec![SettingSource::UserSettings];
        let enabled = get_enabled_sources(&allowed);
        assert!(enabled.contains(&SettingSource::PolicySettings));
        assert!(enabled.contains(&SettingSource::FlagSettings));
        assert!(enabled.contains(&SettingSource::UserSettings));
    }
}
