//! DreamTask -- auto-dream memory consolidation subagent.
//!
//! Ported from ref/tasks/DreamTask/DreamTask.ts.
//!
//! The dream task runs a background agent that consolidates session memories.
//! It is visible in the UI footer pill and task dialog but does not produce
//! model-facing notifications (it's UI-only).  Completion / failure is
//! surfaced through an inline system message.

use crate::types::task::{TaskStatus, TaskType};

// ---------------------------------------------------------------------------
// DreamPhase
// ---------------------------------------------------------------------------

/// The current phase of a dream task.
///
/// We don't parse the dream prompt's 4-stage structure (orient/gather/
/// consolidate/prune).  Instead we flip from `Starting` to `Updating` when
/// the first Edit/Write tool_use lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DreamPhase {
    Starting,
    Updating,
}

// ---------------------------------------------------------------------------
// DreamTurn
// ---------------------------------------------------------------------------

/// A single assistant turn from the dream agent, tool uses collapsed to a count.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DreamTurn {
    pub text: String,
    pub tool_use_count: usize,
}

// ---------------------------------------------------------------------------
// DreamTask
// ---------------------------------------------------------------------------

/// Maximum number of recent turns kept for live display.
const MAX_TURNS: usize = 30;

/// Handle for a dream (memory consolidation) task.
pub struct DreamTask {
    task_id: String,
    status: TaskStatus,
    phase: DreamPhase,
    sessions_reviewing: usize,
    files_touched: Vec<String>,
    turns: Vec<DreamTurn>,
}

impl DreamTask {
    pub fn new(task_id: String, sessions_reviewing: usize) -> Self {
        Self {
            task_id,
            status: TaskStatus::Running,
            phase: DreamPhase::Starting,
            sessions_reviewing,
            files_touched: Vec::new(),
            turns: Vec::new(),
        }
    }

    /// Add an assistant turn.  If the turn touches new files, transitions
    /// the phase to `Updating`.
    pub fn add_turn(&mut self, turn: DreamTurn, touched_paths: Vec<String>) {
        // De-duplicate touched paths.
        let mut seen: std::collections::HashSet<String> =
            self.files_touched.iter().cloned().collect();
        let new_touched: Vec<String> = touched_paths
            .into_iter()
            .filter(|p| seen.insert(p.clone()))
            .collect();

        // Skip no-op turns.
        if turn.text.is_empty() && turn.tool_use_count == 0 && new_touched.is_empty() {
            return;
        }

        if !new_touched.is_empty() {
            self.phase = DreamPhase::Updating;
            self.files_touched.extend(new_touched);
        }

        // Keep only the last MAX_TURNS entries.
        if self.turns.len() >= MAX_TURNS {
            let drain_count = self.turns.len() - (MAX_TURNS - 1);
            self.turns.drain(..drain_count);
        }
        self.turns.push(turn);
    }

    pub fn complete(&mut self) {
        self.status = TaskStatus::Completed;
    }

    pub fn fail(&mut self) {
        self.status = TaskStatus::Failed;
    }

    /// Kill the dream task.
    pub async fn kill(&mut self) {
        // TODO: abort the dream agent's query loop.
        self.status = TaskStatus::Killed;
    }

    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    pub fn status(&self) -> TaskStatus {
        self.status
    }

    pub fn phase(&self) -> DreamPhase {
        self.phase
    }

    pub fn sessions_reviewing(&self) -> usize {
        self.sessions_reviewing
    }

    pub fn files_touched(&self) -> &[String] {
        &self.files_touched
    }

    pub fn turns(&self) -> &[DreamTurn] {
        &self.turns
    }

    pub fn task_type() -> TaskType {
        TaskType::Dream
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_turn_updates_phase() {
        let mut task = DreamTask::new("d12345678".into(), 3);
        assert_eq!(task.phase(), DreamPhase::Starting);

        task.add_turn(
            DreamTurn {
                text: "editing memory".into(),
                tool_use_count: 1,
            },
            vec!["RULES.md".into()],
        );

        assert_eq!(task.phase(), DreamPhase::Updating);
        assert_eq!(task.files_touched(), &["RULES.md"]);
        assert_eq!(task.turns().len(), 1);
    }

    #[test]
    fn skips_empty_no_op_turn() {
        let mut task = DreamTask::new("d12345678".into(), 1);
        task.add_turn(
            DreamTurn {
                text: String::new(),
                tool_use_count: 0,
            },
            vec![],
        );
        assert!(task.turns().is_empty());
    }

    #[test]
    fn caps_turns_at_max() {
        let mut task = DreamTask::new("d12345678".into(), 1);
        for i in 0..40 {
            task.add_turn(
                DreamTurn {
                    text: format!("turn {i}"),
                    tool_use_count: 0,
                },
                vec![],
            );
        }
        assert_eq!(task.turns().len(), MAX_TURNS);
    }
}
