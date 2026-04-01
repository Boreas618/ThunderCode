//! ThunderCode terminal UI engine (Ink equivalent).
//!
//! This crate provides a terminal UI rendering engine with:
//! - Packed cell format (2x u32 per cell) for zero-GC-pressure screen buffers
//! - Interning pools for characters, styles, and hyperlinks
//! - Flexbox layout via `taffy`
//! - Text wrapping and measurement (Unicode-aware)
//! - Double-buffered rendering with damage tracking
//! - ANSI/CSI/SGR/OSC terminal escape sequences
//! - DOM element tree for component composition
//! - Event parsing for keyboard, mouse, focus, paste

pub mod app;
pub mod cell;
pub mod components;
pub mod diff_display;
pub mod dom;
pub mod events;
pub mod layout;
pub mod markdown;
pub mod pools;
pub mod renderer;
pub mod screen;
pub mod style;
pub mod syntax_highlight;
pub mod termio;
pub mod text;
pub mod virtual_list;
