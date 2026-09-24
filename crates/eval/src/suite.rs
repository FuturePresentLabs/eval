//! Backend-neutral execution of every promoted task in an eval suite.
//!
//! A domain harness supplies the one-task operation (load, backend run, score).
//! This module owns the repeated mechanics: deterministic discovery, isolated
//! work directories, continue-on-failure behavior, aggregate counts, and the
//! versioned suite report. CAD, PCB, and future harnesses therefore cannot
//! quietly acquire different definitions of "run all".

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{ScoreReport, TaskMetadata, Verdict};

/// Stable schema written for every suite execution.
pub const SUITE_REPORT_SCHEMA: &str = "eval.suite-report.v1";

/// A complete suite result, including tasks that could not be scored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteReport {
    pub schema: String,
    pub tasks_dir: PathBuf,
    pub results_dir: PathBuf,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub errors: usize,
    pub needs_human: usize,
    pub tasks: Vec<SuiteTaskResult>,
}

impl SuiteReport {
    /// True only when every discovered task produced a report and every
    /// automated criterion passed.
    #[must_use]
    pub fn all_automated_pass(&self) -> bool {
        self.total > 0 && self.failed == 0 && self.errors == 0
    }
}

/// One task's result. Backend/harness errors are data so later tasks still run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteTaskResult {
    pub task_file: PathBuf,
    pub work_dir: PathBuf,
    /// Governance metadata copied into the portable result artifact. The
    /// viewer must not depend on the original task checkout still existing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_metadata: Option<TaskMetadata>,
    /// Present when a task declared metadata that could not be decoded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata_error: Option<String>,
    #[serde(default)]
    pub elapsed_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<ScoreReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Failure in suite orchestration itself, rather than in an evaluated task.
#[derive(Debug, thiserror::Error)]
pub enum SuiteError {
    #[error("reading task directory {path}")]
    ReadTasks {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("creating results directory {path}")]
    CreateResults {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("serializing suite report")]
    Serialize(#[from] serde_json::Error),
    #[error("writing suite report {path}")]
    WriteReport {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("no promoted .toml tasks found directly under {0}")]
    NoTasks(PathBuf),
    #[error("task path has no UTF-8 file stem: {0}")]
    InvalidTaskPath(PathBuf),
}

/// Discovers and runs every `.toml` directly under `tasks_dir`, sorted by path.
///
/// Nested `planned/` directories are deliberately excluded: promotion is the
/// act of moving a task into the suite root. `run_one` owns all domain behavior
/// and may return any displayable error; such errors are recorded and execution
/// continues. The aggregate is written to `results_dir/suite-report.json`.
pub fn run_all<E, F>(
    tasks_dir: &Path,
    results_dir: &Path,
    mut run_one: F,
) -> Result<SuiteReport, SuiteError>
where
    E: std::fmt::Display,
    F: FnMut(&Path, &Path) -> Result<ScoreReport, E>,
{
    let tasks_dir = std::fs::canonicalize(tasks_dir).map_err(|source| SuiteError::ReadTasks {
        path: tasks_dir.to_path_buf(),
        source,
    })?;
    let entries = std::fs::read_dir(&tasks_dir).map_err(|source| SuiteError::ReadTasks {
        path: tasks_dir.clone(),
        source,
    })?;
    let mut task_files = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|source| SuiteError::ReadTasks {
                path: tasks_dir.clone(),
                source,
            })?
            .path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "toml") {
            task_files.push(path);
        }
    }
    task_files.sort();
    if task_files.is_empty() {
        return Err(SuiteError::NoTasks(tasks_dir));
    }
    std::fs::create_dir_all(results_dir).map_err(|source| SuiteError::CreateResults {
        path: results_dir.to_path_buf(),
        source,
    })?;
    let results_dir =
        std::fs::canonicalize(results_dir).map_err(|source| SuiteError::CreateResults {
            path: results_dir.to_path_buf(),
            source,
        })?;

    let total = task_files.len();
    let mut tasks = Vec::with_capacity(total);
    write_suite_report(&tasks_dir, &results_dir, total, &tasks)?;
    for task_file in task_files {
        let (task_metadata, metadata_error) = read_task_metadata(&task_file);
        let stem = task_file
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| SuiteError::InvalidTaskPath(task_file.clone()))?;
        let work_dir = results_dir.join(stem);
        if let Err(error) = std::fs::create_dir_all(&work_dir) {
            tasks.push(SuiteTaskResult {
                task_file,
                work_dir,
                task_metadata,
                metadata_error,
                elapsed_ms: 0,
                report: None,
                error: Some(format!("creating task work directory: {error}")),
            });
            write_suite_report(&tasks_dir, &results_dir, total, &tasks)?;
            continue;
        }
        let started = std::time::Instant::now();
        match run_one(&task_file, &work_dir) {
            Ok(report) => {
                let path = work_dir.join("report.json");
                let json = serde_json::to_string_pretty(&report)?;
                std::fs::write(&path, format!("{json}\n"))
                    .map_err(|source| SuiteError::WriteReport { path, source })?;
                tasks.push(SuiteTaskResult {
                    task_file,
                    work_dir,
                    task_metadata,
                    metadata_error,
                    elapsed_ms: started.elapsed().as_millis(),
                    report: Some(report),
                    error: None,
                });
            }
            Err(error) => tasks.push(SuiteTaskResult {
                task_file,
                work_dir,
                task_metadata,
                metadata_error,
                elapsed_ms: started.elapsed().as_millis(),
                report: None,
                error: Some(error.to_string()),
            }),
        }
        write_suite_report(&tasks_dir, &results_dir, total, &tasks)?;
    }

    write_suite_report(&tasks_dir, &results_dir, total, &tasks)
}

fn read_task_metadata(path: &Path) -> (Option<TaskMetadata>, Option<String>) {
    let value = match std::fs::read_to_string(path)
        .map_err(|error| error.to_string())
        .and_then(|text| toml::from_str::<toml::Value>(&text).map_err(|error| error.to_string()))
    {
        Ok(value) => value,
        Err(error) => return (None, Some(error)),
    };
    let Some(metadata) = value.get("metadata").cloned() else {
        return (None, None);
    };
    match metadata.try_into::<TaskMetadata>() {
        Ok(metadata) => (Some(metadata), None),
        Err(error) => (None, Some(error.to_string())),
    }
}

fn write_suite_report(
    tasks_dir: &Path,
    results_dir: &Path,
    total: usize,
    tasks: &[SuiteTaskResult],
) -> Result<SuiteReport, SuiteError> {
    let errors = tasks.iter().filter(|task| task.error.is_some()).count();
    let failed = tasks
        .iter()
        .filter(|task| {
            task.report
                .as_ref()
                .is_some_and(|report| !report.all_automated_pass())
        })
        .count();
    let passed = tasks
        .iter()
        .filter(|task| {
            task.report
                .as_ref()
                .is_some_and(ScoreReport::all_automated_pass)
        })
        .count();
    let needs_human = tasks
        .iter()
        .filter(|task| {
            task.report.as_ref().is_some_and(|report| {
                report
                    .results
                    .iter()
                    .any(|result| result.verdict == Verdict::NeedsHuman)
            })
        })
        .count();
    let report = SuiteReport {
        schema: SUITE_REPORT_SCHEMA.to_owned(),
        tasks_dir: tasks_dir.to_path_buf(),
        results_dir: results_dir.to_path_buf(),
        total,
        passed,
        failed,
        errors,
        needs_human,
        tasks: tasks.to_vec(),
    };
    let path = results_dir.join("suite-report.json");
    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(&path, format!("{json}\n"))
        .map_err(|source| SuiteError::WriteReport { path, source })?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CriterionResult, Verdict};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("eval-suite-{}-{nonce}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn runs_promoted_tasks_in_order_and_keeps_failures_and_errors() {
        let root = scratch();
        let tasks = root.join("tasks");
        let results = root.join("results");
        std::fs::create_dir_all(tasks.join("planned")).unwrap();
        for name in ["b.toml", "a.toml", "planned/never.toml"] {
            std::fs::write(tasks.join(name), "task").unwrap();
        }
        let suite = run_all(
            &tasks,
            &results,
            |path, _| -> Result<ScoreReport, &'static str> {
                let id = path.file_stem().unwrap().to_str().unwrap();
                if id == "b" {
                    return Err("backend unavailable");
                }
                Ok(ScoreReport {
                    task_id: id.to_owned(),
                    backend: "test".to_owned(),
                    results: vec![CriterionResult {
                        id: "objective".to_owned(),
                        description: "objective".to_owned(),
                        verdict: Verdict::Fail,
                        detail: "wrong".to_owned(),
                    }],
                })
            },
        )
        .unwrap();
        assert_eq!(suite.total, 2, "planned tasks are not promoted");
        assert_eq!((suite.passed, suite.failed, suite.errors), (0, 1, 1));
        assert_eq!(suite.tasks[0].task_file.file_name().unwrap(), "a.toml");
        assert!(results.join("a/report.json").is_file());
        assert!(results.join("suite-report.json").is_file());
        assert!(!suite.all_automated_pass());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn suite_artifact_embeds_task_governance_metadata() {
        let root = scratch();
        let task = root.join("task.toml");
        std::fs::write(
            &task,
            r#"
            id = "t"
            family = "f"
            brief = "b"
            [metadata]
            capabilities = ["electrical.function"]
            difficulty = "advanced"
            source = "synthetic:mutant"
            oracle_version = "spice-v1"
            split = "validation"
            expected_failure_modes = ["no-gain-compression"]
            "#,
        )
        .unwrap();
        let (metadata, error) = read_task_metadata(&task);
        assert!(error.is_none());
        let metadata = metadata.expect("embedded metadata");
        assert_eq!(metadata.capabilities, ["electrical.function"]);
        assert_eq!(metadata.oracle_version, "spice-v1");
        std::fs::remove_dir_all(root).unwrap();
    }
}
