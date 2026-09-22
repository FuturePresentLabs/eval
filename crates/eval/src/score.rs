//! A scored run: what each rubric criterion resolved to, once checked
//! against captured artifacts.
//!
//! Not generic over the domain's `Check` type — once a criterion is
//! scored, all that's left is a [`Verdict`] and a human-readable `detail`
//! string; the domain-specific work of turning a `Check` plus a backend's
//! artifacts into a `Verdict` stays in each consuming crate's own scorer,
//! where the artifact types actually live.

use serde::{Deserialize, Serialize};

/// One criterion's outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    Pass,
    Fail,
    /// Not automated — a human fills this in. Never counts as a failure
    /// ([`ScoreReport::all_automated_pass`] ignores it); never counts as a
    /// silent pass either — [`ScoreReport::needs_human`] surfaces it.
    NeedsHuman,
}

/// One rubric line item, scored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriterionResult {
    pub id: String,
    pub description: String,
    pub verdict: Verdict,
    /// Why, in the scorer's own words — never blank on a [`Verdict::Fail`],
    /// so a failing run is diagnosable from the result file alone.
    pub detail: String,
}

/// A full scored run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreReport {
    pub task_id: String,
    /// Short identifier of the backend that produced the run (e.g.
    /// `"transmog"`, `"legion-of-bom"`).
    pub backend: String,
    pub results: Vec<CriterionResult>,
}

impl ScoreReport {
    /// Every automated criterion passed. [`Verdict::NeedsHuman`] does not
    /// count against this — an unresolved subjective check is not a
    /// failure, it is exactly what it says: unresolved.
    #[must_use]
    pub fn all_automated_pass(&self) -> bool {
        self.results
            .iter()
            .all(|r| !matches!(r.verdict, Verdict::Fail))
    }

    /// Criteria still needing a human, by id.
    #[must_use]
    pub fn needs_human(&self) -> Vec<&str> {
        self.results
            .iter()
            .filter(|r| matches!(r.verdict, Verdict::NeedsHuman))
            .map(|r| r.id.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(verdict: Verdict) -> CriterionResult {
        CriterionResult {
            id: "c".into(),
            description: "d".into(),
            verdict,
            detail: "detail".into(),
        }
    }

    #[test]
    fn needs_human_never_fails_the_run() {
        let report = ScoreReport {
            task_id: "t".into(),
            backend: "b".into(),
            results: vec![result(Verdict::Pass), result(Verdict::NeedsHuman)],
        };
        assert!(report.all_automated_pass());
        assert_eq!(report.needs_human(), vec!["c"]);
    }

    #[test]
    fn a_real_fail_shows_up_in_all_automated_pass() {
        let report = ScoreReport {
            task_id: "t".into(),
            backend: "b".into(),
            results: vec![result(Verdict::Pass), result(Verdict::Fail)],
        };
        assert!(!report.all_automated_pass());
    }
}
