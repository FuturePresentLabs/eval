//! The seam between a harness and whatever design-agent tool it drives.
//!
//! One method, because a backend has exactly one job: attempt a task,
//! leave artifacts behind. Everything that varies between backends — how
//! it's invoked, how many stages it runs, what it writes, what a "run"
//! even looks like for that domain — is behind [`Backend::Outcome`] and
//! [`Backend::Error`], both associated types each consuming crate defines
//! for itself. A harness never links a backend as a dependency; it spawns
//! one as a subprocess and reads what comes back. "Score a different tool"
//! is a new impl of this trait, not a rewrite.

use std::path::Path;

use crate::task::Task;

/// A backend that can attempt a task and leave artifacts behind.
pub trait Backend<C> {
    /// Everything one attempt produced — domain-specific (a `BuildStatus`
    /// and a `design_path` for a CAD backend; a board file and a DRC
    /// result for an EDA one).
    type Outcome;
    /// Why an attempt could not even be scored — a backend that ran and
    /// did badly is not an error, that is an `Outcome` with failing
    /// stages, which is a score, not a crash. This is for the run never
    /// happening at all: the backend couldn't be invoked, or the task
    /// needs a capability the backend doesn't have.
    type Error;

    /// Short identifier recorded in results (`"transmog"`,
    /// `"legion-of-bom"`, ...).
    fn name(&self) -> &str;

    /// Attempts `task`, writing all artifacts under `workdir`.
    ///
    /// # Errors
    /// See [`Backend::Error`].
    fn run(&self, task: &Task<C>, workdir: &Path) -> Result<Self::Outcome, Self::Error>;
}
