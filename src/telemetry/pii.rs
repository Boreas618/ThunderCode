//! PII scrubbing utilities.
//!
//! Removes personally identifiable information from telemetry strings
//! before they are logged or transmitted.  Handles:
//!
//! - Email addresses
//! - IP addresses (IPv4)
//! - Phone numbers (US format)
//! - Social security numbers (US)
//! - Credit card numbers (basic patterns)
//! - Absolute file paths (Unix and Windows)
//!
//! The replacements use `[REDACTED]` or `[PATH]` markers so the
//! surrounding context remains readable for debugging.

use regex::Regex;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Compiled regexes (computed once)
// ---------------------------------------------------------------------------

fn email_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}").unwrap())
}

fn ipv4_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b").unwrap())
}

fn phone_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // US-style phone: (123) 456-7890, 123-456-7890, +1-123-456-7890, etc.
        Regex::new(r"(?:\+?1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b").unwrap()
    })
}

fn ssn_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap())
}

fn credit_card_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // 13-19 digit sequences, optionally separated by spaces or dashes in groups of 4
        Regex::new(r"\b(?:\d[ -]?){12,18}\d\b").unwrap()
    })
}

fn unix_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Absolute Unix paths: /home/user/..., /Users/alice/...
        // Must start with / followed by at least one path component
        Regex::new(r"(?:/(?:home|Users|root|tmp|var|etc|opt|usr)/)\S+").unwrap()
    })
}

fn windows_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Windows absolute paths: C:\Users\..., D:\...
        Regex::new(r"[A-Z]:\\(?:Users|Documents and Settings)\\\S+").unwrap()
    })
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Scrub common PII patterns from `text`.
///
/// Replaces email addresses, IPv4 addresses, US phone numbers, SSNs,
/// and credit-card-like digit sequences with `[REDACTED]`.
pub fn scrub_pii(text: &str) -> String {
    let mut result = text.to_string();

    // Order matters: SSN before phone (SSN is a subset pattern of phone)
    result = ssn_re().replace_all(&result, "[REDACTED]").to_string();
    result = email_re().replace_all(&result, "[REDACTED]").to_string();
    result = credit_card_re().replace_all(&result, "[REDACTED]").to_string();
    result = phone_re().replace_all(&result, "[REDACTED]").to_string();
    result = ipv4_re().replace_all(&result, "[REDACTED]").to_string();

    result
}

/// Replace absolute file paths that may contain usernames with `[PATH]`.
///
/// Targets Unix paths under `/home/`, `/Users/`, `/root/` etc. and
/// Windows paths under `C:\Users\`.
pub fn scrub_file_paths(text: &str) -> String {
    let mut result = text.to_string();
    result = unix_path_re().replace_all(&result, "[PATH]").to_string();
    result = windows_path_re().replace_all(&result, "[PATH]").to_string();
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- scrub_pii --

    #[test]
    fn test_scrub_email() {
        let input = "Contact alice@example.com for details";
        let output = scrub_pii(input);
        assert_eq!(output, "Contact [REDACTED] for details");
    }

    #[test]
    fn test_scrub_multiple_emails() {
        let input = "From a@b.com to c@d.org";
        let output = scrub_pii(input);
        assert!(!output.contains("a@b.com"));
        assert!(!output.contains("c@d.org"));
        assert_eq!(output.matches("[REDACTED]").count(), 2);
    }

    #[test]
    fn test_scrub_ipv4() {
        let input = "Server at 192.168.1.100 responded";
        let output = scrub_pii(input);
        assert_eq!(output, "Server at [REDACTED] responded");
    }

    #[test]
    fn test_scrub_phone() {
        let input = "Call (555) 123-4567 now";
        let output = scrub_pii(input);
        assert!(!output.contains("555"));
        assert!(output.contains("[REDACTED]"));
    }

    #[test]
    fn test_scrub_ssn() {
        let input = "SSN is 123-45-6789";
        let output = scrub_pii(input);
        assert!(!output.contains("123-45-6789"));
        assert!(output.contains("[REDACTED]"));
    }

    #[test]
    fn test_scrub_no_pii() {
        let input = "This is a normal log line with no PII.";
        let output = scrub_pii(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_scrub_empty() {
        assert_eq!(scrub_pii(""), "");
    }

    // -- scrub_file_paths --

    #[test]
    fn test_scrub_unix_path() {
        let input = "File at /Users/alice/project/src/main.rs";
        let output = scrub_file_paths(input);
        assert_eq!(output, "File at [PATH]");
        assert!(!output.contains("alice"));
    }

    #[test]
    fn test_scrub_home_path() {
        let input = "Config in /home/bob/.config/nano.toml";
        let output = scrub_file_paths(input);
        assert!(!output.contains("bob"));
        assert!(output.contains("[PATH]"));
    }

    #[test]
    fn test_scrub_windows_path() {
        let input = r"File at C:\Users\Charlie\Documents\project\file.txt";
        let output = scrub_file_paths(input);
        assert!(!output.contains("Charlie"));
        assert!(output.contains("[PATH]"));
    }

    #[test]
    fn test_scrub_path_no_path() {
        let input = "Just a regular string";
        let output = scrub_file_paths(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_scrub_path_relative_unchanged() {
        let input = "Look at src/main.rs";
        let output = scrub_file_paths(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_combined_pii_and_paths() {
        let input = "User alice@example.com at /Users/alice/code connected from 10.0.0.1";
        let step1 = scrub_pii(input);
        let step2 = scrub_file_paths(&step1);
        assert!(!step2.contains("alice"));
        assert!(!step2.contains("10.0.0.1"));
    }
}
