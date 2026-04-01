//! Bootstrap (global singleton) state.
//!
//! Ported from ref/bootstrap/state.ts -- this is the process-wide state
//! that lives for the entire lifetime of the ThunderCode process.  It tracks
//! execution context, cost/token accounting, session lineage, and auth.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::types::ids::SessionId;

// ---------------------------------------------------------------------------
// ModelUsage
// ---------------------------------------------------------------------------

/// Token and cost accounting for a single model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub cost_usd: f64,
}

impl Default for ModelUsage {
    fn default() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            cost_usd: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// BootstrapState
// ---------------------------------------------------------------------------

/// Global process-scoped state, created once at startup.
///
/// This is **not** held in a `Store` -- it uses interior mutability via
/// `Arc<RwLock<..>>` directly, matching the TypeScript module-level `STATE`
/// singleton pattern from `ref/bootstrap/state.ts`.
#[derive(Debug, Clone)]
pub struct BootstrapState {
    inner: Arc<RwLock<BootstrapStateInner>>,
}

/// The actual data behind the `Arc<RwLock<..>>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BootstrapStateInner {
    // ---- Execution context -----------------------------------------------
    /// Current working directory (resolved, symlinks followed).
    pub cwd: PathBuf,
    /// Stable project root -- set once at startup, never updated mid-session.
    pub project_root: Option<PathBuf>,
    /// Whether the session is running interactively (TTY attached).
    pub is_interactive: bool,

    // ---- Cost tracking ---------------------------------------------------
    pub total_cost_usd: f64,
    pub total_duration_ms: u64,
    /// Per-model token + cost accounting.
    pub model_usage: HashMap<String, ModelUsage>,

    // ---- Session metadata ------------------------------------------------
    pub session_id: SessionId,
    pub parent_session_id: Option<SessionId>,

    // ---- Auth ------------------------------------------------------------
    pub auth_token: Option<String>,

    // ---- Feature state ---------------------------------------------------
    /// Palette of colours assigned to agents in display order.
    pub agent_colors: Vec<String>,
    /// Composite keys of invoked skills (`agentId:skillName`).
    pub invoked_skills: HashSet<String>,
}

impl BootstrapState {
    /// Create a new bootstrap state rooted at `cwd`.
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            inner: Arc::new(RwLock::new(BootstrapStateInner {
                cwd: cwd.clone(),
                project_root: Some(cwd),
                is_interactive: false,
                total_cost_usd: 0.0,
                total_duration_ms: 0,
                model_usage: HashMap::new(),
                session_id: SessionId::new(),
                parent_session_id: None,
                auth_token: None,
                agent_colors: Vec::new(),
                invoked_skills: HashSet::new(),
            })),
        }
    }

    // ---- Getters ---------------------------------------------------------

    /// Return a snapshot clone of the inner state.
    pub fn snapshot(&self) -> BootstrapStateInner {
        self.inner.read().expect("bootstrap lock poisoned").clone()
    }

    pub fn session_id(&self) -> SessionId {
        self.inner
            .read()
            .expect("bootstrap lock poisoned")
            .session_id
            .clone()
    }

    pub fn cwd(&self) -> PathBuf {
        self.inner
            .read()
            .expect("bootstrap lock poisoned")
            .cwd
            .clone()
    }

    pub fn project_root(&self) -> Option<PathBuf> {
        self.inner
            .read()
            .expect("bootstrap lock poisoned")
            .project_root
            .clone()
    }

    pub fn is_interactive(&self) -> bool {
        self.inner
            .read()
            .expect("bootstrap lock poisoned")
            .is_interactive
    }

    pub fn total_cost_usd(&self) -> f64 {
        self.inner
            .read()
            .expect("bootstrap lock poisoned")
            .total_cost_usd
    }

    pub fn auth_token(&self) -> Option<String> {
        self.inner
            .read()
            .expect("bootstrap lock poisoned")
            .auth_token
            .clone()
    }

    // ---- Mutators --------------------------------------------------------

    /// Update the inner state with a closure.
    pub fn update(&self, f: impl FnOnce(&mut BootstrapStateInner)) {
        let mut inner = self.inner.write().expect("bootstrap lock poisoned");
        f(&mut inner);
    }

    /// Set the current working directory.
    pub fn set_cwd(&self, cwd: PathBuf) {
        self.update(|s| s.cwd = cwd);
    }

    /// Set the interactive flag.
    pub fn set_interactive(&self, interactive: bool) {
        self.update(|s| s.is_interactive = interactive);
    }

    /// Set the auth token.
    pub fn set_auth_token(&self, token: Option<String>) {
        self.update(|s| s.auth_token = token);
    }

    /// Record API cost and duration.
    pub fn add_api_cost(&self, cost_usd: f64, duration_ms: u64) {
        self.update(|s| {
            s.total_cost_usd += cost_usd;
            s.total_duration_ms += duration_ms;
        });
    }

    /// Record token usage for a specific model.
    pub fn record_model_usage(
        &self,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: u64,
        cache_write_tokens: u64,
        cost_usd: f64,
    ) {
        self.update(|s| {
            let usage = s
                .model_usage
                .entry(model.to_owned())
                .or_insert_with(ModelUsage::default);
            usage.input_tokens += input_tokens;
            usage.output_tokens += output_tokens;
            usage.cache_read_tokens += cache_read_tokens;
            usage.cache_write_tokens += cache_write_tokens;
            usage.cost_usd += cost_usd;
            s.total_cost_usd += cost_usd;
        });
    }

    /// Regenerate the session ID, optionally setting the old one as parent.
    pub fn regenerate_session_id(&self, set_current_as_parent: bool) -> SessionId {
        let mut inner = self.inner.write().expect("bootstrap lock poisoned");
        if set_current_as_parent {
            inner.parent_session_id = Some(inner.session_id.clone());
        }
        inner.session_id = SessionId::new();
        inner.session_id.clone()
    }

    /// Record that a skill was invoked.
    pub fn record_invoked_skill(&self, key: String) {
        self.update(|s| {
            s.invoked_skills.insert(key);
        });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sets_cwd_and_project_root() {
        let cwd = PathBuf::from("/tmp/test");
        let bs = BootstrapState::new(cwd.clone());
        assert_eq!(bs.cwd(), cwd);
        assert_eq!(bs.project_root(), Some(cwd));
    }

    #[test]
    fn session_id_is_unique() {
        let bs = BootstrapState::new(PathBuf::from("/tmp"));
        let id1 = bs.session_id();
        let id2 = bs.regenerate_session_id(false);
        assert_ne!(id1.as_str(), id2.as_str());
    }

    #[test]
    fn regenerate_preserves_parent() {
        let bs = BootstrapState::new(PathBuf::from("/tmp"));
        let original = bs.session_id();
        bs.regenerate_session_id(true);
        let snap = bs.snapshot();
        assert_eq!(snap.parent_session_id.as_ref().map(|s| s.as_str()), Some(original.as_str()));
    }

    #[test]
    fn record_model_usage_accumulates() {
        let bs = BootstrapState::new(PathBuf::from("/tmp"));
        bs.record_model_usage("primary-4", 100, 50, 10, 5, 0.01);
        bs.record_model_usage("primary-4", 200, 100, 20, 10, 0.02);

        let snap = bs.snapshot();
        let usage = snap.model_usage.get("primary-4").unwrap();
        assert_eq!(usage.input_tokens, 300);
        assert_eq!(usage.output_tokens, 150);
        assert_eq!(usage.cache_read_tokens, 30);
        assert_eq!(usage.cache_write_tokens, 15);
        assert!((usage.cost_usd - 0.03).abs() < 1e-10);
        assert!((snap.total_cost_usd - 0.03).abs() < 1e-10);
    }

    #[test]
    fn add_api_cost_accumulates() {
        let bs = BootstrapState::new(PathBuf::from("/tmp"));
        bs.add_api_cost(0.05, 100);
        bs.add_api_cost(0.10, 200);
        let snap = bs.snapshot();
        assert!((snap.total_cost_usd - 0.15).abs() < 1e-10);
        assert_eq!(snap.total_duration_ms, 300);
    }

    #[test]
    fn set_interactive() {
        let bs = BootstrapState::new(PathBuf::from("/tmp"));
        assert!(!bs.is_interactive());
        bs.set_interactive(true);
        assert!(bs.is_interactive());
    }

    #[test]
    fn clone_shares_inner() {
        let bs = BootstrapState::new(PathBuf::from("/tmp"));
        let bs2 = bs.clone();
        bs.set_interactive(true);
        assert!(bs2.is_interactive());
    }

    #[test]
    fn invoked_skills_tracking() {
        let bs = BootstrapState::new(PathBuf::from("/tmp"));
        bs.record_invoked_skill(":commit".into());
        bs.record_invoked_skill("agent1:review".into());
        let snap = bs.snapshot();
        assert!(snap.invoked_skills.contains(":commit"));
        assert!(snap.invoked_skills.contains("agent1:review"));
        assert_eq!(snap.invoked_skills.len(), 2);
    }
}
