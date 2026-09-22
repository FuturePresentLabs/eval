# eval

[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![tests](https://img.shields.io/badge/tests-9%20passing-brightgreen.svg)](#status)

*(Badge numbers are generated — run `scripts/update-badges.sh` after a test count changes; don't hand-edit them.)*

Shared harness scaffolding for typed-decision-driven design-agent
benchmarks — the `Task`/`Criterion` TOML schema, the `Verdict`/
`ScoreReport` shape, a `Backend` trait for driving a subprocess CLI, and a
generalized "every shipped task file parses and has a sound rubric" test.
Used by [`cadbench`](https://github.com/FuturePresentLabs/cadbench) and
[`pcbbench`](https://github.com/FuturePresentLabs/pcbbench); `dfmbench` and
`cambench` are expected to build on it too rather than becoming a third
and fourth copy of the same shape.

## Why this exists

Surveying this ecosystem found `cadbench` and `pcbbench`'s harnesses
independently converging on the same `Task`/`Criterion`/`ScoreReport`
shape, and then — because there was nowhere shared to put it — quietly
diverging anyway: cadbench evolved a real `Verdict` enum
(`Pass`/`Fail`/`NeedsHuman`) and a `Backend` trait; pcbbench stayed on an
`objective: bool` + `passed: Option<bool>` pair and a hardcoded free
function with no trait at all. Both repos also had their own hand-written
copy of the "every task file actually parses" test, and — once badge
scripts and a `tasks/planned/` convention were added — both of *those*
got duplicated too, by the same agent, in the same sitting, because there
was still nowhere shared to put them.

`eval` is that shared place. `cadbench` migrated onto it cleanly (compiled
on the first try, zero test regressions) — its design was already the
better-factored one; `pcbbench` did a real migration onto that design
(10/10 tests pass, plus a real behavior fix along the way: its CLI
previously always exited 0 regardless of score).

## What this deliberately does not hold

No domain-specific `Check` variants (`Conforms`, `DrcClean`, ...) — those
stay generic (`Task<C>`, `Criterion<C>`), defined in each consuming crate.

No reusable component or subcircuit **content**, even once that
capability exists on some backend. Per an explicit steer from FPL
leadership (via direct discussion with the legion-of-bom agent, 2026-09-22):
known-good design libraries are likely IP that shouldn't live inside
generic tooling at all — the same reason Eurorack panel data and
pedalkernel-pro's own content stay out of legion-of-bom. If/when a
reuse-verification feature lands here, it will be *mechanism* (a `Check`
kind + task-sequencing that can verify "did this reuse X and preserve its
interface"), never the library of reusable things themselves.

That mechanism isn't built yet. Backend shape is still open — a
named/versioned block format? Keyed off legion-of-bom's `spec-chain`
topology-DAG node concept? A separate library-artifact type entirely?
Tracked as `eval-qnbv` in this project's marbles. There's also a distinct,
not-yet-designed idea in the same space worth keeping separate: PCBBench
(and by extension CADBench) as a *progressive curriculum* — building up a
verified library over runs, then constructing increasingly complex
systems from it, rather than every task being a one-shot fresh
generation. Don't design either repo's rubric/task-format around a
specific backend representation until this is settled.

## What's in this crate

- `Task<C>` / `Criterion<C>` — the task container, generic over each
  domain's own `Check` enum.
- `Verdict` / `CriterionResult` / `ScoreReport` — not generic; once a
  criterion is scored, all that's left is `Pass`/`Fail`/`NeedsHuman` and a
  detail string. `ScoreReport::all_automated_pass()` /
  `.needs_human()` — a `NeedsHuman` verdict never fails a run and is
  never silently swallowed either.
- `Backend<C>` — one method (`run`), two associated types (`Outcome`,
  `Error`) each consuming crate defines for itself. A harness never links
  a backend as a dependency; it spawns one as a subprocess. "Score a
  different tool" is a new impl, not a rewrite.
- `testing::assert_every_task_sound` — parses every `.toml` file one
  directory deep under a given `tasks/` dir, checks non-empty id/family/
  brief, runs caller-supplied required-check predicates, checks for
  duplicate rubric ids, and refuses an empty directory as a vacuous pass.
  A `tasks/planned/` subdirectory (not-yet-runnable target specs) is
  naturally excluded — this only reads one directory deep.

## License

MIT OR Apache-2.0 — deliberately permissive, matching `ooda`'s reasoning:
every consumer can depend on this regardless of its own license
(`cadbench`/`pcbbench`: AGPL-3.0-or-later today).
