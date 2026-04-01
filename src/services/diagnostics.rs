//! System diagnostics service.
//!
//! Ported from ref/services/diagnosticTracking.ts` (the IDE diagnostics
//! tracker) and general system health checks. Provides a lightweight
//! diagnostics runner that checks prerequisites like API connectivity,
//! configuration validity, and tool availability.

use std::fmt;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Result of running all diagnostic checks.
#[derive(Debug, Clone)]
pub struct DiagnosticsResult {
    /// Individual check results.
    pub checks: Vec<DiagnosticCheck>,
}

/// A single diagnostic check.
#[derive(Debug, Clone)]
pub struct DiagnosticCheck {
    /// Human-readable check name (e.g. "API Key", "Git").
    pub name: String,
    /// Pass / Warn / Fail status.
    pub status: CheckStatus,
    /// Description of the result or guidance for fixing.
    pub message: String,
}

/// Status of a single diagnostic check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    /// Everything is fine.
    Pass,
    /// Non-critical issue; the tool can still function.
    Warn,
    /// Critical issue; some functionality will not work.
    Fail,
}

impl fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckStatus::Pass => write!(f, "PASS"),
            CheckStatus::Warn => write!(f, "WARN"),
            CheckStatus::Fail => write!(f, "FAIL"),
        }
    }
}

impl DiagnosticsResult {
    /// Whether all checks passed (no warnings or failures).
    pub fn all_pass(&self) -> bool {
        self.checks.iter().all(|c| c.status == CheckStatus::Pass)
    }

    /// Whether any check failed.
    pub fn has_failures(&self) -> bool {
        self.checks.iter().any(|c| c.status == CheckStatus::Fail)
    }

    /// Format diagnostics as a human-readable report.
    pub fn format_report(&self) -> String {
        let mut lines = Vec::new();
        for check in &self.checks {
            lines.push(format!("[{}] {}: {}", check.status, check.name, check.message));
        }
        lines.join("\n")
    }
}

// ---------------------------------------------------------------------------
// run_diagnostics
// ---------------------------------------------------------------------------

/// Run all diagnostic checks and return the results.
///
/// Currently checks:
/// - API key presence (via `THUNDERCODE_API_KEY` env var).
/// - Git availability.
/// - Shell availability.
/// - Voice recording availability.
pub async fn run_diagnostics() -> DiagnosticsResult {
    let mut checks = Vec::new();

    checks.push(check_api_key());
    checks.push(check_git());
    checks.push(check_shell());
    checks.push(check_voice());

    DiagnosticsResult { checks }
}

// ---------------------------------------------------------------------------
// Individual checks
// ---------------------------------------------------------------------------

fn check_api_key() -> DiagnosticCheck {
    let has_key = std::env::var("THUNDERCODE_API_KEY")
        .map(|k| !k.is_empty())
        .unwrap_or(false);

    if has_key {
        DiagnosticCheck {
            name: "API Key".to_owned(),
            status: CheckStatus::Pass,
            message: "THUNDERCODE_API_KEY is set".to_owned(),
        }
    } else {
        DiagnosticCheck {
            name: "API Key".to_owned(),
            status: CheckStatus::Fail,
            message: "THUNDERCODE_API_KEY is not set. Set it to your API key.".to_owned(),
        }
    }
}

fn check_git() -> DiagnosticCheck {
    match std::process::Command::new("git")
        .arg("--version")
        .output()
    {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_owned();
            DiagnosticCheck {
                name: "Git".to_owned(),
                status: CheckStatus::Pass,
                message: version,
            }
        }
        Ok(_) => DiagnosticCheck {
            name: "Git".to_owned(),
            status: CheckStatus::Warn,
            message: "git found but returned non-zero exit code".to_owned(),
        },
        Err(_) => DiagnosticCheck {
            name: "Git".to_owned(),
            status: CheckStatus::Warn,
            message: "git not found on PATH. Some features may not work.".to_owned(),
        },
    }
}

fn check_shell() -> DiagnosticCheck {
    let shell = std::env::var("SHELL").unwrap_or_default();
    if shell.is_empty() {
        DiagnosticCheck {
            name: "Shell".to_owned(),
            status: CheckStatus::Warn,
            message: "SHELL env var not set".to_owned(),
        }
    } else {
        DiagnosticCheck {
            name: "Shell".to_owned(),
            status: CheckStatus::Pass,
            message: format!("Using {shell}"),
        }
    }
}

fn check_voice() -> DiagnosticCheck {
    let available = crate::services::voice::VoiceService::is_available();
    if available {
        DiagnosticCheck {
            name: "Voice Recording".to_owned(),
            status: CheckStatus::Pass,
            message: "Recording backend available".to_owned(),
        }
    } else {
        DiagnosticCheck {
            name: "Voice Recording".to_owned(),
            status: CheckStatus::Warn,
            message: "No recording backend found (install SoX for voice support)".to_owned(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn diagnostics_returns_checks() {
        let result = run_diagnostics().await;
        assert!(!result.checks.is_empty());
    }

    #[test]
    fn format_report_is_nonempty() {
        let result = DiagnosticsResult {
            checks: vec![DiagnosticCheck {
                name: "Test".to_owned(),
                status: CheckStatus::Pass,
                message: "ok".to_owned(),
            }],
        };
        let report = result.format_report();
        assert!(report.contains("[PASS]"));
        assert!(report.contains("Test"));
    }

    #[test]
    fn all_pass_and_has_failures() {
        let passing = DiagnosticsResult {
            checks: vec![DiagnosticCheck {
                name: "A".to_owned(),
                status: CheckStatus::Pass,
                message: "ok".to_owned(),
            }],
        };
        assert!(passing.all_pass());
        assert!(!passing.has_failures());

        let failing = DiagnosticsResult {
            checks: vec![DiagnosticCheck {
                name: "B".to_owned(),
                status: CheckStatus::Fail,
                message: "bad".to_owned(),
            }],
        };
        assert!(!failing.all_pass());
        assert!(failing.has_failures());
    }
}
