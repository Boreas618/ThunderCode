//! Memory directory management.
//!
//! Ported from ref/memdir/memdir.ts` and `ref/memdir/paths.ts`.
//!
//! Each project gets a persistent memory directory under
//! `~/.thundercode/projects/<slug>/memory/`. The slug is derived from the
//! git remote URL or project path.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use crate::memory::frontmatter::parse_frontmatter;
use crate::memory::types::{EntrypointTruncation, MemoryFile, MemoryHeader};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Name of the memory index file.
pub const ENTRYPOINT_NAME: &str = "MEMORY.md";

/// Maximum number of lines in a MEMORY.md before truncation.
pub const MAX_ENTRYPOINT_LINES: usize = 200;

/// Maximum byte size of MEMORY.md before truncation (~125 chars/line at 200 lines).
pub const MAX_ENTRYPOINT_BYTES: usize = 25_000;

/// Maximum number of memory files to return from a scan.
const MAX_MEMORY_FILES: usize = 200;

/// How many lines of frontmatter to read for a scan header.
const FRONTMATTER_MAX_LINES: usize = 30;

// ---------------------------------------------------------------------------
// MemoryDir
// ---------------------------------------------------------------------------

/// A handle to a project's memory directory.
#[derive(Debug, Clone)]
pub struct MemoryDir {
    /// Absolute path to the memory directory.
    pub path: PathBuf,
}

impl MemoryDir {
    /// Create a new [`MemoryDir`] for the given project slug.
    ///
    /// Does not create the directory on disk -- call [`ensure_exists`] for that.
    pub fn new(project_slug: &str) -> Self {
        Self {
            path: get_memory_dir_path(project_slug),
        }
    }

    /// Create a [`MemoryDir`] from an explicit path.
    pub fn from_path(path: PathBuf) -> Self {
        Self { path }
    }

    /// Ensure the memory directory exists on disk. Idempotent.
    pub fn ensure_exists(&self) -> Result<()> {
        fs::create_dir_all(&self.path)
            .with_context(|| format!("failed to create memory directory: {:?}", self.path))?;
        Ok(())
    }

    /// Path to the MEMORY.md index file.
    pub fn memory_index_path(&self) -> PathBuf {
        self.path.join(ENTRYPOINT_NAME)
    }

    /// Read the MEMORY.md index file. Returns an empty string if it does not exist.
    pub fn read_index(&self) -> Result<String> {
        let path = self.memory_index_path();
        match fs::read_to_string(&path) {
            Ok(content) => Ok(content),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(e).with_context(|| format!("failed to read {ENTRYPOINT_NAME}")),
        }
    }

    /// Write content to the MEMORY.md index file.
    pub fn write_index(&self, content: &str) -> Result<()> {
        self.ensure_exists()?;
        let path = self.memory_index_path();
        fs::write(&path, content)
            .with_context(|| format!("failed to write {ENTRYPOINT_NAME}"))?;
        Ok(())
    }

    /// Create a new memory file with frontmatter and content.
    ///
    /// Returns the absolute path of the created file.
    pub fn create_memory(&self, filename: &str, content: &str) -> Result<PathBuf> {
        self.ensure_exists()?;
        let path = self.path.join(filename);
        if path.exists() {
            anyhow::bail!("memory file already exists: {filename}");
        }
        fs::write(&path, content)
            .with_context(|| format!("failed to create memory file: {filename}"))?;
        Ok(path)
    }

    /// Read a memory file's content.
    pub fn read_memory(&self, filename: &str) -> Result<String> {
        let path = self.path.join(filename);
        fs::read_to_string(&path)
            .with_context(|| format!("failed to read memory file: {filename}"))
    }

    /// Update (overwrite) a memory file's content.
    pub fn update_memory(&self, filename: &str, content: &str) -> Result<()> {
        let path = self.path.join(filename);
        if !path.exists() {
            anyhow::bail!("memory file does not exist: {filename}");
        }
        fs::write(&path, content)
            .with_context(|| format!("failed to update memory file: {filename}"))?;
        Ok(())
    }

    /// Delete a memory file.
    pub fn delete_memory(&self, filename: &str) -> Result<()> {
        let path = self.path.join(filename);
        if !path.exists() {
            anyhow::bail!("memory file does not exist: {filename}");
        }
        fs::remove_file(&path)
            .with_context(|| format!("failed to delete memory file: {filename}"))?;
        Ok(())
    }

    /// List all memory files in the directory.
    ///
    /// Reads frontmatter from each `.md` file (excluding MEMORY.md)
    /// and returns fully populated [`MemoryFile`] entries.
    pub fn list_memories(&self) -> Result<Vec<MemoryFile>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let mut memories = Vec::new();
        for entry in fs::read_dir(&self.path)
            .with_context(|| format!("failed to read memory directory: {:?}", self.path))?
        {
            let entry = entry?;
            let path = entry.path();

            // Only .md files, skip MEMORY.md
            if path.extension().map_or(true, |e| e != "md") {
                continue;
            }
            if path.file_name().map_or(false, |n| n == ENTRYPOINT_NAME) {
                continue;
            }

            let raw_content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let (fm, body) = match parse_frontmatter(&raw_content) {
                Ok(r) => r,
                Err(_) => continue,
            };

            memories.push(MemoryFile {
                path: path.clone(),
                name: fm.name,
                description: fm.description,
                memory_type: fm.memory_type,
                content: body,
            });
        }

        Ok(memories)
    }

    /// Scan memory files for their headers (filename, description, type, mtime).
    ///
    /// Returns up to [`MAX_MEMORY_FILES`] headers sorted newest-first.
    /// This is the lightweight alternative to [`list_memories`] used by
    /// the relevance selector.
    pub fn scan_headers(&self) -> Result<Vec<MemoryHeader>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let mut headers = Vec::new();

        Self::scan_headers_recursive(&self.path, &self.path, &mut headers)?;

        // Sort newest-first, cap at MAX_MEMORY_FILES.
        headers.sort_by(|a, b| b.mtime_ms.cmp(&a.mtime_ms));
        headers.truncate(MAX_MEMORY_FILES);
        Ok(headers)
    }

    /// Recursively scan a directory for .md files and collect their headers.
    fn scan_headers_recursive(
        base: &Path,
        dir: &Path,
        headers: &mut Vec<MemoryHeader>,
    ) -> Result<()> {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();

            if path.is_dir() {
                Self::scan_headers_recursive(base, &path, headers)?;
                continue;
            }

            // Only .md files, skip MEMORY.md
            if path.extension().map_or(true, |e| e != "md") {
                continue;
            }
            if path.file_name().map_or(false, |n| n == ENTRYPOINT_NAME) {
                continue;
            }

            // Read only the first FRONTMATTER_MAX_LINES lines
            let raw_content = match read_file_head(&path, FRONTMATTER_MAX_LINES) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let mtime_ms = entry
                .metadata()
                .and_then(|m| m.modified())
                .map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64
                })
                .unwrap_or(0);

            let (fm, _body) = match parse_frontmatter(&raw_content) {
                Ok(r) => r,
                Err(_) => continue,
            };

            let relative_path = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            headers.push(MemoryHeader {
                filename: relative_path,
                file_path: path,
                mtime_ms,
                description: if fm.description.is_empty() {
                    None
                } else {
                    Some(fm.description)
                },
                memory_type: fm.memory_type,
            });
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Compute a project slug from a git remote URL.
///
/// Strips protocol, auth info, and `.git` suffix, then replaces special
/// characters with hyphens. The result is safe for use in file paths.
///
/// ```
/// use crate::memory::memdir::get_project_slug;
///
/// assert_eq!(
///     get_project_slug("https://github.com/user/repo.git"),
///     "github.com-user-repo"
/// );
/// assert_eq!(
///     get_project_slug("git@github.com:user/repo.git"),
///     "github.com-user-repo"
/// );
/// ```
pub fn get_project_slug(git_remote_url: &str) -> String {
    let mut slug = git_remote_url.to_string();

    // Strip protocol
    if let Some(rest) = slug.strip_prefix("https://") {
        slug = rest.to_string();
    } else if let Some(rest) = slug.strip_prefix("http://") {
        slug = rest.to_string();
    } else if let Some(rest) = slug.strip_prefix("ssh://") {
        slug = rest.to_string();
    } else if let Some(rest) = slug.strip_prefix("git://") {
        slug = rest.to_string();
    }

    // Handle SSH git@host:user/repo format
    if let Some(at_pos) = slug.find('@') {
        slug = slug[at_pos + 1..].to_string();
    }

    // Replace colon (SSH format separator) with slash
    slug = slug.replace(':', "/");

    // Strip .git suffix
    if let Some(rest) = slug.strip_suffix(".git") {
        slug = rest.to_string();
    }

    // Strip trailing slashes
    slug = slug.trim_end_matches('/').to_string();

    // Replace path separators and special chars with hyphens
    slug = slug
        .replace('/', "-")
        .replace('\\', "-")
        .replace(' ', "-");

    // Collapse multiple hyphens
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }

    // Trim leading/trailing hyphens
    slug = slug.trim_matches('-').to_string();

    slug
}

/// Compute a project slug from a local filesystem path.
///
/// Sanitizes the path into a safe directory name by replacing
/// separators with hyphens and removing leading slashes.
pub fn get_project_slug_from_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    sanitize_path(&s)
}

/// Sanitize a path string into a safe directory name.
fn sanitize_path(path: &str) -> String {
    let mut result = path.to_string();
    // Remove leading slash
    if result.starts_with('/') {
        result = result[1..].to_string();
    }
    // Replace separators with hyphens
    result = result.replace('/', "-").replace('\\', "-");
    // Collapse multiple hyphens
    while result.contains("--") {
        result = result.replace("--", "-");
    }
    result = result.trim_matches('-').to_string();
    result
}

/// Get the memory directory path for a given project slug.
///
/// Returns `~/.thundercode/projects/<slug>/memory/`.
pub fn get_memory_dir_path(project_slug: &str) -> PathBuf {
    crate::config::config_home_dir()
        .join("projects")
        .join(project_slug)
        .join("memory")
}

// ---------------------------------------------------------------------------
// Entrypoint truncation
// ---------------------------------------------------------------------------

/// Truncate MEMORY.md content to the line AND byte caps.
///
/// Line-truncates first (natural boundary), then byte-truncates at the
/// last newline before the cap so we don't cut mid-line. Appends a warning
/// naming which cap fired.
///
/// Ported from `truncateEntrypointContent` in memdir.ts.
pub fn truncate_entrypoint_content(raw: &str) -> EntrypointTruncation {
    let trimmed = raw.trim();
    let content_lines: Vec<&str> = trimmed.split('\n').collect();
    let line_count = content_lines.len();
    let byte_count = trimmed.len();

    let was_line_truncated = line_count > MAX_ENTRYPOINT_LINES;
    let was_byte_truncated = byte_count > MAX_ENTRYPOINT_BYTES;

    if !was_line_truncated && !was_byte_truncated {
        return EntrypointTruncation {
            content: trimmed.to_string(),
            line_count,
            byte_count,
            was_line_truncated,
            was_byte_truncated,
        };
    }

    let mut truncated = if was_line_truncated {
        content_lines[..MAX_ENTRYPOINT_LINES].join("\n")
    } else {
        trimmed.to_string()
    };

    if truncated.len() > MAX_ENTRYPOINT_BYTES {
        if let Some(cut_at) = truncated[..MAX_ENTRYPOINT_BYTES].rfind('\n') {
            if cut_at > 0 {
                truncated = truncated[..cut_at].to_string();
            } else {
                truncated = truncated[..MAX_ENTRYPOINT_BYTES].to_string();
            }
        } else {
            truncated = truncated[..MAX_ENTRYPOINT_BYTES].to_string();
        }
    }

    let reason = match (was_byte_truncated, was_line_truncated) {
        (true, false) => format!(
            "{} (limit: {}) -- index entries are too long",
            format_file_size(byte_count),
            format_file_size(MAX_ENTRYPOINT_BYTES),
        ),
        (false, true) => format!(
            "{line_count} lines (limit: {MAX_ENTRYPOINT_LINES})"
        ),
        _ => format!(
            "{line_count} lines and {}",
            format_file_size(byte_count)
        ),
    };

    let warning = format!(
        "\n\n> WARNING: {ENTRYPOINT_NAME} is {reason}. \
         Only part of it was loaded. Keep index entries to one line \
         under ~200 chars; move detail into topic files."
    );

    EntrypointTruncation {
        content: truncated + &warning,
        line_count,
        byte_count,
        was_line_truncated,
        was_byte_truncated,
    }
}

// ---------------------------------------------------------------------------
// Memory age helpers
// ---------------------------------------------------------------------------

/// Days elapsed since `mtime_ms`. Floor-rounded -- 0 for today, 1 for
/// yesterday, 2+ for older. Negative inputs (future mtime, clock skew)
/// clamp to 0.
pub fn memory_age_days(mtime_ms: i64) -> u64 {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let diff = now_ms - mtime_ms;
    if diff <= 0 {
        return 0;
    }
    (diff / 86_400_000) as u64
}

/// Human-readable age string.
pub fn memory_age(mtime_ms: i64) -> String {
    let d = memory_age_days(mtime_ms);
    match d {
        0 => "today".to_string(),
        1 => "yesterday".to_string(),
        n => format!("{n} days ago"),
    }
}

/// Plain-text staleness caveat for memories >1 day old.
/// Returns empty string for fresh memories (today/yesterday).
pub fn memory_freshness_text(mtime_ms: i64) -> String {
    let d = memory_age_days(mtime_ms);
    if d <= 1 {
        return String::new();
    }
    format!(
        "This memory is {d} days old. \
         Memories are point-in-time observations, not live state -- \
         claims about code behavior or file:line citations may be outdated. \
         Verify against current code before asserting as fact."
    )
}

/// Format memory headers as a text manifest: one line per file.
///
/// Used by both the recall selector prompt and the extraction-agent prompt.
pub fn format_memory_manifest(memories: &[MemoryHeader]) -> String {
    memories
        .iter()
        .map(|m| {
            let tag = m
                .memory_type
                .map(|t| format!("[{}] ", t.as_str()))
                .unwrap_or_default();
            let ts = chrono::DateTime::from_timestamp_millis(m.mtime_ms)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default();
            match &m.description {
                Some(desc) => format!("- {tag}{} ({ts}): {desc}", m.filename),
                None => format!("- {tag}{} ({ts})", m.filename),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

/// Read the first `max_lines` lines from a file.
fn read_file_head(path: &Path, max_lines: usize) -> Result<String> {
    use std::io::{BufRead, BufReader};
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut lines = Vec::new();
    for line in reader.lines().take(max_lines) {
        lines.push(line?);
    }
    Ok(lines.join("\n"))
}

/// Format a byte count as a human-readable file size.
fn format_file_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // -- Project slug tests --

    #[test]
    fn slug_https() {
        assert_eq!(
            get_project_slug("https://github.com/user/repo.git"),
            "github.com-user-repo"
        );
    }

    #[test]
    fn slug_ssh() {
        assert_eq!(
            get_project_slug("git@github.com:user/repo.git"),
            "github.com-user-repo"
        );
    }

    #[test]
    fn slug_ssh_protocol() {
        assert_eq!(
            get_project_slug("ssh://git@github.com/user/repo.git"),
            "github.com-user-repo"
        );
    }

    #[test]
    fn slug_no_suffix() {
        assert_eq!(
            get_project_slug("https://github.com/user/repo"),
            "github.com-user-repo"
        );
    }

    #[test]
    fn slug_from_path() {
        let slug = get_project_slug_from_path(Path::new("/home/user/projects/myapp"));
        assert_eq!(slug, "home-user-projects-myapp");
    }

    // -- Truncation tests --

    #[test]
    fn truncation_no_truncation() {
        let content = "line 1\nline 2\nline 3\n";
        let result = truncate_entrypoint_content(content);
        assert!(!result.was_line_truncated);
        assert!(!result.was_byte_truncated);
        assert_eq!(result.line_count, 3);
    }

    #[test]
    fn truncation_by_lines() {
        let content = (0..250).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let result = truncate_entrypoint_content(&content);
        assert!(result.was_line_truncated);
        assert_eq!(result.line_count, 250);
        assert!(result.content.contains("WARNING"));
    }

    #[test]
    fn truncation_by_bytes() {
        // Create content under 200 lines but over 25KB
        let long_line = "x".repeat(300);
        let content = (0..100).map(|_| long_line.clone()).collect::<Vec<_>>().join("\n");
        let result = truncate_entrypoint_content(&content);
        assert!(result.was_byte_truncated);
        assert!(result.content.contains("WARNING"));
    }

    // -- MemoryDir CRUD tests --

    #[test]
    fn memory_dir_crud() {
        let tmp = TempDir::new().unwrap();
        let dir = MemoryDir::from_path(tmp.path().to_path_buf());

        // Initially empty
        assert!(dir.list_memories().unwrap().is_empty());

        // Create a memory file
        let content = "---\nname: test\ndescription: a test\ntype: user\n---\n\nBody.\n";
        let path = dir.create_memory("test.md", content).unwrap();
        assert!(path.exists());

        // Read it back
        let read_content = dir.read_memory("test.md").unwrap();
        assert_eq!(read_content, content);

        // List memories
        let memories = dir.list_memories().unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].name, "test");

        // Update
        let new_content = "---\nname: updated\ndescription: updated desc\ntype: feedback\n---\n\nNew body.\n";
        dir.update_memory("test.md", new_content).unwrap();
        let updated = dir.read_memory("test.md").unwrap();
        assert!(updated.contains("updated"));

        // Delete
        dir.delete_memory("test.md").unwrap();
        assert!(dir.list_memories().unwrap().is_empty());
    }

    #[test]
    fn memory_dir_index() {
        let tmp = TempDir::new().unwrap();
        let dir = MemoryDir::from_path(tmp.path().to_path_buf());

        // Initially empty
        assert_eq!(dir.read_index().unwrap(), "");

        // Write index
        dir.write_index("- [Test](test.md) -- a test memory").unwrap();
        let index = dir.read_index().unwrap();
        assert!(index.contains("Test"));
    }

    #[test]
    fn scan_headers() {
        let tmp = TempDir::new().unwrap();
        let dir = MemoryDir::from_path(tmp.path().to_path_buf());

        // Create some files
        let _ = dir.create_memory(
            "user_prefs.md",
            "---\nname: prefs\ndescription: user preferences\ntype: user\n---\n\nPrefs body.\n",
        );
        let _ = dir.create_memory(
            "feedback_testing.md",
            "---\nname: testing feedback\ndescription: use real DB\ntype: feedback\n---\n\nBody.\n",
        );
        // Also create MEMORY.md -- should be excluded from scan
        dir.write_index("- [prefs](user_prefs.md)\n- [testing](feedback_testing.md)\n")
            .unwrap();

        let headers = dir.scan_headers().unwrap();
        assert_eq!(headers.len(), 2);
        // All should have descriptions
        assert!(headers.iter().all(|h| h.description.is_some()));
        // None should be MEMORY.md
        assert!(headers.iter().all(|h| h.filename != ENTRYPOINT_NAME));
    }

    // -- Format file size --

    #[test]
    fn format_sizes() {
        assert_eq!(format_file_size(500), "500B");
        assert_eq!(format_file_size(2048), "2.0KB");
        assert_eq!(format_file_size(1_500_000), "1.4MB");
    }

    // -- Memory age --

    #[test]
    fn age_today() {
        let now_ms = chrono::Utc::now().timestamp_millis();
        assert_eq!(memory_age(now_ms), "today");
    }

    #[test]
    fn age_old() {
        let five_days_ago = chrono::Utc::now().timestamp_millis() - (5 * 86_400_000);
        assert_eq!(memory_age(five_days_ago), "5 days ago");
    }
}
