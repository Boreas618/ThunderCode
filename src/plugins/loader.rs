//! Plugin loading from the filesystem.
//!
//! Loads plugin manifests (JSON) from individual paths or entire directories.
//! Each plugin directory is expected to contain a `plugin.json` manifest file.
//!
//! Ported from plugin loading logic in the TypeScript reference codebase.

use anyhow::{Context, Result};
use crate::types::plugin::PluginManifest;
use std::path::{Path, PathBuf};
use tracing::warn;

/// The expected manifest filename inside a plugin directory.
const MANIFEST_FILENAME: &str = "plugin.json";

// ---------------------------------------------------------------------------
// Single-plugin loading
// ---------------------------------------------------------------------------

/// Load a plugin manifest from a directory path.
///
/// Expects `path` to be a directory containing a `plugin.json` file. If `path`
/// points directly to a JSON file, that file is read as the manifest instead.
pub async fn load_plugin_from_path(path: &Path) -> Result<PluginManifest> {
    let manifest_path = if path.is_file() {
        path.to_path_buf()
    } else {
        path.join(MANIFEST_FILENAME)
    };

    let contents = tokio::fs::read_to_string(&manifest_path)
        .await
        .with_context(|| format!("failed to read manifest at {}", manifest_path.display()))?;

    let manifest: PluginManifest = serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse manifest at {}", manifest_path.display()))?;

    Ok(manifest)
}

// ---------------------------------------------------------------------------
// Directory loading
// ---------------------------------------------------------------------------

/// Load all plugin manifests from a plugins directory.
///
/// Each subdirectory of `dir` is treated as a plugin directory and checked for
/// a `plugin.json` file. Results are returned per-plugin so callers can handle
/// individual failures without losing the entire batch.
pub async fn load_plugins_from_directory(dir: &Path) -> Vec<Result<PluginManifest>> {
    let mut results = Vec::new();

    let entries = match tokio::fs::read_dir(dir).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return results,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            warn!("permission denied reading plugins dir: {}", dir.display());
            return results;
        }
        Err(e) => {
            results.push(Err(e.into()));
            return results;
        }
    };

    let mut entries = entries;
    loop {
        let entry = match entries.next_entry().await {
            Ok(Some(entry)) => entry,
            Ok(None) => break,
            Err(e) => {
                warn!("error reading dir entry in {}: {}", dir.display(), e);
                continue;
            }
        };

        let entry_path = entry.path();

        // Only process directories (or symlinks that might point to dirs).
        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if !file_type.is_dir() && !file_type.is_symlink() {
            continue;
        }

        let manifest_path = entry_path.join(MANIFEST_FILENAME);
        if !manifest_exists(&manifest_path).await {
            continue;
        }

        results.push(load_plugin_from_path(&entry_path).await);
    }

    results
}

/// Check whether a manifest file exists at the given path.
async fn manifest_exists(path: &PathBuf) -> bool {
    tokio::fs::metadata(path).await.is_ok()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_manifest(dir: &Path, name: &str) {
        let plugin_dir = dir.join(name);
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let manifest = serde_json::json!({
            "name": name,
            "description": format!("{} plugin", name),
            "version": "1.0.0"
        });
        std::fs::write(
            plugin_dir.join(MANIFEST_FILENAME),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_load_plugin_from_path() {
        let tmp = TempDir::new().unwrap();
        write_manifest(tmp.path(), "test-plugin");

        let manifest = load_plugin_from_path(&tmp.path().join("test-plugin"))
            .await
            .unwrap();
        assert_eq!(manifest.name, "test-plugin");
        assert_eq!(manifest.description.as_deref(), Some("test-plugin plugin"));
        assert_eq!(manifest.version.as_deref(), Some("1.0.0"));
    }

    #[tokio::test]
    async fn test_load_plugin_from_json_file() {
        let tmp = TempDir::new().unwrap();
        let json_path = tmp.path().join("custom.json");
        let manifest = serde_json::json!({
            "name": "direct-file",
            "description": "loaded from file"
        });
        std::fs::write(&json_path, serde_json::to_string(&manifest).unwrap()).unwrap();

        let result = load_plugin_from_path(&json_path).await.unwrap();
        assert_eq!(result.name, "direct-file");
    }

    #[tokio::test]
    async fn test_load_plugin_missing_manifest() {
        let tmp = TempDir::new().unwrap();
        let empty_dir = tmp.path().join("no-manifest");
        std::fs::create_dir_all(&empty_dir).unwrap();

        let result = load_plugin_from_path(&empty_dir).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_plugin_invalid_json() {
        let tmp = TempDir::new().unwrap();
        let plugin_dir = tmp.path().join("bad-json");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join(MANIFEST_FILENAME), "not json {{{").unwrap();

        let result = load_plugin_from_path(&plugin_dir).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_plugins_from_directory() {
        let tmp = TempDir::new().unwrap();
        let plugins_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        write_manifest(&plugins_dir, "plugin-a");
        write_manifest(&plugins_dir, "plugin-b");

        let results = load_plugins_from_directory(&plugins_dir).await;
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_ok()));

        let mut names: Vec<String> = results
            .into_iter()
            .map(|r| r.unwrap().name)
            .collect();
        names.sort();
        assert_eq!(names, vec!["plugin-a", "plugin-b"]);
    }

    #[tokio::test]
    async fn test_load_plugins_from_empty_directory() {
        let tmp = TempDir::new().unwrap();
        let plugins_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        let results = load_plugins_from_directory(&plugins_dir).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_load_plugins_from_nonexistent_directory() {
        let tmp = TempDir::new().unwrap();
        let results = load_plugins_from_directory(&tmp.path().join("nope")).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_load_plugins_skips_files() {
        let tmp = TempDir::new().unwrap();
        let plugins_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        // A stray file at the top level should be ignored.
        std::fs::write(plugins_dir.join("stray.txt"), "not a plugin").unwrap();

        write_manifest(&plugins_dir, "real-plugin");

        let results = load_plugins_from_directory(&plugins_dir).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].as_ref().unwrap().name, "real-plugin");
    }

    #[tokio::test]
    async fn test_load_plugins_skips_dirs_without_manifest() {
        let tmp = TempDir::new().unwrap();
        let plugins_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(plugins_dir.join("empty-dir")).unwrap();

        write_manifest(&plugins_dir, "has-manifest");

        let results = load_plugins_from_directory(&plugins_dir).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].as_ref().unwrap().name, "has-manifest");
    }
}
