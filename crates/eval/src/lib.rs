//! **eval** — shared harness scaffolding for typed-decision-driven
//! design-agent benchmarks (cadbench, pcbbench, and future siblings).
//!
//! Every consumer surveyed for this crate had independently built the same
//! `Task`/`Criterion` TOML schema, the same `Verdict`/`ScoreReport` shape
//! (or a less-safe version of it — see [`score`]'s docs), a
//! subprocess-driving `Backend` seam (or, in one case, no trait at all —
//! see [`backend`]'s docs), and the same "every shipped task file parses
//! and has a sound rubric" test, written out by hand each time. This crate
//! is that scaffolding, written once.
//!
//! # What this deliberately does not hold
//!
//! No domain-specific `Check` variants (`Conforms`, `DrcClean`, ...) —
//! those stay in each consuming crate, generic over `C`. No reusable
//! component or subcircuit *content* either, even once that capability
//! exists on some backend: per an explicit steer, known-good design
//! libraries are likely IP that shouldn't live inside generic tooling at
//! all (the same reason Eurorack panel data and pedalkernel-pro's own
//! content stay out of legion-of-bom) — this crate would provide the
//! *mechanism* to verify reuse/consistency, never the content being
//! reused. That mechanism isn't built yet; the backend shape it would
//! need to key off (a named/versioned block format? something keyed off a
//! topology-DAG's node concept? a separate library-artifact type?) is
//! still open as of 2026-09-22 — see this crate's `README.md`.
//!
//! # Quick start
//!
//! ```
//! use eval::{Task, Criterion, Verdict, CriterionResult, ScoreReport};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! #[serde(tag = "kind", rename_all = "snake_case")]
//! enum Check {
//!     StagesPass,
//!     MinDecisionConfidence { threshold: f64 },
//!     Subjective,
//!     // ...plus this domain's own variant(s), e.g. Conforms or DrcClean.
//! }
//!
//! type MyTask = Task<Check>;
//! ```
//!
//! Then, in one `#[test]`:
//!
//! ```
//! # use eval::{Criterion, testing::assert_every_task_sound};
//! # #[derive(serde::Deserialize)]
//! # #[serde(tag = "kind", rename_all = "snake_case")]
//! # enum Check { StagesPass }
//! fn has_stages_pass(rubric: &[Criterion<Check>]) -> bool {
//!     rubric.iter().any(|c| matches!(c.check, Check::StagesPass))
//! }
//!
//! # fn run() {
//! assert_every_task_sound::<Check>(
//!     std::path::Path::new("tasks"),
//!     &[("a stages_pass criterion", has_stages_pass)],
//! );
//! # }
//! ```

mod backend;
mod bundle;
mod score;
mod task;
pub mod testing;

pub use backend::Backend;
pub use bundle::{
    ArtifactRef, BenchmarkIdentity, BundleError, CatalogEntry, MetricDefinition, MetricGoal,
    MetricObservation, Provenance, RESULT_BUNDLE_SCHEMA, RESULT_CATALOG_SCHEMA, ResultBundle,
    ResultCatalog, RunRecord, SubjectIdentity,
};
pub use score::{CriterionResult, ScoreReport, Verdict};
pub use task::{Criterion, Task};
