//! The task definition: a design brief plus a rubric to score the result
//! against.
//!
//! Generic over `C`, the domain's own `Check` enum — this crate knows
//! nothing about mounting plates or fuzz pedals, only that a task has an
//! id, a family the backend should target, a free-text brief, and a rubric
//! of named criteria. Every consumer surveyed for this crate had this exact
//! shape already; only the `Check` variants differed.

use serde::{Deserialize, Serialize};

/// One eval task: what to design/build, and what "good" means.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task<C> {
    /// Task id, e.g. `"mounting-plate-v1"`.
    pub id: String,
    /// The backend-specific family/topology this task targets, e.g.
    /// `"mounting-plate"`, `"fuzz-pedal"`.
    pub family: String,
    /// Free-text design brief, carried through to the backend's decision
    /// layer as context. Never parsed for control flow here or in the
    /// backend — the typed decisions, not the prose, choose the design.
    pub brief: String,
    pub rubric: Vec<Criterion<C>>,
}

/// One rubric line item: a name, a human-readable description, and what it
/// actually checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Criterion<C> {
    pub id: String,
    pub description: String,
    #[serde(flatten)]
    pub check: C,
}

impl<C> Task<C> {
    /// Rubric ids, in declaration order. Every prior implementation wanted
    /// this for a duplicate-id check; kept here so that check is one line
    /// at every call site instead of a `.iter().map(...)` re-derived each
    /// time.
    #[must_use]
    pub fn rubric_ids(&self) -> Vec<&str> {
        self.rubric.iter().map(|c| c.id.as_str()).collect()
    }
}
