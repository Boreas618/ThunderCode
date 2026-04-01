//! OS notification service.
//!
//! Ported from ref/services/notifier.ts`. Sends desktop notifications when
//! long-running operations complete. The TypeScript version supports multiple
//! channels (iTerm2, Kitty, Ghostty, terminal bell, etc.). This Rust port
//! uses platform-native tools: `osascript` on macOS, `notify-send` on Linux.

use std::process::Command;

// ---------------------------------------------------------------------------
// NotificationOptions
// ---------------------------------------------------------------------------

/// Options for sending a notification.
#[derive(Debug, Clone)]
pub struct NotificationOptions {
    /// The notification body text.
    pub message: String,
    /// Optional title (defaults to "ThunderCode").
    pub title: Option<String>,
    /// Notification type tag (e.g. "task_complete", "error").
    pub notification_type: String,
}

// ---------------------------------------------------------------------------
// send_notification
// ---------------------------------------------------------------------------

/// Send an OS-level notification.
///
/// On macOS, uses `osascript` to display a native notification. On Linux, uses
/// `notify-send`. On other platforms this is a no-op that logs a debug message.
pub fn send_notification(message: &str, notification_type: &str) {
    send_notification_with_options(&NotificationOptions {
        message: message.to_owned(),
        title: None,
        notification_type: notification_type.to_owned(),
    });
}

/// Send a notification with full options.
pub fn send_notification_with_options(opts: &NotificationOptions) {
    let title = opts.title.as_deref().unwrap_or("ThunderCode");

    tracing::debug!(
        title,
        notification_type = opts.notification_type,
        "notifier: sending notification"
    );

    if cfg!(target_os = "macos") {
        send_macos_notification(title, &opts.message);
    } else if cfg!(target_os = "linux") {
        send_linux_notification(title, &opts.message);
    } else {
        tracing::debug!("notifier: notifications not supported on this platform");
    }
}

// ---------------------------------------------------------------------------
// Platform implementations
// ---------------------------------------------------------------------------

/// macOS: use osascript to trigger a native notification.
fn send_macos_notification(title: &str, message: &str) {
    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        escape_applescript(message),
        escape_applescript(title),
    );

    let result = Command::new("osascript")
        .args(["-e", &script])
        .output();

    match result {
        Ok(output) if !output.status.success() => {
            tracing::warn!(
                "notifier: osascript failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Err(e) => {
            tracing::warn!("notifier: failed to spawn osascript: {e}");
        }
        _ => {}
    }
}

/// Linux: use notify-send.
fn send_linux_notification(title: &str, message: &str) {
    let result = Command::new("notify-send")
        .args([title, message])
        .output();

    match result {
        Ok(output) if !output.status.success() => {
            tracing::warn!(
                "notifier: notify-send failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Err(e) => {
            tracing::warn!("notifier: failed to spawn notify-send: {e}");
        }
        _ => {}
    }
}

/// Escape a string for use inside AppleScript double-quoted strings.
fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_applescript_quotes() {
        assert_eq!(escape_applescript(r#"say "hi""#), r#"say \"hi\""#);
    }

    #[test]
    fn escape_applescript_backslashes() {
        assert_eq!(escape_applescript(r"path\to\file"), r"path\\to\\file");
    }

    #[test]
    fn notification_options_defaults() {
        let opts = NotificationOptions {
            message: "done".to_owned(),
            title: None,
            notification_type: "task_complete".to_owned(),
        };
        // Just verify it constructs without panic.
        assert_eq!(opts.title, None);
    }
}
