//! The task definition: a design brief plus a rubric to score the result
//! against.
//!
//! Generic over `C`, the domain's own `Check` enum — this crate knows
//! nothing about mounting plates or fuzz pedals, only that a task has an
//! id, a family the backend should target, a free-text brief, and a rubric
//! of named criteria. Every consumer surveyed for this crate had this exact
//! shape already; only the `Check` variants differed.

use serde::{Deserialize, Serialize};

/// Benchmark-governance metadata used to stratify scores and keep task
/// provenance explicit. Defaults preserve deserialization of historical task
/// files; [`TaskMetadata::validate`] is the promotion gate for new suites.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskMetadata {
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub difficulty: Difficulty,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default = "unversioned_oracle")]
    pub oracle_version: String,
    #[serde(default)]
    pub split: DatasetSplit,
    #[serde(default)]
    pub expected_failure_modes: Vec<String>,
}

impl Default for TaskMetadata {
    fn default() -> Self {
        Self {
            capabilities: Vec::new(),
            difficulty: Difficulty::Unspecified,
            source: None,
            oracle_version: unversioned_oracle(),
            split: DatasetSplit::Development,
            expected_failure_modes: Vec::new(),
        }
    }
}

impl TaskMetadata {
    /// Refuse metadata that was merely filled by compatibility defaults.
    pub fn validate(&self) -> Result<(), String> {
        if self.capabilities.is_empty() || self.capabilities.iter().any(|v| v.trim().is_empty()) {
            return Err("metadata.capabilities must contain non-empty capability ids".into());
        }
        if self.difficulty == Difficulty::Unspecified {
            return Err("metadata.difficulty must be declared".into());
        }
        if self.source.as_deref().is_none_or(|v| v.trim().is_empty()) {
            return Err("metadata.source must be declared".into());
        }
        if self.oracle_version.trim().is_empty() || self.oracle_version == "unversioned" {
            return Err("metadata.oracle_version must be declared".into());
        }
        if self.expected_failure_modes.is_empty()
            || self
                .expected_failure_modes
                .iter()
                .any(|v| v.trim().is_empty())
        {
            return Err(
                "metadata.expected_failure_modes must contain non-empty failure modes".into(),
            );
        }
        Ok(())
    }
}

fn unversioned_oracle() -> String {
    "unversioned".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Difficulty {
    #[default]
    Unspecified,
    Introductory,
    Intermediate,
    Advanced,
    Adversarial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DatasetSplit {
    #[default]
    Development,
    Validation,
    Holdout,
}

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
    #[serde(default)]
    pub metadata: TaskMetadata,
    /// What the backend is given besides the brief -- input files, stated
    /// parameters -- as the task file's `[input]` table.
    ///
    /// Untyped here on purpose: what a task can hand a backend is the domain
    /// harness's business (an SVG and a height for a CAD part, a netlist for a
    /// board), so each harness parses this into its own type when it loads
    /// the task and refuses one it cannot read. Absent for a brief-only task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<toml::Table>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum Check {
        Anything,
    }

    #[test]
    fn a_tasks_input_table_is_kept_and_a_brief_only_task_has_none() {
        let with: Task<Check> = toml::from_str(
            r#"
            id = "t"
            family = "f"
            brief = "b"
            [input]
            svg = "part.svg"
            height_mm = 12.0
            [[rubric]]
            id = "r"
            description = "d"
            kind = "anything"
            "#,
        )
        .unwrap();
        let input = with.input.expect("the [input] table");
        assert_eq!(input["svg"].as_str(), Some("part.svg"));
        assert_eq!(input["height_mm"].as_float(), Some(12.0));
        assert!(matches!(with.rubric[0].check, Check::Anything));

        let without: Task<Check> =
            toml::from_str("id = \"t\"\nfamily = \"f\"\nbrief = \"b\"\nrubric = []\n").unwrap();
        assert!(without.input.is_none());
        assert_eq!(without.metadata, TaskMetadata::default());
        assert!(without.metadata.validate().is_err());
    }

    #[test]
    fn declared_metadata_is_valid_and_round_trips() {
        let task: Task<Check> = toml::from_str(
            r#"
            id = "t"
            family = "f"
            brief = "b"
            [metadata]
            capabilities = ["geometry.bounds"]
            difficulty = "intermediate"
            source = "synthetic:paired-constraint"
            oracle_version = "geometry-v1"
            split = "holdout"
            expected_failure_modes = ["wrong-width"]
            [[rubric]]
            id = "r"
            description = "d"
            kind = "anything"
            "#,
        )
        .unwrap();
        assert!(task.metadata.validate().is_ok());
        assert_eq!(task.metadata.split, DatasetSplit::Holdout);
        assert_eq!(task.metadata.difficulty, Difficulty::Intermediate);
    }
}
