//! Self-contained, benchmark-neutral HTML viewer for suite reports.

use crate::{SuiteReport, TaskMetadata, Verdict};

/// One of the benchmark disciplines presented by the shared viewer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BenchmarkKind {
    Pcb,
    Cad,
    Cam,
    Dfm,
}

impl BenchmarkKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Pcb => "pcb",
            Self::Cad => "cad",
            Self::Cam => "cam",
            Self::Dfm => "dfm",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Pcb => "PCB Bench",
            Self::Cad => "CAD Bench",
            Self::Cam => "CAM Bench",
            Self::Dfm => "DFM Bench",
        }
    }

    fn emoji(self) -> &'static str {
        match self {
            Self::Pcb => "⚡",
            Self::Cad => "📐",
            Self::Cam => "⚙️",
            Self::Dfm => "🏭",
        }
    }
}

/// A benchmark and its optional latest suite report.
#[derive(Debug)]
pub struct BenchmarkView<'a> {
    pub kind: BenchmarkKind,
    pub report: Option<&'a SuiteReport>,
}

/// One decision model and the suite report it produced for each discipline.
pub struct ModelView<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub organization: Option<&'a str>,
    pub harness: &'a str,
    pub rlcd_model: Option<&'a str>,
    pub generative_model: Option<&'a str>,
    pub alias: Option<&'a str>,
    pub pcb: Option<&'a SuiteReport>,
    pub cad: Option<&'a SuiteReport>,
    pub cam: Option<&'a SuiteReport>,
    pub dfm: Option<&'a SuiteReport>,
    pub pcb_metrics: RunMetrics,
    pub cad_metrics: RunMetrics,
    pub cam_metrics: RunMetrics,
    pub dfm_metrics: RunMetrics,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RunMetrics {
    pub average_seconds_per_task: Option<f64>,
    pub average_cost_usd_per_task: Option<f64>,
}

pub struct BenchmarkPrimer {
    pub kind: BenchmarkKind,
    pub html: String,
}

impl ModelView<'_> {
    fn report(&self, kind: BenchmarkKind) -> Option<&SuiteReport> {
        match kind {
            BenchmarkKind::Pcb => self.pcb,
            BenchmarkKind::Cad => self.cad,
            BenchmarkKind::Cam => self.cam,
            BenchmarkKind::Dfm => self.dfm,
        }
    }

    fn metrics(&self, kind: BenchmarkKind) -> RunMetrics {
        match kind {
            BenchmarkKind::Pcb => self.pcb_metrics,
            BenchmarkKind::Cad => self.cad_metrics,
            BenchmarkKind::Cam => self.cam_metrics,
            BenchmarkKind::Dfm => self.dfm_metrics,
        }
    }
}

/// Renders the model-first leaderboard. Benchmark cells link to run evidence.
#[must_use]
pub fn render_leaderboard(models: &[ModelView<'_>], primers: &[BenchmarkPrimer]) -> String {
    let kinds = [
        BenchmarkKind::Pcb,
        BenchmarkKind::Cad,
        BenchmarkKind::Cam,
        BenchmarkKind::Dfm,
    ];
    let winners = kinds.map(|kind| {
        models
            .iter()
            .filter_map(|model| model.report(kind).map(score))
            .fold(0.0_f64, f64::max)
    });
    let overall_winner = models
        .iter()
        .filter(|model| coverage(model) > 0)
        .map(overall)
        .fold(0.0_f64, f64::max);
    let mut order: Vec<_> = models.iter().collect();
    order.sort_by(|a, b| {
        overall(b)
            .total_cmp(&overall(a))
            .then_with(|| a.name.cmp(b.name))
    });
    let rows = order.iter().enumerate().map(|(rank, model)| {
        let scores = kinds.iter().enumerate().map(|(column, kind)| match model.report(*kind) {
            Some(report) => {
                let value = score(report);
                let text = format!("{value:.1}");
                let shown = if (value - winners[column]).abs() < 0.0001 { format!("<strong>{text}</strong>") } else { text };
                format!("<td data-score='{value}'><a class=score-cell href='/prompt/{}/{}/0'><span>{shown}</span><small>{}/{} tasks</small><small>{} review · {} errors</small></a></td>", esc(model.id), kind.id(), report.passed, report.total, report.needs_human, report.errors)
            }
            None => format!(
                "<td class=missing-score data-score='-1'><span>Not run</span><small>No {} suite report in this index</small></td>",
                kind.name()
            ),
        }).collect::<String>();
        let is_model = |value: &&str| !matches!(*value, "Not recorded" | "TBD" | "Not applicable");
        let configuration = [model.rlcd_model, model.generative_model]
            .into_iter()
            .flatten()
            .filter(is_model)
            .map(esc)
            .collect::<Vec<_>>()
            .join(" → ");
        let covered = coverage(model);
        let overall_cell = if covered == 0 { "<td data-score='-1'>—</td>".to_owned() } else { let value = overall(model); let shown = if (value - overall_winner).abs() < 0.0001 { format!("<strong>{value:.1}</strong>") } else { format!("{value:.1}") }; format!("<td data-score='{value:.3}'><span class=score-cell><span>{shown}</span><small>mean of {covered} suite rate(s)</small></span></td>") };
        format!("<tr><td>{}</td><th scope=row><code>{}</code></th><td>{}</td>{overall_cell}{scores}</tr>", rank + 1, if configuration.is_empty() { "—".to_owned() } else { configuration }, esc(model.harness))
    }).collect::<String>();
    let evidence = String::new();
    let chart_data = chart_data(models, &kinds);
    let active = Some(BenchmarkKind::Cad);
    let switcher = kinds.iter().map(|kind| {
        format!("<li><button class=benchmark-tab role=tab data-bench={} aria-controls=primer-{} aria-selected={}><span>{}</span> {}</button></li>", kind.id(), kind.id(), active == Some(*kind), kind.emoji(), kind.name())
    }).collect::<String>();
    let primers_html = kinds.iter().map(|kind| {
        let (status, body) = primers
            .iter()
            .find(|primer| primer.kind == *kind)
            .map_or_else(
                || (
                    "Benchmark definition not recorded",
                    "<p>No README source is configured for this benchmark yet.</p>".to_owned(),
                ),
                |primer| (
                    "README-backed benchmark definition",
                    format!("<section class=readme-copy>{}</section>", primer.html),
                ),
            );
        format!("<article id=primer-{} class='benchmark-primer-panel {}' data-bench={} role=tabpanel {}><header><hgroup><p>{}</p><h2>{}</h2></hgroup></header>{body}</article>", kind.id(), kind.id(), kind.id(), if active == Some(*kind) { "" } else { "hidden" }, kind.emoji(), if status == "Benchmark definition not recorded" { status } else { kind.name() })
    }).collect::<String>();
    let rows = if rows.is_empty() {
        "<tr><td colspan=8>No benchmark configurations recorded.</td></tr>".to_owned()
    } else {
        rows
    };
    format!(
        "<!doctype html><html lang=en><head><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'><title>FPL decision model leaderboard</title><style>{CSS}{LEADERBOARD_CSS}{SCORE_EXPLAINER_CSS}</style></head><body><header class=site-head><a class=wordmark href=#leaderboard>FPL <span>decision model index</span></a><div class=header-actions><div class=monitor-controls><button type=button id=refresh>Refresh</button><label><input id=live type=checkbox checked> Live</label></div><nav class=benchmark-switcher aria-label='Benchmark' role=tablist>{switcher}</nav></div></header><main id=leaderboard><div class=leader-head><div><h1>Decision models × harnesses</h1><p>Comparable configurations. Column leaders are bold.</p></div><span>{} evaluated</span></div><details class=score-method open><summary><strong>How scoring works</strong></summary><div class=grid><p><strong>Suite score</strong><br>Tasks passing every automated criterion ÷ all discovered tasks. Harness errors stay in the denominator.</p><p><strong>Human review</strong><br><code>NeedsHuman</code> is excluded from pass/fail, so it neither adds nor removes a task pass.</p><p><strong>Overall</strong><br>Arithmetic mean of available benchmark suite percentages. Missing benchmarks are excluded, not scored as zero.</p><p><strong>Current proxy behavior</strong><br>A passing proxy still counts as an automated pass in the v1 score; prompt pages mark it provisional.</p></div></details><section class=benchmark-guide>{primers_html}</section><article class=pareto-shell><header><h2>Price / score frontier</h2><select id=pareto-bench aria-label=Benchmark><option value=overall>Overall</option><option value=pcb>PCB</option><option value=cad>CAD</option><option value=cam>CAM</option><option value=dfm>DFM</option></select><select id=pareto-x aria-label='X axis'><option value=cost>Cost / task</option></select></header><svg id=pareto role=img aria-label='Model score versus price' viewBox='0 0 1000 430' width='100%' height='430' hidden></svg><p id=pareto-empty hidden>Come back soon.</p></article><div class=leader-wrap><table class=leader-table><thead><tr><th>Rank</th><th>Configuration</th><th>Harness</th><th><a href=# data-column=3>Overall</a></th><th><a href=# data-column=4>⚡ PCB</a></th><th><a href=# data-column=5>📐 CAD</a></th><th><a href=# data-column=6>⚙️ CAM</a></th><th><a href=# data-column=7>🏭 DFM</a></th></tr></thead><tbody>{rows}</tbody></table></div>{evidence}</main><script>const paretoData={chart_data};{LEADERBOARD_JS}</script></body></html>",
        models.len(),
    )
    .replace(&format!("<span>{} evaluated</span>", models.len()), "")
    .replace("FPL decision model leaderboard", "FPL evaluation leaderboard")
    .replace("decision model index", "evaluation index")
    .replace("Decision models × harnesses", "Evaluation leaderboard")
    .replace("<p>Comparable configurations. Column leaders are bold.</p>", "")
    .replace("<nav class=benchmark-switcher aria-label='Benchmark' role=tablist>", "<nav class=benchmark-switcher aria-label='Benchmark' role=tablist><ul>")
    .replace("</nav></div></header><main id=leaderboard>", "</ul></nav></div></header><main id=leaderboard>")
    .replace("<meta name=viewport content='width=device-width,initial-scale=1'>", "<meta name=viewport content='width=device-width,initial-scale=1'><meta name=color-scheme content='light dark'>")
    .replace("<header class=site-head><a class=wordmark href=#leaderboard>FPL <span>evaluation index</span></a><div class=header-actions><div class=monitor-controls><button type=button id=refresh>Refresh</button><label><input id=live type=checkbox checked> Live</label></div><nav class=benchmark-switcher aria-label='Benchmark' role=tablist><ul>", "<header class=container><nav aria-label='Benchmark'><ul><li><strong>FPL evaluation index</strong></li></ul><ul>")
    .replace("</ul></nav></div></header><main id=leaderboard>", "<li><button id=theme-toggle class=secondary type=button>Theme</button></li></ul></nav></header><main id=leaderboard class=container>")
    .replace("<div class=leader-wrap>", "<div class=overflow-auto>")
    .replace("<table class=leader-table>", "<table class='leader-table striped'>")
    .replace("<th><button data-column=3>Overall</button></th><th><button data-column=4>⚡ PCB</button></th><th><button data-column=5>📐 CAD</button></th><th><button data-column=6>⚙️ CAM</button></th><th><button data-column=7>🏭 DFM</button></th>", "<th><a href=# data-column=3>Overall</a></th><th><a href=# data-column=4>⚡ PCB</a></th><th><a href=# data-column=5>📐 CAD</a></th><th><a href=# data-column=6>⚙️ CAM</a></th><th><a href=# data-column=7>🏭 DFM</a></th>")
    .replace("<th><button data-column=5>Overall</button></th><th><button data-column=6>⚡ PCB</button></th><th><button data-column=7>📐 CAD</button></th><th><button data-column=8>⚙️ CAM</button></th><th><button data-column=9>🏭 DFM</button></th>", "<th><a href=# data-column=5>Overall</a></th><th><a href=# data-column=6>⚡ PCB</a></th><th><a href=# data-column=7>📐 CAD</a></th><th><a href=# data-column=8>⚙️ CAM</a></th><th><a href=# data-column=9>🏭 DFM</a></th>")
    .replace("<th>Decision model</th><th>Harness</th><th><button data-column=3>Overall</button></th><th><button data-column=4>⚡ PCB</button></th><th><button data-column=5>📐 CAD</button></th><th><button data-column=6>⚙️ CAM</button></th><th><button data-column=7>🏭 DFM</button></th>", "<th>Configuration</th><th>RLCD model</th><th>Generative model</th><th>Harness</th><th><button data-column=5>Overall</button></th><th><button data-column=6>⚡ PCB</button></th><th><button data-column=7>📐 CAD</button></th><th><button data-column=8>⚙️ CAM</button></th><th><button data-column=9>🏭 DFM</button></th>")
    .replace("<th>Decision model</th><th>Harness</th><th><a href=# data-column=3>Overall</a></th><th><a href=# data-column=4>⚡ PCB</a></th><th><a href=# data-column=5>📐 CAD</a></th><th><a href=# data-column=6>⚙️ CAM</a></th><th><a href=# data-column=7>🏭 DFM</a></th>", "<th>Configuration</th><th>RLCD model</th><th>Generative model</th><th>Harness</th><th><a href=# data-column=5>Overall</a></th><th><a href=# data-column=6>⚡ PCB</a></th><th><a href=# data-column=7>📐 CAD</a></th><th><a href=# data-column=8>⚙️ CAM</a></th><th><a href=# data-column=9>🏭 DFM</a></th>")
}

fn chart_data(models: &[ModelView<'_>], kinds: &[BenchmarkKind; 4]) -> String {
    let rows = models.iter().map(|model| {
        let benchmarks = kinds.iter().filter_map(|kind| model.report(*kind).map(|report| {
            let metrics = model.metrics(*kind);
            (kind.id().to_owned(), serde_json::json!({"score": score(report), "cost": metrics.average_cost_usd_per_task, "time": metrics.average_seconds_per_task}))
        })).collect::<serde_json::Map<_, _>>();
        let costs = kinds.iter().filter_map(|kind| model.report(*kind).and_then(|_| model.metrics(*kind).average_cost_usd_per_task)).collect::<Vec<_>>();
        let times = kinds.iter().filter_map(|kind| model.report(*kind).and_then(|_| model.metrics(*kind).average_seconds_per_task)).collect::<Vec<_>>();
        serde_json::json!({"id": model.id, "name": model.name, "overall": {"score": overall(model), "cost": mean(&costs), "time": mean(&times)}, "benchmarks": benchmarks})
    }).collect::<Vec<_>>();
    serde_json::to_string(&rows).expect("chart data serializes")
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

fn score(report: &SuiteReport) -> f64 {
    if report.total == 0 {
        0.0
    } else {
        report.passed as f64 * 100.0 / report.total as f64
    }
}
fn coverage(model: &ModelView<'_>) -> usize {
    [model.pcb, model.cad, model.cam, model.dfm]
        .iter()
        .filter(|report| report.is_some())
        .count()
}
fn overall(model: &ModelView<'_>) -> f64 {
    let values = [model.pcb, model.cad, model.cam, model.dfm]
        .into_iter()
        .flatten()
        .map(score)
        .collect::<Vec<_>>();
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

/// Renders all four benchmark disciplines into one self-contained page.
#[must_use]
pub fn render(views: &[BenchmarkView<'_>]) -> String {
    let all = [
        BenchmarkKind::Pcb,
        BenchmarkKind::Cad,
        BenchmarkKind::Cam,
        BenchmarkKind::Dfm,
    ];
    let available = |kind| {
        views
            .iter()
            .find(|view| view.kind == kind)
            .and_then(|view| view.report)
    };
    let active = all
        .iter()
        .copied()
        .find(|kind| available(*kind).is_some())
        .unwrap_or(BenchmarkKind::Cad);
    let switcher = all
        .iter()
        .map(|kind| {
            let disabled = available(*kind).is_none();
            format!(
                "<button type=button id=tab-{} class=bench-tab data-bench={} role=tab aria-controls=panel-{} {} {}><span aria-hidden=true>{}</span><span>{}</span></button>",
                    kind.id().to_owned(),
                kind.id(), kind.id(),
                if *kind == active { "aria-selected=true" } else { "aria-selected=false" },
                if disabled { "disabled title=\"No suite report loaded\"" } else { "" },
                kind.emoji(), kind.name()
            )
        })
        .collect::<String>();
    let panels = all
        .iter()
        .map(|kind| panel(*kind, available(*kind), *kind == active))
        .collect::<String>();
    format!(
        "<!doctype html><html lang=en><head><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'><title>FPL benchmark viewer</title><style>{CSS}</style></head><body><header class=site-head><a class=wordmark href=#>FPL <span>evaluation works</span></a><div class=monitor-controls><button type=button id=refresh>Refresh</button><label><input id=live type=checkbox checked> Live</label><label><input id=failed-only type=checkbox> Failed only</label></div><nav class=bench-switcher aria-label='Benchmark' role=tablist>{switcher}</nav></header><main>{panels}</main><script>{JS}</script></body></html>"
    )
}

fn panel(kind: BenchmarkKind, report: Option<&SuiteReport>, active: bool) -> String {
    let hidden = if active { "" } else { " hidden" };
    let Some(report) = report else {
        return format!(
            "<section id=panel-{} class='bench-panel {}' data-bench={} role=tabpanel aria-labelledby=tab-{}{hidden}><div class=empty><span>{}</span><h1>{}</h1><p>No suite report loaded.</p></div></section>",
            kind.id(),
            kind.id(),
            kind.id(),
            kind.id(),
            kind.emoji(),
            kind.name()
        );
    };
    let rate = if report.total == 0 {
        0.0
    } else {
        100.0 * report.passed as f64 / report.total as f64
    };
    let tasks = report
        .tasks
        .iter()
        .enumerate()
        .map(|(index, task)| task_card(None, kind, index, task))
        .collect::<String>();
    let complete = report.tasks.len();
    format!(
        "<section id=panel-{} class='bench-panel {}' data-bench={} role=tabpanel aria-labelledby=tab-{}{hidden}><div class=hero><div><p class=identity><span>{}</span>{}</p><h1>{:.0}% <small>strict task pass rate · {complete}/{} complete</small></h1></div><dl><div><dt>Passed</dt><dd>{}/{}</dd></div><div><dt>Failed</dt><dd>{}</dd></div><div><dt>Harness errors</dt><dd>{}</dd></div><div><dt>Human review</dt><dd>{}</dd></div></dl></div><div class=task-list>{tasks}</div></section>",
        kind.id(),
        kind.id(),
        kind.id(),
        kind.id(),
        kind.emoji(),
        kind.name(),
        rate,
        report.total,
        report.passed,
        report.total,
        report.failed,
        report.errors,
        report.needs_human
    )
}

fn task_card(
    model_id: Option<&str>,
    kind: BenchmarkKind,
    index: usize,
    task: &crate::SuiteTaskResult,
) -> String {
    let brief = task_brief(&task.task_file);
    let prompt = task_prompt(brief.as_deref());
    let preview = brief
        .as_deref()
        .map(prompt_preview)
        .unwrap_or_else(|| "Prompt unavailable".to_owned());
    let prompt_href = model_id.map(|model| format!("/prompt/{model}/{}/{index}", kind.id()));
    let artifacts = artifact_links(model_id, kind, index, &task.work_dir);
    let standards = task_check_definitions(task);
    let Some(report) = &task.report else {
        return format!(
            "<article class='task error'><header><h2>{}</h2><span>Harness error</span></header><p>{}</p>{prompt}{artifacts}</article>",
            esc(task
                .task_file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown task")),
            esc(task.error.as_deref().unwrap_or("No score report"))
        );
    };
    let passed = report.all_automated_pass();
    let open = if index == 0 || !passed { " open" } else { "" };
    let checks = report
        .results
        .iter()
        .map(|result| {
            let (class, mark) = match result.verdict {
                Verdict::Pass => ("pass", "Pass"),
                Verdict::Fail => ("fail", "Fail"),
                Verdict::NeedsHuman => ("human", "Review"),
            };
            let standard = standards
                .get(&result.id)
                .map(check_heading)
                .unwrap_or_else(|| result.id.clone());
            format!("<details class='check {class}' title='{}'><summary><span class='verdict {class}'>{mark}</span><span><strong>{}</strong><small>{}</small></span></summary><div class=check-detail><h4>Evidence</h4><p>{}</p><code>{}</code></div></details>", esc(&result.detail), esc(&standard), esc(&result.description), esc(&result.detail), esc(&result.id))
        })
        .collect::<String>();
    let open_prompt = prompt_href.map_or(String::new(), |href| {
        format!("<a class=open-prompt href='{href}'>Open prompt →</a>")
    });
    format!(
        "<article class='task {}'><details{open}><summary><span><strong>{}</strong><small class=prompt-preview>{}</small><small>{} automated checks</small></span><b>{}</b></summary>{open_prompt}{prompt}<section class=checks><h3>Standards & checks</h3>{checks}</section>{artifacts}</details></article>",
        if passed { "passed" } else { "failed" },
        esc(&report.task_id),
        esc(&preview),
        report
            .results
            .iter()
            .filter(|result| result.verdict != Verdict::NeedsHuman)
            .count(),
        if passed { "Passed" } else { "Failed" }
    )
}

/// Renders one prompt as a navigable review page with its final STL and standards.
#[must_use]
pub fn render_prompt_page(
    model_id: &str,
    model_name: &str,
    harness: &str,
    kind: BenchmarkKind,
    report: &SuiteReport,
    index: usize,
) -> Option<String> {
    let task = report.tasks.get(index)?;
    let scored = task.report.as_ref();
    let task_id = scored.map_or_else(
        || "Prompt failed before scoring".to_owned(),
        |value| value.task_id.clone(),
    );
    let brief = task_brief(&task.task_file);
    let standards = task_check_definitions(task);
    let deterministic = scored.map_or(0, |value| {
        value
            .results
            .iter()
            .filter(|result| result.verdict != Verdict::NeedsHuman && !is_proxy(result))
            .count()
    });
    let passed = scored.map_or(0, |value| {
        value
            .results
            .iter()
            .filter(|result| result.verdict == Verdict::Pass && !is_proxy(result))
            .count()
    });
    let failed = scored.map_or(0, |value| {
        value
            .results
            .iter()
            .filter(|result| result.verdict == Verdict::Fail)
            .count()
    });
    let reviews = scored.map_or(0, |value| {
        value
            .results
            .iter()
            .filter(|result| result.verdict == Verdict::NeedsHuman)
            .count()
    });
    let proxies = scored.map_or(0, |value| {
        value
            .results
            .iter()
            .filter(|result| is_proxy(result))
            .count()
    });
    let state = if task.error.is_some() {
        ("error", "HARNESS ERROR")
    } else if failed > 0 {
        ("fail", "FAIL")
    } else if proxies > 0 {
        ("warn", "PROVISIONAL")
    } else {
        ("pass", "PASS")
    };
    let checks = scored.map_or_else(|| format!("<p><strong>No score report</strong><br>{}</p>", esc(task.error.as_deref().unwrap_or("No harness error was recorded."))), |value| value.results.iter().map(|result| {
        let proxy = is_proxy(result);
        let (class, mark, treatment) = if proxy { ("proxy", "PROXY", "Counts today; result remains provisional") } else { match result.verdict { Verdict::Pass => ("pass", "PASS", "Counts toward task pass"), Verdict::Fail => ("fail", "FAIL", "Fails the task"), Verdict::NeedsHuman => ("human", "REVIEW", "Excluded from objective score") } };
        let definition = standards.get(&result.id);
        let standard = definition.and_then(|value| value.standard_profile.as_deref()).unwrap_or("Benchmark-defined requirement");
        let verification = definition.map(check_verification).unwrap_or_else(|| "Check definition unavailable in this legacy report".into());
        format!("<tr class={class}><td><strong>{mark}</strong><br><small>{treatment}</small></td><th scope=row>{}<br><small>{}</small></th><td>{}<br><small><code>{}</code></small></td><td>{}</td></tr>", esc(&result.description), esc(standard), esc(&verification), esc(&result.id), esc(&result.detail))
    }).collect::<String>());
    let mut artifacts = Vec::new();
    collect_artifacts(&task.work_dir, &task.work_dir, 0, &mut artifacts);
    let final_path = final_stl_path(&task.work_dir);
    let duplicates = final_path.as_ref().map_or(0, |selected| {
        report
            .tasks
            .iter()
            .enumerate()
            .filter(|(other_index, other)| {
                *other_index != index
                    && final_stl_path(&other.work_dir)
                        .is_some_and(|candidate| same_file(selected, &candidate))
            })
            .count()
    });
    let duplicate_warning = if duplicates > 0 {
        format!(
            "<blockquote><strong>Duplicate geometry</strong><br>Same final STL as {duplicates} other prompts.</blockquote>"
        )
    } else {
        String::new()
    };
    let material_name = design_material(&task.work_dir);
    let material_matches = material_name
        .as_deref()
        .zip(brief.as_deref())
        .is_none_or(|(material, prompt)| prompt_mentions_material(prompt, material));
    let material = material_name.as_deref().map_or_else(
        || "<p><strong>Material</strong><br>Not recorded</p>".to_owned(),
        |name| {
            format!(
                "<p><strong>Material</strong><br>{} {}</p>",
                esc(name),
                if material_matches {
                    ""
                } else {
                    "<mark>Prompt mismatch</mark>"
                }
            )
        },
    );
    let pbr = pbr_for_material(material_name.as_deref());
    let final_stl = final_path
        .as_ref()
        .and_then(|path| path.strip_prefix(&task.work_dir).ok())
        .map(|path| path.to_string_lossy().into_owned());
    let model = final_stl.map_or_else(|| "<div class=model-empty><strong>Final STL not recorded</strong><p>This run did not produce an STL artifact.</p></div>".to_owned(), |relative| {
        let href = format!("/artifact/{model_id}/{}/{index}/{relative}", kind.id());
        format!("<div id=stl-viewer data-src='{href}' data-color='{}' data-metalness='{}' data-roughness='{}' aria-label='Interactive final STL viewer' aria-busy=true>Loading model…</div><footer><small>Drag to rotate · scroll to zoom</small> · <a href='{href}' download>Download STL</a></footer>", pbr.0, pbr.1, pbr.2)
    });
    let inspector = artifact_inspector(model_id, kind, index, &task.work_dir, &artifacts, model);
    let options = report
        .tasks
        .iter()
        .enumerate()
        .map(|(position, candidate)| {
            let name = candidate.report.as_ref().map_or_else(
                || {
                    candidate
                        .task_file
                        .file_stem()
                        .and_then(|value| value.to_str())
                        .unwrap_or("Unscored prompt")
                },
                |value| value.task_id.as_str(),
            );
            format!(
                "<option value='/prompt/{model_id}/{}/{position}' {}>{:02} · {}</option>",
                kind.id(),
                if position == index { "selected" } else { "" },
                position + 1,
                esc(name)
            )
        })
        .collect::<String>();
    let previous = (index > 0).then(|| format!("/prompt/{model_id}/{}/{}", kind.id(), index - 1));
    let next = (index + 1 < report.tasks.len())
        .then(|| format!("/prompt/{model_id}/{}/{}", kind.id(), index + 1));
    let nav_link = |href: Option<String>, label: &str| {
        href.map_or_else(
            || format!("<span class=nav-disabled>{label}</span>"),
            |href| format!("<a href='{href}'>{label}</a>"),
        )
    };
    let prompt = brief.as_deref().map_or_else(
        || "<p><strong>Prompt not recorded</strong></p>".to_owned(),
        |value| format!("<p>{}</p>", esc(value).replace('\n', "<br>")),
    );
    let rigor = rigor_ledger(task);
    let decisions = design_decisions(&task.work_dir);
    Some(format!(
        "<!doctype html><html lang=en><head><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'><meta name=color-scheme content='light dark'><title>{} · {}</title><style>{CSS}{PROMPT_INSPECTOR_CSS}</style></head><body><header class=container><nav><ul><li><a href='/#benchmark-{}'>← Leaderboard</a></li><li><strong>{}</strong></li></ul><ul><li><small>{} · {}</small></li></ul></nav></header><main class=container><nav aria-label='Prompt carousel'><ul><li>{}</li></ul><ul><li><label>Prompt {}/{}<select id=prompt-select>{options}</select></label></li></ul><ul><li>{}</li></ul></nav><hgroup><p>{} · Prompt {}/{}</p><h1>{}</h1></hgroup><p><mark>{}</mark> &nbsp; {passed}/{deterministic} deterministic &nbsp; {proxies} proxy &nbsp; {reviews} review</p><section>{inspector}</section><section class=grid><article><header><h2>Manufacturing context</h2></header>{material}{duplicate_warning}</article><article><header><h2>Prompt</h2></header>{prompt}</article></section>{decisions}<section><h2>Rigor ledger</h2>{rigor}</section><section><h2>Standards and checks</h2><p>Each row names the requirement, verifier, recorded evidence, and exact score treatment.</p><div class=overflow-auto><table class=striped><thead><tr><th>Score treatment</th><th>Requirement / standard</th><th>Verification method</th><th>Evidence</th></tr></thead><tbody>{checks}</tbody></table></div></section></main><script type=module>{PROMPT_PAGE_JS}</script></body></html>",
        esc(&task_id),
        kind.name(),
        kind.id(),
        esc(model_name),
        kind.name(),
        esc(harness),
        nav_link(previous, "← Previous"),
        index + 1,
        report.tasks.len(),
        nav_link(next, "Next →"),
        kind.name(),
        index + 1,
        report.tasks.len(),
        esc(&task_id),
        state.1
    ))
}

fn design_decisions(work_dir: &std::path::Path) -> String {
    let stock = match std::fs::read_to_string(work_dir.join("starting-stock.json")) {
        Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(value) => {
                let dimensions = value.get("size_mm").and_then(|v| v.as_array()).map(|values| values.iter().map(|v| v.as_f64().map_or_else(|| "?".into(), |n| format!("{n}"))).collect::<Vec<_>>().join(" × ")).unwrap_or_else(|| "Not recorded".into());
                format!("<article><header><h3>Starting stock · generative</h3></header><p><strong>{dimensions} mm</strong><br>{}<br><small>{}</small></p><footer><code>{}</code></footer></article>", esc(value.get("material").and_then(|v| v.as_str()).unwrap_or("Not recorded")), esc(value.get("basis").and_then(|v| v.as_str()).unwrap_or("No basis recorded")), esc(value.get("model").and_then(|v| v.as_str()).unwrap_or("Model not recorded")))
            }
            Err(error) => format!("<article><header><h3>Starting stock · generative</h3></header><p>Invalid extraction artifact: {}</p></article>", esc(&error.to_string())),
        },
        Err(_) => "<article><header><h3>Starting stock · generative</h3></header><p>Not recorded</p></article>".into(),
    };
    let trace = match std::fs::read_to_string(work_dir.join("decisions.json")) {
        Ok(text) => match serde_json::from_str::<Vec<serde_json::Value>>(&text) {
            Ok(values) if !values.is_empty() => {
                let rows = values.iter().map(|value| {
                    let confidence = value.get("confidence").and_then(|v| v.as_f64());
                    let gate = confidence.map_or("Not recorded", |score| if score >= 0.70 { "Accepted" } else { "Below 0.70" });
                    format!("<tr><th scope=row><code>{}</code></th><td>{}</td><td>{}</td><td>{gate}</td></tr>", esc(value.get("key").and_then(|v| v.as_str()).unwrap_or("Unknown")), esc(value.get("chosen").and_then(|v| v.as_str()).unwrap_or("Not recorded")), confidence.map_or_else(|| "—".into(), |v| format!("{v:.2}")))
                }).collect::<String>();
                format!(
                    "<div class=overflow-auto><table class=striped><thead><tr><th>Question</th><th>Applied choice</th><th>Confidence</th><th>Gate</th></tr></thead><tbody>{rows}</tbody></table></div>"
                )
            }
            Ok(_) => "<p>No bounded decisions recorded.</p>".into(),
            Err(error) => format!("<p>Invalid decision trace: {}</p>", esc(&error.to_string())),
        },
        Err(_) => "<p>Not recorded</p>".into(),
    };
    format!(
        "<section><h2>Design decisions</h2><div class=grid>{stock}<article><header><h3>Geometry · RLCD</h3></header>{trace}</article></div></section>"
    )
}

fn artifact_inspector(
    model_id: &str,
    kind: BenchmarkKind,
    index: usize,
    work_dir: &std::path::Path,
    artifacts: &[std::path::PathBuf],
    model: String,
) -> String {
    let mut views: Vec<(String, String)> = Vec::new();
    if final_stl_path(work_dir).is_some() {
        views.push(("3D model".into(), model));
    }
    let mut paths = artifacts.iter().collect::<Vec<_>>();
    paths.sort();
    for path in paths {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        let name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Artifact");
        let lower = name.to_ascii_lowercase();
        let relative = path
            .strip_prefix(work_dir)
            .expect("artifact below work dir");
        let relative_text = relative.to_string_lossy();
        let href = format!("/artifact/{model_id}/{}/{index}/{relative_text}", kind.id());
        if extension == "pdf" {
            views.push((format!("📄 {name}"), format!("<div class=inspection-stage><iframe src='{href}#view=FitH&toolbar=1&navpanes=0' title='{} PDF' loading=lazy></iframe></div><footer><span>Scroll normally through every page.</span><span><a href='{href}' target=_blank>Open separately</a> · <a href='{href}' download>Download PDF</a></span></footer>", esc(name))));
        } else if extension == "glb" {
            views.push((
                format!("3D board · {name}"),
                format!("<div class='inspection-stage glb-stage' data-glb-viewer data-src='{href}' aria-label='Interactive 3D board model' aria-busy=true>Loading board model…</div><footer><span>Drag to rotate · scroll to zoom.</span><a href='{href}' download>Download GLB</a></footer>"),
            ));
        } else if matches!(extension, "png" | "jpg" | "jpeg" | "svg")
            && (lower.contains("schematic") || lower.contains("board") || lower.contains("pcb"))
        {
            let label = if lower.contains("schematic") {
                "Schematic"
            } else {
                "PCB layout"
            };
            views.push((label.into(), format!("<div class='inspection-stage image-stage'><img src='{href}' alt='{}'></div><footer><span>Generated {label} artifact.</span><span><a href='{href}' target=_blank>Open full size</a> · <a href='{href}' download>Download</a></span></footer>", esc(name))));
        }
    }
    if views.is_empty() {
        return "<article><header><h2>Work product</h2></header><p>No inspectable model, schematic, layout, or PDF was recorded.</p></article>".into();
    }
    let tabs = views.iter().enumerate().map(|(position, (label, _))| format!("<button type=button role=tab aria-selected={} aria-controls=artifact-view-{position} id=artifact-tab-{position} data-artifact-tab=artifact-view-{position}>{}</button>", position == 0, esc(label))).collect::<String>();
    let panels = views.into_iter().enumerate().map(|(position, (_, body))| format!("<div role=tabpanel id=artifact-view-{position} aria-labelledby=artifact-tab-{position} data-artifact-panel {}>{body}</div>", if position == 0 { "" } else { "hidden" })).collect::<String>();
    format!(
        "<article class=artifact-inspector id=artifact-inspector><header><div><h2>Work product</h2><small>Model, drawings, schematic, and board evidence from this prompt.</small></div><button type=button class=outline data-fullscreen-inspector>Fullscreen</button></header><nav role=tablist aria-label='Work product'>{tabs}</nav>{panels}</article>"
    )
}

fn rigor_ledger(task: &crate::SuiteTaskResult) -> String {
    let fallback = task_metadata(&task.task_file);
    let metadata = task.task_metadata.as_ref().or(fallback.as_ref());
    let Some(metadata) = metadata else {
        let detail = task.metadata_error.as_deref().map_or(
            "This legacy result has no embedded task metadata.",
            |error| error,
        );
        return format!(
            "<article><header><strong>Metadata unavailable</strong></header><p>{}</p><footer>It does not add or remove score.</footer></article>",
            esc(detail)
        );
    };
    let validity = match metadata.validate() {
        Ok(()) => "Complete",
        Err(_) => "Incomplete",
    };
    let capabilities = metadata
        .capabilities
        .iter()
        .map(|value| format!("<li><code>{}</code></li>", esc(value)))
        .collect::<String>();
    let failure_modes = metadata
        .expected_failure_modes
        .iter()
        .map(|value| format!("<li>{}</li>", esc(value)))
        .collect::<String>();
    let source = metadata.source.as_deref().unwrap_or("Not declared");
    format!(
        "<div class=grid><article><header><strong>{validity} benchmark contract</strong></header><dl><dt>Difficulty</dt><dd>{}</dd><dt>Dataset split</dt><dd>{}</dd><dt>Oracle version</dt><dd><code>{}</code></dd><dt>Source</dt><dd>{}</dd></dl></article><article><header><strong>Capabilities under test</strong></header><ul>{capabilities}</ul><strong>Designed to catch</strong><ul>{failure_modes}</ul></article><article><header><strong>How this affects scoring</strong></header><p>Pass/fail oracle results determine this prompt’s objective score.</p><p>Capabilities group the prompt into capability-macro reporting. Difficulty, split, source, oracle version, and expected failure modes are provenance—they do not award points.</p></article></div>",
        esc(&format!("{:?}", metadata.difficulty)),
        esc(&format!("{:?}", metadata.split)),
        esc(&metadata.oracle_version),
        esc(source),
    )
}

fn task_metadata(path: &std::path::Path) -> Option<TaskMetadata> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str::<toml::Value>(&text).ok())
        .and_then(|task| task.get("metadata").cloned())
        .and_then(|metadata| metadata.try_into().ok())
}

fn is_proxy(result: &crate::CriterionResult) -> bool {
    result.detail.to_ascii_lowercase().contains("proxy")
}

fn final_stl_path(work_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut paths = Vec::new();
    collect_artifacts(work_dir, work_dir, 0, &mut paths);
    paths
        .into_iter()
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("stl"))
        .max_by_key(|path| stl_rank(path))
}

fn same_file(left: &std::path::Path, right: &std::path::Path) -> bool {
    std::fs::metadata(left).ok().map(|value| value.len())
        == std::fs::metadata(right).ok().map(|value| value.len())
        && std::fs::read(left).ok() == std::fs::read(right).ok()
}

fn design_material(work_dir: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(work_dir.join("design.ron")).ok()?;
    let intent = text.rsplit_once("intent: ManufacturingIntent(")?.1;
    let material = intent.split_once("material: Some(IntentText(")?.1;
    let quoted = material.split_once("text: \"")?.1;
    Some(quoted.split_once('"')?.0.to_owned())
}

fn pbr_for_material(material: Option<&str>) -> (&'static str, f32, f32) {
    let name = material.unwrap_or("").to_ascii_lowercase();
    if name.contains("brass") || name.contains("bronze") {
        ("#b8873d", 0.9, 0.28)
    } else if name.contains("stainless") {
        ("#aeb5ba", 0.95, 0.24)
    } else if name.contains("al ") || name.contains("aluminium") || name.contains("aluminum") {
        ("#aeb7c2", 0.88, 0.32)
    } else if name.contains("titanium") {
        ("#8d9299", 0.9, 0.38)
    } else if name.contains("delrin") || name.contains("acetal") {
        ("#222629", 0.0, 0.42)
    } else if name.contains("iron") || name.contains("steel") {
        ("#626a70", 0.85, 0.48)
    } else {
        ("#87939a", 0.0, 0.62)
    }
}

fn prompt_mentions_material(prompt: &str, material: &str) -> bool {
    let prompt = prompt.to_ascii_lowercase();
    let material = material.to_ascii_lowercase();
    let families: &[(&[&str], &[&str])] = &[
        (
            &["6061", "al ", "aluminum", "aluminium"],
            &["6061", "aluminum", "aluminium"],
        ),
        (&["304", "stainless"], &["304", "stainless"]),
        (&["brass"], &["brass"]),
        (&["titanium"], &["titanium"]),
        (&["delrin", "acetal"], &["delrin", "acetal"]),
        (&["cast iron"], &["cast iron"]),
    ];
    families
        .iter()
        .find(|(aliases, _)| aliases.iter().any(|alias| material.contains(alias)))
        .is_none_or(|(_, expected)| expected.iter().any(|alias| prompt.contains(alias)))
}

fn stl_rank(path: &std::path::Path) -> (u8, u64) {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if matches!(stem, "final" | "model" | "output") {
        return (2, u64::MAX);
    }
    (
        1,
        stem.strip_prefix("step-")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
    )
}

fn task_check_definitions(
    task: &crate::SuiteTaskResult,
) -> std::collections::BTreeMap<String, crate::CriterionDefinition> {
    if !task.criterion_definitions.is_empty() {
        return task.criterion_definitions.clone();
    }
    task_check_definitions_from_file(&task.task_file)
}

fn task_check_definitions_from_file(
    path: &std::path::Path,
) -> std::collections::BTreeMap<String, crate::CriterionDefinition> {
    let Some(rubric) = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str::<toml::Value>(&text).ok())
        .and_then(|task| task.get("rubric").and_then(toml::Value::as_array).cloned())
    else {
        return std::collections::BTreeMap::new();
    };
    rubric
        .into_iter()
        .filter_map(|criterion| {
            let id = criterion.get("id")?.as_str()?.to_owned();
            let table = criterion.as_table()?;
            let kind = table.get("kind")?.as_str()?.to_owned();
            let standard_profile = table
                .get("profile")
                .or_else(|| table.get("standard"))
                .and_then(toml::Value::as_str)
                .map(str::to_owned);
            let parameters = table
                .iter()
                .filter(|(key, _)| {
                    !matches!(
                        key.as_str(),
                        "id" | "description" | "kind" | "profile" | "standard"
                    )
                })
                .map(|(key, value)| (key.clone(), value.to_string()))
                .collect();
            Some((
                id,
                crate::CriterionDefinition {
                    kind,
                    standard_profile,
                    parameters,
                },
            ))
        })
        .collect()
}

fn check_heading(definition: &crate::CriterionDefinition) -> String {
    definition
        .standard_profile
        .clone()
        .unwrap_or_else(|| check_name(&definition.kind))
}

fn check_verification(definition: &crate::CriterionDefinition) -> String {
    let mut text = check_name(&definition.kind);
    if !definition.parameters.is_empty() {
        text.push_str(": ");
        text.push_str(
            &definition
                .parameters
                .iter()
                .map(|(key, value)| format!("{}={value}", check_name(key)))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    text
}

fn check_name(name: &str) -> String {
    match name {
        "stages_pass" => "Pipeline completion".to_owned(),
        "min_decision_confidence" => "Decision confidence".to_owned(),
        "conforms" => "Design conformance".to_owned(),
        "subjective" => "Visual review".to_owned(),
        other => other
            .split('_')
            .map(|word| {
                let mut chars = word.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_uppercase().collect::<String>() + chars.as_str()
                })
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn artifact_links(
    model_id: Option<&str>,
    kind: BenchmarkKind,
    index: usize,
    work_dir: &std::path::Path,
) -> String {
    let mut paths = Vec::new();
    collect_artifacts(work_dir, work_dir, 0, &mut paths);
    paths.sort();
    if paths.is_empty() {
        return "<section class=prompt><h3>Output</h3><p>Artifacts appear here as they are written.</p></section>".to_owned();
    }
    let mut groups = std::collections::BTreeMap::<&'static str, Vec<String>>::new();
    for path in &paths {
        let relative = path.strip_prefix(work_dir).expect("collected below root");
        let relative_text = relative.to_string_lossy();
        let href = match model_id {
            Some(model) => format!("/artifact/{model}/{}/{index}/{relative_text}", kind.id()),
            None => format!("/artifact/{}/{index}/{relative_text}", kind.id()),
        };
        let extension = path.extension().and_then(|v| v.to_str()).unwrap_or("");
        let link = if matches!(extension, "png" | "jpg" | "jpeg" | "svg") {
            format!(
                "<a href='{href}' target=_blank><img src='{href}' alt='{} output' style='max-width:100%;max-height:360px;object-fit:contain'><br>{}</a>",
                esc(&relative_text),
                esc(&relative_text)
            )
        } else {
            format!("<a href='{href}' target=_blank>{}</a>", esc(&relative_text))
        };
        let category = artifact_category(relative, extension);
        groups.entry(category).or_default().push(link);
    }
    let grouped = groups.into_iter().map(|(category, links)| format!("<details class=file-group {}><summary><strong>{category}</strong><span>{} file(s)</span></summary><div>{}</div></details>", category.to_ascii_lowercase().replace(' ', "-"), links.len(), links.join(""))).collect::<String>();
    format!("<section class=artifacts><h3>Files</h3>{grouped}</section>")
}

fn artifact_category(path: &std::path::Path, extension: &str) -> &'static str {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if extension == "pdf" {
        "Drawings and documents"
    } else if matches!(extension, "stl" | "step" | "stp" | "glb" | "gltf") {
        "3D models"
    } else if name.contains("timeline") || name.starts_with("step-") {
        "Build timeline"
    } else if matches!(extension, "ron") || name.contains("design") || name.contains("spec") {
        "Design source"
    } else if matches!(extension, "png" | "jpg" | "jpeg" | "svg") {
        "Visual previews"
    } else {
        "Reports and data"
    }
}

fn collect_artifacts(
    root: &std::path::Path,
    dir: &std::path::Path,
    depth: usize,
    paths: &mut Vec<std::path::PathBuf>,
) {
    if depth > 2 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_artifacts(root, &path, depth + 1, paths);
        } else if path.strip_prefix(root).is_ok()
            && matches!(
                path.extension().and_then(|v| v.to_str()).unwrap_or(""),
                "png"
                    | "jpg"
                    | "jpeg"
                    | "svg"
                    | "pdf"
                    | "stl"
                    | "step"
                    | "stp"
                    | "glb"
                    | "gltf"
                    | "json"
                    | "ron"
                    | "csv"
            )
        {
            paths.push(path);
        }
    }
}

fn task_brief(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str::<toml::Value>(&text).ok())
        .and_then(|task| {
            task.get("brief")
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
        })
}

fn task_prompt(brief: Option<&str>) -> String {
    let Some(brief) = brief else {
        return String::new();
    };
    format!(
        "<section class=prompt><h3>Prompt</h3><pre>{}</pre></section>",
        esc(brief)
    )
}

fn prompt_preview(brief: &str) -> String {
    let compact = brief.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 150 {
        compact
    } else {
        format!("{}…", compact.chars().take(149).collect::<String>())
    }
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const JS: &str = r#"
const tabs=[...document.querySelectorAll('.bench-tab:not(:disabled)')];
const panels=[...document.querySelectorAll('.bench-panel')];
function select(id){tabs.forEach(t=>t.setAttribute('aria-selected',String(t.dataset.bench===id)));panels.forEach(p=>p.hidden=p.dataset.bench!==id);history.replaceState(null,'','#'+id)}
tabs.forEach((tab,i)=>{tab.addEventListener('click',()=>select(tab.dataset.bench));tab.addEventListener('keydown',e=>{if(e.key==='ArrowLeft'||e.key==='ArrowRight'){e.preventDefault();const d=e.key==='ArrowRight'?1:-1;const next=tabs[(i+d+tabs.length)%tabs.length];next.focus();select(next.dataset.bench)}})});
const requested=location.hash.slice(1);if(tabs.some(t=>t.dataset.bench===requested))select(requested);
document.querySelector('#refresh').addEventListener('click',()=>location.reload());
document.querySelector('#failed-only').addEventListener('change',e=>document.body.classList.toggle('failed-only',e.target.checked));
let revision=null;setInterval(async()=>{if(!document.querySelector('#live').checked)return;try{const next=await fetch('/api/revision',{cache:'no-store'}).then(r=>r.ok?r.text():null);if(revision===null)revision=next;else if(next&&next!==revision)location.reload()}catch(_){}},1000);
"#;

const LEADERBOARD_JS: &str = r#"
const savedTheme=localStorage.getItem('eval-theme');if(savedTheme)document.documentElement.dataset.theme=savedTheme;
const themeToggle=document.querySelector('#theme-toggle');
const activeTheme=()=>document.documentElement.dataset.theme||(matchMedia('(prefers-color-scheme:dark)').matches?'dark':'light');
const updateThemeLabel=()=>{if(themeToggle)themeToggle.textContent=activeTheme()==='dark'?'Light':'Dark'};
themeToggle?.addEventListener('click',()=>{const next=activeTheme()==='dark'?'light':'dark';document.documentElement.dataset.theme=next;localStorage.setItem('eval-theme',next);updateThemeLabel()});updateThemeLabel();
document.querySelector('#refresh')?.addEventListener('click',()=>location.reload());
let revision=null;setInterval(async()=>{const live=document.querySelector('#live');if(live&&!live.checked)return;try{const next=await fetch('/api/revision',{cache:'no-store'}).then(r=>r.ok?r.text():null);if(revision===null)revision=next;else if(next&&next!==revision)location.reload()}catch(_){}},1000);
const benchmarkTabs=[...document.querySelectorAll('.benchmark-tab:not(:disabled)')],primerPanels=[...document.querySelectorAll('.benchmark-primer-panel')];
function selectBenchmark(id){benchmarkTabs.forEach(tab=>tab.setAttribute('aria-selected',String(tab.dataset.bench===id)));primerPanels.forEach(panel=>panel.hidden=panel.dataset.bench!==id);const option=[...bench.options].find(option=>option.value===id);if(option){bench.value=id;drawPareto()}history.replaceState(null,'','#benchmark-'+id)}
benchmarkTabs.forEach((tab,index)=>{tab.addEventListener('click',()=>selectBenchmark(tab.dataset.bench));tab.addEventListener('keydown',event=>{if(event.key==='ArrowLeft'||event.key==='ArrowRight'){event.preventDefault();const direction=event.key==='ArrowRight'?1:-1;const next=benchmarkTabs[(index+direction+benchmarkTabs.length)%benchmarkTabs.length];next.focus();selectBenchmark(next.dataset.bench)}})});
document.querySelectorAll('.leader-table thead [data-column]').forEach(control=>control.addEventListener('click',event=>{event.preventDefault();const body=document.querySelector('.leader-table tbody');const rows=[...body.rows];const column=Number(control.dataset.column);rows.sort((a,b)=>Number(b.cells[column].dataset.score)-Number(a.cells[column].dataset.score));rows.forEach((row,index)=>{row.cells[0].textContent=index+1;body.append(row)})}));
const svg=document.querySelector('#pareto'),bench=document.querySelector('#pareto-bench'),axis=document.querySelector('#pareto-x'),empty=document.querySelector('#pareto-empty');axis.remove();
function drawPareto(){const key=bench.value;const points=paretoData.map(model=>{const result=key==='overall'?model.overall:model.benchmarks[key];return result&&result.cost!=null?{name:model.name,score:result.score,x:result.cost}:null}).filter(Boolean).sort((a,b)=>a.x-b.x);svg.replaceChildren();empty.hidden=points.length>0;svg.toggleAttribute('hidden',points.length===0);if(!points.length)return;const ns='http://www.w3.org/2000/svg',left=72,right=970,top=24,bottom=374,maxX=Math.max(...points.map(p=>p.x))*1.08||1;const sx=x=>left+x/maxX*(right-left),sy=y=>bottom-y/100*(bottom-top);const add=(tag,attrs,text)=>{const node=document.createElementNS(ns,tag);Object.entries(attrs).forEach(([k,v])=>node.setAttribute(k,v));if(text)node.textContent=text;svg.append(node);return node};add('line',{x1:left,y1:top,x2:left,y2:bottom,stroke:'currentColor','stroke-width':'2'});add('line',{x1:left,y1:bottom,x2:right,y2:bottom,stroke:'currentColor','stroke-width':'2'});[0,25,50,75,100].forEach(v=>{add('line',{x1:left,y1:sy(v),x2:right,y2:sy(v),stroke:'currentColor',opacity:'.15'});add('text',{x:left-12,y:sy(v)+4,'text-anchor':'end',fill:'currentColor'},v)});add('text',{x:(left+right)/2,y:420,'text-anchor':'middle',fill:'currentColor'},'Average price per task (USD) →');add('text',{x:18,y:(top+bottom)/2,transform:`rotate(-90 18 ${(top+bottom)/2})`,'text-anchor':'middle',fill:'currentColor'},'Score ↑');let best=-1;const frontier=points.filter(p=>{if(p.score>best){best=p.score;return true}return false});add('polyline',{points:frontier.map(p=>`${sx(p.x)},${sy(p.score)}`).join(' '),fill:'none',stroke:'var(--pico-primary)','stroke-width':'4'});points.forEach(p=>{const group=add('g',{tabindex:'0'});const circle=document.createElementNS(ns,'circle');circle.setAttribute('cx',sx(p.x));circle.setAttribute('cy',sy(p.score));circle.setAttribute('r',frontier.includes(p)?7:5);circle.setAttribute('fill','var(--pico-primary)');group.append(circle);const title=document.createElementNS(ns,'title');title.textContent=`${p.name}: ${p.score.toFixed(1)} score, $${p.x.toFixed(4)} / task`;group.append(title);const label=document.createElementNS(ns,'text');label.setAttribute('x',sx(p.x)+10);label.setAttribute('y',sy(p.score)-10);label.setAttribute('fill','currentColor');label.textContent=p.name;group.append(label)})}
bench.addEventListener('change',drawPareto);const requested=location.hash.replace('#benchmark-','');if(benchmarkTabs.some(tab=>tab.dataset.bench===requested))selectBenchmark(requested);else drawPareto();
"#;

// Pico CSS v2.1.1 provides the accessible, class-light UI foundation. It is
// vendored so the tailnet viewer remains complete without public internet.
// Source: https://github.com/picocss/pico (MIT)
const CSS: &str = include_str!("../assets/pico.min.css");

const PROMPT_INSPECTOR_CSS: &str = r#"
.artifact-inspector>header{display:flex;align-items:center;justify-content:space-between;gap:1rem}.artifact-inspector>header h2{margin-bottom:.15rem}.artifact-inspector>nav[role=tablist]{display:flex;gap:.45rem;overflow-x:auto;padding:.65rem 0;border-bottom:1px solid var(--pico-muted-border-color)}.artifact-inspector [role=tab]{width:auto;margin:0;padding:.55rem .85rem;white-space:nowrap}.artifact-inspector [role=tab][aria-selected=false]{background:transparent;color:var(--pico-muted-color)}.inspection-stage{min-height:620px;background:#151a20}.inspection-stage iframe{display:block;width:100%;height:min(78vh,980px);min-height:620px;border:0;background:#d7d9dc}.image-stage{display:grid;place-items:center;overflow:auto;padding:1rem;background:#30343a}.image-stage img{display:block;max-width:none;width:auto;min-width:min(100%,900px);height:auto}.glb-stage canvas{display:block;width:100%;height:100%}.artifact-inspector [role=tabpanel]>footer{display:flex;justify-content:space-between;gap:1rem;padding-top:.75rem}.artifact-inspector:fullscreen{overflow:auto;padding:1rem;background:var(--pico-background-color)}.artifact-inspector:fullscreen .inspection-stage,.artifact-inspector:fullscreen .inspection-stage iframe{height:calc(100vh - 11rem);min-height:0}.artifact-inspector:fullscreen #stl-viewer{height:calc(100vh - 11rem);min-height:0}@media(max-width:700px){.inspection-stage,.inspection-stage iframe{min-height:480px}.artifact-inspector [role=tabpanel]>footer{align-items:flex-start;flex-direction:column}}
"#;

const SCORE_EXPLAINER_CSS: &str = r#"
.score-method{margin:1rem 0 2rem}.score-method>.grid{padding-top:1rem}.score-method p{margin:0}.score-cell{display:flex;flex-direction:column;min-width:7rem;text-decoration:none}.score-cell>span:first-child{font-size:1.1rem;font-variant-numeric:tabular-nums}.score-cell small{color:var(--pico-muted-color);white-space:nowrap}
"#;

#[allow(dead_code)]
const _REMOVED_CUSTOM_CSS: &str = concat!(
    include_str!("../assets/pico.min.css"),
    r#"
:root{--paper:#f4f6f2;--surface:#fff;--ink:#202621;--muted:#687169;--rule:#c9d0c9;--pcb:#b94c21;--cad:#245fa3;--cam:#6e4f92;--dfm:#27715b;color-scheme:light}*{box-sizing:border-box}body{margin:0;background:var(--paper);color:var(--ink);font:15px/1.5 "Aptos","Helvetica Neue",Arial,sans-serif}.site-head{position:sticky;top:0;z-index:5;display:flex;align-items:center;justify-content:space-between;gap:24px;padding:14px clamp(18px,4vw,56px);border-bottom:1px solid var(--rule);background:rgba(244,246,242,.96);backdrop-filter:blur(10px)}.wordmark{color:var(--ink);font-weight:800;letter-spacing:-.02em;text-decoration:none}.wordmark span{color:var(--muted);font-weight:500}.bench-switcher{display:flex;gap:3px;padding:3px;border:1px solid var(--rule);background:#e7ebe6}.bench-tab{display:flex;align-items:center;gap:7px;min-height:36px;padding:6px 10px;border:0;background:transparent;color:var(--muted);font:600 13px/1 inherit;cursor:pointer}.bench-tab[aria-selected=true]{background:var(--surface);color:var(--ink);box-shadow:0 1px 3px #1b281b1f}.bench-tab:disabled{cursor:not-allowed;opacity:.35}.bench-tab:focus-visible{outline:3px solid #111;outline-offset:2px}main{max-width:1440px;margin:auto;padding:clamp(28px,5vw,72px) clamp(18px,4vw,56px)}.bench-panel.pcb{--accent:var(--pcb)}.bench-panel.cad{--accent:var(--cad)}.bench-panel.cam{--accent:var(--cam)}.bench-panel.dfm{--accent:var(--dfm)}.hero{display:grid;grid-template-columns:minmax(280px,1fr) minmax(380px,1fr);gap:48px;align-items:end;padding-bottom:32px;border-bottom:3px solid var(--accent)}.identity{display:flex;align-items:center;gap:10px;margin:0 0 12px;color:var(--accent);font-size:18px;font-weight:750}.identity span{font-size:25px}.hero h1{margin:0;font-size:clamp(52px,9vw,112px);line-height:.82;letter-spacing:-.075em}.hero h1 small{display:block;margin-top:20px;color:var(--muted);font-size:16px;font-weight:550;letter-spacing:0}.hero dl{display:grid;grid-template-columns:repeat(4,1fr);margin:0}.hero dl div{padding:0 14px;border-left:1px solid var(--rule)}dt{color:var(--muted);font-size:12px}dd{margin:3px 0 0;font-size:24px;font-weight:750}.task-list{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:12px;margin-top:28px}.task{border:1px solid var(--rule);background:var(--surface)}.task.failed{border-left:4px solid #b7352d}.task.error{border-left:4px solid #b77719;padding:18px}.task details>summary{display:flex;justify-content:space-between;align-items:center;gap:18px;padding:17px 18px;cursor:pointer;list-style:none}.task summary::-webkit-details-marker{display:none}.task summary span{display:flex;min-width:0;flex-direction:column}.task summary strong{overflow:hidden;text-overflow:ellipsis}.task summary small{color:var(--muted)}.task summary b{color:var(--accent);font-size:12px}.criteria{overflow:auto;border-top:1px solid var(--rule)}table{width:100%;border-collapse:collapse}th,td{padding:10px 12px;border-bottom:1px solid #e5e9e4;text-align:left;vertical-align:top}th{font-weight:650}td:last-child{color:var(--muted)}.verdict{display:inline-block;min-width:48px;font-size:12px;font-weight:750}.verdict.pass{color:#27715b}.verdict.fail{color:#b7352d}.verdict.human{color:#8b651c}.empty{display:grid;min-height:60vh;place-items:center;align-content:center;text-align:center}.empty span{font-size:52px;filter:grayscale(1)}.empty h1{margin:10px 0 0}.empty p{color:var(--muted)}[hidden]{display:none!important}@media(max-width:900px){.site-head{align-items:flex-start}.wordmark span{display:none}.bench-tab span:last-child{display:none}.hero{grid-template-columns:1fr}.hero dl{grid-template-columns:repeat(2,1fr);gap:16px}.task-list{grid-template-columns:1fr}}@media(prefers-reduced-motion:reduce){*{scroll-behavior:auto!important}}
"#,
    r#"
/* Product-specific composition on top of Pico's controls, typography, and states. */
:root{--pico-font-family-sans-serif:Inter,ui-sans-serif,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;--pico-font-size:94%;--pico-border-radius:.7rem;--pico-primary:#3659db;--pico-primary-hover:#2946b9;--pico-primary-focus:rgba(54,89,219,.2);--paper:#f6f7fb;--surface:#fff;--ink:#18202b;--muted:#697386;--rule:#dfe3ea}
body{background:linear-gradient(180deg,#fafbfe 0,#f4f6fa 280px);letter-spacing:-.005em}
.site-head{background:rgba(255,255,255,.9);border-color:#e6e9ef;box-shadow:0 1px 0 rgba(23,31,44,.04);padding-block:11px}
.wordmark{font-size:.93rem;letter-spacing:-.01em}.monitor-controls{display:flex;align-items:center;gap:.7rem}.monitor-controls button{width:auto;margin:0;padding:.42rem .72rem;font-size:.78rem}.monitor-controls label{display:flex;align-items:center;gap:.4rem;margin:0;font-size:.78rem}.monitor-controls input{margin:0}
.benchmark-switcher,.bench-switcher{gap:4px;padding:4px;border:0;border-radius:.8rem;background:#eef1f6;box-shadow:inset 0 0 0 1px #e2e6ed}.benchmark-tab,.bench-tab{margin:0;border-radius:.55rem}.benchmark-tab[aria-selected=true],.bench-tab[aria-selected=true]{box-shadow:0 1px 2px rgba(25,35,55,.12),0 4px 12px rgba(25,35,55,.06)}
main{padding-top:2.3rem}.leader-head{padding-bottom:1.4rem}.leader-head h1{font-size:clamp(2rem,4vw,3.15rem);font-weight:720;letter-spacing:-.045em}.leader-head>span{padding:.3rem .65rem;border:1px solid var(--rule);border-radius:999px;background:var(--surface);font-size:.75rem}
.benchmark-primer-panel,.leader-wrap,.pareto-shell,.task,.prompt-copy,.standards-list,.model-stage{border:1px solid #e2e6ed;border-radius:.8rem;box-shadow:0 1px 2px rgba(20,29,43,.04),0 10px 28px rgba(20,29,43,.035);overflow:hidden}
.benchmark-primer-panel{border-top-width:3px}.leader-wrap{border-top-width:1px}.leader-table thead th{background:#f7f8fb}.leader-table tbody tr:hover{background:#fafbfe}.leader-table tbody tr:last-child>*{border-bottom:0}
.task details>summary{background:#fff}.task details[open]>summary{background:#fafbfe}.task summary b,.verdict,.result-mark{letter-spacing:.035em}.open-prompt{border:0;border-radius:.5rem;background:#eef2ff;color:#2946b9}
.pareto-shell{padding:1.35rem}.pareto-shell select,.prompt-carousel select{margin:0}.evidence>header{border-bottom:1px solid var(--rule);padding-bottom:.7rem}.evidence h2{font-size:1.55rem}
.prompt-carousel>a,.nav-disabled{border-radius:.55rem;background:var(--surface);box-shadow:0 1px 2px rgba(20,29,43,.05)}.prompt-head{border-bottom:1px solid var(--rule)}.prompt-head h1{font-size:clamp(1.8rem,4vw,3rem)}.run-verdict{border-radius:.65rem;box-shadow:0 1px 2px rgba(20,29,43,.06)}.standard-card{margin:0;border-radius:0;box-shadow:none}.evidence-copy{border-radius:.45rem}.model-stage canvas{background:radial-gradient(circle at 50% 42%,#28313b 0,#151a20 72%)}
@media(max-width:900px){.site-head{padding-inline:1rem}.header-actions{margin-left:auto}.leader-head{align-items:flex-start;flex-direction:column}.benchmark-switcher,.bench-switcher{max-width:100%;overflow-x:auto}}
"#
);

const LEADERBOARD_CSS: &str = "";

#[allow(dead_code)]
const _REMOVED_LEADERBOARD_CSS: &str = concat!(
    r#"
.leader-head{display:flex;align-items:end;justify-content:space-between;gap:24px;padding-bottom:24px}.leader-head h1{margin:0;font-size:clamp(38px,7vw,82px);letter-spacing:-.06em;line-height:.95}.leader-head p{margin:14px 0 0;color:var(--muted)}.leader-head>span{font-variant-numeric:tabular-nums;color:var(--muted)}.leader-wrap{overflow:auto;border-top:3px solid var(--ink);border-bottom:1px solid var(--ink);background:var(--surface)}.leader-table{min-width:920px;font-variant-numeric:tabular-nums}.leader-table th,.leader-table td{padding:15px 14px}.leader-table thead th{position:sticky;top:65px;z-index:2;background:var(--paper);color:var(--muted);font-size:12px}.leader-table thead button{border:0;background:none;color:inherit;font:inherit;font-weight:700;cursor:pointer}.leader-table tbody th{min-width:220px}.leader-table tbody th strong{display:block;font-size:16px}.leader-table small{display:block;color:var(--muted);font-weight:400}.leader-table td>a{color:inherit;text-decoration:none}.leader-table td>a:hover{text-decoration:underline}.rank{width:48px;color:var(--muted)}.missing-score{color:var(--muted);background:#f7f8f6}.missing-score span{font-weight:650}.missing-identity{color:#8b651c}.empty-row td{padding:36px;text-align:center;color:var(--muted)}.empty-row strong,.empty-row span{display:block}.primer-empty{max-width:760px;margin:14px 64px 24px;padding:16px;border:1px dashed var(--rule);color:var(--muted)}.primer-empty p{margin:4px 0 0}.specialty{min-width:190px;color:#76590c}.evidence{padding-top:72px;scroll-margin-top:64px}.evidence>header{display:flex;justify-content:space-between;align-items:end;border-bottom:3px solid var(--ink)}.evidence h2{margin:0;font-size:30px}.evidence p{color:var(--muted)}.evidence>header a{padding-bottom:14px;color:var(--ink)}
"#,
    r#"
.pareto-shell{margin:12px 0 40px;padding:22px;border:1px solid var(--rule);background:var(--surface)}.pareto-shell>header{display:flex;align-items:start;justify-content:space-between;gap:20px}.pareto-shell h2{margin:0;font-size:23px}.pareto-shell p{margin:5px 0;color:var(--muted)}.pareto-shell select{padding:7px 9px;border:1px solid var(--rule);background:var(--surface);color:var(--ink)}#pareto{display:block;width:100%;height:auto;min-height:320px}.chart-axis{stroke:var(--ink);stroke-width:1.5}.chart-grid{stroke:#e0e5df;stroke-width:1}.chart-label,.chart-title,.chart-point text{fill:var(--muted);font:12px "Aptos","Helvetica Neue",Arial,sans-serif}.frontier{fill:none;stroke:#a87813;stroke-width:3}.chart-point circle{fill:#84908a;stroke:var(--surface);stroke-width:2}.frontier-point circle{fill:#a87813}.chart-point:focus{outline:none}.chart-point:focus circle{stroke:var(--ink);stroke-width:4}
"#,
    r#"
.checks,.artifacts{padding:0 18px 18px}.checks>h3,.artifacts>h3{margin:0 0 8px;color:var(--muted);font-size:11px;letter-spacing:.08em;text-transform:uppercase}.check{border-top:1px solid #e5e9e4}.check:last-child{border-bottom:1px solid #e5e9e4}.task .check>summary{justify-content:flex-start;padding:11px 4px}.check>summary>span:last-child{display:flex;flex-direction:column}.check>summary strong{font-size:14px}.check>summary small{color:var(--muted)}.check-detail{padding:0 64px 14px}.check-detail h4{margin:0;color:var(--muted);font-size:11px}.check-detail p{margin:3px 0 8px}.check-detail code{color:var(--muted);font-size:11px}.file-group{border-top:1px solid #e5e9e4}.task .file-group>summary{padding:10px 4px}.file-group>summary span{color:var(--muted);font-size:12px}.file-group>div{display:flex;flex-wrap:wrap;gap:8px;padding:0 4px 12px}.file-group a{max-width:100%;padding:6px 9px;border:1px solid var(--rule);color:var(--ink);font-size:12px;text-decoration:none}.file-group img{display:block;margin-bottom:5px}
"#,
    r#"
.header-actions{display:flex;align-items:center;gap:18px}.benchmark-switcher{display:flex;gap:3px;padding:3px;border:1px solid var(--rule);background:#e7ebe6}.benchmark-tab{min-height:36px;padding:6px 11px;border:0;background:transparent;color:var(--muted);font:600 13px/1 inherit;cursor:pointer}.benchmark-tab[aria-selected=true]{background:var(--surface);color:var(--ink);box-shadow:0 1px 3px #1b281b1f}.benchmark-tab:focus-visible{outline:3px solid #111;outline-offset:2px}.benchmark-guide{margin:8px 0 34px}.benchmark-primer-panel{border-top:3px solid var(--cad);border-bottom:1px solid var(--rule);background:var(--surface)}.benchmark-primer-panel.pcb{border-top-color:var(--pcb)}.benchmark-primer-panel.cam{border-top-color:var(--cam)}.benchmark-primer-panel.dfm{border-top-color:var(--dfm)}.benchmark-primer-panel>header{display:flex;align-items:center;gap:14px;padding:18px 22px 4px}.benchmark-primer-panel>header>span{font-size:28px}.benchmark-primer-panel h2{margin:0;font-size:22px}.benchmark-primer-panel header p{margin:1px 0;color:var(--muted)}.readme-copy{max-width:900px;padding:4px 64px 24px}.readme-copy h2{margin-top:25px;font-size:18px}.readme-copy p,.readme-copy li{max-width:78ch}.readme-copy pre{overflow:auto;padding:13px;background:var(--paper)}.readme-copy table{font-size:12px}.evidence .task-list{grid-template-columns:1fr}.prompt{padding:0 18px 18px}.prompt pre{max-width:100%;margin:0;white-space:pre-wrap;overflow-wrap:anywhere;word-break:break-word;font:inherit}.prompt-preview{display:block;max-width:90ch;color:var(--muted);font-weight:400;white-space:normal;overflow-wrap:anywhere;word-break:break-word}.task details>summary>span{flex:1;max-width:100%}.task details[open]>summary{border-bottom:1px solid var(--rule)}@media(max-width:900px){.header-actions{gap:8px}.monitor-controls{display:none}}@media(max-width:700px){.benchmark-tab{padding:6px 8px}.benchmark-tab span{display:none}.readme-copy{padding:4px 18px 20px}.primer-empty{margin:12px 18px 20px}}
"#,
    r#".leader-head h1{font-size:clamp(34px,5vw,54px);letter-spacing:-.045em;line-height:1}.open-prompt{display:inline-block;margin:16px 18px 0;padding:8px 11px;border:1px solid var(--ink);color:var(--ink);font-weight:700;text-decoration:none}"#
);

#[allow(dead_code)]
const _REMOVED_PROMPT_PAGE_CSS: &str = r#"
.material-chip.mismatch strong,.material-chip.mismatch span{color:#a33a32}
.model-stage>header{display:flex;align-items:center;justify-content:space-between;gap:18px}.material-chip,.material-missing{display:flex;flex-direction:column;align-items:flex-end}.material-chip span,.material-missing span{font-size:11px;color:var(--muted)}.artifact-warning{margin:14px;padding:12px 14px;border:1px solid #d69e2e;border-radius:.55rem;background:#fff8df;color:#6d4d0b}.artifact-warning p{margin:3px 0 0}.run-verdict.warn{border-color:#d69e2e}.run-verdict.warn>strong{color:#8b651c}.standard-card.proxy .result-mark{background:#fff0bd;color:#76590c}#stl-viewer{position:relative;width:100%;height:min(66vh,680px);min-height:420px;background:radial-gradient(circle at 50% 42%,#28313b 0,#151a20 72%);overflow:hidden}#stl-viewer canvas{display:block;width:100%;height:100%}.model-loading,.model-error{position:absolute;inset:0;display:grid;place-content:center;padding:24px;text-align:center;color:#d9e0e7}.model-error p{margin:4px 0 0;color:#aeb8c2}
.muted{color:var(--muted)}.prompt-page{max-width:1680px}.prompt-carousel{display:grid;grid-template-columns:120px minmax(240px,560px) 120px;align-items:end;justify-content:space-between;gap:18px;margin-bottom:28px}.prompt-carousel>a,.nav-disabled{padding:10px 12px;border:1px solid var(--rule);text-align:center;text-decoration:none;color:var(--ink)}.nav-disabled{color:var(--muted);opacity:.45}.prompt-carousel label{display:flex;flex-direction:column;gap:5px}.prompt-carousel label span{color:var(--muted);font-size:12px}.prompt-carousel select{width:100%;padding:10px;border:1px solid var(--rule);background:var(--surface);font:inherit}.prompt-head{display:flex;align-items:end;justify-content:space-between;gap:28px;padding-bottom:24px;border-bottom:3px solid var(--ink)}.prompt-head p{margin:0;color:var(--muted)}.prompt-head h1{max-width:900px;margin:5px 0 0;font-size:clamp(30px,5vw,62px);line-height:1;letter-spacing:-.045em;overflow-wrap:anywhere}.run-verdict{min-width:230px;padding:16px 18px;border-left:7px solid var(--rule);background:var(--surface)}.run-verdict strong,.run-verdict span,.run-verdict small{display:block}.run-verdict strong{font-size:22px}.run-verdict.pass{border-color:#27715b}.run-verdict.pass strong{color:#27715b}.run-verdict.fail,.run-verdict.error{border-color:#b7352d}.run-verdict.fail strong,.run-verdict.error strong{color:#b7352d}.run-verdict small{color:var(--muted)}.prompt-layout{display:grid;grid-template-columns:minmax(0,1fr) minmax(440px,.9fr);gap:28px;margin-top:28px}.prompt-copy,.standards-list,.model-stage{border:1px solid var(--rule);background:var(--surface)}.prompt-copy h2,.standards-list>header,.model-stage>header{margin:0;padding:15px 18px;border-bottom:1px solid var(--rule)}.prompt-copy pre{max-width:100%;margin:0;padding:20px;white-space:pre-wrap;overflow-wrap:anywhere;word-break:break-word;font:15px/1.6 inherit}.standards-list{margin-top:18px}.standards-list>header h2,.model-stage h2{margin:0}.standards-list>header p,.model-stage header span{color:var(--muted)}.standard-card{padding:17px 18px;border-bottom:1px solid var(--rule)}.standard-card:last-child{border-bottom:0}.standard-card>header{display:flex;gap:14px;align-items:start}.standard-card h3{margin:0;font-size:17px}.standard-card code{color:var(--muted)}.standard-card>p{margin:10px 0}.result-mark{min-width:64px;padding:3px 7px;text-align:center;font-size:11px;font-weight:800}.standard-card.pass .result-mark{background:#dcebe4;color:#176345}.standard-card.fail .result-mark{background:#f2ddda;color:#9f2924}.standard-card.human .result-mark{background:#f1e8cf;color:#76590c}.evidence-copy{padding:11px 13px;background:var(--paper)}.evidence-copy strong{font-size:11px;text-transform:uppercase;letter-spacing:.07em}.evidence-copy p{margin:3px 0}.model-stage{position:sticky;top:84px;align-self:start}.model-stage canvas{display:block;width:100%;height:min(66vh,680px);background:#181c1a;cursor:grab}.model-stage canvas:active{cursor:grabbing}.model-tools{display:flex;justify-content:space-between;gap:16px;padding:11px 14px;color:var(--muted);font-size:12px}.model-tools a{color:var(--ink)}.model-empty,.empty-detail{margin:18px;padding:18px;border:1px dashed var(--rule);color:var(--muted)}.model-empty p,.empty-detail p{margin:4px 0 0}@media(max-width:980px){.prompt-layout{grid-template-columns:1fr}.model-stage{position:static;grid-row:1}.model-stage canvas{height:52vh}}@media(max-width:620px){.prompt-carousel{grid-template-columns:1fr 1fr}.prompt-carousel label{grid-column:1/-1;grid-row:1}.prompt-head{align-items:stretch;flex-direction:column}.run-verdict{min-width:0}}
"#;

const PROMPT_PAGE_JS: &str = r#"
document.querySelector('#prompt-select').addEventListener('change',event=>location.href=event.target.value);
const inspector=document.querySelector('#artifact-inspector');
const artifactTabs=[...document.querySelectorAll('[data-artifact-tab]')];
const artifactPanels=[...document.querySelectorAll('[data-artifact-panel]')];
function selectArtifact(id){artifactTabs.forEach(tab=>tab.setAttribute('aria-selected',String(tab.dataset.artifactTab===id)));artifactPanels.forEach(panel=>panel.hidden=panel.id!==id)}
artifactTabs.forEach((tab,index)=>{tab.addEventListener('click',()=>selectArtifact(tab.dataset.artifactTab));tab.addEventListener('keydown',event=>{if(event.key==='ArrowLeft'||event.key==='ArrowRight'){event.preventDefault();const direction=event.key==='ArrowRight'?1:-1;const next=artifactTabs[(index+direction+artifactTabs.length)%artifactTabs.length];next.focus();selectArtifact(next.dataset.artifactTab)}})});
document.querySelector('[data-fullscreen-inspector]')?.addEventListener('click',async()=>{if(document.fullscreenElement)await document.exitFullscreen();else await inspector?.requestFullscreen()});
document.addEventListener('fullscreenchange',()=>{const button=document.querySelector('[data-fullscreen-inspector]');if(button)button.textContent=document.fullscreenElement?'Exit fullscreen':'Fullscreen'});
const host=document.querySelector('#stl-viewer');
if(host){try{
const THREE=await import('https://esm.sh/three@0.180.0');
const {STLLoader}=await import('https://esm.sh/three@0.180.0/examples/jsm/loaders/STLLoader.js');
const {OrbitControls}=await import('https://esm.sh/three@0.180.0/examples/jsm/controls/OrbitControls.js');
const renderer=new THREE.WebGLRenderer({antialias:true,alpha:true});renderer.setPixelRatio(Math.min(devicePixelRatio,2));renderer.outputColorSpace=THREE.SRGBColorSpace;renderer.toneMapping=THREE.ACESFilmicToneMapping;renderer.toneMappingExposure=1.15;host.replaceChildren(renderer.domElement);
const scene=new THREE.Scene(),camera=new THREE.PerspectiveCamera(36,1,.01,1000),controls=new OrbitControls(camera,renderer.domElement);controls.enableDamping=true;controls.autoRotate=false;
scene.add(new THREE.HemisphereLight(0xffffff,0x303842,2.1));const key=new THREE.DirectionalLight(0xffffff,3.2);key.position.set(3,4,5);scene.add(key);const rim=new THREE.DirectionalLight(0x9db8ff,1.8);rim.position.set(-4,1,-3);scene.add(rim);
const geometry=await new STLLoader().loadAsync(host.dataset.src);geometry.computeVertexNormals();geometry.center();const bounds=new THREE.Box3().setFromBufferAttribute(geometry.attributes.position),size=bounds.getSize(new THREE.Vector3()),radius=Math.max(size.x,size.y,size.z)/2||1;
const material=new THREE.MeshStandardMaterial({color:new THREE.Color(host.dataset.color),metalness:Number(host.dataset.metalness),roughness:Number(host.dataset.roughness)}),mesh=new THREE.Mesh(geometry,material);mesh.rotation.x=-Math.PI/2;scene.add(mesh);camera.position.set(radius*2.4,radius*1.7,radius*2.4);camera.near=radius/100;camera.far=radius*100;camera.updateProjectionMatrix();controls.target.set(0,0,0);controls.minDistance=radius*.6;controls.maxDistance=radius*8;
const resize=()=>{const w=host.clientWidth,h=Math.min(Math.max(Math.round(w*.62),360),620);renderer.setSize(w,h,true);camera.aspect=w/h;camera.updateProjectionMatrix()};new ResizeObserver(resize).observe(host);resize();host.removeAttribute('aria-busy');renderer.setAnimationLoop(()=>{controls.update();renderer.render(scene,camera)});
}catch(error){console.error(error);host.innerHTML='<div class=model-error><strong>STL preview unavailable</strong><p>The artifact is still available from the download link below.</p></div>'}}
for(const glbHost of document.querySelectorAll('[data-glb-viewer]')){try{
const THREE=await import('https://esm.sh/three@0.180.0');
const {GLTFLoader}=await import('https://esm.sh/three@0.180.0/examples/jsm/loaders/GLTFLoader.js');
const {OrbitControls}=await import('https://esm.sh/three@0.180.0/examples/jsm/controls/OrbitControls.js');
const renderer=new THREE.WebGLRenderer({antialias:true,alpha:true});renderer.setPixelRatio(Math.min(devicePixelRatio,2));renderer.outputColorSpace=THREE.SRGBColorSpace;renderer.toneMapping=THREE.ACESFilmicToneMapping;renderer.toneMappingExposure=1.15;glbHost.replaceChildren(renderer.domElement);
const scene=new THREE.Scene(),camera=new THREE.PerspectiveCamera(36,1,.01,1000),controls=new OrbitControls(camera,renderer.domElement);controls.enableDamping=true;scene.add(new THREE.HemisphereLight(0xffffff,0x303842,2.2));const key=new THREE.DirectionalLight(0xffffff,3);key.position.set(3,5,4);scene.add(key);
const loaded=await new GLTFLoader().loadAsync(glbHost.dataset.src);scene.add(loaded.scene);const bounds=new THREE.Box3().setFromObject(loaded.scene),center=bounds.getCenter(new THREE.Vector3()),size=bounds.getSize(new THREE.Vector3()),radius=Math.max(size.x,size.y,size.z)/2||1;loaded.scene.position.sub(center);camera.position.set(radius*2.2,radius*1.8,radius*2.2);camera.near=radius/100;camera.far=radius*100;camera.updateProjectionMatrix();controls.minDistance=radius*.5;controls.maxDistance=radius*8;
const resize=()=>{const w=glbHost.clientWidth,h=Math.min(Math.max(Math.round(w*.62),360),620);renderer.setSize(w,h,true);camera.aspect=w/h;camera.updateProjectionMatrix()};new ResizeObserver(resize).observe(glbHost);resize();glbHost.removeAttribute('aria-busy');renderer.setAnimationLoop(()=>{controls.update();renderer.render(scene,camera)});
}catch(error){console.error(error);glbHost.innerHTML='<div class=model-error><strong>3D board preview unavailable</strong><p>The GLB remains available from the download link below.</p></div>'}}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CriterionResult, DatasetSplit, Difficulty, ScoreReport, SuiteTaskResult, TaskMetadata,
    };
    use std::path::PathBuf;

    #[test]
    fn switcher_has_all_disciplines_and_disables_missing_reports() {
        let html = render(&[]);
        for text in ["PCB Bench", "CAD Bench", "CAM Bench", "DFM Bench"] {
            assert!(html.contains(text), "missing {text}");
        }
        assert_eq!(html.matches("disabled title").count(), 4);
        assert!(html.contains("aria-label='Benchmark'"));
        assert_eq!(html.matches("role=tab ").count(), 4);
        assert_eq!(html.matches("aria-labelledby=tab-").count(), 4);
    }

    #[test]
    fn leaderboard_explains_missing_data_and_keeps_all_benchmarks_selectable() {
        let html = render_leaderboard(&[], &[]);
        assert_eq!(html.matches("class=benchmark-tab").count(), 4);
        assert_eq!(html.matches("Benchmark definition not recorded").count(), 4);
        assert!(html.contains("No benchmark configurations recorded"));
        assert!(html.contains("Price / score frontier"));
        assert!(html.contains("Come back soon."));
        assert!(!html.contains("Specialties"));
        assert!(html.contains("data-column=4>⚡ PCB"));
        assert!(html.contains("data-column=5>📐 CAD"));
        assert!(html.contains("data-column=6>⚙️ CAM"));
        assert!(html.contains("data-column=7>🏭 DFM"));
        assert!(!html.contains("class='bench-primer"));
        assert!(html.contains("How scoring works"));
        assert!(html.contains("Missing benchmarks are excluded"));
        assert!(html.contains("passing proxy still counts"));
    }

    #[test]
    fn leaderboard_score_cells_show_their_denominators() {
        let report = SuiteReport {
            schema: "eval.suite-report.v1".into(),
            tasks_dir: PathBuf::from("tasks"),
            results_dir: PathBuf::from("results"),
            total: 4,
            passed: 3,
            failed: 1,
            errors: 0,
            needs_human: 2,
            tasks: Vec::new(),
        };
        let model = ModelView {
            id: "m",
            name: "Model",
            organization: None,
            harness: "harness",
            rlcd_model: Some("router"),
            generative_model: Some("generator"),
            alias: None,
            pcb: None,
            cad: Some(&report),
            cam: None,
            dfm: None,
            pcb_metrics: RunMetrics::default(),
            cad_metrics: RunMetrics::default(),
            cam_metrics: RunMetrics::default(),
            dfm_metrics: RunMetrics::default(),
        };
        let html = render_leaderboard(&[model], &[]);
        assert!(html.contains("75.0"));
        assert!(html.contains("3/4 tasks"));
        assert!(html.contains("2 review · 0 errors"));
        assert!(html.contains("mean of 1 suite rate(s)"));
    }

    #[test]
    fn leaderboard_renders_readme_backed_benchmark_content() {
        let primers = [BenchmarkPrimer {
            kind: BenchmarkKind::Cad,
            html: "<h2>What it tests</h2><p>Parametric mechanical reasoning.</p>".to_owned(),
        }];
        let html = render_leaderboard(&[], &primers);
        assert!(html.contains("Parametric mechanical reasoning."));
        assert_eq!(html.matches("Parametric mechanical reasoning.").count(), 1);
    }

    #[test]
    fn prompt_page_shows_rigor_metadata_and_score_semantics() {
        let metadata = TaskMetadata {
            capabilities: vec!["geometry.true-surfaces".into()],
            difficulty: Difficulty::Adversarial,
            source: Some("synthetic:surface-mutant".into()),
            oracle_version: "step-surface-v1".into(),
            split: DatasetSplit::Validation,
            expected_failure_modes: vec!["faceted-cylinder".into()],
        };
        let suite = SuiteReport {
            schema: "eval.suite-report.v1".into(),
            tasks_dir: PathBuf::from("tasks"),
            results_dir: PathBuf::from("results"),
            total: 1,
            passed: 1,
            failed: 0,
            errors: 0,
            needs_human: 0,
            tasks: vec![SuiteTaskResult {
                task_file: PathBuf::from("missing-task.toml"),
                work_dir: PathBuf::from("missing-results"),
                task_metadata: Some(metadata),
                metadata_error: None,
                criterion_definitions: [(
                    "surfaces".into(),
                    crate::CriterionDefinition {
                        kind: "true_surfaces".into(),
                        standard_profile: Some("ISO 10303 STEP geometry".into()),
                        parameters: [("min_cylinders".into(), "4".into())].into_iter().collect(),
                    },
                )]
                .into_iter()
                .collect(),
                elapsed_ms: 1,
                report: Some(ScoreReport {
                    task_id: "surface-mutant".into(),
                    backend: "transmog".into(),
                    results: vec![CriterionResult {
                        id: "surfaces".into(),
                        description: "uses true cylinders".into(),
                        verdict: Verdict::Pass,
                        detail: "artifact contains 4 cylindrical surfaces".into(),
                    }],
                }),
                error: None,
            }],
        };
        let html = render_prompt_page("model", "Model", "transmog", BenchmarkKind::Cad, &suite, 0)
            .expect("prompt page");
        for expected in [
            "Rigor ledger",
            "geometry.true-surfaces",
            "step-surface-v1",
            "faceted-cylinder",
            "capability-macro reporting",
            "do not award points",
            "1/1 deterministic",
            "ISO 10303 STEP geometry",
            "True Surfaces: Min Cylinders=4",
            "Counts toward task pass",
        ] {
            assert!(html.contains(expected), "missing {expected}");
        }
    }

    #[test]
    fn artifact_inspector_switches_model_pdf_schematic_and_board_views() {
        let root =
            std::env::temp_dir().join(format!("eval-artifact-inspector-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        for name in [
            "final.stl",
            "drawing.pdf",
            "schematic.svg",
            "board-render.png",
            "board.glb",
        ] {
            std::fs::write(root.join(name), b"fixture").unwrap();
        }
        let mut artifacts = Vec::new();
        collect_artifacts(&root, &root, 0, &mut artifacts);
        let html = artifact_inspector(
            "model",
            BenchmarkKind::Pcb,
            0,
            &root,
            &artifacts,
            "<div id=stl-viewer></div>".into(),
        );
        for expected in [
            "3D model",
            "📄 drawing",
            "Schematic",
            "PCB layout",
            "3D board · board",
            "data-glb-viewer",
            "Scroll normally through every page.",
            "data-fullscreen-inspector",
            "role=tablist",
        ] {
            assert!(html.contains(expected), "missing {expected}");
        }
        assert!(html.contains("drawing.pdf#view=FitH"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn prompt_decisions_show_generative_stock_and_rlcd_choices() {
        let root =
            std::env::temp_dir().join(format!("eval-prompt-decisions-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("starting-stock.json"),
            r#"{"schema":"transmog.starting-stock.v1","size_mm":[100,60,20],"material":"304 stainless steel","model":"or/z-ai/glm-5.3-flash","basis":"The brief states it."}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("decisions.json"),
            r#"[{"key":"bore_layout","type":"choice","chosen":"rectangular_four","confidence":1.0}]"#,
        )
        .unwrap();
        let html = design_decisions(&root);
        for expected in [
            "Starting stock · generative",
            "100 × 60 × 20 mm",
            "304 stainless steel",
            "or/z-ai/glm-5.3-flash",
            "Geometry · RLCD",
            "bore_layout",
            "rectangular_four",
            "Accepted",
        ] {
            assert!(html.contains(expected), "missing {expected}");
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
