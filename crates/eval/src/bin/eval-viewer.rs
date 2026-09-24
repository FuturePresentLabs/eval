use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use clap::Parser;
use eval::SuiteReport;
use eval::viewer::{BenchmarkKind, BenchmarkPrimer, BenchmarkView, ModelView};
use pulldown_cmark::{Options, Parser as MarkdownParser, html};

#[derive(Debug, Parser)]
#[command(about = "Build one self-contained PCB/CAD/CAM/DFM benchmark viewer")]
struct Args {
    /// Model-to-benchmark result index (`eval.leaderboard.v1`).
    #[arg(long)]
    index: Option<PathBuf>,
    #[arg(long)]
    pcb: Option<PathBuf>,
    #[arg(long)]
    cad: Option<PathBuf>,
    #[arg(long)]
    cam: Option<PathBuf>,
    #[arg(long)]
    dfm: Option<PathBuf>,
    #[arg(long, default_value = "benchmark-viewer.html")]
    out: PathBuf,
    /// Serve a live, auto-refreshing monitor on this localhost port.
    #[arg(long)]
    serve: Option<u16>,
    /// Address to bind in serve mode. Use the machine's Tailscale IP for tailnet access.
    #[arg(long, default_value = "127.0.0.1")]
    bind: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if let Some(port) = args.serve {
        return serve(&args, &args.bind, port);
    }
    write_snapshot(&args)?;
    println!("viewer -> {}", args.out.display());
    Ok(())
}

fn write_snapshot(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let html = render(args)?;
    if let Some(parent) = args
        .out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.out, html)?;
    Ok(())
}

fn render(args: &Args) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(path) = &args.index {
        return render_index(path);
    }
    let pcb = load(args.pcb.as_deref())?;
    let cad = load(args.cad.as_deref())?;
    let cam = load(args.cam.as_deref())?;
    let dfm = load(args.dfm.as_deref())?;
    let views = [
        BenchmarkView {
            kind: BenchmarkKind::Pcb,
            report: pcb.as_ref(),
        },
        BenchmarkView {
            kind: BenchmarkKind::Cad,
            report: cad.as_ref(),
        },
        BenchmarkView {
            kind: BenchmarkKind::Cam,
            report: cam.as_ref(),
        },
        BenchmarkView {
            kind: BenchmarkKind::Dfm,
            report: dfm.as_ref(),
        },
    ];
    Ok(eval::viewer::render(&views))
}

struct LoadedModel {
    id: String,
    name: String,
    organization: Option<String>,
    harness: String,
    rlcd_model: Option<String>,
    generative_model: Option<String>,
    alias: Option<String>,
    pcb: Option<SuiteReport>,
    cad: Option<SuiteReport>,
    cam: Option<SuiteReport>,
    dfm: Option<SuiteReport>,
    pcb_metrics: eval::viewer::RunMetrics,
    cad_metrics: eval::viewer::RunMetrics,
    cam_metrics: eval::viewer::RunMetrics,
    dfm_metrics: eval::viewer::RunMetrics,
}

fn render_index(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let index: eval::LeaderboardIndex = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    if index.schema != eval::LEADERBOARD_SCHEMA {
        return Err(format!("unsupported leaderboard schema {:?}", index.schema).into());
    }
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let mut primers = Vec::new();
    for (id, source) in &index.benchmarks {
        let Some(kind) = benchmark_kind(id) else {
            continue;
        };
        let readme_path = root.join(&source.readme);
        let markdown = std::fs::read_to_string(&readme_path).map_err(|error| {
            format!(
                "cannot read {id} benchmark README {}: {error}",
                readme_path.display()
            )
        })?;
        primers.push(BenchmarkPrimer {
            kind,
            html: readme_primer(&markdown, &source.sections),
        });
    }
    let mut loaded = Vec::new();
    for model in index.models {
        let report = |kind: &str| -> Result<Option<SuiteReport>, Box<dyn std::error::Error>> {
            let Some(result) = model.results.get(kind) else {
                return Ok(None);
            };
            load(Some(&root.join(&result.report)))
        };
        let metrics = |kind: &str, report: Option<&SuiteReport>| {
            model.results.get(kind).map_or(
                eval::viewer::RunMetrics {
                    average_seconds_per_task: report.and_then(average_seconds),
                    average_cost_usd_per_task: None,
                },
                |result| eval::viewer::RunMetrics {
                    average_seconds_per_task: result
                        .average_seconds_per_task
                        .or_else(|| report.and_then(average_seconds)),
                    average_cost_usd_per_task: result.average_cost_usd_per_task,
                },
            )
        };
        let pcb = report("pcb")?;
        let cad = report("cad")?;
        let cam = report("cam")?;
        let dfm = report("dfm")?;
        loaded.push(LoadedModel {
            id: model.id,
            name: model.name,
            organization: model.organization,
            harness: model.harness,
            rlcd_model: model.rlcd_model,
            generative_model: model.generative_model,
            alias: model.alias,
            pcb_metrics: metrics("pcb", pcb.as_ref()),
            cad_metrics: metrics("cad", cad.as_ref()),
            cam_metrics: metrics("cam", cam.as_ref()),
            dfm_metrics: metrics("dfm", dfm.as_ref()),
            pcb,
            cad,
            cam,
            dfm,
        });
    }
    let views = loaded
        .iter()
        .map(|model| ModelView {
            id: &model.id,
            name: &model.name,
            organization: model.organization.as_deref(),
            harness: &model.harness,
            rlcd_model: model.rlcd_model.as_deref(),
            generative_model: model.generative_model.as_deref(),
            alias: model.alias.as_deref(),
            pcb: model.pcb.as_ref(),
            cad: model.cad.as_ref(),
            cam: model.cam.as_ref(),
            dfm: model.dfm.as_ref(),
            pcb_metrics: model.pcb_metrics,
            cad_metrics: model.cad_metrics,
            cam_metrics: model.cam_metrics,
            dfm_metrics: model.dfm_metrics,
        })
        .collect::<Vec<_>>();
    Ok(eval::viewer::render_leaderboard(&views, &primers))
}

fn benchmark_kind(id: &str) -> Option<BenchmarkKind> {
    match id {
        "pcb" => Some(BenchmarkKind::Pcb),
        "cad" => Some(BenchmarkKind::Cad),
        "cam" => Some(BenchmarkKind::Cam),
        "dfm" => Some(BenchmarkKind::Dfm),
        _ => None,
    }
}

fn readme_primer(markdown: &str, selected: &[String]) -> String {
    let lines = markdown
        .lines()
        .filter(|line| {
            !line.contains("[![")
                && !line.contains("Badge numbers are generated")
                && !line.contains("scripts/update-badges.sh")
        })
        .collect::<Vec<_>>();
    let mut excerpt = Vec::new();
    let intro_start = lines
        .iter()
        .position(|line| line.starts_with("# "))
        .map_or(0, |index| index + 1);
    let intro_end = lines[intro_start..]
        .iter()
        .position(|line| line.starts_with("## "))
        .map_or(lines.len(), |index| intro_start + index);
    excerpt.extend_from_slice(&lines[intro_start..intro_end]);
    for heading in selected {
        if let Some(start) = lines.iter().position(|line| {
            line.strip_prefix("## ")
                .is_some_and(|value| value.trim() == heading)
        }) {
            let end = lines[start + 1..]
                .iter()
                .position(|line| line.starts_with("## "))
                .map_or(lines.len(), |index| start + 1 + index);
            excerpt.extend_from_slice(&lines[start..end]);
        }
    }
    let markdown = excerpt.join("\n");
    let parser = MarkdownParser::new_ext(&markdown, Options::ENABLE_TABLES);
    let mut rendered = String::new();
    html::push_html(&mut rendered, parser);
    rendered
}

fn average_seconds(report: &SuiteReport) -> Option<f64> {
    (!report.tasks.is_empty()).then(|| {
        report
            .tasks
            .iter()
            .map(|task| task.elapsed_ms as f64)
            .sum::<f64>()
            / report.tasks.len() as f64
            / 1000.0
    })
}

fn serve(args: &Args, bind: &str, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind((bind, port))?;
    println!("live viewer -> http://{bind}:{port}");
    for stream in listener.incoming() {
        let mut stream = stream?;
        let mut request = [0_u8; 2048];
        let read = stream.read(&mut request)?;
        let first_line = String::from_utf8_lossy(&request[..read]);
        let path = first_line
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");
        if path.starts_with("/prompt/") {
            let (status, body) = match prompt_page(args, path)? {
                Some(body) => ("200 OK", body),
                None => ("404 Not Found", "prompt not found\n".to_owned()),
            };
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )?;
            continue;
        }
        if let Some((content_type, bytes)) = artifact(args, path)? {
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
                bytes.len()
            )?;
            stream.write_all(&bytes)?;
            continue;
        }
        let (status, content_type, body) = match path {
            "/" | "/index.html" => ("200 OK", "text/html; charset=utf-8", render(args)?),
            "/api/revision" => (
                "200 OK",
                "text/plain; charset=utf-8",
                revision(args).to_string(),
            ),
            _ => (
                "404 Not Found",
                "text/plain; charset=utf-8",
                "not found\n".to_owned(),
            ),
        };
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )?;
    }
    Ok(())
}

fn prompt_page(args: &Args, path: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let Some(index_path) = &args.index else {
        return Ok(None);
    };
    let parts = path.trim_start_matches('/').split('/').collect::<Vec<_>>();
    if parts.len() != 4 || parts[0] != "prompt" {
        return Ok(None);
    }
    let Some(kind) = benchmark_kind(parts[2]) else {
        return Ok(None);
    };
    let Some(position) = parts[3].parse::<usize>().ok() else {
        return Ok(None);
    };
    let index: eval::LeaderboardIndex =
        serde_json::from_str(&std::fs::read_to_string(index_path)?)?;
    let Some(model) = index.models.iter().find(|model| model.id == parts[1]) else {
        return Ok(None);
    };
    let Some(result) = model.results.get(parts[2]) else {
        return Ok(None);
    };
    let root = index_path.parent().unwrap_or_else(|| Path::new("."));
    let Some(report) = load(Some(&root.join(&result.report)))? else {
        return Ok(None);
    };
    Ok(eval::viewer::render_prompt_page(
        &model.id,
        &model.name,
        &model.harness,
        kind,
        &report,
        position,
    ))
}

fn artifact(
    args: &Args,
    path: &str,
) -> Result<Option<(&'static str, Vec<u8>)>, Box<dyn std::error::Error>> {
    let Some(rest) = path.strip_prefix("/artifact/") else {
        return Ok(None);
    };
    let parts = rest.split('/').collect::<Vec<_>>();
    let first = parts.first().copied().unwrap_or("");
    let (model_id, kind, offset) = if matches!(first, "pcb" | "cad" | "cam" | "dfm") {
        (None, first, 1)
    } else {
        (Some(first), parts.get(1).copied().unwrap_or(""), 2)
    };
    let Some(index) = parts
        .get(offset)
        .and_then(|value| value.parse::<usize>().ok())
    else {
        return Ok(None);
    };
    if parts.len() <= offset + 1 {
        return Ok(None);
    }
    let relative = parts[offset + 1..].join("/");
    let report_path = if let Some(model_id) = model_id {
        let Some(index_path) = &args.index else {
            return Ok(None);
        };
        let index: eval::LeaderboardIndex =
            serde_json::from_str(&std::fs::read_to_string(index_path)?)?;
        let Some(result) = index
            .models
            .iter()
            .find(|model| model.id == model_id)
            .and_then(|model| model.results.get(kind))
        else {
            return Ok(None);
        };
        Some(
            index_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(&result.report),
        )
    } else {
        match kind {
            "pcb" => args.pcb.clone(),
            "cad" => args.cad.clone(),
            "cam" => args.cam.clone(),
            "dfm" => args.dfm.clone(),
            _ => None,
        }
    };
    let Some(report) = load(report_path.as_deref())? else {
        return Ok(None);
    };
    let Some(task) = report.tasks.get(index) else {
        return Ok(None);
    };
    let root = task.work_dir.canonicalize()?;
    let file = root.join(&relative).canonicalize()?;
    if !file.starts_with(&root) || !file.is_file() {
        return Ok(None);
    }
    let content_type = match file
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "json" => "application/json",
        "csv" => "text/csv; charset=utf-8",
        "glb" => "model/gltf-binary",
        "gltf" => "model/gltf+json",
        "stl" => "model/stl",
        _ => "application/octet-stream",
    };
    Ok(Some((content_type, std::fs::read(file)?)))
}

fn revision(args: &Args) -> u128 {
    let mut paths = [&args.index, &args.pcb, &args.cad, &args.cam, &args.dfm]
        .into_iter()
        .filter_map(Option::as_ref)
        .cloned()
        .collect::<Vec<_>>();
    if let Some(index_path) = &args.index {
        if let Ok(index) = std::fs::read_to_string(index_path)
            .ok()
            .and_then(|text| serde_json::from_str::<eval::LeaderboardIndex>(&text).ok())
            .ok_or(())
        {
            let root = index_path.parent().unwrap_or_else(|| Path::new("."));
            paths.extend(index.models.into_iter().flat_map(|model| {
                model
                    .results
                    .into_values()
                    .map(|result| root.join(result.report))
            }));
        }
    }
    paths
        .iter()
        .filter_map(|path| std::fs::metadata(path).ok()?.modified().ok())
        .filter_map(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .max()
        .unwrap_or(0)
}

fn load(path: Option<&Path>) -> Result<Option<SuiteReport>, Box<dyn std::error::Error>> {
    path.map(|path| {
        let report: SuiteReport = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        if report.schema != "eval.suite-report.v1" {
            return Err(format!(
                "{} uses unsupported schema {:?}; expected eval.suite-report.v1",
                path.display(),
                report.schema
            )
            .into());
        }
        Ok(report)
    })
    .transpose()
}
