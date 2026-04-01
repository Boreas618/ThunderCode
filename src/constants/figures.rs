//! Unicode box-drawing / UI characters used in terminal output.

/// Black circle – platform-sensitive in TS, we default to the macOS glyph.
pub const BLACK_CIRCLE: &str = "\u{23FA}"; // ⏺
/// Fallback for non-macOS platforms.
pub const BLACK_CIRCLE_FALLBACK: &str = "\u{25CF}"; // ●

pub const BULLET_OPERATOR: &str = "\u{2219}"; // ∙
pub const TEARDROP_ASTERISK: &str = "\u{273B}"; // ✻
pub const UP_ARROW: &str = "\u{2191}"; // ↑
pub const DOWN_ARROW: &str = "\u{2193}"; // ↓
pub const LIGHTNING_BOLT: &str = "\u{21AF}"; // ↯
pub const EFFORT_LOW: &str = "\u{25CB}"; // ○
pub const EFFORT_MEDIUM: &str = "\u{25D0}"; // ◐
pub const EFFORT_HIGH: &str = "\u{25CF}"; // ●
pub const EFFORT_MAX: &str = "\u{25C9}"; // ◉

// Media / trigger status indicators
pub const PLAY_ICON: &str = "\u{25B6}"; // ▶
pub const PAUSE_ICON: &str = "\u{23F8}"; // ⏸

// MCP subscription indicators
pub const REFRESH_ARROW: &str = "\u{21BB}"; // ↻
pub const CHANNEL_ARROW: &str = "\u{2190}"; // ←
pub const INJECTED_ARROW: &str = "\u{2192}"; // →
pub const FORK_GLYPH: &str = "\u{2442}"; // ⑂

// Review status indicators (ultrareview diamond states)
pub const DIAMOND_OPEN: &str = "\u{25C7}"; // ◇
pub const DIAMOND_FILLED: &str = "\u{25C6}"; // ◆
pub const REFERENCE_MARK: &str = "\u{203B}"; // ※

// Issue flag indicator
pub const FLAG_ICON: &str = "\u{2691}"; // ⚑

// Blockquote / box-drawing
pub const BLOCKQUOTE_BAR: &str = "\u{258E}"; // ▎
pub const HEAVY_HORIZONTAL: &str = "\u{2501}"; // ━

// Bridge status indicators
pub const BRIDGE_SPINNER_FRAMES: [&str; 4] = [
    "\u{00B7}|\u{00B7}",
    "\u{00B7}/\u{00B7}",
    "\u{00B7}\u{2014}\u{00B7}",
    "\u{00B7}\\\u{00B7}",
];
pub const BRIDGE_READY_INDICATOR: &str = "\u{00B7}\u{2714}\u{FE0E}\u{00B7}";
pub const BRIDGE_FAILED_INDICATOR: &str = "\u{00D7}"; // ×
