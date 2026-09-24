# eval

[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![tests](https://img.shields.io/badge/tests-9%20passing-brightgreen.svg)](#status)

*(Badge numbers are generated — run `scripts/update-badges.sh` after a test count changes; don't hand-edit them.)*

Shared harness scaffolding for typed-decision-driven design-agent
benchmarks — the `Task`/`Criterion` TOML schema, the `Verdict`/
`ScoreReport` shape, a `Backend` trait for driving a subprocess CLI, and a
generalized "every shipped task file parses and has a sound rubric" test.
It also owns whole-suite execution and aggregation, so every consuming
benchmark gets the same definition of "run all."
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
  domain's own `Check` enum. `TaskMetadata` records capability, difficulty,
  provenance, oracle version, dataset split, and expected failure modes.
  Historical files deserialize with compatibility defaults, while
  `testing::assert_every_task_rigorous` rejects those defaults for promoted
  suites.
- `Verdict` / `CriterionResult` / `ScoreReport` — not generic; once a
  criterion is scored, all that's left is `Pass`/`Fail`/`NeedsHuman` and a
  detail string. `ScoreReport::all_automated_pass()` /
  `.needs_human()` — a `NeedsHuman` verdict never fails a run and is
  never silently swallowed either.
- `Backend<C>` — one method (`run`), two associated types (`Outcome`,
  `Error`) each consuming crate defines for itself. A harness never links
  a backend as a dependency; it spawns one as a subprocess. "Score a
  different tool" is a new impl, not a rewrite.
- `run_all` / `SuiteReport` — discovers promoted task TOMLs in deterministic
  order, gives each an isolated result directory, continues after task-level
  backend errors, and writes `eval.suite-report.v1` to `suite-report.json`.
  Nested `tasks/planned/` contracts are excluded until promoted.
- `RunProtocol` / `TrialObservation` — records subjects, tasks, trials,
  sampling parameters, budgets, reset identities, and result references. Its
  validator fails closed unless the complete subject × task × trial matrix is
  present and paired subjects used the same task snapshot.
- `ScoreComposition` — reports objective passes, objective failures, and
  unresolved human review independently. An objective rate never includes
  `NeedsHuman` in its denominator.

## Rigorous task metadata

Promoted task files should include explicit governance metadata:

```toml
[metadata]
capabilities = ["geometry.bounds", "geometry.holes"]
difficulty = "intermediate"
source = "synthetic:single-constraint-pair"
oracle_version = "geometry-v1"
split = "validation"
expected_failure_modes = ["wrong-width", "missing-hole"]
```

Use development tasks while building a scorer, validation tasks for routine
model comparison, and non-public holdout tasks for final claims. Task counts
are not a coverage metric: publish per-capability results and macro-average
capability families so duplicating an easy family cannot dominate the score.

## Unified benchmark viewer

`eval-viewer` combines any available PCB, CAD, CAM, and DFM suite reports into
one self-contained HTML dashboard. The top-right switcher always shows all four
disciplines; benchmarks without a loaded report remain visible but disabled.

```bash
cargo run -p eval --bin eval-viewer -- \
  --pcb ../pcbbench/suite-report.json \
  --cad ../cadbench/suite-report.json \
  --cam ../cambench/suite-report.json \
  --dfm ../dfmbench/suite-report.json \
  --out benchmark-viewer.html
```

Every input is optional, so the same command works while individual benchmark
runners are still being brought onto the shared `eval.suite-report.v1` contract.

Add `--serve 8123` to run the same dashboard as a localhost monitor instead of
writing a snapshot. It polls the input reports once per second, preserves the
selected benchmark in the URL hash, and offers live-refresh, manual-refresh,
and failed-only controls. Suite runners checkpoint their aggregate report after
each prompt, so completed tasks appear before the rest of the run finishes.

```bash
cargo run -p eval --bin eval-viewer -- \
  --pcb ../pcbbench/out/suite-report.json \
  --cad ../cadbench/out/suite-report.json \
  --serve 8123
```

For tailnet access, bind the server to the host's Tailscale address with
`--bind "$(tailscale ip -4)"`; the default remains loopback-only.

For configuration comparison, pass an `eval.leaderboard.v1` index instead of
individual benchmark reports. Each entry independently names the actual
decision model and its execution harness, then points to measured suite
evidence. Router/deployment aliases are provenance only.

```json
{
  "schema": "eval.leaderboard.v1",
  "models": [{
    "id": "example-model",
    "name": "Transmog reference configuration",
    "organization": "Example Lab",
    "harness": "transmog",
    "rlcd_model": "example/bounded-decider",
    "generative_model": "GLM-5.3-Flash",
    "alias": "production-cad-router",
    "results": {
      "cad": {
        "report": "runs/example/cad/suite-report.json",
        "average_cost_usd_per_task": 0.012
      }
    }
  }]
}
```

Price remains unavailable unless the benchmark runner records it; the viewer
does not reconstruct price from a hand-maintained provider table. Wall time is
retained as run metadata but is not used as the leaderboard's value axis.
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
