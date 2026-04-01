//! Animated spinner with verb rotation and elapsed time.
//!
//! Mirrors the ref's `<SpinnerWithVerb>` component. Shows an animated braille
//! spinner with a verb message ("Thinking...", "Reading...", etc.) and elapsed
//! time counter.

use std::time::Instant;

/// Spinner animation frames (braille dots, forward and reverse).
const SPINNER_CHARS: &[&str] = &[
    "\u{2801}", "\u{2809}", "\u{2819}", "\u{281b}",
    "\u{281e}", "\u{2856}", "\u{28c4}", "\u{28e0}",
    "\u{28a0}", "\u{2820}",
];

/// Verbs shown during spinning (randomly picked at start, then cycled).
const SPINNER_VERBS: &[&str] = &[
    "Thinking",
    "Reasoning",
    "Analyzing",
    "Considering",
    "Processing",
    "Evaluating",
    "Pondering",
    "Working",
];

/// Spinner state for animated terminal display.
pub struct Spinner {
    /// Current animation frame index.
    frame: usize,
    /// When this spinner started.
    start_time: Instant,
    /// The verb to display.
    verb: String,
    /// Optional override message.
    override_message: Option<String>,
}

impl Spinner {
    /// Create a new spinner with a random verb.
    pub fn new() -> Self {
        let verb_idx = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize)
            % SPINNER_VERBS.len();
        Self {
            frame: 0,
            start_time: Instant::now(),
            verb: SPINNER_VERBS[verb_idx].to_string(),
            override_message: None,
        }
    }

    /// Create a spinner with a specific message.
    pub fn with_message(message: impl Into<String>) -> Self {
        Self {
            override_message: Some(message.into()),
            ..Self::new()
        }
    }

    /// Set the verb text.
    pub fn set_verb(&mut self, verb: impl Into<String>) {
        self.verb = verb.into();
    }

    /// Set an override message (replaces the verb).
    pub fn set_override_message(&mut self, msg: Option<String>) {
        self.override_message = msg;
    }

    /// Get the current spinner character.
    pub fn current_char(&self) -> &str {
        SPINNER_CHARS[self.frame % SPINNER_CHARS.len()]
    }

    /// Get the display message.
    pub fn message(&self) -> String {
        let msg = self
            .override_message
            .as_deref()
            .unwrap_or(&self.verb);
        format!("{}\u{2026}", msg)
    }

    /// Get elapsed time as a formatted string.
    pub fn elapsed_str(&self) -> String {
        let elapsed = self.start_time.elapsed();
        let secs = elapsed.as_secs();
        if secs < 60 {
            format!("{}s", secs)
        } else {
            format!("{}m{}s", secs / 60, secs % 60)
        }
    }

    /// Advance to the next frame. Returns the rendered line.
    pub fn tick(&mut self) -> SpinnerFrame {
        self.frame = self.frame.wrapping_add(1);
        SpinnerFrame {
            spinner_char: self.current_char().to_string(),
            message: self.message(),
            elapsed: self.elapsed_str(),
        }
    }

    /// Reset the spinner timer (e.g., after receiving first token).
    pub fn reset_timer(&mut self) {
        self.start_time = Instant::now();
    }

    /// Get start time.
    pub fn start_time(&self) -> Instant {
        self.start_time
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

/// A single rendered frame of the spinner.
pub struct SpinnerFrame {
    pub spinner_char: String,
    pub message: String,
    pub elapsed: String,
}

impl SpinnerFrame {
    /// Render the spinner frame as an ANSI-colored string.
    /// Uses the primary theme color (magenta/purple).
    pub fn render(&self) -> String {
        // Shimmer effect: apply magenta to the spinner char, dim to elapsed time
        format!(
            "\x1b[35m{}\x1b[0m \x1b[35m{}\x1b[0m \x1b[2m({})\x1b[0m",
            self.spinner_char, self.message, self.elapsed
        )
    }
}
