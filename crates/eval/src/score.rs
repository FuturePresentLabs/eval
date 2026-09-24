//! A scored run: what each rubric criterion resolved to, once checked
//! against captured artifacts.
//!
//! Not generic over the domain's `Check` type — once a criterion is
//! scored, all that's left is a [`Verdict`] and a human-readable `detail`
//! string; the domain-specific work of turning a `Check` plus a backend's
//! artifacts into a `Verdict` stays in each consuming crate's own scorer,
//! where the artifact types actually live.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::TaskMetadata;

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

/// Non-overlapping score components. Keeping unresolved review separate stops
/// an absent human rating from inflating objective correctness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoreComposition {
    pub objective_pass: usize,
    pub objective_fail: usize,
    pub needs_human: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityScore {
    pub capability: String,
    pub scored_tasks: usize,
    pub passed_tasks: usize,
    pub pass_rate: f64,
}

/// Compute one equally weighted task score per capability. Tasks with no
/// automated criteria are excluded rather than converting human review into a
/// pass. Consumers can average the returned rates for a capability-macro score.
#[must_use]
pub fn scores_by_capability<'a>(
    tasks: impl IntoIterator<Item = (&'a TaskMetadata, &'a ScoreReport)>,
) -> Vec<CapabilityScore> {
    let mut totals: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for (metadata, report) in tasks {
        let composition = report.composition();
        if composition.objective_pass + composition.objective_fail == 0 {
            continue;
        }
        let passed = usize::from(composition.objective_fail == 0);
        for capability in &metadata.capabilities {
            let entry = totals.entry(capability).or_default();
            entry.0 += 1;
            entry.1 += passed;
        }
    }
    totals
        .into_iter()
        .map(
            |(capability, (scored_tasks, passed_tasks))| CapabilityScore {
                capability: capability.into(),
                scored_tasks,
                passed_tasks,
                pass_rate: passed_tasks as f64 / scored_tasks as f64,
            },
        )
        .collect()
}

impl ScoreComposition {
    #[must_use]
    pub fn objective_rate(self) -> Option<f64> {
        let total = self.objective_pass + self.objective_fail;
        (total > 0).then(|| self.objective_pass as f64 / total as f64)
    }
}

impl ScoreReport {
    #[must_use]
    pub fn composition(&self) -> ScoreComposition {
        self.results.iter().fold(
            ScoreComposition {
                objective_pass: 0,
                objective_fail: 0,
                needs_human: 0,
            },
            |mut counts, result| {
                match result.verdict {
                    Verdict::Pass => counts.objective_pass += 1,
                    Verdict::Fail => counts.objective_fail += 1,
                    Verdict::NeedsHuman => counts.needs_human += 1,
                }
                counts
            },
        )
    }

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
        let composition = report.composition();
        assert_eq!(composition.objective_pass, 1);
        assert_eq!(composition.objective_fail, 1);
        assert_eq!(composition.objective_rate(), Some(0.5));
    }

    #[test]
    fn capability_scores_are_task_weighted_and_exclude_human_only_tasks() {
        let metadata = TaskMetadata {
            capabilities: vec!["geometry.bounds".into()],
            ..TaskMetadata::default()
        };
        let pass = ScoreReport {
            task_id: "pass".into(),
            backend: "b".into(),
            results: vec![result(Verdict::Pass), result(Verdict::Pass)],
        };
        let fail = ScoreReport {
            task_id: "fail".into(),
            backend: "b".into(),
            results: vec![result(Verdict::Fail)],
        };
        let human = ScoreReport {
            task_id: "human".into(),
            backend: "b".into(),
            results: vec![result(Verdict::NeedsHuman)],
        };
        let scores =
            scores_by_capability([(&metadata, &pass), (&metadata, &fail), (&metadata, &human)]);
        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0].scored_tasks, 2);
        assert_eq!(scores[0].pass_rate, 0.5);
    }
}
