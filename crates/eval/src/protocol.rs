//! Reproducible paired benchmark protocol and fail-closed matrix validation.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// Conditions that must be held constant across compared subjects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunProtocol {
    pub id: String,
    pub subjects: Vec<String>,
    pub task_ids: Vec<String>,
    pub trials: u32,
    pub budget: RunBudget,
    pub sampling: SamplingParameters,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunBudget {
    #[serde(default)]
    pub max_input_tokens: Option<u64>,
    #[serde(default)]
    pub max_output_tokens: Option<u64>,
    #[serde(default)]
    pub max_tool_calls: Option<u64>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SamplingParameters {
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    /// One seed per trial. A provider that cannot accept seeds must still
    /// record deterministic trial identities in observations.
    pub seeds: Vec<u64>,
}

/// Identity/provenance for one cell of the subject × task × trial matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrialObservation {
    pub subject: String,
    pub task_id: String,
    pub trial: u32,
    /// Proves every subject started the paired task from the same snapshot.
    pub reset_identity: String,
    /// Stable path or content digest for the corresponding scored run.
    pub result_ref: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("protocol id, subjects, and task ids must be non-empty")]
    EmptyIdentity,
    #[error("trials must be greater than zero")]
    NoTrials,
    #[error("sampling.seeds must contain exactly one seed per trial")]
    SeedCount,
    #[error("run budget must declare at least one finite limit")]
    EmptyBudget,
    #[error("duplicate matrix cell for subject={subject}, task={task}, trial={trial}")]
    DuplicateCell {
        subject: String,
        task: String,
        trial: u32,
    },
    #[error("unexpected matrix cell for subject={subject}, task={task}, trial={trial}")]
    UnexpectedCell {
        subject: String,
        task: String,
        trial: u32,
    },
    #[error("missing {0} paired matrix cell(s)")]
    MissingCells(usize),
    #[error("paired subjects disagree on reset identity for task={task}, trial={trial}")]
    ResetMismatch { task: String, trial: u32 },
    #[error("observation identity and result_ref must be non-empty")]
    EmptyObservation,
}

impl RunProtocol {
    /// Validate protocol shape and the complete Cartesian result matrix.
    pub fn validate(&self, observations: &[TrialObservation]) -> Result<(), ProtocolError> {
        if self.id.trim().is_empty()
            || self.subjects.is_empty()
            || self.task_ids.is_empty()
            || self.subjects.iter().any(|v| v.trim().is_empty())
            || self.task_ids.iter().any(|v| v.trim().is_empty())
        {
            return Err(ProtocolError::EmptyIdentity);
        }
        if self.trials == 0 {
            return Err(ProtocolError::NoTrials);
        }
        if self.sampling.seeds.len() != self.trials as usize {
            return Err(ProtocolError::SeedCount);
        }
        if self.budget.max_input_tokens.is_none()
            && self.budget.max_output_tokens.is_none()
            && self.budget.max_tool_calls.is_none()
            && self.budget.timeout_ms.is_none()
        {
            return Err(ProtocolError::EmptyBudget);
        }

        let subjects: BTreeSet<_> = self.subjects.iter().cloned().collect();
        let tasks: BTreeSet<_> = self.task_ids.iter().cloned().collect();
        if subjects.len() != self.subjects.len() || tasks.len() != self.task_ids.len() {
            return Err(ProtocolError::EmptyIdentity);
        }
        let mut cells = BTreeSet::new();
        let mut resets: BTreeMap<(String, u32), String> = BTreeMap::new();
        for observation in observations {
            if observation.reset_identity.trim().is_empty()
                || observation.result_ref.trim().is_empty()
            {
                return Err(ProtocolError::EmptyObservation);
            }
            if !subjects.contains(&observation.subject)
                || !tasks.contains(&observation.task_id)
                || observation.trial >= self.trials
            {
                return Err(ProtocolError::UnexpectedCell {
                    subject: observation.subject.clone(),
                    task: observation.task_id.clone(),
                    trial: observation.trial,
                });
            }
            let cell = (
                observation.subject.clone(),
                observation.task_id.clone(),
                observation.trial,
            );
            if !cells.insert(cell) {
                return Err(ProtocolError::DuplicateCell {
                    subject: observation.subject.clone(),
                    task: observation.task_id.clone(),
                    trial: observation.trial,
                });
            }
            let pair = (observation.task_id.clone(), observation.trial);
            match resets.get(&pair) {
                Some(expected) if expected != &observation.reset_identity => {
                    return Err(ProtocolError::ResetMismatch {
                        task: observation.task_id.clone(),
                        trial: observation.trial,
                    });
                }
                None => {
                    resets.insert(pair, observation.reset_identity.clone());
                }
                _ => {}
            }
        }
        let expected = subjects.len() * tasks.len() * self.trials as usize;
        if cells.len() != expected {
            return Err(ProtocolError::MissingCells(expected - cells.len()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn protocol() -> RunProtocol {
        RunProtocol {
            id: "paired-v1".into(),
            subjects: vec!["a".into(), "b".into()],
            task_ids: vec!["task".into()],
            trials: 2,
            budget: RunBudget {
                max_input_tokens: None,
                max_output_tokens: Some(2_000),
                max_tool_calls: Some(20),
                timeout_ms: Some(60_000),
            },
            sampling: SamplingParameters {
                temperature: Some(0.2),
                top_p: None,
                seeds: vec![10, 11],
            },
        }
    }

    fn complete() -> Vec<TrialObservation> {
        ["a", "b"]
            .into_iter()
            .flat_map(|subject| {
                (0..2).map(move |trial| TrialObservation {
                    subject: subject.into(),
                    task_id: "task".into(),
                    trial,
                    reset_identity: format!("snapshot-{trial}"),
                    result_ref: format!("{subject}-{trial}.json"),
                })
            })
            .collect()
    }

    #[test]
    fn complete_paired_matrix_passes() {
        assert_eq!(protocol().validate(&complete()), Ok(()));
    }

    #[test]
    fn missing_cell_fails_closed() {
        let mut observations = complete();
        observations.pop();
        assert_eq!(
            protocol().validate(&observations),
            Err(ProtocolError::MissingCells(1))
        );
    }

    #[test]
    fn mismatched_reset_identity_is_rejected() {
        let mut observations = complete();
        observations[2].reset_identity = "different".into();
        assert!(matches!(
            protocol().validate(&observations),
            Err(ProtocolError::ResetMismatch { .. })
        ));
    }
}
