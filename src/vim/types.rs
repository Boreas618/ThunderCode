//! Core types for the vim state machine.
//!
//! State Diagram:
//! ```text
//!                              VimState
//!   +------------------------------+--------------------------------------+
//!   |  INSERT                      |  NORMAL                              |
//!   |  (tracks inserted_text)      |  (CommandState machine)              |
//!   |                              |                                      |
//!   |                              |  Idle --+-[d/c/y]--> Operator        |
//!   |                              |         +-[1-9]----> Count           |
//!   |                              |         +-[fFtT]---> Find            |
//!   |                              |         +-[g]------> G               |
//!   |                              |         +-[r]------> Replace         |
//!   |                              |         +-[><]-----> Indent          |
//!   |                              |                                      |
//!   |                              |  Operator -+-[motion]--> execute     |
//!   |                              |            +-[0-9]----> OperatorCount|
//!   |                              |            +-[ia]-----> OperatorTextObj
//!   |                              |            +-[fFtT]---> OperatorFind |
//!   +------------------------------+--------------------------------------+
//! ```

use serde::{Deserialize, Serialize};

// ============================================================================
// Core Types
// ============================================================================

/// Vim operator (d, c, y).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operator {
    Delete,
    Change,
    Yank,
}

impl Operator {
    /// The key that triggers this operator (also used for line-op detection: dd, cc, yy).
    pub fn key(self) -> char {
        match self {
            Operator::Delete => 'd',
            Operator::Change => 'c',
            Operator::Yank => 'y',
        }
    }

    pub fn from_key(ch: char) -> Option<Self> {
        match ch {
            'd' => Some(Operator::Delete),
            'c' => Some(Operator::Change),
            'y' => Some(Operator::Yank),
            _ => None,
        }
    }
}

/// Find motion type (f, F, t, T).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FindType {
    /// `f` - find forward (inclusive)
    F,
    /// `F` - find backward (inclusive)
    FBack,
    /// `t` - till forward (exclusive - one before)
    T,
    /// `T` - till backward (exclusive - one after)
    TBack,
}

impl FindType {
    pub fn from_key(ch: char) -> Option<Self> {
        match ch {
            'f' => Some(FindType::F),
            'F' => Some(FindType::FBack),
            't' => Some(FindType::T),
            'T' => Some(FindType::TBack),
            _ => None,
        }
    }

    /// Reverse the direction of this find type.
    pub fn flipped(self) -> Self {
        match self {
            FindType::F => FindType::FBack,
            FindType::FBack => FindType::F,
            FindType::T => FindType::TBack,
            FindType::TBack => FindType::T,
        }
    }
}

/// Scope for text objects: `i` (inner) or `a` (around).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextObjScope {
    Inner,
    Around,
}

impl TextObjScope {
    pub fn from_key(ch: char) -> Option<Self> {
        match ch {
            'i' => Some(TextObjScope::Inner),
            'a' => Some(TextObjScope::Around),
            _ => None,
        }
    }
}

// ============================================================================
// State Machine Types
// ============================================================================

/// Top-level vim mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VimMode {
    Insert,
    Normal,
}

/// Complete vim state. Mode determines what data is tracked.
///
/// INSERT mode: Track text being typed (for dot-repeat).
/// NORMAL mode: Track command being parsed (state machine).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VimState {
    Insert { inserted_text: String },
    Normal { command: CommandState },
}

impl VimState {
    pub fn mode(&self) -> VimMode {
        match self {
            VimState::Insert { .. } => VimMode::Insert,
            VimState::Normal { .. } => VimMode::Normal,
        }
    }

    pub fn is_insert(&self) -> bool {
        matches!(self, VimState::Insert { .. })
    }

    pub fn is_normal(&self) -> bool {
        matches!(self, VimState::Normal { .. })
    }
}

/// Command state machine for NORMAL mode.
///
/// Each variant knows exactly what input it's waiting for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandState {
    /// Waiting for first input.
    Idle,
    /// Accumulating count digits (e.g. "23").
    Count { digits: String },
    /// Got an operator, waiting for motion/text-object.
    Operator { op: Operator, count: usize },
    /// Inside an operator, accumulating a motion count (e.g. d3w).
    OperatorCount {
        op: Operator,
        count: usize,
        digits: String,
    },
    /// Inside an operator, waiting for the character after f/F/t/T.
    OperatorFind {
        op: Operator,
        count: usize,
        find: FindType,
    },
    /// Inside an operator, got `i` or `a`, waiting for text-object type.
    OperatorTextObj {
        op: Operator,
        count: usize,
        scope: TextObjScope,
    },
    /// Waiting for the character after f/F/t/T (standalone motion).
    Find { find: FindType, count: usize },
    /// Got `g`, waiting for second key (gg, gj, gk).
    G { count: usize },
    /// Inside an operator, got `g`, waiting for second key.
    OperatorG { op: Operator, count: usize },
    /// Got `r`, waiting for replacement character.
    Replace { count: usize },
    /// Got `>` or `<`, waiting for repeat (>> or <<).
    Indent { dir: char, count: usize },
}

// ============================================================================
// Persistent State
// ============================================================================

/// Persistent state that survives across commands.
/// This is the "memory" of vim - what gets recalled for repeats and pastes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistentState {
    pub last_change: Option<RecordedChange>,
    pub last_find: Option<LastFind>,
    pub register: String,
    pub register_is_linewise: bool,
}

impl Default for PersistentState {
    fn default() -> Self {
        Self {
            last_change: None,
            last_find: None,
            register: String::new(),
            register_is_linewise: false,
        }
    }
}

/// Stored last find for `;` and `,` repeat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastFind {
    pub find_type: FindType,
    pub ch: char,
}

/// Recorded change for dot-repeat.
/// Captures everything needed to replay a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordedChange {
    Insert {
        text: String,
    },
    Operator {
        op: Operator,
        motion: String,
        count: usize,
    },
    OperatorTextObj {
        op: Operator,
        obj_type: String,
        scope: TextObjScope,
        count: usize,
    },
    OperatorFind {
        op: Operator,
        find: FindType,
        ch: char,
        count: usize,
    },
    Replace {
        ch: char,
        count: usize,
    },
    X {
        count: usize,
    },
    ToggleCase {
        count: usize,
    },
    Indent {
        dir: char,
        count: usize,
    },
    OpenLine {
        direction: OpenLineDirection,
    },
    Join {
        count: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenLineDirection {
    Above,
    Below,
}

// ============================================================================
// Key Classification
// ============================================================================

/// Check if a character is an operator key (d, c, y).
pub fn is_operator_key(ch: char) -> bool {
    matches!(ch, 'd' | 'c' | 'y')
}

/// Check if a string is a simple motion key.
pub fn is_simple_motion(key: &str) -> bool {
    matches!(
        key,
        "h" | "l" | "j" | "k" | "w" | "b" | "e" | "W" | "B" | "E" | "0" | "^" | "$"
    )
}

/// Check if a character is a find key (f, F, t, T).
pub fn is_find_key(ch: char) -> bool {
    matches!(ch, 'f' | 'F' | 't' | 'T')
}

/// Check if a character is a text-object type.
pub fn is_text_obj_type(ch: char) -> bool {
    matches!(
        ch,
        'w' | 'W' | '"' | '\'' | '`' | '(' | ')' | 'b' | '[' | ']' | '{' | '}' | 'B' | '<'
            | '>'
    )
}

/// Maximum count value to prevent runaway.
pub const MAX_VIM_COUNT: usize = 10000;

// ============================================================================
// State Factories
// ============================================================================

/// Create the initial vim state (INSERT mode with empty inserted text).
pub fn create_initial_vim_state() -> VimState {
    VimState::Insert {
        inserted_text: String::new(),
    }
}

/// Create initial persistent state.
pub fn create_initial_persistent_state() -> PersistentState {
    PersistentState::default()
}
