//! Portable, attributed benchmark results for files, APIs, viewers, and CI.

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ScoreReport;

pub const RESULT_BUNDLE_SCHEMA: &str = "eval.result-bundle.v1";
pub const RESULT_CATALOG_SCHEMA: &str = "eval.result-catalog.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultBundle {
    pub schema: String,
    pub id: String,
    pub generated_at: String,
    pub benchmark: BenchmarkIdentity,
    pub subject: SubjectIdentity,
    pub provenance: Provenance,
    pub metrics: Vec<MetricDefinition>,
    pub observations: Vec<MetricObservation>,
    pub runs: Vec<RunRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkIdentity {
    pub id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectIdentity {
    pub kind: String,
    pub provider: String,
    pub name: String,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub harness_commit: String,
    pub task_set_commit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scorer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDefinition {
    pub id: String,
    pub label: String,
    pub unit: String,
    pub goal: MetricGoal,
    pub aggregation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricGoal {
    Minimize,
    Maximize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricObservation {
    pub metric_id: String,
    pub value: f64,
    pub sample_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lower_bound: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upper_bound: Option<f64>,
    pub run_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: String,
    pub task_id: String,
    pub trial: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<ScoreReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ArtifactRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRef {
    pub name: String,
    pub media_type: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultCatalog {
    pub schema: String,
    pub bundles: Vec<CatalogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub url: String,
    pub benchmark_id: String,
    pub subject: SubjectIdentity,
    pub generated_at: String,
}

#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("result bundle must use schema {RESULT_BUNDLE_SCHEMA}")]
    WrongSchema,
    #[error("duplicate {kind} id: {id}")]
    DuplicateId { kind: &'static str, id: String },
    #[error("observation references unknown metric: {0}")]
    UnknownMetric(String),
    #[error("observation references unknown run: {0}")]
    UnknownRun(String),
    #[error("metric observation must reference at least one sample and run")]
    EmptyObservation,
    #[error("confidence interval is invalid for metric {0}")]
    InvalidBounds(String),
    #[error("reading result bundle {path}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("parsing result bundle")]
    Parse(#[from] serde_json::Error),
    #[error("writing result bundle {path}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

impl ResultBundle {
    pub fn validate(&self) -> Result<(), BundleError> {
        if self.schema != RESULT_BUNDLE_SCHEMA {
            return Err(BundleError::WrongSchema);
        }
        let mut metric_ids = HashSet::new();
        for metric in &self.metrics {
            if !metric_ids.insert(metric.id.as_str()) {
                return Err(BundleError::DuplicateId {
                    kind: "metric",
                    id: metric.id.clone(),
                });
            }
        }
        let mut run_ids = HashSet::new();
        for run in &self.runs {
            if !run_ids.insert(run.id.as_str()) {
                return Err(BundleError::DuplicateId {
                    kind: "run",
                    id: run.id.clone(),
                });
            }
        }
        for observation in &self.observations {
            if !metric_ids.contains(observation.metric_id.as_str()) {
                return Err(BundleError::UnknownMetric(observation.metric_id.clone()));
            }
            if observation.sample_count == 0 || observation.run_ids.is_empty() {
                return Err(BundleError::EmptyObservation);
            }
            if observation
                .run_ids
                .iter()
                .any(|id| !run_ids.contains(id.as_str()))
            {
                let id = observation
                    .run_ids
                    .iter()
                    .find(|id| !run_ids.contains(id.as_str()))
                    .unwrap();
                return Err(BundleError::UnknownRun(id.clone()));
            }
            if matches!((observation.lower_bound, observation.upper_bound), (Some(low), Some(high)) if low > observation.value || high < observation.value || low > high)
            {
                return Err(BundleError::InvalidBounds(observation.metric_id.clone()));
            }
        }
        Ok(())
    }

    pub fn from_json_file(path: &Path) -> Result<Self, BundleError> {
        let bytes = std::fs::read(path).map_err(|source| BundleError::Read {
            path: path.display().to_string(),
            source,
        })?;
        let bundle: Self = serde_json::from_slice(&bytes)?;
        bundle.validate()?;
        Ok(bundle)
    }

    pub fn write_json_file(&self, path: &Path) -> Result<(), BundleError> {
        self.validate()?;
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        std::fs::write(path, bytes).map_err(|source| BundleError::Write {
            path: path.display().to_string(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle() -> ResultBundle {
        ResultBundle {
            schema: RESULT_BUNDLE_SCHEMA.into(),
            id: "run-1".into(),
            generated_at: "2026-09-24T00:00:00Z".into(),
            benchmark: BenchmarkIdentity {
                id: "cadbench".into(),
                version: "1".into(),
                title: None,
            },
            subject: SubjectIdentity {
                kind: "model".into(),
                provider: "example".into(),
                name: "model-a".into(),
                revision: "2026-09".into(),
            },
            provenance: Provenance {
                harness_commit: "abc".into(),
                task_set_commit: "def".into(),
                scorer: Some("objective-v1".into()),
                source_url: None,
            },
            metrics: vec![MetricDefinition {
                id: "pass_rate".into(),
                label: "Pass rate".into(),
                unit: "percent".into(),
                goal: MetricGoal::Maximize,
                aggregation: "mean".into(),
            }],
            observations: vec![MetricObservation {
                metric_id: "pass_rate".into(),
                value: 0.75,
                sample_count: 1,
                lower_bound: None,
                upper_bound: None,
                run_ids: vec!["r1".into()],
                task_ids: vec!["task-1".into()],
            }],
            runs: vec![RunRecord {
                id: "r1".into(),
                task_id: "task-1".into(),
                trial: 1,
                started_at: None,
                duration_ms: Some(100),
                cost_usd: None,
                score: None,
                artifacts: vec![],
            }],
        }
    }

    #[test]
    fn validates_attributed_observations() {
        bundle().validate().unwrap();
    }

    #[test]
    fn rejects_unknown_run_attribution() {
        let mut value = bundle();
        value.observations[0].run_ids[0] = "missing".into();
        assert!(matches!(value.validate(), Err(BundleError::UnknownRun(_))));
    }

    #[test]
    fn round_trips_json() {
        let value = bundle();
        let encoded = serde_json::to_string(&value).unwrap();
        let decoded: ResultBundle = serde_json::from_str(&encoded).unwrap();
        decoded.validate().unwrap();
    }
}
