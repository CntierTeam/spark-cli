use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use regex::Regex;

use crate::filter::{FilterOpts, WindowSelect};
use crate::graph::ProfileGraph;
use crate::load::{detect_kind, load_bytes, ContentKind};
use crate::render::{self, Format, LabelMode, RenderOpts};
use crate::views::{self, FlatMode, ProfileView, SourcesMode};

#[derive(Parser, Debug)]
#[command(
    name = "spark-dump",
    about = "lucko spark CLI — viewer-parity dump, filters, views (all/flat/sources/flame)",
    version,
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Auto-detect profile/heap/health and dump
    Dump(DumpArgs),
    /// Sampler profile views (all / flat / sources / flame)
    Profile(ProfileArgs),
    /// Heap histogram
    Heap(HeapArgs),
    /// Health report
    Health(HealthArgs),
    /// Raw protobuf → JSON (spark2json-compatible)
    Raw(RawArgs),
    /// Metadata / widgets only
    Meta(MetaArgs),
    /// Built-in tutorial
    Tutorial {
        /// Topic: overview | views | filters | examples (default: overview)
        topic: Option<String>,
    },
    /// List threads ranked by time (multi-thread profiles)
    Threads(ThreadsArgs),
    /// List Refine time windows (index / id / share) — web timeline buckets
    Windows(WindowsArgs),
    /// Plugin/mod occupancy (web Sources view)
    Plugins(PluginsArgs),
}

#[derive(Debug, Clone, ValueEnum)]
pub enum FormatArg {
    Text,
    Json,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ViewArg {
    All,
    Flat,
    Sources,
    Flame,
    Summary,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum FlatModeArg {
    SelfTime,
    TotalTime,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum SourcesModeArg {
    Merge,
    Separate,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum LabelArg {
    Percent,
    Absolute,
}

#[derive(Args, Debug)]
pub struct CommonOut {
    /// Output file (default stdout)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
    /// Output format
    #[arg(short, long, value_enum, default_value_t = FormatArg::Text)]
    pub format: FormatArg,
}

#[derive(Args, Debug)]
pub struct FilterArgs {
    /// Search query (web SearchBar): class/method/source/thread substring
    #[arg(long, short = 's')]
    pub search: Option<String>,
    /// Only threads whose name matches (substring or regex if --regex)
    #[arg(long)]
    pub thread: Option<String>,
    /// Include only nodes matching (repeatable). Matches class/method/source.
    #[arg(long = "include")]
    pub include: Vec<String>,
    /// Exclude nodes matching (repeatable)
    #[arg(long = "exclude")]
    pub exclude: Vec<String>,
    /// Treat --thread/--include/--exclude as regex
    #[arg(long)]
    pub regex: bool,
    /// Hide tree nodes below this % of current root
    #[arg(long, default_value_t = 0.5)]
    pub min_percent: f64,
    /// Max tree depth
    #[arg(long, default_value_t = 16)]
    pub depth: usize,
    /// Flat/sources top N (web flat default 250)
    #[arg(long, default_value_t = 250)]
    pub top: usize,
    /// Time window ids (comma-separated) or "all". Aligns with web Refine.
    /// Prefer indices with `@0,@3` / `i0,i3` when picking from `windows` output.
    #[arg(long)]
    pub windows: Option<String>,
    /// Time window index range: start,end (inclusive indices into timeWindows)
    #[arg(long)]
    pub window_range: Option<String>,
    /// Only dump the top N threads by time (0 = all)
    #[arg(long, default_value_t = 0)]
    pub top_threads: usize,
    /// Hide threads whose share of profile total is below this percent
    #[arg(long, default_value_t = 0.0)]
    pub min_thread_percent: f64,
}

#[derive(Args, Debug)]
pub struct WindowFilterArgs {
    /// Time window ids (`29815743`), indices (`@0,@3` / `i0,i3`), or `all`
    #[arg(long)]
    pub windows: Option<String>,
    /// Inclusive index range: start,end
    #[arg(long)]
    pub window_range: Option<String>,
}

#[derive(Args, Debug)]
pub struct ThreadsArgs {
    pub input: String,
    #[command(flatten)]
    pub out: CommonOut,
    #[command(flatten)]
    pub window: WindowFilterArgs,
    /// Substring / regex filter on thread name
    #[arg(long, short = 's')]
    pub search: Option<String>,
    #[arg(long)]
    pub regex: bool,
    /// Show top N (default 50, 0 = all)
    #[arg(long, default_value_t = 50)]
    pub top: usize,
}

#[derive(Args, Debug)]
pub struct WindowsArgs {
    pub input: String,
    #[command(flatten)]
    pub out: CommonOut,
    /// Show only top N hottest windows (0 = all)
    #[arg(long, default_value_t = 0)]
    pub top: usize,
    /// Hide windows below this % of profile total
    #[arg(long, default_value_t = 0.0)]
    pub min_percent: f64,
    /// Also print ready-to-run profile commands for each shown window
    #[arg(long)]
    pub commands: bool,
}

#[derive(Args, Debug)]
pub struct PluginsArgs {
    pub input: String,
    #[command(flatten)]
    pub out: CommonOut,
    #[command(flatten)]
    pub filter: FilterArgs,
    /// Merge same method signatures (web Merge Mode)
    #[arg(long, value_enum, default_value_t = SourcesModeArg::Merge)]
    pub sources_mode: SourcesModeArg,
    /// Also print per-plugin hot methods
    #[arg(long, default_value_t = true)]
    pub detail: bool,
    #[arg(long)]
    pub no_detail: bool,
}

#[derive(Args, Debug)]
pub struct DumpArgs {
    /// Input file or bytebin code
    pub input: String,
    #[command(flatten)]
    pub out: CommonOut,
    #[command(flatten)]
    pub filter: FilterArgs,
    #[arg(long, value_enum, default_value_t = ViewArg::Summary)]
    pub view: ViewArg,
}

#[derive(Args, Debug)]
pub struct ProfileArgs {
    pub input: String,
    #[command(flatten)]
    pub out: CommonOut,
    #[command(flatten)]
    pub filter: FilterArgs,
    /// View mode (web: All / Flat / Sources / Flame)
    #[arg(long, short = 'v', value_enum, default_value_t = ViewArg::All)]
    pub view: ViewArg,
    /// Flat self vs total (web Self Time Mode)
    #[arg(long, value_enum, default_value_t = FlatModeArg::SelfTime)]
    pub flat_mode: FlatModeArg,
    /// Sources merge vs separate (web Merge Mode)
    #[arg(long, value_enum, default_value_t = SourcesModeArg::Merge)]
    pub sources_mode: SourcesModeArg,
    /// Bottom-up parents in flat view
    #[arg(long)]
    pub bottom_up: bool,
    /// Flame: pick thread by name substring (default: first / only)
    #[arg(long)]
    pub flame_thread: Option<String>,
    /// Label style
    #[arg(long, value_enum, default_value_t = LabelArg::Percent)]
    pub label: LabelArg,
    /// Also print metadata/widgets header
    #[arg(long, default_value_t = true)]
    pub meta: bool,
    #[arg(long)]
    pub no_meta: bool,
}

#[derive(Args, Debug)]
pub struct HeapArgs {
    pub input: String,
    #[command(flatten)]
    pub out: CommonOut,
    #[arg(long, short = 's')]
    pub search: Option<String>,
    #[arg(long, default_value_t = 100)]
    pub top: usize,
}

#[derive(Args, Debug)]
pub struct HealthArgs {
    pub input: String,
    #[command(flatten)]
    pub out: CommonOut,
}

#[derive(Args, Debug)]
pub struct RawArgs {
    pub input: String,
    #[command(flatten)]
    pub out: CommonOut,
    /// Force type
    #[arg(long, value_enum)]
    pub r#type: Option<TypeArg>,
}

#[derive(Args, Debug)]
pub struct MetaArgs {
    pub input: String,
    #[command(flatten)]
    pub out: CommonOut,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum TypeArg {
    Sampler,
    Heap,
    Health,
}

fn write_out(out: &Option<PathBuf>, s: &str) -> io::Result<()> {
    match out {
        Some(p) => {
            fs::write(p, s)?;
            eprintln!("Wrote {} ({} bytes)", p.display(), s.len());
        }
        None => {
            let mut stdout = io::stdout().lock();
            stdout.write_all(s.as_bytes())?;
        }
    }
    Ok(())
}

fn format_of(f: &FormatArg) -> Format {
    match f {
        FormatArg::Text => Format::Text,
        FormatArg::Json => Format::Json,
    }
}

fn compile_pat(s: &str, regex: bool) -> Result<FilterPat, Box<dyn std::error::Error>> {
    if regex {
        Ok(FilterPat::Regex(Regex::new(s)?))
    } else {
        Ok(FilterPat::Substr(s.to_ascii_lowercase()))
    }
}

#[derive(Clone)]
pub enum FilterPat {
    Substr(String),
    Regex(Regex),
}

impl FilterPat {
    pub fn is_match(&self, hay: &str) -> bool {
        match self {
            FilterPat::Substr(s) => hay.to_ascii_lowercase().contains(s),
            FilterPat::Regex(r) => r.is_match(hay),
        }
    }
}

fn build_filter(f: &FilterArgs) -> Result<FilterOpts, Box<dyn std::error::Error>> {
    let mut includes = Vec::new();
    for s in &f.include {
        includes.push(compile_pat(s, f.regex)?);
    }
    let mut excludes = Vec::new();
    for s in &f.exclude {
        excludes.push(compile_pat(s, f.regex)?);
    }
    let thread = f
        .thread
        .as_ref()
        .map(|s| compile_pat(s, f.regex))
        .transpose()?;
    let windows = parse_window_select(f.windows.as_deref(), f.window_range.as_deref())?;
    Ok(FilterOpts {
        search: f.search.clone().map(|s| s.to_ascii_lowercase()),
        thread,
        includes,
        excludes,
        min_percent: f.min_percent,
        depth: f.depth,
        top: f.top,
        windows,
        top_threads: f.top_threads,
        min_thread_percent: f.min_thread_percent,
    })
}

fn parse_window_select(
    windows: Option<&str>,
    window_range: Option<&str>,
) -> Result<WindowSelect, Box<dyn std::error::Error>> {
    if let Some(range) = window_range {
        let parts: Vec<_> = range.split(',').collect();
        if parts.len() != 2 {
            return Err("--window-range expects start,end".into());
        }
        let a: usize = parts[0].trim().parse()?;
        let b: usize = parts[1].trim().parse()?;
        return Ok(WindowSelect::Range(a, b));
    }
    if let Some(w) = windows {
        if w.eq_ignore_ascii_case("all") {
            return Ok(WindowSelect::All);
        }
        let tokens: Vec<&str> = w.split(',').map(|x| x.trim()).filter(|x| !x.is_empty()).collect();
        if tokens.is_empty() {
            return Err("--windows is empty".into());
        }
        let index_like = tokens.iter().all(|t| {
            t.starts_with('@')
                || t.starts_with('i')
                || t.starts_with('I')
        });
        if index_like {
            let mut idxs = Vec::new();
            for t in tokens {
                let num = t
                    .trim_start_matches(['@', 'i', 'I'])
                    .parse::<usize>()
                    .map_err(|_| format!("invalid window index token '{t}' (use @0 or i0)"))?;
                idxs.push(num);
            }
            return Ok(WindowSelect::Indices(idxs));
        }
        let ids: Result<Vec<i32>, _> = tokens.iter().map(|x| x.parse()).collect();
        return Ok(WindowSelect::Ids(ids.map_err(|e| {
            format!("invalid --windows value (use ids or @index): {e}")
        })?));
    }
    Ok(WindowSelect::All)
}

fn resolve_windows(
    graph: &ProfileGraph,
    sel: &WindowSelect,
) -> Result<std::collections::HashSet<usize>, Box<dyn std::error::Error>> {
    let set = graph.selected_window_indices(sel);
    if set.is_empty() {
        return Err(format!(
            "no time windows matched selection (profile has {} window(s); try `spark-dump windows <file>`)",
            graph.window_count()
        )
        .into());
    }
    Ok(set)
}

fn fmt_offset_ms(ms: i64) -> String {
    if ms < 0 {
        return "-".into();
    }
    let s = ms / 1000;
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if h > 0 {
        format!("{h}h{m:02}m{sec:02}s")
    } else if m > 0 {
        format!("{m}m{sec:02}s")
    } else {
        format!("{sec}s")
    }
}

fn view_of(v: &ViewArg) -> ProfileView {
    match v {
        ViewArg::All => ProfileView::All,
        ViewArg::Flat => ProfileView::Flat,
        ViewArg::Sources => ProfileView::Sources,
        ViewArg::Flame => ProfileView::Flame,
        ViewArg::Summary => ProfileView::Summary,
    }
}

pub fn run_dump(args: DumpArgs) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = load_bytes(&args.input)?;
    let kind = detect_kind(&args.input, None, &bytes)?;
    match kind {
        ContentKind::Sampler => {
            let profile = ProfileArgs {
                input: args.input,
                out: args.out,
                filter: args.filter,
                view: args.view,
                flat_mode: FlatModeArg::SelfTime,
                sources_mode: SourcesModeArg::Merge,
                bottom_up: false,
                flame_thread: None,
                label: LabelArg::Percent,
                meta: true,
                no_meta: false,
            };
            run_profile(profile)
        }
        ContentKind::Heap => run_heap(HeapArgs {
            input: args.input,
            out: args.out,
            search: args.filter.search,
            top: args.filter.top,
        }),
        ContentKind::Health => run_health(HealthArgs {
            input: args.input,
            out: args.out,
        }),
    }
}

pub fn run_profile(args: ProfileArgs) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = load_bytes(&args.input)?;
    let graph = ProfileGraph::from_sampler_bytes(&bytes)?;
    let filter = build_filter(&args.filter)?;
    let _ = resolve_windows(&graph, &filter.windows)?;
    let render_opts = RenderOpts {
        format: format_of(&args.out.format),
        label: match args.label {
            LabelArg::Percent => LabelMode::Percent,
            LabelArg::Absolute => LabelMode::Absolute,
        },
        show_meta: args.meta && !args.no_meta,
        view: view_of(&args.view),
        flat_mode: match args.flat_mode {
            FlatModeArg::SelfTime => FlatMode::SelfTime,
            FlatModeArg::TotalTime => FlatMode::TotalTime,
        },
        sources_mode: match args.sources_mode {
            SourcesModeArg::Merge => SourcesMode::Merge,
            SourcesModeArg::Separate => SourcesMode::Separate,
        },
        bottom_up: args.bottom_up,
        flame_thread: args.flame_thread.clone(),
    };
    let out = views::render_profile(&graph, &filter, &render_opts)?;
    write_out(&args.out.out, &out)?;
    Ok(())
}

pub fn run_heap(args: HeapArgs) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = load_bytes(&args.input)?;
    let out = render::render_heap_bytes(&bytes, args.search.as_deref(), args.top, format_of(&args.out.format))?;
    write_out(&args.out.out, &out)?;
    Ok(())
}

pub fn run_health(args: HealthArgs) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = load_bytes(&args.input)?;
    let out = render::render_health_bytes(&bytes, format_of(&args.out.format))?;
    write_out(&args.out.out, &out)?;
    Ok(())
}

pub fn run_raw(args: RawArgs) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = load_bytes(&args.input)?;
    let kind = detect_kind(
        &args.input,
        args.r#type.as_ref().map(|t| match t {
            TypeArg::Sampler => ContentKind::Sampler,
            TypeArg::Heap => ContentKind::Heap,
            TypeArg::Health => ContentKind::Health,
        }),
        &bytes,
    )?;
    let out = render::render_raw_json(&bytes, kind)?;
    write_out(&args.out.out, &out)?;
    Ok(())
}

pub fn run_meta(args: MetaArgs) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = load_bytes(&args.input)?;
    let kind = detect_kind(&args.input, None, &bytes)?;
    let out = render::render_meta_only(&bytes, kind, format_of(&args.out.format))?;
    write_out(&args.out.out, &out)?;
    Ok(())
}

pub fn run_threads(args: ThreadsArgs) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = load_bytes(&args.input)?;
    let graph = ProfileGraph::from_sampler_bytes(&bytes)?;
    let pat = args
        .search
        .as_ref()
        .map(|s| compile_pat(s, args.regex))
        .transpose()?;
    let sel = parse_window_select(args.window.windows.as_deref(), args.window.window_range.as_deref())?;
    let windows = resolve_windows(&graph, &sel)?;
    let mut rows: Vec<_> = graph
        .threads
        .iter()
        .map(|t| (t.time_for(&windows), t.name.as_str(), t.children.len()))
        .collect();
    rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    if let Some(pat) = &pat {
        rows.retain(|(_, name, _)| pat.is_match(name));
    }
    let total: f64 = rows.iter().map(|r| r.0).sum();
    if args.top > 0 {
        rows.truncate(args.top);
    }
    let mut sel_idxs: Vec<_> = windows.iter().copied().collect();
    sel_idxs.sort_unstable();
    if matches!(args.out.format, FormatArg::Json) {
        let v = serde_json::json!({
            "thread_count": graph.threads.len(),
            "shown": rows.len(),
            "total_time": total,
            "windows_selected": sel_idxs,
            "threads": rows.iter().map(|(time, name, children)| serde_json::json!({
                "name": name,
                "time": time,
                "percent": if total > 0.0 { 100.0 * time / total } else { 0.0 },
                "root_children": children,
            })).collect::<Vec<_>>(),
        });
        write_out(&args.out.out, &(serde_json::to_string_pretty(&v)? + "\n"))?;
        return Ok(());
    }
    let mut lines = Vec::new();
    lines.push(format!(
        "=== Threads ({} total, showing {}) ===",
        graph.threads.len(),
        rows.len()
    ));
    if graph.window_count() > 1 {
        lines.push(format!(
            "Windows selected: {} / {}  indices {:?}",
            windows.len(),
            graph.window_count(),
            sel_idxs
        ));
    }
    lines.push(format!(
        "{:>8}  {:>10}  {:>8}  name",
        "%", "time", "roots"
    ));
    for (time, name, children) in rows {
        let pct = if total > 0.0 {
            100.0 * time / total
        } else {
            0.0
        };
        lines.push(format!(
            "{:>7.1}%  {:>10}  {:>8}  {name}",
            pct, time as i64, children
        ));
    }
    write_out(&args.out.out, &(lines.join("\n") + "\n"))?;
    Ok(())
}

pub fn run_windows(args: WindowsArgs) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = load_bytes(&args.input)?;
    let graph = ProfileGraph::from_sampler_bytes(&bytes)?;
    let mut rows = graph.window_times();
    let total: f64 = rows.iter().map(|r| r.2).sum();
    rows.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    if args.min_percent > 0.0 && total > 0.0 {
        rows.retain(|r| 100.0 * r.2 / total >= args.min_percent);
    }
    if args.top > 0 {
        rows.truncate(args.top);
    }
    let n = graph.window_count();
    let win_ms = if n > 0 && graph.meta.duration_ms > 0 {
        graph.meta.duration_ms as f64 / n as f64
    } else {
        0.0
    };

    if matches!(args.out.format, FormatArg::Json) {
        let v = serde_json::json!({
            "window_count": n,
            "total_time": total,
            "duration_ms": graph.meta.duration_ms,
            "start_time_ms": graph.meta.start_time_ms,
            "windows": rows.iter().map(|(idx, id, time)| {
                let pct = if total > 0.0 { 100.0 * time / total } else { 0.0 };
                let approx_start = if win_ms > 0.0 {
                    Some(graph.meta.start_time_ms + (*idx as f64 * win_ms) as i64)
                } else {
                    None
                };
                serde_json::json!({
                    "index": idx,
                    "id": id,
                    "time": time,
                    "percent": pct,
                    "approx_start_ms": approx_start,
                    "select": format!("@{idx}"),
                    "window_range": format!("{idx},{idx}"),
                })
            }).collect::<Vec<_>>(),
        });
        write_out(&args.out.out, &(serde_json::to_string_pretty(&v)? + "\n"))?;
        return Ok(());
    }

    let mut lines = Vec::new();
    lines.push(format!(
        "=== Time windows ({} total, showing {}; Refine buckets ≈1m each) ===",
        n,
        rows.len()
    ));
    if graph.meta.duration_ms > 0 {
        lines.push(format!(
            "Profile duration: {}  start_ms={}",
            fmt_offset_ms(graph.meta.duration_ms),
            graph.meta.start_time_ms
        ));
    }
    lines.push(format!(
        "{:>5}  {:>12}  {:>8}  {:>10}  {:>10}  select",
        "idx", "id", "%", "time", "t+offset"
    ));
    for (idx, id, time) in &rows {
        let pct = if total > 0.0 {
            100.0 * time / total
        } else {
            0.0
        };
        let id_s = id.map(|v| v.to_string()).unwrap_or_else(|| "-".into());
        let offset = if win_ms > 0.0 {
            fmt_offset_ms((*idx as f64 * win_ms) as i64)
        } else {
            "-".into()
        };
        lines.push(format!(
            "{:>5}  {:>12}  {:>7.1}%  {:>10}  {:>10}  --windows @{idx}",
            idx, id_s, pct, *time as i64, offset
        ));
    }
    if args.commands {
        lines.push(String::new());
        lines.push("Commands:".into());
        for (idx, _, _) in &rows {
            lines.push(format!(
                "  spark-dump profile {} --windows @{idx} --view flat --top 40",
                args.input
            ));
        }
    } else if n > 1 {
        lines.push(String::new());
        lines.push("Tip: spark-dump profile <file> --windows @N --view flat".into());
        lines.push("     spark-dump threads <file> --window-range N,N".into());
        lines.push("     spark-dump windows <file> --commands".into());
    }
    write_out(&args.out.out, &(lines.join("\n") + "\n"))?;
    Ok(())
}

pub fn run_plugins(args: PluginsArgs) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = load_bytes(&args.input)?;
    let graph = ProfileGraph::from_sampler_bytes(&bytes)?;
    let filter = build_filter(&args.filter)?;
    let _ = resolve_windows(&graph, &filter.windows)?;
    let render_opts = RenderOpts {
        format: format_of(&args.out.format),
        label: LabelMode::Percent,
        show_meta: false,
        view: ProfileView::Sources,
        flat_mode: FlatMode::SelfTime,
        sources_mode: match args.sources_mode {
            SourcesModeArg::Merge => SourcesMode::Merge,
            SourcesModeArg::Separate => SourcesMode::Separate,
        },
        bottom_up: false,
        flame_thread: None,
    };
    let detail = args.detail && !args.no_detail;
    let out = views::render_plugins(&graph, &filter, &render_opts, detail)?;
    write_out(&args.out.out, &out)?;
    Ok(())
}
