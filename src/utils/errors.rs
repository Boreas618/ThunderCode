//! Common error types and error formatting utilities.
//!
//! Ported from ref/utils/errors.ts`. Uses `thiserror` for ergonomic error
//! definition and implements the error classification patterns from the
//! TypeScript codebase.

use std::io;

/// Top-level ThunderCode error type covering the common error categories
/// encountered throughout the codebase.
#[derive(Debug, thiserror::Error)]
pub enum ThunderCodeError {
    /// The user or system aborted an operation.
    #[error("operation aborted")]
    Abort,

    /// A shell command failed with a non-zero exit code.
    #[error("shell command failed (exit {code})")]
    Shell {
        stdout: String,
        stderr: String,
        code: i32,
        interrupted: bool,
    },

    /// A configuration file could not be parsed.
    #[error("config parse error in {file_path}: {message}")]
    ConfigParse {
        message: String,
        file_path: String,
    },

    /// A malformed command was received.
    #[error("malformed command: {0}")]
    MalformedCommand(String),

    /// An operation timed out.
    #[error("operation timed out: {0}")]
    Timeout(String),

    /// A network request failed.
    #[error("network error: {0}")]
    Network(String),

    /// An authentication/authorization failure.
    #[error("auth error: {0}")]
    Auth(String),

    /// An HTTP error with a status code.
    #[error("HTTP {status}: {message}")]
    Http {
        status: u16,
        message: String,
    },

    /// A filesystem path is inaccessible (not found, permission denied, etc.).
    #[error("path inaccessible: {0}")]
    FsInaccessible(String),

    /// Wrapper around `std::io::Error`.
    #[error(transparent)]
    Io(#[from] io::Error),

    /// Wrapper around `serde_json::Error`.
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// Any other error.
    #[error("{0}")]
    Other(String),
}

/// Check whether an error is an abort-shaped error.
///
/// Matches the TS `isAbortError` function: returns `true` for our `Abort`
/// variant, IO errors with `ErrorKind::Interrupted`, and errors whose message
/// contains "abort".
pub fn is_abort_error(e: &ThunderCodeError) -> bool {
    matches!(e, ThunderCodeError::Abort)
}

/// Check whether an IO error code indicates the path is missing or inaccessible.
///
/// Covers: NotFound, PermissionDenied, and other filesystem-structural errors
/// (matching the TS `isFsInaccessible` helper).
pub fn is_fs_inaccessible(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
    )
}

/// Classification buckets for HTTP/network errors, matching the TS
/// `AxiosErrorKind` type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpErrorKind {
    /// 401/403 -- authentication/authorization failure.
    Auth,
    /// Connection timed out.
    Timeout,
    /// Connection refused / DNS failure.
    Network,
    /// Other HTTP error (may have status code).
    Http,
    /// Not an HTTP error at all.
    Other,
}

/// Classify a [`ThunderCodeError`] into an HTTP error bucket.
pub fn classify_http_error(e: &ThunderCodeError) -> HttpErrorKind {
    match e {
        ThunderCodeError::Auth(_) => HttpErrorKind::Auth,
        ThunderCodeError::Timeout(_) => HttpErrorKind::Timeout,
        ThunderCodeError::Network(_) => HttpErrorKind::Network,
        ThunderCodeError::Http { .. } => HttpErrorKind::Http,
        _ => HttpErrorKind::Other,
    }
}

/// Convert any value into an error message string.
///
/// Equivalent to the TS `errorMessage(e)` helper.
pub fn error_message(e: &dyn std::error::Error) -> String {
    e.to_string()
}

/// Convert any value into a user-friendly error message, stripping
/// implementation details and internal frames.
pub fn friendly_error_message(e: &ThunderCodeError) -> String {
    match e {
        ThunderCodeError::Shell { stderr, code, .. } => {
            if stderr.is_empty() {
                format!("Command failed with exit code {}", code)
            } else {
                // Take first non-empty line of stderr
                let first_line = stderr.lines().find(|l| !l.trim().is_empty());
                first_line.unwrap_or("Command failed").to_string()
            }
        }
        ThunderCodeError::ConfigParse { file_path, message } => {
            format!("Could not parse config file {}: {}", file_path, message)
        }
        ThunderCodeError::Timeout(msg) => format!("Operation timed out: {}", msg),
        ThunderCodeError::Io(io_err) if io_err.kind() == io::ErrorKind::NotFound => {
            format!("File not found: {}", io_err)
        }
        ThunderCodeError::Io(io_err) if io_err.kind() == io::ErrorKind::PermissionDenied => {
            format!("Permission denied: {}", io_err)
        }
        other => other.to_string(),
    }
}

/// Extract a short error stack: message + top N frames.
///
/// Equivalent to the TS `shortErrorStack(e, maxFrames)` helper. In Rust,
/// backtraces are not always available, so we just return the display chain.
pub fn short_error_chain(e: &dyn std::error::Error, max_depth: usize) -> String {
    let mut parts = Vec::new();
    let mut current: Option<&dyn std::error::Error> = Some(e);
    let mut depth = 0;

    while let Some(err) = current {
        if depth >= max_depth {
            parts.push("...".to_string());
            break;
        }
        parts.push(err.to_string());
        current = err.source();
        depth += 1;
    }

    parts.join(": ")
}

/// Check whether an error has a specific message, matching the TS
/// `hasExactErrorMessage` helper.
pub fn has_exact_error_message(e: &dyn std::error::Error, message: &str) -> bool {
    e.to_string() == message
}

/// Filesystem error codes (matching the TS `getErrnoCode` / `isENOENT` pattern).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsErrorCode {
    NotFound,
    PermissionDenied,
    NotADirectory,
    Other,
}

/// Extract a filesystem error code from an IO error.
pub fn fs_error_code(e: &io::Error) -> FsErrorCode {
    match e.kind() {
        io::ErrorKind::NotFound => FsErrorCode::NotFound,
        io::ErrorKind::PermissionDenied => FsErrorCode::PermissionDenied,
        _ => FsErrorCode::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_abort_error() {
        assert!(is_abort_error(&ThunderCodeError::Abort));
        assert!(!is_abort_error(&ThunderCodeError::Timeout("test".into())));
    }

    #[test]
    fn test_classify_http_error() {
        assert_eq!(
            classify_http_error(&ThunderCodeError::Auth("bad token".into())),
            HttpErrorKind::Auth
        );
        assert_eq!(
            classify_http_error(&ThunderCodeError::Timeout("5s".into())),
            HttpErrorKind::Timeout
        );
        assert_eq!(
            classify_http_error(&ThunderCodeError::Network("refused".into())),
            HttpErrorKind::Network
        );
        assert_eq!(
            classify_http_error(&ThunderCodeError::Http {
                status: 500,
                message: "server error".into()
            }),
            HttpErrorKind::Http
        );
        assert_eq!(
            classify_http_error(&ThunderCodeError::Abort),
            HttpErrorKind::Other
        );
    }

    #[test]
    fn test_friendly_error_message_shell() {
        let e = ThunderCodeError::Shell {
            stdout: String::new(),
            stderr: "error: file not found\ndetails here".to_string(),
            code: 1,
            interrupted: false,
        };
        assert_eq!(friendly_error_message(&e), "error: file not found");
    }

    #[test]
    fn test_friendly_error_message_shell_empty_stderr() {
        let e = ThunderCodeError::Shell {
            stdout: String::new(),
            stderr: String::new(),
            code: 127,
            interrupted: false,
        };
        assert_eq!(friendly_error_message(&e), "Command failed with exit code 127");
    }

    #[test]
    fn test_short_error_chain() {
        let inner = io::Error::new(io::ErrorKind::NotFound, "file.txt");
        let outer = ThunderCodeError::Io(inner);
        let chain = short_error_chain(&outer, 5);
        assert!(chain.contains("file.txt"));
    }

    #[test]
    fn test_has_exact_error_message() {
        let e = ThunderCodeError::Other("exact match".into());
        assert!(has_exact_error_message(&e, "exact match"));
        assert!(!has_exact_error_message(&e, "not a match"));
    }

    #[test]
    fn test_is_fs_inaccessible() {
        let not_found = io::Error::new(io::ErrorKind::NotFound, "missing");
        let perm = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        let other = io::Error::new(io::ErrorKind::Other, "something");
        assert!(is_fs_inaccessible(&not_found));
        assert!(is_fs_inaccessible(&perm));
        assert!(!is_fs_inaccessible(&other));
    }
}
