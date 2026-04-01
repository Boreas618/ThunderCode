//! Diff parsing and rendering.
//!
//! Parses unified diff format (as produced by `git diff`) into structured
//! types, and provides helpers to generate diffs for individual files or
//! the entire staging area.  The parser is ported from the TypeScript
//! reference `ref/utils/gitDiff.ts`.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use regex::Regex;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single line inside a hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    /// Unchanged context line.
    Context(String),
    /// Line added in the new version.
    Added(String),
    /// Line removed from the old version.
    Removed(String),
}

/// A contiguous hunk of changes inside a file diff.
#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

/// A parsed file entry from a unified diff.
#[derive(Debug, Clone)]
pub struct DiffFile {
    pub old_path: String,
    pub new_path: String,
    pub hunks: Vec<DiffHunk>,
    pub is_binary: bool,
    pub is_new: bool,
    pub is_deleted: bool,
    pub is_renamed: bool,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse a unified diff string (multi-file) into a list of [`DiffFile`]s.
///
/// The input should be the raw output of `git diff` (or similar tool) using
/// the default unified format.
pub fn parse_diff(diff_text: &str) -> Vec<DiffFile> {
    if diff_text.trim().is_empty() {
        return Vec::new();
    }

    // Split on file boundaries.  Each section starts with "diff --git".
    let file_sections: Vec<&str> = split_on_diff_headers(diff_text);

    let hunk_re =
        Regex::new(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@").unwrap();

    let mut result = Vec::new();

    for section in file_sections {
        let lines: Vec<&str> = section.lines().collect();
        if lines.is_empty() {
            continue;
        }

        // --- extract file paths from the header line -------------------------
        // Format: "diff --git a/path b/path"  (the "diff --git " prefix was
        // already stripped by the splitter for all sections after the first,
        // but the first section still contains the full line).
        let header = lines[0];

        let (old_path, new_path) = extract_paths(header);

        // --- detect meta-data from subsequent header lines -------------------
        let mut is_binary = false;
        let mut is_new = false;
        let mut is_deleted = false;
        let mut is_renamed = false;

        for &line in &lines[1..] {
            if line.starts_with("Binary files") {
                is_binary = true;
            } else if line.starts_with("new file") {
                is_new = true;
            } else if line.starts_with("deleted file") {
                is_deleted = true;
            } else if line.starts_with("rename from")
                || line.starts_with("rename to")
                || line.starts_with("similarity index")
            {
                is_renamed = true;
            } else if line.starts_with("@@") {
                break; // hunks start; stop scanning headers
            }
        }

        // --- parse hunks -----------------------------------------------------
        let mut hunks: Vec<DiffHunk> = Vec::new();
        let mut current_hunk: Option<DiffHunk> = None;

        for &line in &lines[1..] {
            // Skip diff metadata lines (everything before the first hunk).
            if line.starts_with("index ")
                || line.starts_with("---")
                || line.starts_with("+++")
                || line.starts_with("new file")
                || line.starts_with("deleted file")
                || line.starts_with("old mode")
                || line.starts_with("new mode")
                || line.starts_with("rename from")
                || line.starts_with("rename to")
                || line.starts_with("similarity index")
                || line.starts_with("dissimilarity index")
                || line.starts_with("Binary files")
                || line.starts_with("GIT binary patch")
            {
                continue;
            }

            if let Some(caps) = hunk_re.captures(line) {
                // Flush the previous hunk.
                if let Some(h) = current_hunk.take() {
                    hunks.push(h);
                }

                let old_start: u32 = caps.get(1).map_or(0, |m| m.as_str().parse().unwrap_or(0));
                let old_lines: u32 = caps.get(2).map_or(1, |m| m.as_str().parse().unwrap_or(1));
                let new_start: u32 = caps.get(3).map_or(0, |m| m.as_str().parse().unwrap_or(0));
                let new_lines: u32 = caps.get(4).map_or(1, |m| m.as_str().parse().unwrap_or(1));

                current_hunk = Some(DiffHunk {
                    old_start,
                    old_lines,
                    new_start,
                    new_lines,
                    lines: Vec::new(),
                });
                continue;
            }

            // Inside a hunk -- classify lines.
            if let Some(ref mut hunk) = current_hunk {
                if let Some(rest) = line.strip_prefix('+') {
                    hunk.lines.push(DiffLine::Added(rest.to_string()));
                } else if let Some(rest) = line.strip_prefix('-') {
                    hunk.lines.push(DiffLine::Removed(rest.to_string()));
                } else if let Some(rest) = line.strip_prefix(' ') {
                    hunk.lines.push(DiffLine::Context(rest.to_string()));
                } else if line.is_empty() {
                    // An empty line within a hunk is a context line with empty content.
                    hunk.lines.push(DiffLine::Context(String::new()));
                } else if line.starts_with('\\') {
                    // "\ No newline at end of file" -- skip.
                }
            }
        }

        // Flush the last hunk.
        if let Some(h) = current_hunk.take() {
            hunks.push(h);
        }

        result.push(DiffFile {
            old_path,
            new_path,
            hunks,
            is_binary,
            is_new,
            is_deleted,
            is_renamed,
        });
    }

    result
}

/// Generate a diff for a single file (unstaged changes) by invoking
/// `git diff -- <file_path>`.
pub fn generate_diff(repo_path: &Path, file_path: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["--no-optional-locks", "diff", "--", file_path])
        .current_dir(repo_path)
        .output()
        .context("failed to execute git diff")?;

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Return the staged diff (index vs HEAD) for the entire repository.
pub fn get_staged_diff(repo_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["--no-optional-locks", "diff", "--cached"])
        .current_dir(repo_path)
        .output()
        .context("failed to execute git diff --cached")?;

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Split the raw diff output into per-file sections.
///
/// Each section starts with `diff --git `.  We return the text of each
/// section (including its "diff --git" line).
fn split_on_diff_headers(text: &str) -> Vec<&str> {
    let marker = "diff --git ";
    let mut sections = Vec::new();
    let mut start = 0;

    // Walk through the text and split at each "diff --git " occurrence
    // that appears at the beginning of a line.
    let bytes = text.as_bytes();
    let marker_bytes = marker.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        // Check if this position is at the start of a line and starts with the marker.
        let at_line_start = pos == 0 || bytes[pos - 1] == b'\n';
        if at_line_start && bytes[pos..].starts_with(marker_bytes) {
            if pos > start {
                let section = &text[start..pos];
                if !section.trim().is_empty() {
                    sections.push(section.trim_end());
                }
            }
            start = pos;
        }
        pos += 1;
    }

    // Remaining tail.
    if start < text.len() {
        let section = &text[start..];
        if !section.trim().is_empty() {
            sections.push(section.trim_end());
        }
    }

    sections
}

/// Extract `(old_path, new_path)` from a "diff --git a/... b/..." header.
fn extract_paths(header: &str) -> (String, String) {
    // Strip "diff --git " prefix if present.
    let rest = header
        .strip_prefix("diff --git ")
        .unwrap_or(header);

    // Paths are "a/<path> b/<path>".  A naive split on ' b/' works for
    // paths without spaces; for quoted paths git uses C-style escapes.
    if let Some(idx) = rest.find(" b/") {
        let old = rest[..idx].strip_prefix("a/").unwrap_or(&rest[..idx]);
        let new = &rest[idx + 1..];
        let new = new.strip_prefix("b/").unwrap_or(new);
        (old.to_string(), new.to_string())
    } else {
        (rest.to_string(), rest.to_string())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_DIFF: &str = "\
diff --git a/src/main.rs b/src/main.rs
index abc1234..def5678 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,6 @@
 use std::io;

+use anyhow::Result;
 fn main() {
-    println!(\"hello\");
+    println!(\"goodbye\");
 }
";

    #[test]
    fn test_parse_single_file_diff() {
        let files = parse_diff(SAMPLE_DIFF);
        assert_eq!(files.len(), 1);
        let f = &files[0];
        assert_eq!(f.old_path, "src/main.rs");
        assert_eq!(f.new_path, "src/main.rs");
        assert!(!f.is_binary);
        assert!(!f.is_new);
        assert!(!f.is_deleted);
        assert!(!f.is_renamed);

        assert_eq!(f.hunks.len(), 1);
        let hunk = &f.hunks[0];
        assert_eq!(hunk.old_start, 1);
        assert_eq!(hunk.old_lines, 5);
        assert_eq!(hunk.new_start, 1);
        assert_eq!(hunk.new_lines, 6);

        // Count line types
        let added = hunk
            .lines
            .iter()
            .filter(|l| matches!(l, DiffLine::Added(_)))
            .count();
        let removed = hunk
            .lines
            .iter()
            .filter(|l| matches!(l, DiffLine::Removed(_)))
            .count();
        let context = hunk
            .lines
            .iter()
            .filter(|l| matches!(l, DiffLine::Context(_)))
            .count();

        assert_eq!(added, 2);
        assert_eq!(removed, 1);
        // 4 context lines: " use std::io;", empty line, " fn main() {", " }"
        assert_eq!(context, 4);
    }

    #[test]
    fn test_parse_new_file() {
        let diff = "\
diff --git a/new.txt b/new.txt
new file mode 100644
index 0000000..abc1234
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,3 @@
+line 1
+line 2
+line 3
";
        let files = parse_diff(diff);
        assert_eq!(files.len(), 1);
        assert!(files[0].is_new);
        assert_eq!(files[0].hunks.len(), 1);
        assert_eq!(files[0].hunks[0].lines.len(), 3);
    }

    #[test]
    fn test_parse_deleted_file() {
        let diff = "\
diff --git a/old.txt b/old.txt
deleted file mode 100644
index abc1234..0000000
--- a/old.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-removed line 1
-removed line 2
";
        let files = parse_diff(diff);
        assert_eq!(files.len(), 1);
        assert!(files[0].is_deleted);
    }

    #[test]
    fn test_parse_binary() {
        let diff = "\
diff --git a/image.png b/image.png
index abc1234..def5678 100644
Binary files a/image.png and b/image.png differ
";
        let files = parse_diff(diff);
        assert_eq!(files.len(), 1);
        assert!(files[0].is_binary);
        assert!(files[0].hunks.is_empty());
    }

    #[test]
    fn test_parse_renamed_file() {
        let diff = "\
diff --git a/old_name.rs b/new_name.rs
similarity index 95%
rename from old_name.rs
rename to new_name.rs
index abc1234..def5678 100644
--- a/old_name.rs
+++ b/new_name.rs
@@ -1,3 +1,3 @@
 fn foo() {
-    bar();
+    baz();
 }
";
        let files = parse_diff(diff);
        assert_eq!(files.len(), 1);
        assert!(files[0].is_renamed);
        assert_eq!(files[0].old_path, "old_name.rs");
        assert_eq!(files[0].new_path, "new_name.rs");
    }

    #[test]
    fn test_parse_multi_file_diff() {
        let diff = "\
diff --git a/a.txt b/a.txt
index abc1234..def5678 100644
--- a/a.txt
+++ b/a.txt
@@ -1,2 +1,2 @@
 first
-second
+SECOND
diff --git a/b.txt b/b.txt
new file mode 100644
index 0000000..abc1234
--- /dev/null
+++ b/b.txt
@@ -0,0 +1 @@
+new file content
";
        let files = parse_diff(diff);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].new_path, "a.txt");
        assert_eq!(files[1].new_path, "b.txt");
        assert!(files[1].is_new);
    }

    #[test]
    fn test_parse_empty_diff() {
        assert!(parse_diff("").is_empty());
        assert!(parse_diff("  \n  ").is_empty());
    }

    #[test]
    fn test_parse_multiple_hunks() {
        let diff = "\
diff --git a/file.rs b/file.rs
index abc1234..def5678 100644
--- a/file.rs
+++ b/file.rs
@@ -1,3 +1,4 @@
 fn a() {}
+fn b() {}
 fn c() {}
 fn d() {}
@@ -10,3 +11,4 @@
 fn x() {}
+fn y() {}
 fn z() {}
 fn w() {}
";
        let files = parse_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].hunks.len(), 2);
        assert_eq!(files[0].hunks[0].old_start, 1);
        assert_eq!(files[0].hunks[1].old_start, 10);
    }

    #[test]
    fn test_diff_line_content() {
        let diff = "\
diff --git a/f.txt b/f.txt
index abc..def 100644
--- a/f.txt
+++ b/f.txt
@@ -1,3 +1,3 @@
 keep this
-remove this
+add this
 keep too
";
        let files = parse_diff(diff);
        let hunk = &files[0].hunks[0];
        assert_eq!(hunk.lines[0], DiffLine::Context("keep this".to_string()));
        assert_eq!(
            hunk.lines[1],
            DiffLine::Removed("remove this".to_string())
        );
        assert_eq!(hunk.lines[2], DiffLine::Added("add this".to_string()));
        assert_eq!(hunk.lines[3], DiffLine::Context("keep too".to_string()));
    }
}
