//! A generalized "every shipped task file is part of the harness's
//! contract, not sample data" test — call this from one `#[test]` in each
//! consuming crate instead of writing it out by hand per repo. Every prior
//! implementation of this test (cadbench, pcbbench) was near-identical;
//! this is that test, written once.

use std::path::Path;

use serde::de::DeserializeOwned;

use crate::task::{Criterion, Task};

/// Parses every `.toml` file directly under `tasks_dir` (one directory
/// deep — a `tasks/planned/` subdirectory of not-yet-runnable tasks is
/// deliberately not swept up) as a `Task<C>`, and asserts:
///
/// - it parses at all, with the file path in the panic message;
/// - `id`, `family`, and `brief` are non-empty;
/// - every predicate in `required` passes against the task's rubric (name
///   each predicate for a legible panic message — e.g.
///   `("a stages_pass criterion", |r| r.iter().any(|c| matches!(c.check, Check::StagesPass)))`);
/// - rubric ids within one task are unique (they're how a [`crate::ScoreReport`]'s
///   results are keyed; a duplicate would silently overwrite in any
///   downstream report).
///
/// Also asserts `tasks_dir` actually contained at least one `.toml` file —
/// an empty directory is a harness that stopped testing anything, not a
/// vacuous pass.
///
/// # Panics
/// On the first task file that fails any of the above, with the file path
/// and the specific failure in the message.
pub fn assert_every_task_sound<C: DeserializeOwned>(
    tasks_dir: &Path,
    required: &[(&str, fn(&[Criterion<C>]) -> bool)],
) {
    let mut checked = 0;
    let entries = std::fs::read_dir(tasks_dir)
        .unwrap_or_else(|e| panic!("{} is readable: {e}", tasks_dir.display()));

    for entry in entries {
        let path = entry.expect("dir entry readable").path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }

        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()));
        let task: Task<C> = toml::from_str(&text)
            .unwrap_or_else(|e| panic!("{} parses: {e}", path.display()));

        assert!(!task.id.trim().is_empty(), "{}: empty id", path.display());
        assert!(
            !task.family.trim().is_empty(),
            "{}: empty family",
            path.display()
        );
        assert!(
            !task.brief.trim().is_empty(),
            "{}: empty brief",
            path.display()
        );

        for (label, predicate) in required {
            assert!(
                predicate(&task.rubric),
                "{}: missing {label}",
                path.display()
            );
        }

        let mut ids = task.rubric_ids();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), before, "{}: duplicate rubric ids", path.display());

        checked += 1;
    }

    assert!(checked > 0, "{} has no .toml files to check", tasks_dir.display());
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum Check {
        StagesPass,
        Subjective,
    }

    fn has_stages_pass(rubric: &[Criterion<Check>]) -> bool {
        rubric.iter().any(|c| matches!(c.check, Check::StagesPass))
    }

    #[test]
    fn a_sound_task_directory_passes() {
        let dir = std::env::temp_dir().join(format!("eval-testing-sound-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("a.toml"),
            r#"
id = "a"
family = "f"
brief = "b"

[[rubric]]
id = "stages"
description = "d"
kind = "stages_pass"
"#,
        )
        .unwrap();

        assert_every_task_sound::<Check>(&dir, &[("a stages_pass criterion", has_stages_pass)]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    #[should_panic(expected = "missing a stages_pass criterion")]
    fn a_task_missing_a_required_check_fails() {
        let dir = std::env::temp_dir().join(format!("eval-testing-missing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("a.toml"),
            r#"
id = "a"
family = "f"
brief = "b"

[[rubric]]
id = "vibe"
description = "d"
kind = "subjective"
"#,
        )
        .unwrap();

        assert_every_task_sound::<Check>(&dir, &[("a stages_pass criterion", has_stages_pass)]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    #[should_panic(expected = "duplicate rubric ids")]
    fn duplicate_rubric_ids_fail() {
        let dir =
            std::env::temp_dir().join(format!("eval-testing-duplicate-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("a.toml"),
            r#"
id = "a"
family = "f"
brief = "b"

[[rubric]]
id = "x"
description = "d"
kind = "stages_pass"

[[rubric]]
id = "x"
description = "d2"
kind = "subjective"
"#,
        )
        .unwrap();

        assert_every_task_sound::<Check>(&dir, &[]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    #[should_panic(expected = "has no .toml files")]
    fn an_empty_directory_is_not_a_vacuous_pass() {
        let dir = std::env::temp_dir().join(format!("eval-testing-empty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_every_task_sound::<Check>(&dir, &[]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_planned_subdirectory_is_not_swept_up() {
        let dir = std::env::temp_dir().join(format!("eval-testing-planned-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("planned")).unwrap();
        std::fs::write(
            dir.join("a.toml"),
            r#"
id = "a"
family = "f"
brief = "b"

[[rubric]]
id = "stages"
description = "d"
kind = "stages_pass"
"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("planned/not-real-yet.toml"),
            "this is not even valid TOML for a Task {{{",
        )
        .unwrap();

        // Would panic on the malformed planned/ file if it were swept up.
        assert_every_task_sound::<Check>(&dir, &[("a stages_pass criterion", has_stages_pass)]);
        std::fs::remove_dir_all(&dir).ok();
    }
}
