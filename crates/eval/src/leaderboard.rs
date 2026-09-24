//! Stable input contract for comparing decision models across benchmark suites.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const LEADERBOARD_SCHEMA: &str = "eval.leaderboard.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardIndex {
    pub schema: String,
    #[serde(default)]
    pub benchmarks: BTreeMap<String, BenchmarkSource>,
    pub models: Vec<ModelResultIndex>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSource {
    pub readme: PathBuf,
    #[serde(default)]
    pub sections: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResultIndex {
    pub id: String,
    /// Configuration label; not a model identity.
    pub name: String,
    #[serde(default)]
    pub organization: Option<String>,
    /// Execution environment/tooling used to turn decisions into benchmark artifacts.
    pub harness: String,
    /// Model used for bounded RLCD choices, when the harness has that role.
    #[serde(default)]
    pub rlcd_model: Option<String>,
    /// Model used for open-ended generation/reasoning, when present.
    #[serde(default)]
    pub generative_model: Option<String>,
    /// Optional user-facing router or deployment alias (for provenance only).
    #[serde(default)]
    pub alias: Option<String>,
    /// Benchmark id (`pcb`, `cad`, `cam`, `dfm`) to measured run evidence.
    pub results: BTreeMap<String, BenchmarkResultIndex>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResultIndex {
    pub report: PathBuf,
    #[serde(default)]
    pub average_seconds_per_task: Option<f64>,
    #[serde(default)]
    pub average_cost_usd_per_task: Option<f64>,
}
