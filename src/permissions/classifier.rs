//! Bash command safety classification.
//!
//! Ported from ref/utils/permissions/dangerousPatterns.ts` and
//! `ref/utils/permissions/bashClassifier.ts`.
//!
//! Detects destructive commands, file modification commands, and
//! read-only vs. mutating operations for permission decisions.

use regex::Regex;
use std::sync::OnceLock;

// ============================================================================
// Dangerous patterns  (TS: dangerousPatterns.ts)
// ============================================================================

/// Cross-platform code-execution entry points.
pub const CROSS_PLATFORM_CODE_EXEC: &[&str] = &[
    // Interpreters
    "python",
    "python3",
    "python2",
    "node",
    "deno",
    "tsx",
    "ruby",
    "perl",
    "php",
    "lua",
    // Package runners
    "npx",
    "bunx",
    "npm run",
    "yarn run",
    "pnpm run",
    "bun run",
    // Shells
    "bash",
    "sh",
    // Remote command
    "ssh",
];

/// Patterns that are dangerous as broad Bash allow-rule prefixes.
/// An allow rule like `Bash(python:*)` lets the model run arbitrary code.
pub fn dangerous_bash_patterns() -> &'static [&'static str] {
    static PATTERNS: OnceLock<Vec<&str>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        let mut v: Vec<&str> = CROSS_PLATFORM_CODE_EXEC.to_vec();
        v.extend_from_slice(&["zsh", "fish", "eval", "exec", "env", "xargs", "sudo"]);
        v
    })
}

// ============================================================================
// Command safety classification
// ============================================================================

/// Safety classification for a bash command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandSafety {
    /// Command is read-only and safe to run without permission.
    ReadOnly,
    /// Command modifies files but is not destructive.
    FileModification {
        description: String,
    },
    /// Command is potentially destructive.
    Destructive {
        description: String,
    },
    /// Command cannot be classified -- ask user.
    Unknown,
}

impl CommandSafety {
    /// Returns `true` if the command is classified as safe (read-only).
    pub fn is_safe(&self) -> bool {
        matches!(self, CommandSafety::ReadOnly)
    }

    /// Returns `true` if the command is destructive.
    pub fn is_destructive(&self) -> bool {
        matches!(self, CommandSafety::Destructive { .. })
    }
}

/// Well-known read-only commands.
const READ_ONLY_COMMANDS: &[&str] = &[
    "ls", "cat", "head", "tail", "less", "more", "wc", "find", "grep", "rg", "ag", "ack",
    "tree", "file", "stat", "du", "df", "which", "whereis", "type", "echo", "printf",
    "date", "whoami", "hostname", "uname", "env", "printenv", "pwd", "id",
    "diff", "cmp", "md5sum", "sha256sum", "sha1sum", "xxd", "od",
    "man", "help", "info",
    "ps", "top", "htop", "uptime", "free", "vmstat", "iostat",
    "ping", "dig", "nslookup", "host", "traceroute", "curl", "wget",
    "git status", "git log", "git diff", "git show", "git branch",
    "git remote", "git tag", "git stash list", "git rev-parse",
    "cargo check", "cargo test", "cargo clippy", "cargo doc",
    "npm test", "npm list", "npm outdated", "npm info",
    "yarn test", "yarn list", "yarn info",
    "pnpm test", "pnpm list", "pnpm info",
];

/// Commands that destroy or irreversibly modify data.
static DESTRUCTIVE_PATTERNS: OnceLock<Vec<DestructivePattern>> = OnceLock::new();

struct DestructivePattern {
    regex: Regex,
    description: &'static str,
}

fn destructive_patterns() -> &'static Vec<DestructivePattern> {
    DESTRUCTIVE_PATTERNS.get_or_init(|| {
        vec![
            DestructivePattern {
                regex: Regex::new(r"(?:^|\s|;|&&|\|\|)rm\s").unwrap(),
                description: "rm: removes files or directories",
            },
            DestructivePattern {
                regex: Regex::new(r"(?:^|\s|;|&&|\|\|)rmdir\s").unwrap(),
                description: "rmdir: removes directories",
            },
            DestructivePattern {
                regex: Regex::new(r"(?:^|\s|;|&&|\|\|)chmod\s").unwrap(),
                description: "chmod: changes file permissions",
            },
            DestructivePattern {
                regex: Regex::new(r"(?:^|\s|;|&&|\|\|)chown\s").unwrap(),
                description: "chown: changes file ownership",
            },
            DestructivePattern {
                regex: Regex::new(r"git\s+reset\s+--hard").unwrap(),
                description: "git reset --hard: discards all uncommitted changes",
            },
            DestructivePattern {
                regex: Regex::new(r"git\s+clean\s+-[a-zA-Z]*f").unwrap(),
                description: "git clean -f: removes untracked files",
            },
            DestructivePattern {
                regex: Regex::new(r"git\s+push\s+.*--force").unwrap(),
                description: "git push --force: force pushes, potentially destroying remote history",
            },
            DestructivePattern {
                regex: Regex::new(r"git\s+push\s+.*-f\b").unwrap(),
                description: "git push -f: force pushes",
            },
            DestructivePattern {
                regex: Regex::new(r"git\s+checkout\s+--\s").unwrap(),
                description: "git checkout --: discards changes to files",
            },
            DestructivePattern {
                regex: Regex::new(r"git\s+branch\s+-[dD]\s").unwrap(),
                description: "git branch -d/-D: deletes branches",
            },
            DestructivePattern {
                regex: Regex::new(r"(?:^|\s|;|&&|\|\|)dd\s").unwrap(),
                description: "dd: low-level data copy, can overwrite devices",
            },
            DestructivePattern {
                regex: Regex::new(r"(?:^|\s|;|&&|\|\|)mkfs").unwrap(),
                description: "mkfs: creates filesystem, destroying existing data",
            },
            DestructivePattern {
                regex: Regex::new(r"(?:^|\s|;|&&|\|\|)format\s").unwrap(),
                description: "format: formats drives",
            },
            DestructivePattern {
                regex: Regex::new(r">\s*/dev/").unwrap(),
                description: "redirect to /dev/: writing to devices",
            },
            DestructivePattern {
                regex: Regex::new(r"(?:^|\s|;|&&|\|\|)kill\s").unwrap(),
                description: "kill: terminates processes",
            },
            DestructivePattern {
                regex: Regex::new(r"(?:^|\s|;|&&|\|\|)killall\s").unwrap(),
                description: "killall: terminates processes by name",
            },
            DestructivePattern {
                regex: Regex::new(r"(?:^|\s|;|&&|\|\|)pkill\s").unwrap(),
                description: "pkill: terminates processes by pattern",
            },
        ]
    })
}

/// Commands that modify files without being destructive.
static FILE_MODIFICATION_PATTERNS: OnceLock<Vec<FileModPattern>> = OnceLock::new();

struct FileModPattern {
    regex: Regex,
    description: &'static str,
}

fn file_modification_patterns() -> &'static Vec<FileModPattern> {
    FILE_MODIFICATION_PATTERNS.get_or_init(|| {
        vec![
            FileModPattern {
                regex: Regex::new(r"(?:^|\s|;|&&|\|\|)(?:tee|sed|awk)\s").unwrap(),
                description: "file content modification",
            },
            FileModPattern {
                regex: Regex::new(r"(?:^|\s|;|&&|\|\|)(?:cp|mv)\s").unwrap(),
                description: "file copy/move",
            },
            FileModPattern {
                regex: Regex::new(r"(?:^|\s|;|&&|\|\|)mkdir\s").unwrap(),
                description: "directory creation",
            },
            FileModPattern {
                regex: Regex::new(r"(?:^|\s|;|&&|\|\|)touch\s").unwrap(),
                description: "file creation/timestamp update",
            },
            FileModPattern {
                regex: Regex::new(r"(?:^|\s|;|&&|\|\|)(?:ln)\s").unwrap(),
                description: "link creation",
            },
            FileModPattern {
                // Detect single > redirection (but not >>).
                // We match `>` followed by optional space and a non-> char.
                regex: Regex::new(r">[^>]\s*\S").unwrap(),
                description: "output redirection (truncate)",
            },
            FileModPattern {
                regex: Regex::new(r">>\s*\S").unwrap(),
                description: "output redirection (append)",
            },
            FileModPattern {
                regex: Regex::new(r"git\s+(?:add|commit|merge|rebase|cherry-pick|stash\s+(?:push|pop|drop))").unwrap(),
                description: "git repository modification",
            },
            FileModPattern {
                regex: Regex::new(r"(?:npm|yarn|pnpm|bun)\s+(?:install|add|remove|uninstall)").unwrap(),
                description: "package management modification",
            },
            FileModPattern {
                regex: Regex::new(r"(?:pip|pip3)\s+(?:install|uninstall)").unwrap(),
                description: "Python package modification",
            },
            FileModPattern {
                regex: Regex::new(r"(?:cargo)\s+(?:install|add|remove)").unwrap(),
                description: "Rust package modification",
            },
        ]
    })
}

/// Classify a bash command's safety.
///
/// This is a heuristic classifier -- it checks known patterns.
/// Compound commands (`;`, `&&`, `||`) are split and each part is
/// individually classified, returning the most dangerous result.
pub fn classify_bash_command(command: &str) -> CommandSafety {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return CommandSafety::ReadOnly;
    }

    // Split on compound operators and classify each part.
    let subcommands = split_compound_command(trimmed);

    let mut worst = CommandSafety::ReadOnly;

    for subcmd in &subcommands {
        let sub_trimmed = subcmd.trim();
        if sub_trimmed.is_empty() {
            continue;
        }
        let result = classify_single_command(sub_trimmed);
        worst = most_dangerous(worst, result);
    }

    worst
}

/// Classify a single (non-compound) command.
fn classify_single_command(command: &str) -> CommandSafety {
    // Check destructive patterns first.
    for dp in destructive_patterns() {
        if dp.regex.is_match(command) {
            return CommandSafety::Destructive {
                description: dp.description.to_string(),
            };
        }
    }

    // Check file modification patterns.
    for fm in file_modification_patterns() {
        if fm.regex.is_match(command) {
            return CommandSafety::FileModification {
                description: fm.description.to_string(),
            };
        }
    }

    // Check if it starts with a known read-only command.
    let first_word = extract_first_word(command);
    for &ro in READ_ONLY_COMMANDS {
        // Some entries are multi-word (e.g. "git status"), check prefix.
        if command == ro || command.starts_with(&format!("{ro} ")) || command.starts_with(&format!("{ro}\t")) {
            return CommandSafety::ReadOnly;
        }
        // Single-word match.
        if first_word == ro {
            return CommandSafety::ReadOnly;
        }
    }

    CommandSafety::Unknown
}

/// Extract the first whitespace-delimited word from a command string.
fn extract_first_word(cmd: &str) -> &str {
    cmd.split_whitespace().next().unwrap_or("")
}

/// Split a compound command on `;`, `&&`, and `||`.
/// This is a simplified parser that does NOT handle nested quotes or subshells.
fn split_compound_command(cmd: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let bytes = cmd.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while i < len {
        let b = bytes[i];

        // Toggle quote state (simple; does not handle \' inside '').
        if b == b'\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            i += 1;
            continue;
        }
        if b == b'"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            i += 1;
            continue;
        }

        if in_single_quote || in_double_quote {
            i += 1;
            continue;
        }

        // Check for `&&` or `||`.
        if i + 1 < len {
            if (b == b'&' && bytes[i + 1] == b'&') || (b == b'|' && bytes[i + 1] == b'|') {
                parts.push(&cmd[start..i]);
                i += 2;
                start = i;
                continue;
            }
        }

        // Check for `;`.
        if b == b';' {
            parts.push(&cmd[start..i]);
            i += 1;
            start = i;
            continue;
        }

        i += 1;
    }

    if start < len {
        parts.push(&cmd[start..]);
    }

    parts
}

/// Return the more dangerous of two safety classifications.
fn most_dangerous(a: CommandSafety, b: CommandSafety) -> CommandSafety {
    fn severity(s: &CommandSafety) -> u8 {
        match s {
            CommandSafety::ReadOnly => 0,
            CommandSafety::FileModification { .. } => 1,
            CommandSafety::Unknown => 2,
            CommandSafety::Destructive { .. } => 3,
        }
    }
    if severity(&b) > severity(&a) {
        b
    } else {
        a
    }
}

/// Check whether a command prefix is a dangerous allow-rule pattern.
///
/// Returns `true` if allowing `"Bash(<prefix>:*)"` or similar would
/// grant unchecked arbitrary code execution.
pub fn is_dangerous_bash_prefix(prefix: &str) -> bool {
    let lower = prefix.to_lowercase();
    for &p in dangerous_bash_patterns() {
        if lower == p.to_lowercase() {
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

    #[test]
    fn read_only_commands() {
        assert!(classify_bash_command("ls -la").is_safe());
        assert!(classify_bash_command("cat foo.txt").is_safe());
        assert!(classify_bash_command("git status").is_safe());
        assert!(classify_bash_command("git log --oneline").is_safe());
        assert!(classify_bash_command("grep -r pattern .").is_safe());
        assert!(classify_bash_command("cargo test").is_safe());
    }

    #[test]
    fn destructive_commands() {
        assert!(classify_bash_command("rm -rf /").is_destructive());
        assert!(classify_bash_command("rm foo.txt").is_destructive());
        assert!(classify_bash_command("git reset --hard HEAD").is_destructive());
        assert!(classify_bash_command("git push --force").is_destructive());
        assert!(classify_bash_command("git clean -fd").is_destructive());
        assert!(classify_bash_command("dd if=/dev/zero of=/dev/sda").is_destructive());
    }

    #[test]
    fn file_modification_commands() {
        let result = classify_bash_command("cp a.txt b.txt");
        assert!(matches!(result, CommandSafety::FileModification { .. }));

        let result = classify_bash_command("mkdir new_dir");
        assert!(matches!(result, CommandSafety::FileModification { .. }));

        let result = classify_bash_command("npm install lodash");
        assert!(matches!(result, CommandSafety::FileModification { .. }));

        let result = classify_bash_command("git commit -m 'msg'");
        assert!(matches!(result, CommandSafety::FileModification { .. }));
    }

    #[test]
    fn compound_commands_worst_wins() {
        // read-only && destructive => destructive
        let result = classify_bash_command("ls -la && rm -rf /tmp/foo");
        assert!(result.is_destructive());

        // read-only ; modification => modification
        let result = classify_bash_command("ls -la; touch new.txt");
        assert!(matches!(result, CommandSafety::FileModification { .. }));
    }

    #[test]
    fn empty_command_is_safe() {
        assert!(classify_bash_command("").is_safe());
        assert!(classify_bash_command("   ").is_safe());
    }

    #[test]
    fn unknown_command() {
        let result = classify_bash_command("some_custom_tool --flag");
        assert!(matches!(result, CommandSafety::Unknown));
    }

    #[test]
    fn dangerous_prefixes() {
        assert!(is_dangerous_bash_prefix("python"));
        assert!(is_dangerous_bash_prefix("node"));
        assert!(is_dangerous_bash_prefix("bash"));
        assert!(is_dangerous_bash_prefix("ssh"));
        assert!(is_dangerous_bash_prefix("eval"));
        assert!(!is_dangerous_bash_prefix("ls"));
        assert!(!is_dangerous_bash_prefix("cat"));
    }

    #[test]
    fn compound_with_quotes_preserved() {
        // Semicolons inside quotes should not split.
        let result = classify_bash_command(r#"echo "hello; world""#);
        assert!(result.is_safe());
    }

    #[test]
    fn redirections_detected() {
        let result = classify_bash_command("echo hello > output.txt");
        assert!(matches!(result, CommandSafety::FileModification { .. }));
    }
}
