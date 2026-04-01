//! Task output management -- disk-based output files for background tasks.
//!
//! Ported from ref/utils/task/diskOutput.ts.
//!
//! Each task writes its stdout/stderr to a file under `~/.thundercode/task-output/`.
//! Readers can request content from a byte offset to get deltas without
//! re-reading the entire file.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::fs;
use tokio::io::AsyncReadExt;

/// Default maximum bytes to read in a single `read_task_output` call.
const DEFAULT_MAX_READ_BYTES: usize = 8 * 1024 * 1024; // 8 MB

/// Disk cap for task output files (5 GB).
pub const MAX_TASK_OUTPUT_BYTES: u64 = 5 * 1024 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Directory helpers
// ---------------------------------------------------------------------------

/// Return the base directory for task output files: `~/.thundercode/task-output/`.
fn task_output_base_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".thundercode")
        .join("task-output")
}

/// Ensure the task output directory exists, creating it if needed.
///
/// Returns the directory path on success.
pub async fn ensure_task_output_dir() -> Result<PathBuf> {
    let dir = task_output_base_dir();
    fs::create_dir_all(&dir)
        .await
        .with_context(|| format!("failed to create task output dir: {}", dir.display()))?;
    Ok(dir)
}

/// Get the output file path for a given task ID.
///
/// The file is `<task_output_dir>/<task_id>.output`.
pub fn get_task_output_path(task_id: &str) -> PathBuf {
    task_output_base_dir().join(format!("{}.output", task_id))
}

// ---------------------------------------------------------------------------
// Read helpers
// ---------------------------------------------------------------------------

/// Read task output starting from `offset` bytes into the file.
///
/// Returns `(content, new_offset)` where `new_offset` is the byte position
/// after the last byte read -- pass it back as `offset` to get the next delta.
///
/// If the file does not exist yet the function returns an empty string and
/// the same offset unchanged.
pub async fn read_task_output(task_id: &str, offset: usize) -> Result<(String, usize)> {
    read_task_output_with_limit(task_id, offset, DEFAULT_MAX_READ_BYTES).await
}

/// Like [`read_task_output`] but with an explicit byte cap.
pub async fn read_task_output_with_limit(
    task_id: &str,
    offset: usize,
    max_bytes: usize,
) -> Result<(String, usize)> {
    let path = get_task_output_path(task_id);
    read_file_delta(&path, offset, max_bytes).await
}

/// Read the tail of a task's output file, capped at `max_bytes`.
///
/// If the file is smaller than `max_bytes`, the entire file is returned.
/// Otherwise only the last `max_bytes` are returned, prefixed with a truncation
/// notice.
pub async fn read_task_output_tail(task_id: &str, max_bytes: usize) -> Result<String> {
    let path = get_task_output_path(task_id);
    match fs::metadata(&path).await {
        Ok(meta) => {
            let size = meta.len() as usize;
            if size <= max_bytes {
                let content = fs::read_to_string(&path).await.unwrap_or_default();
                return Ok(content);
            }
            // Read the tail.
            let offset = size - max_bytes;
            let (content, _) = read_file_delta(&path, offset, max_bytes).await?;
            let skipped_kb = offset / 1024;
            Ok(format!(
                "[{}KB of earlier output omitted]\n{}",
                skipped_kb, content
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.into()),
    }
}

/// Get the current byte size of a task's output file, or 0 if it doesn't exist.
pub async fn get_task_output_size(task_id: &str) -> u64 {
    let path = get_task_output_path(task_id);
    match fs::metadata(&path).await {
        Ok(meta) => meta.len(),
        Err(_) => 0,
    }
}

// ---------------------------------------------------------------------------
// Write helpers
// ---------------------------------------------------------------------------

/// Initialize an empty output file for a new task.
///
/// Creates the output directory if needed, then creates / truncates the file.
/// Returns the output file path.
pub async fn init_task_output(task_id: &str) -> Result<PathBuf> {
    ensure_task_output_dir().await?;
    let path = get_task_output_path(task_id);
    fs::write(&path, b"")
        .await
        .with_context(|| format!("failed to init task output: {}", path.display()))?;
    Ok(path)
}

/// Append content to a task's output file.
///
/// Creates the file if it doesn't exist.
pub async fn append_task_output(task_id: &str, content: &str) -> Result<()> {
    use tokio::io::AsyncWriteExt;

    let path = get_task_output_path(task_id);

    // Ensure directory exists (cheap no-op when already present).
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
        .with_context(|| format!("failed to open task output for append: {}", path.display()))?;

    file.write_all(content.as_bytes()).await?;
    file.flush().await?;
    Ok(())
}

/// Delete a task's output file, ignoring "not found" errors.
pub async fn cleanup_task_output(task_id: &str) {
    let path = get_task_output_path(task_id);
    let _ = fs::remove_file(&path).await;
}

// ---------------------------------------------------------------------------
// Internal
// ---------------------------------------------------------------------------

/// Read up to `max_bytes` from `path` starting at byte `offset`.
///
/// Returns `(utf8_content, new_offset)`.
async fn read_file_delta(
    path: &Path,
    offset: usize,
    max_bytes: usize,
) -> Result<(String, usize)> {
    let mut file = match fs::File::open(path).await {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok((String::new(), offset));
        }
        Err(e) => return Err(e.into()),
    };

    // Seek to offset.
    use tokio::io::AsyncSeekExt;
    file.seek(std::io::SeekFrom::Start(offset as u64)).await?;

    let mut buf = vec![0u8; max_bytes];
    let n = file.read(&mut buf).await?;
    buf.truncate(n);

    let content = String::from_utf8_lossy(&buf).into_owned();
    Ok((content, offset + n))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_path_contains_task_id() {
        let path = get_task_output_path("b12345678");
        assert!(path.to_string_lossy().contains("b12345678.output"));
    }

    #[test]
    fn output_path_lives_under_dot_rules() {
        let path = get_task_output_path("test");
        assert!(path.to_string_lossy().contains(".primary/task-output"));
    }

    #[tokio::test]
    async fn init_and_read_output() {
        let task_id = format!("test_{}", nanoid::nanoid!(8));
        let path = init_task_output(&task_id).await.unwrap();
        assert!(path.exists());

        append_task_output(&task_id, "hello world").await.unwrap();

        let (content, new_offset) = read_task_output(&task_id, 0).await.unwrap();
        assert_eq!(content, "hello world");
        assert_eq!(new_offset, 11);

        // Delta read from the end returns nothing new.
        let (content2, offset2) = read_task_output(&task_id, new_offset).await.unwrap();
        assert_eq!(content2, "");
        assert_eq!(offset2, new_offset);

        cleanup_task_output(&task_id).await;
    }
}
