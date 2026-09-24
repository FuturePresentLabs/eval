//! Self-contained, benchmark-neutral HTML viewer for suite reports.

use crate::{SuiteReport, Verdict};

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
                format!("<td data-score='{value}'><a href='#evidence-{}-{}'>{shown}<small>{}/{}</small></a></td>", esc(model.id), kind.id(), report.passed, report.total)
            }
            None => "<td data-score='-1' class=missing-score><span>Not run</span><small>No result recorded</small></td>".to_owned(),
        }).collect::<String>();
        let specialties = kinds.iter().enumerate().filter_map(|(column, kind)| model.report(*kind).map(score).filter(|value| (*value - winners[column]).abs() < 0.0001).map(|_| format!("{} {}", kind.emoji(), kind.name()))).collect::<Vec<_>>().join(", ");
        let alias = model.alias.map_or(String::new(), |alias| format!("<small>Alias: {}</small>", esc(alias)));
        let model_role = |value: Option<&str>| value.map_or_else(|| "<span class=not-applicable>Not applicable</span>".to_owned(), |value| if value == "Not recorded" { "<span class=missing-identity>Not recorded</span><small>Identity missing from run</small>".to_owned() } else { format!("<strong>{}</strong>", esc(value)) });
        let covered = coverage(model);
        let overall_cell = if covered == 0 { "<td data-score='-1' class=missing-score><span>Not run</span><small>No benchmark results</small></td>".to_owned() } else { format!("<td data-score='{:.3}'><strong>{:.1}</strong><small>{covered}/4 benchmarks run</small></td>", overall(model), overall(model)) };
        format!("<tr><td class=rank>{}</td><th scope=row><strong>{}</strong><small>{}</small>{alias}</th><td>{}</td><td>{}</td><td><strong>{}</strong></td>{overall_cell}{scores}<td class=specialty>{}</td></tr>", rank + 1, esc(model.name), esc(model.organization.unwrap_or("Independent")), model_role(model.rlcd_model), model_role(model.generative_model), esc(model.harness), if specialties.is_empty() { "No leading result".to_owned() } else { esc(&specialties) })
    }).collect::<String>();
    let evidence = order.iter().flat_map(|model| kinds.iter().filter_map(move |kind| model.report(*kind).map(|report| {
        let tasks = report.tasks.iter().enumerate().map(|(index, task)| task_card(Some(model.id), *kind, index, task)).collect::<String>();
        format!("<section class=evidence id='evidence-{}-{}'><header><div><h2>{}</h2><p>{} · {:.1} · {}/{} passed</p></div><a href=#leaderboard>Back to leaderboard</a></header><div class=task-list>{tasks}</div></section>", esc(model.id), kind.id(), esc(model.name), kind.name(), score(report), report.passed, report.total)
    }))).collect::<String>();
    let chart_data = chart_data(models, &kinds);
    let active = Some(BenchmarkKind::Cad);
    let switcher = kinds.iter().map(|kind| {
        format!("<button class=benchmark-tab role=tab data-bench={} aria-controls=primer-{} aria-selected={}><span>{}</span> {}</button>", kind.id(), kind.id(), active == Some(*kind), kind.emoji(), kind.name())
    }).collect::<String>();
    let primers_html = kinds.iter().map(|kind| {
        let body = primers.iter().find(|primer| primer.kind == *kind).map_or_else(|| "<div class=primer-empty><strong>Description not recorded</strong><p>Add this benchmark's README source to the leaderboard index. Results can still be displayed independently.</p></div>".to_owned(), |primer| format!("<div class=readme-copy>{}</div>", primer.html));
        format!("<article id=primer-{} class='benchmark-primer-panel {}' data-bench={} role=tabpanel {}><header><span>{}</span><div><h2>{}</h2><p>What it tests and how it is scored</p></div></header>{body}</article>", kind.id(), kind.id(), kind.id(), if active == Some(*kind) { "" } else { "hidden" }, kind.emoji(), kind.name())
    }).collect::<String>();
    let rows = if rows.is_empty() {
        "<tr class=empty-row><td colspan=11><strong>No benchmark configurations recorded</strong><span>Add model roles, a harness, and a suite report to the leaderboard index.</span></td></tr>".to_owned()
    } else {
        rows
    };
    format!(
        "<!doctype html><html lang=en><head><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'><title>FPL decision model leaderboard</title><style>{CSS}{LEADERBOARD_CSS}</style></head><body><header class=site-head><a class=wordmark href=#leaderboard>FPL <span>decision model index</span></a><div class=header-actions><div class=monitor-controls><button type=button id=refresh>Refresh</button><label><input id=live type=checkbox checked> Live</label></div><nav class=benchmark-switcher aria-label='Benchmark' role=tablist>{switcher}</nav></div></header><main id=leaderboard><div class=leader-head><div><h1>Decision models × harnesses</h1><p>Comparable configurations. Column leaders are bold.</p></div><span>{} evaluated</span></div><section class=benchmark-guide>{primers_html}</section><div class=leader-wrap><table class=leader-table><thead><tr><th>Rank</th><th>Decision model</th><th>Harness</th><th><button data-column=3>Overall</button></th><th><button data-column=4>⚡ PCB</button></th><th><button data-column=5>📐 CAD</button></th><th><button data-column=6>⚙️ CAM</button></th><th><button data-column=7>🏭 DFM</button></th><th>Specialties</th></tr></thead><tbody>{rows}</tbody></table></div><section class=pareto-shell><header><div><h2>Quality frontier</h2><p>Best tradeoffs rise toward the upper left.</p></div><div><select id=pareto-bench aria-label=Benchmark><option value=overall>Overall</option><option value=pcb>PCB</option><option value=cad>CAD</option><option value=cam>CAM</option><option value=dfm>DFM</option></select><select id=pareto-x aria-label='X axis'><option value=cost>Cost / task</option></select></div></header><svg id=pareto role=img aria-label='Model score versus price' viewBox='0 0 1000 430'></svg><p id=pareto-empty hidden>No measured price per task for this benchmark.</p></section>{evidence}</main><script>const paretoData={chart_data};{LEADERBOARD_JS}</script></body></html>",
        models.len(),
    )
    .replace("FPL decision model leaderboard", "FPL evaluation leaderboard")
    .replace("decision model index", "evaluation index")
    .replace("Decision models × harnesses", "Evaluation leaderboard")
    .replace("Comparable configurations. Column leaders are bold.", "Decision model × harness configurations. Planned rows remain visible before results exist.")
    .replace(" evaluated</span>", " configurations</span>")
    .replace("<th>Decision model</th><th>Harness</th><th><button data-column=3>Overall</button></th><th><button data-column=4>⚡ PCB</button></th><th><button data-column=5>📐 CAD</button></th><th><button data-column=6>⚙️ CAM</button></th><th><button data-column=7>🏭 DFM</button></th>", "<th>Configuration</th><th>RLCD model</th><th>Generative model</th><th>Harness</th><th><button data-column=5>Overall</button></th><th><button data-column=6>⚡ PCB</button></th><th><button data-column=7>📐 CAD</button></th><th><button data-column=8>⚙️ CAM</button></th><th><button data-column=9>🏭 DFM</button></th>")
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
    let standards = task_standards(&task.task_file);
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
                .map(String::as_str)
                .unwrap_or(&result.id);
            format!("<details class='check {class}' title='{}'><summary><span class='verdict {class}'>{mark}</span><span><strong>{}</strong><small>{}</small></span></summary><div class=check-detail><h4>Evidence</h4><p>{}</p><code>{}</code></div></details>", esc(&result.detail), esc(standard), esc(&result.description), esc(&result.detail), esc(&result.id))
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
    let standards = task_standards(&task.task_file);
    let automated = scored.map_or(0, |value| {
        value
            .results
            .iter()
            .filter(|result| result.verdict != Verdict::NeedsHuman)
            .count()
    });
    let passed = scored.map_or(0, |value| {
        value
            .results
            .iter()
            .filter(|result| result.verdict == Verdict::Pass)
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
    let state = if task.error.is_some() {
        ("error", "HARNESS ERROR")
    } else if failed > 0 {
        ("fail", "FAIL")
    } else {
        ("pass", "PASS")
    };
    let checks = scored.map_or_else(|| format!("<div class='empty-detail error'><strong>No score report</strong><p>{}</p></div>", esc(task.error.as_deref().unwrap_or("The harness did not record an error."))), |value| value.results.iter().map(|result| {
        let (class, mark) = match result.verdict { Verdict::Pass => ("pass", "PASS"), Verdict::Fail => ("fail", "FAIL"), Verdict::NeedsHuman => ("human", "REVIEW") };
        let standard = standards.get(&result.id).map(String::as_str).unwrap_or(&result.id);
        format!("<article class='standard-card {class}'><header><span class='result-mark'>{mark}</span><div><h3>{}</h3><code>{}</code></div></header><p>{}</p><div class=evidence-copy><strong>Evidence</strong><p>{}</p></div></article>", esc(standard), esc(&result.id), esc(&result.description), esc(&result.detail))
    }).collect::<String>());
    let mut artifacts = Vec::new();
    collect_artifacts(&task.work_dir, &task.work_dir, 0, &mut artifacts);
    let final_stl = artifacts
        .iter()
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("stl"))
        .max_by_key(|path| stl_rank(path))
        .and_then(|path| path.strip_prefix(&task.work_dir).ok())
        .map(|path| path.to_string_lossy().into_owned());
    let model = final_stl.map_or_else(|| "<div class=model-empty><strong>Final STL not recorded</strong><p>The prompt page is available, but this run did not produce an STL artifact.</p></div>".to_owned(), |relative| {
        let href = format!("/artifact/{model_id}/{}/{index}/{relative}", kind.id());
        format!("<canvas id=stl-viewer data-src='{href}' aria-label='Interactive final STL viewer'></canvas><div class=model-tools><span>Drag to rotate · scroll to zoom</span><a href='{href}' download>Download STL</a></div>")
    });
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
    Some(format!("<!doctype html><html lang=en><head><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'><title>{} · {}</title><style>{CSS}{LEADERBOARD_CSS}{PROMPT_PAGE_CSS}</style></head><body><header class=site-head><a class=wordmark href='/#evidence-{model_id}-{}'>← Leaderboard</a><div><strong>{}</strong> <span class=muted>× {}</span></div></header><main class=prompt-page><nav class=prompt-carousel>{}<label><span>Prompt {}/{}</span><select id=prompt-select>{options}</select></label>{}</nav><header class=prompt-head><div><p>{} · {}</p><h1>{}</h1></div><div class='run-verdict {}'><strong>{}</strong><span>{passed}/{automated} automated checks passed</span><small>{reviews} human review</small></div></header><section class=prompt-layout><div><section class=prompt-copy><h2>Prompt</h2>{}</section><section class=standards-list><header><h2>Standards & checks</h2><p>Named requirement, verdict, and recorded evidence.</p></header>{checks}</section></div><aside class=model-stage><header><h2>Final model</h2><span>Final STL produced by this prompt</span></header>{model}</aside></section></main><script>{PROMPT_PAGE_JS}</script></body></html>", esc(&task_id), kind.name(), kind.id(), esc(model_name), esc(harness), nav_link(previous, "← Previous"), index + 1, report.tasks.len(), nav_link(next, "Next →"), kind.name(), esc(harness), esc(&task_id), state.0, state.1, brief.as_deref().map_or_else(|| "<div class=empty-detail><strong>Prompt not recorded</strong><p>The task file did not contain a brief.</p></div>".to_owned(), |value| format!("<pre>{}</pre>", esc(value)))))
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

fn task_standards(path: &std::path::Path) -> std::collections::BTreeMap<String, String> {
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
            let name = criterion
                .get("standard")
                .and_then(toml::Value::as_str)
                .or_else(|| criterion.get("kind").and_then(toml::Value::as_str))?;
            Some((id, check_name(name)))
        })
        .collect()
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
    if matches!(extension, "stl" | "step" | "stp" | "glb" | "gltf") {
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
document.querySelector('#refresh').addEventListener('click',()=>location.reload());
let revision=null;setInterval(async()=>{if(!document.querySelector('#live').checked)return;try{const next=await fetch('/api/revision',{cache:'no-store'}).then(r=>r.ok?r.text():null);if(revision===null)revision=next;else if(next&&next!==revision)location.reload()}catch(_){}},1000);
const benchmarkTabs=[...document.querySelectorAll('.benchmark-tab:not(:disabled)')],primerPanels=[...document.querySelectorAll('.benchmark-primer-panel')];
function selectBenchmark(id){benchmarkTabs.forEach(tab=>tab.setAttribute('aria-selected',String(tab.dataset.bench===id)));primerPanels.forEach(panel=>panel.hidden=panel.dataset.bench!==id);const option=[...bench.options].find(option=>option.value===id);if(option){bench.value=id;drawPareto()}history.replaceState(null,'','#benchmark-'+id)}
benchmarkTabs.forEach((tab,index)=>{tab.addEventListener('click',()=>selectBenchmark(tab.dataset.bench));tab.addEventListener('keydown',event=>{if(event.key==='ArrowLeft'||event.key==='ArrowRight'){event.preventDefault();const direction=event.key==='ArrowRight'?1:-1;const next=benchmarkTabs[(index+direction+benchmarkTabs.length)%benchmarkTabs.length];next.focus();selectBenchmark(next.dataset.bench)}})});
document.querySelectorAll('.leader-table thead button').forEach(button=>button.addEventListener('click',()=>{const body=document.querySelector('.leader-table tbody');const rows=[...body.rows];const column=Number(button.dataset.column);rows.sort((a,b)=>Number(b.cells[column].dataset.score)-Number(a.cells[column].dataset.score));rows.forEach((row,index)=>{row.cells[0].textContent=index+1;body.append(row)})}));
const svg=document.querySelector('#pareto'),bench=document.querySelector('#pareto-bench'),axis=document.querySelector('#pareto-x'),empty=document.querySelector('#pareto-empty'),paretoShell=document.querySelector('.pareto-shell');axis.remove();empty.textContent='No measured price per task for this benchmark.';
function drawPareto(){const key=bench.value;const points=paretoData.map(model=>{const result=key==='overall'?model.overall:model.benchmarks[key];return result&&result.cost!=null?{name:model.name,score:result.score,x:result.cost}:null}).filter(Boolean).sort((a,b)=>a.x-b.x);svg.replaceChildren();empty.hidden=points.length>0;if(!points.length)return;const ns='http://www.w3.org/2000/svg',left=72,right=970,top=24,bottom=374,maxX=Math.max(...points.map(p=>p.x))*1.08||1;const sx=x=>left+x/maxX*(right-left),sy=y=>bottom-y/100*(bottom-top);const add=(tag,attrs,text)=>{const node=document.createElementNS(ns,tag);Object.entries(attrs).forEach(([k,v])=>node.setAttribute(k,v));if(text)node.textContent=text;svg.append(node);return node};add('line',{x1:left,y1:top,x2:left,y2:bottom,class:'chart-axis'});add('line',{x1:left,y1:bottom,x2:right,y2:bottom,class:'chart-axis'});[0,25,50,75,100].forEach(v=>{add('line',{x1:left,y1:sy(v),x2:right,y2:sy(v),class:'chart-grid'});add('text',{x:left-12,y:sy(v)+4,'text-anchor':'end',class:'chart-label'},v)});add('text',{x:(left+right)/2,y:420,'text-anchor':'middle',class:'chart-title'},'Average price per task (USD) →');add('text',{x:18,y:(top+bottom)/2,transform:`rotate(-90 18 ${(top+bottom)/2})`,'text-anchor':'middle',class:'chart-title'},'Score ↑');let best=-1;const frontier=points.filter(p=>{if(p.score>best){best=p.score;return true}return false});add('polyline',{points:frontier.map(p=>`${sx(p.x)},${sy(p.score)}`).join(' '),class:'frontier'});points.forEach(p=>{const group=add('g',{tabindex:'0',class:frontier.includes(p)?'chart-point frontier-point':'chart-point'});const circle=document.createElementNS(ns,'circle');circle.setAttribute('cx',sx(p.x));circle.setAttribute('cy',sy(p.score));circle.setAttribute('r',frontier.includes(p)?7:5);group.append(circle);const title=document.createElementNS(ns,'title');title.textContent=`${p.name}: ${p.score.toFixed(1)} score, $${p.x.toFixed(4)} / task`;group.append(title);const label=document.createElementNS(ns,'text');label.setAttribute('x',sx(p.x)+10);label.setAttribute('y',sy(p.score)-10);label.textContent=p.name;group.append(label)})}
bench.addEventListener('change',drawPareto);const requested=location.hash.replace('#benchmark-','');if(benchmarkTabs.some(tab=>tab.dataset.bench===requested))selectBenchmark(requested);else drawPareto();if(!paretoData.some(model=>model.overall.cost!=null))paretoShell.hidden=true;
"#;

const CSS: &str = r#"
:root{--paper:#f4f6f2;--surface:#fff;--ink:#202621;--muted:#687169;--rule:#c9d0c9;--pcb:#b94c21;--cad:#245fa3;--cam:#6e4f92;--dfm:#27715b;color-scheme:light}*{box-sizing:border-box}body{margin:0;background:var(--paper);color:var(--ink);font:15px/1.5 "Aptos","Helvetica Neue",Arial,sans-serif}.site-head{position:sticky;top:0;z-index:5;display:flex;align-items:center;justify-content:space-between;gap:24px;padding:14px clamp(18px,4vw,56px);border-bottom:1px solid var(--rule);background:rgba(244,246,242,.96);backdrop-filter:blur(10px)}.wordmark{color:var(--ink);font-weight:800;letter-spacing:-.02em;text-decoration:none}.wordmark span{color:var(--muted);font-weight:500}.bench-switcher{display:flex;gap:3px;padding:3px;border:1px solid var(--rule);background:#e7ebe6}.bench-tab{display:flex;align-items:center;gap:7px;min-height:36px;padding:6px 10px;border:0;background:transparent;color:var(--muted);font:600 13px/1 inherit;cursor:pointer}.bench-tab[aria-selected=true]{background:var(--surface);color:var(--ink);box-shadow:0 1px 3px #1b281b1f}.bench-tab:disabled{cursor:not-allowed;opacity:.35}.bench-tab:focus-visible{outline:3px solid #111;outline-offset:2px}main{max-width:1440px;margin:auto;padding:clamp(28px,5vw,72px) clamp(18px,4vw,56px)}.bench-panel.pcb{--accent:var(--pcb)}.bench-panel.cad{--accent:var(--cad)}.bench-panel.cam{--accent:var(--cam)}.bench-panel.dfm{--accent:var(--dfm)}.hero{display:grid;grid-template-columns:minmax(280px,1fr) minmax(380px,1fr);gap:48px;align-items:end;padding-bottom:32px;border-bottom:3px solid var(--accent)}.identity{display:flex;align-items:center;gap:10px;margin:0 0 12px;color:var(--accent);font-size:18px;font-weight:750}.identity span{font-size:25px}.hero h1{margin:0;font-size:clamp(52px,9vw,112px);line-height:.82;letter-spacing:-.075em}.hero h1 small{display:block;margin-top:20px;color:var(--muted);font-size:16px;font-weight:550;letter-spacing:0}.hero dl{display:grid;grid-template-columns:repeat(4,1fr);margin:0}.hero dl div{padding:0 14px;border-left:1px solid var(--rule)}dt{color:var(--muted);font-size:12px}dd{margin:3px 0 0;font-size:24px;font-weight:750}.task-list{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:12px;margin-top:28px}.task{border:1px solid var(--rule);background:var(--surface)}.task.failed{border-left:4px solid #b7352d}.task.error{border-left:4px solid #b77719;padding:18px}.task details>summary{display:flex;justify-content:space-between;align-items:center;gap:18px;padding:17px 18px;cursor:pointer;list-style:none}.task summary::-webkit-details-marker{display:none}.task summary span{display:flex;min-width:0;flex-direction:column}.task summary strong{overflow:hidden;text-overflow:ellipsis}.task summary small{color:var(--muted)}.task summary b{color:var(--accent);font-size:12px}.criteria{overflow:auto;border-top:1px solid var(--rule)}table{width:100%;border-collapse:collapse}th,td{padding:10px 12px;border-bottom:1px solid #e5e9e4;text-align:left;vertical-align:top}th{font-weight:650}td:last-child{color:var(--muted)}.verdict{display:inline-block;min-width:48px;font-size:12px;font-weight:750}.verdict.pass{color:#27715b}.verdict.fail{color:#b7352d}.verdict.human{color:#8b651c}.empty{display:grid;min-height:60vh;place-items:center;align-content:center;text-align:center}.empty span{font-size:52px;filter:grayscale(1)}.empty h1{margin:10px 0 0}.empty p{color:var(--muted)}[hidden]{display:none!important}@media(max-width:900px){.site-head{align-items:flex-start}.wordmark span{display:none}.bench-tab span:last-child{display:none}.hero{grid-template-columns:1fr}.hero dl{grid-template-columns:repeat(2,1fr);gap:16px}.task-list{grid-template-columns:1fr}}@media(prefers-reduced-motion:reduce){*{scroll-behavior:auto!important}}
"#;

const LEADERBOARD_CSS: &str = concat!(
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

const PROMPT_PAGE_CSS: &str = r#"
.muted{color:var(--muted)}.prompt-page{max-width:1680px}.prompt-carousel{display:grid;grid-template-columns:120px minmax(240px,560px) 120px;align-items:end;justify-content:space-between;gap:18px;margin-bottom:28px}.prompt-carousel>a,.nav-disabled{padding:10px 12px;border:1px solid var(--rule);text-align:center;text-decoration:none;color:var(--ink)}.nav-disabled{color:var(--muted);opacity:.45}.prompt-carousel label{display:flex;flex-direction:column;gap:5px}.prompt-carousel label span{color:var(--muted);font-size:12px}.prompt-carousel select{width:100%;padding:10px;border:1px solid var(--rule);background:var(--surface);font:inherit}.prompt-head{display:flex;align-items:end;justify-content:space-between;gap:28px;padding-bottom:24px;border-bottom:3px solid var(--ink)}.prompt-head p{margin:0;color:var(--muted)}.prompt-head h1{max-width:900px;margin:5px 0 0;font-size:clamp(30px,5vw,62px);line-height:1;letter-spacing:-.045em;overflow-wrap:anywhere}.run-verdict{min-width:230px;padding:16px 18px;border-left:7px solid var(--rule);background:var(--surface)}.run-verdict strong,.run-verdict span,.run-verdict small{display:block}.run-verdict strong{font-size:22px}.run-verdict.pass{border-color:#27715b}.run-verdict.pass strong{color:#27715b}.run-verdict.fail,.run-verdict.error{border-color:#b7352d}.run-verdict.fail strong,.run-verdict.error strong{color:#b7352d}.run-verdict small{color:var(--muted)}.prompt-layout{display:grid;grid-template-columns:minmax(0,1fr) minmax(440px,.9fr);gap:28px;margin-top:28px}.prompt-copy,.standards-list,.model-stage{border:1px solid var(--rule);background:var(--surface)}.prompt-copy h2,.standards-list>header,.model-stage>header{margin:0;padding:15px 18px;border-bottom:1px solid var(--rule)}.prompt-copy pre{max-width:100%;margin:0;padding:20px;white-space:pre-wrap;overflow-wrap:anywhere;word-break:break-word;font:15px/1.6 inherit}.standards-list{margin-top:18px}.standards-list>header h2,.model-stage h2{margin:0}.standards-list>header p,.model-stage header span{color:var(--muted)}.standard-card{padding:17px 18px;border-bottom:1px solid var(--rule)}.standard-card:last-child{border-bottom:0}.standard-card>header{display:flex;gap:14px;align-items:start}.standard-card h3{margin:0;font-size:17px}.standard-card code{color:var(--muted)}.standard-card>p{margin:10px 0}.result-mark{min-width:64px;padding:3px 7px;text-align:center;font-size:11px;font-weight:800}.standard-card.pass .result-mark{background:#dcebe4;color:#176345}.standard-card.fail .result-mark{background:#f2ddda;color:#9f2924}.standard-card.human .result-mark{background:#f1e8cf;color:#76590c}.evidence-copy{padding:11px 13px;background:var(--paper)}.evidence-copy strong{font-size:11px;text-transform:uppercase;letter-spacing:.07em}.evidence-copy p{margin:3px 0}.model-stage{position:sticky;top:84px;align-self:start}.model-stage canvas{display:block;width:100%;height:min(66vh,680px);background:#181c1a;cursor:grab}.model-stage canvas:active{cursor:grabbing}.model-tools{display:flex;justify-content:space-between;gap:16px;padding:11px 14px;color:var(--muted);font-size:12px}.model-tools a{color:var(--ink)}.model-empty,.empty-detail{margin:18px;padding:18px;border:1px dashed var(--rule);color:var(--muted)}.model-empty p,.empty-detail p{margin:4px 0 0}@media(max-width:980px){.prompt-layout{grid-template-columns:1fr}.model-stage{position:static;grid-row:1}.model-stage canvas{height:52vh}}@media(max-width:620px){.prompt-carousel{grid-template-columns:1fr 1fr}.prompt-carousel label{grid-column:1/-1;grid-row:1}.prompt-head{align-items:stretch;flex-direction:column}.run-verdict{min-width:0}}
"#;

const PROMPT_PAGE_JS: &str = r#"
document.querySelector('#prompt-select').addEventListener('change',event=>location.href=event.target.value);
const canvas=document.querySelector('#stl-viewer');
if(canvas){const context=canvas.getContext('2d');let triangles=[],rx=-.55,ry=.65,zoom=1,drag=false,lastX=0,lastY=0;
const vector=(view,offset)=>[view.getFloat32(offset,true),view.getFloat32(offset+4,true),view.getFloat32(offset+8,true)];
function parse(buffer){const view=new DataView(buffer),count=buffer.byteLength>=84?view.getUint32(80,true):0;if(84+count*50===buffer.byteLength){for(let i=0;i<count;i++){const base=84+i*50;triangles.push([vector(view,base+12),vector(view,base+24),vector(view,base+36)])}}else{const text=new TextDecoder().decode(buffer),vertices=[...text.matchAll(/vertex\s+([\-\d.e+]+)\s+([\-\d.e+]+)\s+([\-\d.e+]+)/gi)].map(match=>[+match[1],+match[2],+match[3]]);for(let i=0;i+2<vertices.length;i+=3)triangles.push(vertices.slice(i,i+3))}fit();draw()}
function fit(){const values=triangles.flat(2),xs=values.filter((_,i)=>i%3===0),ys=values.filter((_,i)=>i%3===1),zs=values.filter((_,i)=>i%3===2),cx=(Math.min(...xs)+Math.max(...xs))/2,cy=(Math.min(...ys)+Math.max(...ys))/2,cz=(Math.min(...zs)+Math.max(...zs))/2,extent=Math.max(Math.max(...xs)-Math.min(...xs),Math.max(...ys)-Math.min(...ys),Math.max(...zs)-Math.min(...zs))||1;triangles=triangles.map(triangle=>triangle.map(vertex=>[(vertex[0]-cx)/extent,(vertex[1]-cy)/extent,(vertex[2]-cz)/extent]))}
function rotate(vertex){let[x,y,z]=vertex,cy=Math.cos(ry),sy=Math.sin(ry),cx=Math.cos(rx),sx=Math.sin(rx),x1=x*cy+z*sy,z1=-x*sy+z*cy;return[x1,y*cx-z1*sx,y*sx+z1*cx]}
function draw(){const ratio=devicePixelRatio||1,w=canvas.clientWidth,h=canvas.clientHeight;canvas.width=w*ratio;canvas.height=h*ratio;context.setTransform(ratio,0,0,ratio,0,0);context.clearRect(0,0,w,h);const scale=Math.min(w,h)*.78*zoom,project=vertex=>{const p=rotate(vertex);return{x:w/2+p[0]*scale,y:h/2-p[1]*scale,z:p[2]}};const faces=triangles.map(triangle=>triangle.map(project)).sort((a,b)=>a.reduce((s,p)=>s+p.z,0)-b.reduce((s,p)=>s+p.z,0));faces.forEach(face=>{const ax=face[1].x-face[0].x,ay=face[1].y-face[0].y,bx=face[2].x-face[0].x,by=face[2].y-face[0].y,light=Math.max(.16,Math.min(.92,.48+(ax*by-ay*bx)/Math.max(1,Math.abs(ax*by-ay*bx))*.24));context.beginPath();context.moveTo(face[0].x,face[0].y);context.lineTo(face[1].x,face[1].y);context.lineTo(face[2].x,face[2].y);context.closePath();context.fillStyle=`rgb(${70+light*90},${82+light*100},${76+light*92})`;context.fill();context.strokeStyle='#242b27';context.lineWidth=.35;context.stroke()})}
canvas.addEventListener('pointerdown',event=>{drag=true;lastX=event.clientX;lastY=event.clientY;canvas.setPointerCapture(event.pointerId)});canvas.addEventListener('pointermove',event=>{if(!drag)return;ry+=(event.clientX-lastX)*.01;rx+=(event.clientY-lastY)*.01;lastX=event.clientX;lastY=event.clientY;draw()});canvas.addEventListener('pointerup',()=>drag=false);canvas.addEventListener('wheel',event=>{event.preventDefault();zoom=Math.max(.35,Math.min(4,zoom*Math.exp(-event.deltaY*.001)));draw()},{passive:false});addEventListener('resize',draw);fetch(canvas.dataset.src).then(response=>response.arrayBuffer()).then(parse).catch(()=>{canvas.replaceWith(Object.assign(document.createElement('p'),{textContent:'STL could not be loaded.'}))})}
"#;

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(html.matches("Description not recorded").count(), 4);
        assert!(html.contains("No benchmark configurations recorded"));
        assert!(!html.contains("class='bench-primer"));
    }
}
