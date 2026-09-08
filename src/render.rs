use prost::Message;
use serde_json::{json, Value};

use crate::graph::{MetaSnapshot, ProfileGraph};
use crate::load::ContentKind;
use crate::spark::{HealthData, HeapData, SamplerData};
use crate::views::{FlatMode, ProfileView, SourcesMode};

#[derive(Debug, Clone, Copy)]
pub enum Format {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy)]
pub enum LabelMode {
    Percent,
    Absolute,
}

pub struct RenderOpts {
    pub format: Format,
    pub label: LabelMode,
    pub show_meta: bool,
    pub view: ProfileView,
    pub flat_mode: FlatMode,
    pub sources_mode: SourcesMode,
    pub bottom_up: bool,
    pub flame_thread: Option<String>,
}

pub fn format_meta_text(m: &MetaSnapshot) -> String {
    let mut lines = Vec::new();
    lines.push("=== Spark Sampler Profile ===".into());
    let brand = if m.platform_brand.is_empty() {
        m.platform_name.as_str()
    } else {
        m.platform_brand.as_str()
    };
    lines.push(format!("Platform     : {brand} ({})", m.platform_type));
    if m.minecraft_version.is_empty() || m.platform_version.contains(&m.minecraft_version) {
        lines.push(format!("Version      : {}", m.platform_version));
    } else {
        lines.push(format!(
            "Version      : {}  MC {}",
            m.platform_version, m.minecraft_version
        ));
    }
    lines.push(format!("Spark        : v{}", m.spark_version));
    lines.push(format!(
        "Engine       : {} {}",
        m.sampler_engine, m.sampler_engine_version
    ));
    lines.push(format!("Mode         : {}", m.sampler_mode));
    lines.push(format!(
        "Interval     : {:.3} ms",
        m.interval_us as f64 / 1000.0
    ));
    lines.push(format!("Duration     : {}", fmt_duration(m.duration_ms)));
    if !m.user.is_empty() {
        lines.push(format!("Uploaded by  : {}", m.user));
    }
    if !m.comment.is_empty() {
        lines.push(format!("Comment      : {}", m.comment));
    }
    if let (Some(a), Some(b), Some(c)) = (m.tps_1m, m.tps_5m, m.tps_15m) {
        lines.push(format!(
            "TPS          : 1m={a:.2}  5m={b:.2}  15m={c:.2}  target={}",
            m.tps_target.unwrap_or(20)
        ));
    }
    if let (Some(mean), Some(med), Some(p95), Some(max)) =
        (m.mspt_mean, m.mspt_median, m.mspt_p95, m.mspt_max)
    {
        lines.push(format!(
            "MSPT (1m)    : mean={mean:.3} median={med:.3} p95={p95:.3} max={max:.3}"
        ));
    }
    if let Some(players) = m.players {
        lines.push(format!("Players      : {players}"));
    }
    if let (Some(used), Some(committed)) = (m.heap_used, m.heap_committed) {
        lines.push(format!(
            "Heap         : {} / {}",
            fmt_bytes(used),
            fmt_bytes(committed)
        ));
    }
    if !m.os.is_empty() {
        lines.push(format!("OS           : {} ({})", m.os, m.arch));
    }
    if let Some(threads) = m.cpu_threads {
        let pu = m.cpu_process_1m.unwrap_or(0.0) * 100.0;
        let su = m.cpu_system_1m.unwrap_or(0.0) * 100.0;
        lines.push(format!(
            "CPU          : {threads} threads | process 1m={pu:.1}%  system 1m={su:.1}%"
        ));
    }
    if !m.cpu_model.is_empty() {
        lines.push(format!("CPU model    : {}", m.cpu_model));
    }
    if !m.java.is_empty() {
        lines.push(format!("Java         : {}", m.java));
    }
    if !m.plugins_mods.is_empty() {
        lines.push(format!(
            "Plugins/Mods : {} ({})",
            m.plugins_mods.len(),
            m.plugins_mods.iter().take(12).cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    if m.time_window_ids.len() > 1 {
        lines.push(format!(
            "Windows      : {} {:?}",
            m.time_window_ids.len(),
            m.time_window_ids
        ));
    }
    lines.join("\n") + "\n"
}

pub fn render_heap_bytes(
    bytes: &[u8],
    search: Option<&str>,
    top: usize,
    format: Format,
) -> Result<String, Box<dyn std::error::Error>> {
    let data = HeapData::decode(bytes)?;
    let q = search.map(|s| s.to_ascii_lowercase());
    let mut entries = data.entries;
    entries.sort_by(|a, b| b.size.cmp(&a.size));
    if let Some(q) = &q {
        entries.retain(|e| e.r#type.to_ascii_lowercase().contains(q));
    }
    entries.truncate(top);

    if matches!(format, Format::Json) {
        let v = json!({
            "type": "heap",
            "entries": entries.iter().map(|e| json!({
                "order": e.order,
                "instances": e.instances,
                "size": e.size,
                "type": e.r#type,
            })).collect::<Vec<_>>(),
        });
        return Ok(serde_json::to_string_pretty(&v)? + "\n");
    }

    let mut lines = Vec::new();
    lines.push("=== Spark Heap Summary ===".into());
    if let Some(md) = &data.metadata {
        if let Some(p) = &md.platform {
            lines.push(format!(
                "Platform: {} {}",
                if p.brand.is_empty() { &p.name } else { &p.brand },
                p.version
            ));
        }
    }
    lines.push(format!("Showing {} entries", entries.len()));
    lines.push(String::new());
    for e in &entries {
        lines.push(format!(
            "  {:>12}  n={:>8}  {}",
            fmt_bytes(e.size),
            e.instances,
            e.r#type
        ));
    }
    Ok(lines.join("\n") + "\n")
}

pub fn render_health_bytes(
    bytes: &[u8],
    format: Format,
) -> Result<String, Box<dyn std::error::Error>> {
    let data = HealthData::decode(bytes)?;
    if matches!(format, Format::Json) {
        // lightweight JSON from metadata fields
        let md = data.metadata.as_ref();
        let v = json!({
            "type": "health",
            "generated_time": md.map(|m| m.generated_time),
            "platform": md.and_then(|m| m.platform.as_ref()).map(|p| json!({
                "brand": p.brand,
                "name": p.name,
                "version": p.version,
                "minecraftVersion": p.minecraft_version,
            })),
            "windows": data.time_window_statistics.len(),
        });
        return Ok(serde_json::to_string_pretty(&v)? + "\n");
    }
    let mut lines = Vec::new();
    lines.push("=== Spark Health Report ===".into());
    if let Some(md) = &data.metadata {
        if let Some(p) = &md.platform {
            lines.push(format!(
                "Platform: {} {} ({})",
                if p.brand.is_empty() { &p.name } else { &p.brand },
                p.version,
                p.minecraft_version
            ));
        }
        if let Some(u) = &md.user {
            lines.push(format!("User    : {}", u.name));
        }
        lines.push(format!("Generated: {}", md.generated_time));
        if let Some(ps) = &md.platform_statistics {
            if let Some(tps) = &ps.tps {
                lines.push(format!(
                    "TPS     : 1m={:.2} 5m={:.2} 15m={:.2}",
                    tps.last1m, tps.last5m, tps.last15m
                ));
            }
            if let Some(mspt) = ps.mspt.as_ref().and_then(|m| m.last1m.as_ref()) {
                lines.push(format!(
                    "MSPT    : mean={:.3} median={:.3} max={:.3}",
                    mspt.mean, mspt.median, mspt.max
                ));
            }
        }
        if let Some(ss) = &md.system_statistics {
            if let Some(os) = &ss.os {
                lines.push(format!("OS      : {} {} ({})", os.name, os.version, os.arch));
            }
            if let Some(cpu) = &ss.cpu {
                lines.push(format!("CPU     : {} threads {}", cpu.threads, cpu.model_name));
            }
        }
        let mut plugs: Vec<_> = md.sources.keys().cloned().collect();
        plugs.sort();
        if !plugs.is_empty() {
            lines.push(format!("Sources : {}", plugs.join(", ")));
        }
    }
    lines.push(format!(
        "Windows : {}",
        data.time_window_statistics.len()
    ));
    let mut wins: Vec<_> = data.time_window_statistics.iter().collect();
    wins.sort_by_key(|(k, _)| *k);
    for (id, w) in wins.into_iter().take(20) {
        lines.push(format!(
            "  window {id}: tps={:.2} msptMed={:.3} msptMax={:.3} cpuP={:.1}% players={}",
            w.tps,
            w.mspt_median,
            w.mspt_max,
            w.cpu_process * 100.0,
            w.players
        ));
    }
    Ok(lines.join("\n") + "\n")
}

pub fn render_raw_json(
    bytes: &[u8],
    kind: ContentKind,
) -> Result<String, Box<dyn std::error::Error>> {
    // Decode then re-encode via Debug-ish JSON using prost reflection-less approach:
    // For true raw dump we serialize key fields; full protobuf JSON isn't built-in.
    // Use a practical approach: for sampler, normalize into graph meta + thread tree summary JSON.
    match kind {
        ContentKind::Sampler => {
            let data = SamplerData::decode(bytes)?;
            let graph = ProfileGraph::from_sampler(data);
            let threads: Vec<Value> = graph
                .threads
                .iter()
                .map(|t| {
                    json!({
                        "id": t.id,
                        "name": t.name,
                        "time": t.times.iter().sum::<f64>(),
                        "children": t.children.len(),
                    })
                })
                .collect();
            Ok(serde_json::to_string_pretty(&json!({
                "type": "sampler",
                "meta": graph.meta,
                "threads": threads,
                "node_count": graph.nodes_by_id.len(),
                "sources": graph.all_sources,
                "time_windows": graph.time_windows,
            }))? + "\n")
        }
        ContentKind::Heap => {
            let data = HeapData::decode(bytes)?;
            Ok(serde_json::to_string_pretty(&json!({
                "type": "heap",
                "entries": data.entries.iter().map(|e| json!({
                    "order": e.order, "instances": e.instances, "size": e.size, "type": e.r#type
                })).collect::<Vec<_>>(),
            }))? + "\n")
        }
        ContentKind::Health => {
            let data = HealthData::decode(bytes)?;
            Ok(serde_json::to_string_pretty(&json!({
                "type": "health",
                "windows": data.time_window_statistics.len(),
            }))? + "\n")
        }
    }
}

pub fn render_meta_only(
    bytes: &[u8],
    kind: ContentKind,
    format: Format,
) -> Result<String, Box<dyn std::error::Error>> {
    match kind {
        ContentKind::Sampler => {
            let data = SamplerData::decode(bytes)?;
            let graph = ProfileGraph::from_sampler(data);
            if matches!(format, Format::Json) {
                Ok(serde_json::to_string_pretty(&graph.meta)? + "\n")
            } else {
                Ok(format_meta_text(&graph.meta))
            }
        }
        ContentKind::Heap => render_heap_bytes(bytes, None, 0, format),
        ContentKind::Health => render_health_bytes(bytes, format),
    }
}

fn fmt_duration(ms: i64) -> String {
    if ms < 1000 {
        return format!("{ms} ms");
    }
    let s = ms as f64 / 1000.0;
    if s < 60.0 {
        return format!("{s:.1} s");
    }
    let m = (s / 60.0).floor() as i64;
    let rem = s - m as f64 * 60.0;
    format!("{m}m {rem:.1}s")
}

fn fmt_bytes(n: i64) -> String {
    let mut v = n as f64;
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut i = 0;
    while v >= 1024.0 && i < units.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} {}", units[i])
    } else {
        format!("{v:.2} {}", units[i])
    }
}
