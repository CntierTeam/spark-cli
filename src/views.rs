use std::collections::{HashMap, HashSet};

use crate::cli::FilterPat;
use crate::filter::FilterOpts;
use crate::graph::{Node, ProfileGraph};
use crate::render::{self, Format, LabelMode, RenderOpts};

#[derive(Debug, Clone, Copy)]
pub enum ProfileView {
    All,
    Flat,
    Sources,
    Flame,
    Summary,
}

#[derive(Debug, Clone, Copy)]
pub enum FlatMode {
    SelfTime,
    TotalTime,
}

#[derive(Debug, Clone, Copy)]
pub enum SourcesMode {
    Merge,
    Separate,
}

fn ranked_threads<'a>(
    graph: &'a ProfileGraph,
    filter: &FilterOpts,
    windows: &std::collections::HashSet<usize>,
) -> Vec<&'a Node> {
    let mut threads: Vec<_> = graph
        .threads
        .iter()
        .filter(|t| thread_allowed(graph, t.id, filter))
        .collect();
    threads.sort_by(|a, b| {
        b.time_for(windows)
            .partial_cmp(&a.time_for(windows))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let total: f64 = threads.iter().map(|t| t.time_for(windows)).sum();
    if filter.min_thread_percent > 0.0 && total > 0.0 {
        threads.retain(|t| 100.0 * t.time_for(windows) / total >= filter.min_thread_percent);
    }
    if filter.top_threads > 0 && threads.len() > filter.top_threads {
        threads.truncate(filter.top_threads);
    }
    threads
}

pub fn render_profile(
    graph: &ProfileGraph,
    filter: &FilterOpts,
    opts: &RenderOpts,
) -> Result<String, Box<dyn std::error::Error>> {
    let windows = graph.selected_window_indices(&filter.windows);
    let visible = resolve_visibility(graph, filter, &windows);
    let threads = ranked_threads(graph, filter, &windows);

    if matches!(opts.format, Format::Json) {
        return Ok(render_json(graph, filter, opts, &windows, &visible, &threads)?);
    }

    let mut out = String::new();
    if opts.show_meta {
        out.push_str(&render::format_meta_text(&graph.meta));
        out.push('\n');
    }

    match opts.view {
        ProfileView::Summary => {
            out.push_str(&render_summary(graph, filter, &windows, &threads));
            out.push_str("\nHint: spark-dump threads <file>   # ranked thread list\n");
            out.push_str("      spark-dump profile <file> --view flat --top-threads 10\n");
            out.push_str("      spark-dump tutorial filters\n");
        }
        ProfileView::All => out.push_str(&render_all(graph, filter, opts, &windows, &visible, &threads)),
        ProfileView::Flat => out.push_str(&render_flat(graph, filter, opts, &windows, &visible, &threads)),
        ProfileView::Sources => {
            out.push_str(&render_sources(graph, filter, opts, &windows, &visible, &threads))
        }
        ProfileView::Flame => {
            out.push_str(&render_flame(graph, filter, opts, &windows, &visible, &threads))
        }
    }
    Ok(out)
}

fn thread_allowed(graph: &ProfileGraph, id: u32, filter: &FilterOpts) -> bool {
    let Some(n) = graph.get(id) else {
        return false;
    };
    if let Some(pat) = &filter.thread {
        if !pat.is_match(&n.name) {
            return false;
        }
    }
    true
}

/// Web SearchResolver: match → include self + all parents + all descendants.
/// Plus include/exclude filters on stack nodes.
fn resolve_visibility(
    graph: &ProfileGraph,
    filter: &FilterOpts,
    _windows: &HashSet<usize>,
) -> Option<HashSet<u32>> {
    let need_search = filter.search.is_some() || !filter.includes.is_empty() || !filter.excludes.is_empty();
    if !need_search && filter.thread.is_none() {
        return None; // no filtering
    }

    let mut acc = HashSet::new();

    for thread in &graph.threads {
        if !thread_allowed(graph, thread.id, filter) {
            continue;
        }
        if filter.search.is_none() && filter.includes.is_empty() && filter.excludes.is_empty() {
            // thread filter only → keep whole thread subtree
            add_descendants(graph, thread.id, &mut acc);
            acc.insert(thread.id);
            continue;
        }
        walk_search(graph, thread.id, filter, &mut acc);
    }
    Some(acc)
}

fn walk_search(graph: &ProfileGraph, id: u32, filter: &FilterOpts, acc: &mut HashSet<u32>) {
    let Some(node) = graph.get(id) else {
        return;
    };
    if matches_filters(graph, node, filter) {
        acc.insert(id);
        add_descendants(graph, id, acc);
        add_ancestors(graph, id, acc);
    }
    for &c in &node.children {
        walk_search(graph, c, filter, acc);
    }
}

fn matches_filters(graph: &ProfileGraph, node: &Node, filter: &FilterOpts) -> bool {
    let source = graph.source_of(node.id);
    if node.is_thread {
        if let Some(q) = &filter.search {
            if !node.name.to_ascii_lowercase().contains(q) {
                return false;
            }
        }
        return filter.node_text_allowed(&node.name, "", source);
    }

    if let Some(q) = &filter.search {
        let ok = node.class_name.to_ascii_lowercase().contains(q)
            || node.method_name.to_ascii_lowercase().contains(q)
            || source
                .map(|s| s.to_ascii_lowercase().contains(q))
                .unwrap_or(false);
        if !ok {
            return false;
        }
    }
    filter.node_text_allowed(&node.class_name, &node.method_name, source)
}

fn add_descendants(graph: &ProfileGraph, id: u32, acc: &mut HashSet<u32>) {
    let Some(node) = graph.get(id) else {
        return;
    };
    for &c in &node.children {
        acc.insert(c);
        add_descendants(graph, c, acc);
    }
}

fn add_ancestors(graph: &ProfileGraph, id: u32, acc: &mut HashSet<u32>) {
    if let Some(ps) = graph.parents.get(&id) {
        for &p in ps {
            acc.insert(p);
            add_ancestors(graph, p, acc);
        }
    }
}

fn is_vis(visible: &Option<HashSet<u32>>, id: u32) -> bool {
    visible.as_ref().map(|s| s.contains(&id)).unwrap_or(true)
}

fn passes_include_exclude(graph: &ProfileGraph, id: u32, filter: &FilterOpts) -> bool {
    let Some(node) = graph.get(id) else {
        return false;
    };
    if node.is_thread {
        return true;
    }
    filter.node_text_allowed(
        &node.class_name,
        &node.method_name,
        graph.source_of(id),
    )
}

fn render_summary(
    graph: &ProfileGraph,
    filter: &FilterOpts,
    windows: &std::collections::HashSet<usize>,
    threads: &[&Node],
) -> String {
    let mut lines = Vec::new();
    let all_total: f64 = graph.threads.iter().map(|t| t.time_for(windows)).sum();
    lines.push(format!(
        "=== Threads ({} total, showing {}) ===",
        graph.threads.len(),
        threads.len()
    ));
    if filter.top_threads > 0 || filter.min_thread_percent > 0.0 {
        lines.push(format!(
            "(filtered: top_threads={}, min_thread_percent={})",
            filter.top_threads, filter.min_thread_percent
        ));
    }
    for t in threads {
        let time = t.time_for(windows);
        let pct = if all_total > 0.0 {
            100.0 * time / all_total
        } else {
            0.0
        };
        lines.push(format!("  {:>6.1}%  {:>10}  {}", pct, time as i64, t.name));
    }
    if !graph.all_sources.is_empty() {
        lines.push(format!(
            "\nSources/plugins seen in map: {}",
            graph.all_sources.len()
        ));
    }
    if graph.time_windows.len() > 1 {
        lines.push(format!(
            "Time windows: {} ids {:?}",
            graph.time_windows.len(),
            graph.time_windows
        ));
    }
    lines.join("\n") + "\n"
}

fn render_all(
    graph: &ProfileGraph,
    filter: &FilterOpts,
    opts: &RenderOpts,
    windows: &std::collections::HashSet<usize>,
    visible: &Option<std::collections::HashSet<u32>>,
    threads: &[&Node],
) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "Dumping {} / {} threads (sorted by time)\n",
        threads.len(),
        graph.threads.len()
    ));
    for thread in threads {
        if !is_vis(visible, thread.id) {
            continue;
        }
        let total = thread.time_for(windows);
        lines.push(format!("=== Thread: {} ===", thread.name));
        lines.push(format!("Total: {}", fmt_label(total, total, opts.label)));
        lines.push(String::new());
        let mut kids: Vec<_> = thread
            .children
            .iter()
            .copied()
            .filter(|id| is_vis(visible, *id))
            .collect();
        kids.sort_by(|a, b| {
            let ta = graph.get(*a).map(|n| n.time_for(windows)).unwrap_or(0.0);
            let tb = graph.get(*b).map(|n| n.time_for(windows)).unwrap_or(0.0);
            tb.partial_cmp(&ta).unwrap_or(std::cmp::Ordering::Equal)
        });
        for id in kids {
            dump_tree(
                &mut lines,
                graph,
                id,
                total,
                1,
                filter,
                opts,
                windows,
                visible,
            );
        }
        lines.push(String::new());
    }
    lines.join("\n")
}

fn dump_tree(
    lines: &mut Vec<String>,
    graph: &ProfileGraph,
    id: u32,
    root_total: f64,
    depth: usize,
    filter: &FilterOpts,
    opts: &RenderOpts,
    windows: &HashSet<usize>,
    visible: &Option<HashSet<u32>>,
) {
    let Some(node) = graph.get(id) else {
        return;
    };
    if !passes_include_exclude(graph, id, filter) {
        for &cid in &node.children {
            if is_vis(visible, cid) {
                dump_tree(
                    lines, graph, cid, root_total, depth, filter, opts, windows, visible,
                );
            }
        }
        return;
    }
    let time = node.time_for(windows);
    if time <= 0.0 {
        return;
    }
    let share = if root_total > 0.0 {
        100.0 * time / root_total
    } else {
        0.0
    };
    if depth > 0 && share < filter.min_percent {
        return;
    }
    let src = graph
        .source_of(id)
        .map(|s| format!("  [{s}]"))
        .unwrap_or_default();
    let indent = "  ".repeat(depth);
    lines.push(format!(
        "{indent}{:>10}  {}{src}",
        fmt_label(time, root_total, opts.label),
        node.label()
    ));
    if depth >= filter.depth {
        return;
    }
    let mut kids: Vec<_> = node
        .children
        .iter()
        .copied()
        .filter(|cid| is_vis(visible, *cid))
        .collect();
    kids.sort_by(|a, b| {
        let ta = graph.get(*a).map(|n| n.time_for(windows)).unwrap_or(0.0);
        let tb = graph.get(*b).map(|n| n.time_for(windows)).unwrap_or(0.0);
        tb.partial_cmp(&ta).unwrap_or(std::cmp::Ordering::Equal)
    });
    for cid in kids {
        dump_tree(
            lines, graph, cid, root_total, depth + 1, filter, opts, windows, visible,
        );
    }
}

fn render_flat(
    graph: &ProfileGraph,
    filter: &FilterOpts,
    opts: &RenderOpts,
    windows: &HashSet<usize>,
    visible: &Option<HashSet<u32>>,
    threads: &[&Node],
) -> String {
    let mut lines = Vec::new();
    let mode = opts.flat_mode;
    lines.push(format!(
        "Dumping {} / {} threads (sorted by time)\n",
        threads.len(),
        graph.threads.len()
    ));
    for thread in threads {
        if !is_vis(visible, thread.id) {
            continue;
        }
        let total = thread.time_for(windows);
        lines.push(format!("=== Thread: {} ===", thread.name));
        lines.push(format!(
            "-- Flat ({}) top {}{} --",
            match mode {
                FlatMode::SelfTime => "self time",
                FlatMode::TotalTime => "total time",
            },
            filter.top,
            if opts.bottom_up { ", bottom-up parents" } else { "" }
        ));

        let mut self_acc: HashMap<String, FlatAcc> = HashMap::new();
        let mut total_acc: HashMap<String, FlatAcc> = HashMap::new();
        let mut seen = HashSet::new();
        for &cid in &thread.children {
            if is_vis(visible, cid) {
                collect_flat(
                    graph, cid, filter, windows, visible, &mut self_acc, &mut total_acc, &mut seen,
                );
            }
        }
        let mut items: Vec<FlatAcc> = match mode {
            FlatMode::SelfTime => self_acc.into_values().collect(),
            FlatMode::TotalTime => total_acc.into_values().collect(),
        };
        items.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
        items.truncate(filter.top);
        let max_v = items.first().map(|e| e.value).unwrap_or(1.0).max(1.0);
        for e in &items {
            let bar = bar(e.value, max_v, 28);
            let src = e
                .source
                .as_ref()
                .map(|s| format!("  [{s}]"))
                .unwrap_or_default();
            lines.push(format!(
                "  {:>10}  {:<28}  {}{src}",
                fmt_label(e.value, total, opts.label),
                bar,
                e.label
            ));
            if opts.bottom_up {
                if let Some(id) = e.sample_id {
                    let parents = graph.parents.get(&id).cloned().unwrap_or_default();
                    for pid in parents.iter().take(5) {
                        if let Some(p) = graph.get(*pid) {
                            lines.push(format!("           ↳ {}", p.label()));
                        }
                    }
                }
            }
        }
        lines.push(String::new());
    }
    lines.join("\n")
}

struct FlatAcc {
    label: String,
    value: f64,
    source: Option<String>,
    sample_id: Option<u32>,
}

fn collect_flat(
    graph: &ProfileGraph,
    id: u32,
    filter: &FilterOpts,
    windows: &HashSet<usize>,
    visible: &Option<HashSet<u32>>,
    self_acc: &mut HashMap<String, FlatAcc>,
    total_acc: &mut HashMap<String, FlatAcc>,
    seen: &mut HashSet<String>,
) {
    let Some(node) = graph.get(id) else {
        return;
    };
    if node.is_thread {
        for &c in &node.children {
            if is_vis(visible, c) {
                collect_flat(graph, c, filter, windows, visible, self_acc, total_acc, seen);
            }
        }
        return;
    }
    if !passes_include_exclude(graph, id, filter) {
        for &c in &node.children {
            if is_vis(visible, c) {
                collect_flat(graph, c, filter, windows, visible, self_acc, total_acc, seen);
            }
        }
        return;
    }
    let key = node.key();
    let skip = seen.contains(&key);
    if !skip {
        seen.insert(key.clone());
    }
    let mut child_time = 0.0;
    for &c in &node.children {
        if is_vis(visible, c) {
            collect_flat(graph, c, filter, windows, visible, self_acc, total_acc, seen);
            if passes_include_exclude(graph, c, filter) {
                child_time += graph.get(c).map(|n| n.time_for(windows)).unwrap_or(0.0);
            }
        }
    }
    if !skip {
        seen.remove(&key);
    }
    let time = node.time_for(windows);
    let self_time = (time - child_time).max(0.0);
    if !skip {
        let src = graph.source_of(id).map(|s| s.to_string());
        self_acc
            .entry(key.clone())
            .and_modify(|e| e.value += self_time)
            .or_insert_with(|| FlatAcc {
                label: node.label(),
                value: self_time,
                source: src.clone(),
                sample_id: Some(id),
            });
        total_acc
            .entry(key)
            .and_modify(|e| e.value += time)
            .or_insert_with(|| FlatAcc {
                label: node.label(),
                value: time,
                source: src,
                sample_id: Some(id),
            });
    }
}

fn render_sources(
    graph: &ProfileGraph,
    filter: &FilterOpts,
    opts: &RenderOpts,
    windows: &HashSet<usize>,
    visible: &Option<HashSet<u32>>,
    threads: &[&Node],
) -> String {
    render_plugins_inner(graph, filter, opts, windows, visible, threads, true)
}

/// Dedicated plugin/mod occupancy report (web Sources).
pub fn render_plugins(
    graph: &ProfileGraph,
    filter: &FilterOpts,
    opts: &RenderOpts,
    detail: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let windows = graph.selected_window_indices(&filter.windows);
    let visible = resolve_visibility(graph, filter, &windows);
    let threads = ranked_threads(graph, filter, &windows);
    if matches!(opts.format, Format::Json) {
        let stats = collect_plugin_stats(graph, filter, opts, &windows, &visible, &threads);
        return Ok(serde_json::to_string_pretty(&stats.to_json())? + "\n");
    }
    Ok(render_plugins_inner(
        graph, filter, opts, &windows, &visible, &threads, detail,
    ))
}

struct PluginStats {
    thread_total: f64,
    attributed: f64,
    rows: Vec<PluginRow>,
    idle_plugins: Vec<String>,
    threads_shown: usize,
    threads_total: usize,
}

struct PluginRow {
    name: String,
    time: f64,
    methods: Vec<(String, f64)>,
}

impl PluginStats {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "threads_shown": self.threads_shown,
            "threads_total": self.threads_total,
            "thread_total_time": self.thread_total,
            "plugin_attributed_time": self.attributed,
            "unattributed_time": (self.thread_total - self.attributed).max(0.0),
            "plugins": self.rows.iter().map(|r| serde_json::json!({
                "name": r.name,
                "time": r.time,
                "percent_of_threads": if self.thread_total > 0.0 { 100.0 * r.time / self.thread_total } else { 0.0 },
                "percent_of_plugins": if self.attributed > 0.0 { 100.0 * r.time / self.attributed } else { 0.0 },
                "methods": r.methods.iter().map(|(n,t)| serde_json::json!({"name": n, "time": t})).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "installed_with_zero_samples": self.idle_plugins,
        })
    }
}

fn collect_plugin_stats(
    graph: &ProfileGraph,
    filter: &FilterOpts,
    opts: &RenderOpts,
    windows: &HashSet<usize>,
    visible: &Option<HashSet<u32>>,
    threads: &[&Node],
) -> PluginStats {
    let thread_total: f64 = threads.iter().map(|t| t.time_for(windows)).sum();
    let mut by_source: HashMap<String, f64> = HashMap::new();
    let mut by_source_nodes: HashMap<String, Vec<(String, f64, String)>> = HashMap::new();

    for thread in threads {
        let matches = find_source_roots(graph, thread.id, windows, visible);
        for (src, node_id, time) in matches {
            if let Some(node) = graph.get(node_id) {
                if hide_carpet_node(node, &src) {
                    continue;
                }
            }
            if let Some(q) = &filter.search {
                if !src.to_ascii_lowercase().contains(q)
                    && !graph
                        .get(node_id)
                        .map(|n| {
                            n.class_name.to_ascii_lowercase().contains(q)
                                || n.method_name.to_ascii_lowercase().contains(q)
                        })
                        .unwrap_or(false)
                {
                    continue;
                }
            }
            if !filter.node_text_allowed(
                graph
                    .get(node_id)
                    .map(|n| n.class_name.as_str())
                    .unwrap_or(""),
                graph
                    .get(node_id)
                    .map(|n| n.method_name.as_str())
                    .unwrap_or(""),
                Some(&src),
            ) {
                continue;
            }
            let label = graph.get(node_id).map(|n| n.label()).unwrap_or_default();
            let key = match opts.sources_mode {
                SourcesMode::Merge => {
                    graph.get(node_id).map(|n| n.key()).unwrap_or(label.clone())
                }
                SourcesMode::Separate => format!("{node_id}:{label}"),
            };
            by_source
                .entry(src.clone())
                .and_modify(|t| *t += time)
                .or_insert(time);
            by_source_nodes
                .entry(src)
                .or_default()
                .push((key, time, label));
        }
    }

    let attributed: f64 = by_source.values().sum();
    let mut rows: Vec<PluginRow> = by_source
        .into_iter()
        .map(|(name, time)| {
            let mut merged: HashMap<String, (f64, String)> = HashMap::new();
            for (k, t, label) in by_source_nodes.remove(&name).unwrap_or_default() {
                merged
                    .entry(k)
                    .and_modify(|(tv, _)| *tv += t)
                    .or_insert((t, label));
            }
            let mut methods: Vec<(String, f64)> = merged
                .into_iter()
                .map(|(_, (t, l))| (l, t))
                .collect();
            methods.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            if filter.top > 0 {
                methods.truncate(filter.top.min(50));
            }
            PluginRow {
                name,
                time,
                methods,
            }
        })
        .collect();
    rows.sort_by(|a, b| b.time.partial_cmp(&a.time).unwrap_or(std::cmp::Ordering::Equal));
    if filter.top > 0 {
        // keep summary table complete; detail truncation already on methods
    }

    let seen: HashSet<_> = rows.iter().map(|r| r.name.to_ascii_lowercase()).collect();
    let mut idle: Vec<String> = graph
        .meta
        .plugins_mods
        .iter()
        .filter(|p| !seen.contains(&p.to_ascii_lowercase()))
        // also skip if source name variants already present (torunto vs Torunto)
        .filter(|p| {
            !rows.iter().any(|r| {
                r.name.eq_ignore_ascii_case(p)
                    || r.name.to_ascii_lowercase().replace('-', "")
                        == p.to_ascii_lowercase().replace('-', "")
            })
        })
        .cloned()
        .collect();
    idle.sort();

    PluginStats {
        thread_total,
        attributed,
        rows,
        idle_plugins: idle,
        threads_shown: threads.len(),
        threads_total: graph.threads.len(),
    }
}

fn hide_carpet_node(node: &Node, source: &str) -> bool {
    source.to_ascii_lowercase().contains("carpet")
        && node.class_name.starts_with("net.minecraft.server.MinecraftServer")
        && (node.method_name.ends_with("modifiedRunLoop")
            || node.method_name.ends_with("fixUpdateSuppressionCrashTick"))
}

fn render_plugins_inner(
    graph: &ProfileGraph,
    filter: &FilterOpts,
    opts: &RenderOpts,
    windows: &HashSet<usize>,
    visible: &Option<HashSet<u32>>,
    threads: &[&Node],
    detail: bool,
) -> String {
    let mut lines = Vec::new();
    if graph.id_to_source.is_empty() {
        lines.push(
            "No plugin/mod source map in this profile (plugin occupancy unavailable).\n".into(),
        );
        lines.push("Installed plugins/mods from metadata:\n".into());
        for p in &graph.meta.plugins_mods {
            lines.push(format!("  - {p}"));
        }
        return lines.join("\n") + "\n";
    }

    let stats = collect_plugin_stats(graph, filter, opts, windows, visible, threads);
    let unattr = (stats.thread_total - stats.attributed).max(0.0);

    lines.push("=== Plugin / Mod Occupancy ===".into());
    lines.push(format!(
        "Threads: {} / {}  |  mode: {}",
        stats.threads_shown,
        stats.threads_total,
        match opts.sources_mode {
            SourcesMode::Merge => "merge",
            SourcesMode::Separate => "separate",
        }
    ));
    for t in threads.iter().take(8) {
        lines.push(format!(
            "  • {} ({})",
            t.name,
            t.time_for(windows) as i64
        ));
    }
    if threads.len() > 8 {
        lines.push(format!("  • … +{} more", threads.len() - 8));
    }
    lines.push(String::new());
    lines.push(format!(
        "{:>8}  {:>8}  {:>10}  plugin",
        "%thread", "%plugins", "time"
    ));

    for row in &stats.rows {
        let pct_t = if stats.thread_total > 0.0 {
            100.0 * row.time / stats.thread_total
        } else {
            0.0
        };
        let pct_p = if stats.attributed > 0.0 {
            100.0 * row.time / stats.attributed
        } else {
            0.0
        };
        lines.push(format!(
            "{:>7.1}%  {:>7.1}%  {:>10}  {}",
            pct_t,
            pct_p,
            row.time as i64,
            row.name
        ));
    }
    lines.push(format!(
        "{:>7.1}%  {:>7}  {:>10}  (unattributed / vanilla / native)",
        if stats.thread_total > 0.0 {
            100.0 * unattr / stats.thread_total
        } else {
            0.0
        },
        "-",
        unattr as i64
    ));
    lines.push(format!(
        "{:>7.1}%  {:>7.1}%  {:>10}  TOTAL",
        100.0,
        100.0,
        stats.thread_total as i64
    ));

    if detail {
        lines.push("\n-- Hot methods per plugin --".into());
        for row in &stats.rows {
            lines.push(format!(
                "\n## {}  ({:.1}% of threads)",
                row.name,
                if stats.thread_total > 0.0 {
                    100.0 * row.time / stats.thread_total
                } else {
                    0.0
                }
            ));
            for (label, t) in &row.methods {
                lines.push(format!(
                    "  {:>10}  {label}",
                    fmt_label(*t, row.time.max(1.0), opts.label)
                ));
            }
        }
    }

    if !stats.idle_plugins.is_empty() {
        lines.push("\n-- Installed but no samples in selected threads --".into());
        for p in &stats.idle_plugins {
            lines.push(format!("  - {p}"));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

/// Highest matching source nodes (don't descend into a match).
fn find_source_roots(
    graph: &ProfileGraph,
    id: u32,
    windows: &HashSet<usize>,
    visible: &Option<HashSet<u32>>,
) -> Vec<(String, u32, f64)> {
    let mut out = Vec::new();
    find_source_roots_rec(graph, id, windows, visible, &mut out);
    out
}

fn find_source_roots_rec(
    graph: &ProfileGraph,
    id: u32,
    windows: &HashSet<usize>,
    visible: &Option<HashSet<u32>>,
    out: &mut Vec<(String, u32, f64)>,
) {
    if !is_vis(visible, id) {
        return;
    }
    let Some(node) = graph.get(id) else {
        return;
    };
    if let Some(src) = graph.source_of(id) {
        out.push((src.to_string(), id, node.time_for(windows)));
        return;
    }
    for &c in &node.children {
        find_source_roots_rec(graph, c, windows, visible, out);
    }
}

fn render_flame(
    graph: &ProfileGraph,
    filter: &FilterOpts,
    opts: &RenderOpts,
    windows: &HashSet<usize>,
    visible: &Option<HashSet<u32>>,
    threads: &[&Node],
) -> String {
    let mut lines = Vec::new();
    if threads.is_empty() {
        return "No matching threads for flame view.\n".into();
    }

    let picked = pick_thread(graph, filter, opts.flame_thread.as_deref(), threads, windows);
    let Some(thread) = picked else {
        return "No matching thread for flame view.\n".into();
    };

    if threads.len() > 1
        && opts.flame_thread.is_none()
        && filter.thread.is_none()
    {
        lines.push(format!(
            "Note: {} threads matched; flame defaults to heaviest.\nUse --thread / --flame-thread to pick, or --top-threads N.\n",
            threads.len()
        ));
        lines.push("Top candidates:".into());
        for t in threads.iter().take(8) {
            lines.push(format!("  {:>10}  {}", t.time_for(windows) as i64, t.name));
        }
        lines.push(String::new());
    }

    let total = thread.time_for(windows);
    lines.push(format!("=== Flame: {} ===", thread.name));
    lines.push("(folded stacks — value is inclusive time)\n".into());
    for &cid in &thread.children {
        if is_vis(visible, cid) {
            flame_lines(
                &mut lines,
                graph,
                cid,
                total,
                0,
                filter,
                windows,
                visible,
                opts,
            );
        }
    }
    lines.join("\n") + "\n"
}

fn flame_lines(
    lines: &mut Vec<String>,
    graph: &ProfileGraph,
    id: u32,
    root_total: f64,
    depth: usize,
    filter: &FilterOpts,
    windows: &HashSet<usize>,
    visible: &Option<HashSet<u32>>,
    opts: &RenderOpts,
) {
    let Some(node) = graph.get(id) else {
        return;
    };
    if !passes_include_exclude(graph, id, filter) {
        for &c in &node.children {
            if is_vis(visible, c) {
                flame_lines(
                    lines, graph, c, root_total, depth, filter, windows, visible, opts,
                );
            }
        }
        return;
    }
    let time = node.time_for(windows);
    if time <= 0.0 {
        return;
    }
    let share = if root_total > 0.0 {
        100.0 * time / root_total
    } else {
        0.0
    };
    if share < filter.min_percent {
        return;
    }
    let width = ((share / 100.0) * 48.0).round() as usize;
    let bar = "█".repeat(width.max(1));
    let indent = "│".to_string() + &" ".repeat(depth);
    lines.push(format!(
        "{indent}{bar} {:>8}  {}",
        fmt_label(time, root_total, opts.label),
        node.label()
    ));
    if depth >= filter.depth {
        return;
    }
    let mut kids: Vec<_> = node
        .children
        .iter()
        .copied()
        .filter(|c| is_vis(visible, *c))
        .collect();
    kids.sort_by(|a, b| {
        let ta = graph.get(*a).map(|n| n.time_for(windows)).unwrap_or(0.0);
        let tb = graph.get(*b).map(|n| n.time_for(windows)).unwrap_or(0.0);
        tb.partial_cmp(&ta).unwrap_or(std::cmp::Ordering::Equal)
    });
    for c in kids {
        flame_lines(
            lines, graph, c, root_total, depth + 1, filter, windows, visible, opts,
        );
    }
}

fn pick_thread<'a>(
    graph: &'a ProfileGraph,
    filter: &FilterOpts,
    flame_thread: Option<&str>,
    ranked: &[&'a Node],
    windows: &HashSet<usize>,
) -> Option<&'a Node> {
    let q = flame_thread
        .map(|s| s.to_ascii_lowercase())
        .or_else(|| {
            filter.thread.as_ref().and_then(|p| match p {
                FilterPat::Substr(s) => Some(s.clone()),
                FilterPat::Regex(_) => None,
            })
        });
    if let Some(q) = q {
        // Prefer heaviest among name matches
        let mut matches: Vec<_> = ranked
            .iter()
            .copied()
            .filter(|t| t.name.to_ascii_lowercase().contains(&q))
            .collect();
        if matches.is_empty() {
            matches = graph
                .threads
                .iter()
                .filter(|t| t.name.to_ascii_lowercase().contains(&q))
                .collect();
            matches.sort_by(|a, b| {
                b.time_for(windows)
                    .partial_cmp(&a.time_for(windows))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        return matches.into_iter().next();
    }
    // Heaviest overall (ranked is already sorted)
    ranked.first().copied()
}

fn fmt_label(part: f64, whole: f64, mode: LabelMode) -> String {
    match mode {
        LabelMode::Absolute => format!("{}", part as i64),
        LabelMode::Percent => {
            if whole <= 0.0 {
                "0.0%".into()
            } else {
                format!("{:.1}%", 100.0 * part / whole)
            }
        }
    }
}

fn bar(value: f64, max: f64, width: usize) -> String {
    let n = if max <= 0.0 {
        1
    } else {
        ((width as f64 * value / max).round() as usize).max(1)
    };
    "#".repeat(n.min(width))
}

fn render_json(
    graph: &ProfileGraph,
    filter: &FilterOpts,
    opts: &RenderOpts,
    windows: &HashSet<usize>,
    visible: &Option<HashSet<u32>>,
    threads: &[&Node],
) -> Result<String, Box<dyn std::error::Error>> {
    let text = match opts.view {
        ProfileView::Summary => render_summary(graph, filter, windows, threads),
        ProfileView::All => render_all(graph, filter, opts, windows, visible, threads),
        ProfileView::Flat => render_flat(graph, filter, opts, windows, visible, threads),
        ProfileView::Sources => render_sources(graph, filter, opts, windows, visible, threads),
        ProfileView::Flame => render_flame(graph, filter, opts, windows, visible, threads),
    };
    let v = serde_json::json!({
        "meta": graph.meta,
        "view": format!("{:?}", opts.view),
        "windows_selected": windows.iter().copied().collect::<Vec<_>>(),
        "thread_count": graph.threads.len(),
        "threads_shown": threads.len(),
        "threads": threads.iter().map(|t| serde_json::json!({
            "name": t.name,
            "time": t.time_for(windows),
        })).collect::<Vec<_>>(),
        "source_count": graph.all_sources.len(),
        "text": text,
    });
    Ok(serde_json::to_string_pretty(&v)? + "\n")
}