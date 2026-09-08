use std::collections::{HashMap, HashSet};

use prost::Message;
use serde::Serialize;

use crate::filter::WindowSelect;
use crate::spark::{SamplerData, StackTraceNode, ThreadNode};

#[derive(Debug, Clone, Serialize)]
pub struct MetaSnapshot {
    pub platform_brand: String,
    pub platform_name: String,
    pub platform_version: String,
    pub minecraft_version: String,
    pub spark_version: i32,
    pub platform_type: String,
    pub user: String,
    pub comment: String,
    pub start_time_ms: i64,
    pub end_time_ms: i64,
    pub duration_ms: i64,
    pub interval_us: i32,
    pub sampler_mode: String,
    pub sampler_engine: String,
    pub sampler_engine_version: String,
    pub tps_1m: Option<f64>,
    pub tps_5m: Option<f64>,
    pub tps_15m: Option<f64>,
    pub tps_target: Option<i32>,
    pub mspt_mean: Option<f64>,
    pub mspt_median: Option<f64>,
    pub mspt_p95: Option<f64>,
    pub mspt_max: Option<f64>,
    pub players: Option<i64>,
    pub heap_used: Option<i64>,
    pub heap_committed: Option<i64>,
    pub os: String,
    pub arch: String,
    pub cpu_threads: Option<i32>,
    pub cpu_model: String,
    pub cpu_process_1m: Option<f64>,
    pub cpu_system_1m: Option<f64>,
    pub java: String,
    pub plugins_mods: Vec<String>,
    pub time_window_ids: Vec<i32>,
}

#[derive(Debug, Clone)]
pub struct ProfileGraph {
    pub meta: MetaSnapshot,
    pub threads: Vec<Node>,
    pub nodes_by_id: HashMap<u32, Node>,
    pub parents: HashMap<u32, Vec<u32>>,
    pub id_to_source: HashMap<u32, String>,
    pub all_sources: Vec<String>,
    /// Ordered window ids matching times[] index
    pub time_windows: Vec<i32>,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: u32,
    pub is_thread: bool,
    pub name: String,
    pub class_name: String,
    pub method_name: String,
    pub method_desc: String,
    pub line_number: i32,
    pub times: Vec<f64>,
    pub children: Vec<u32>,
}

impl Node {
    pub fn label(&self) -> String {
        if self.is_thread {
            return self.name.clone();
        }
        let line = if self.line_number > 0 {
            format!(":{}", self.line_number)
        } else {
            String::new()
        };
        if self.class_name == "native" {
            if self.method_name.is_empty() {
                format!("native{line}")
            } else {
                format!("native {}{line}", self.method_name)
            }
        } else if !self.class_name.is_empty() && !self.method_name.is_empty() {
            format!("{}.{}{line}", self.class_name, self.method_name)
        } else if !self.class_name.is_empty() {
            format!("{}{line}", self.class_name)
        } else if !self.method_name.is_empty() {
            format!("{}{line}", self.method_name)
        } else {
            "(unknown)".into()
        }
    }

    pub fn key(&self) -> String {
        format!(
            "{}\0{}\0{}",
            self.class_name, self.method_name, self.method_desc
        )
    }

    pub fn time_for(&self, selected: &HashSet<usize>) -> f64 {
        if self.times.is_empty() {
            return 0.0;
        }
        if selected.is_empty() {
            return self.times.iter().sum();
        }
        self.times
            .iter()
            .enumerate()
            .filter(|(i, _)| selected.contains(i))
            .map(|(_, t)| *t)
            .sum()
    }
}

impl ProfileGraph {
    pub fn from_sampler_bytes(bytes: &[u8]) -> Result<Self, prost::DecodeError> {
        let data = SamplerData::decode(bytes)?;
        Ok(Self::from_sampler(data))
    }

    pub fn from_sampler(data: SamplerData) -> Self {
        let meta = meta_from_sampler(&data);
        let time_windows = data.time_windows.clone();
        let mut next_id: u32 = 1;
        let mut nodes_by_id = HashMap::new();
        let mut parents: HashMap<u32, Vec<u32>> = HashMap::new();
        let mut threads = Vec::new();

        for thread in data.threads {
            let tid = build_thread(
                thread,
                &mut next_id,
                &mut nodes_by_id,
                &mut parents,
            );
            threads.push(nodes_by_id.get(&tid).unwrap().clone());
        }

        let (all_sources, id_to_source) =
            build_sources(&nodes_by_id, &data.class_sources, &data.method_sources, &data.line_sources);

        Self {
            meta,
            threads,
            nodes_by_id,
            parents,
            id_to_source,
            all_sources,
            time_windows,
        }
    }

    pub fn selected_window_indices(&self, sel: &WindowSelect) -> HashSet<usize> {
        let n = self.time_windows.len();
        match sel {
            WindowSelect::All => {
                if n == 0 {
                    // times[] may still have a single bucket with no window ids
                    let mut s = HashSet::new();
                    s.insert(0);
                    s
                } else {
                    (0..n).collect()
                }
            }
            WindowSelect::Ids(ids) => {
                let mut set = HashSet::new();
                for (i, wid) in self.time_windows.iter().enumerate() {
                    if ids.contains(wid) {
                        set.insert(i);
                    }
                }
                if set.is_empty() {
                    set.insert(0);
                }
                set
            }
            WindowSelect::Range(a, b) => {
                let mut set = HashSet::new();
                if n == 0 {
                    set.insert(0);
                } else {
                    let lo = (*a).min(n.saturating_sub(1));
                    let hi = (*b).min(n.saturating_sub(1));
                    let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
                    for i in lo..=hi {
                        set.insert(i);
                    }
                }
                set
            }
        }
    }

    pub fn get(&self, id: u32) -> Option<&Node> {
        self.nodes_by_id.get(&id)
    }

    pub fn source_of(&self, id: u32) -> Option<&str> {
        self.id_to_source.get(&id).map(|s| s.as_str())
    }
}

fn build_thread(
    mut thread: ThreadNode,
    next_id: &mut u32,
    nodes: &mut HashMap<u32, Node>,
    parents: &mut HashMap<u32, Vec<u32>>,
) -> u32 {
    let flat = std::mem::take(&mut thread.children);
    let refs = std::mem::take(&mut thread.children_refs);
    let tid = *next_id;
    *next_id += 1;

    let child_ids = if !refs.is_empty() {
        refs.into_iter()
            .filter_map(|idx| {
                build_from_flat(&flat, idx as usize, tid, next_id, nodes, parents)
            })
            .collect()
    } else {
        flat.into_iter()
            .map(|n| build_owned(n, tid, next_id, nodes, parents))
            .collect()
    };

    let times = if thread.times.is_empty() {
        vec![thread.time]
    } else {
        thread.times
    };

    let node = Node {
        id: tid,
        is_thread: true,
        name: thread.name,
        class_name: String::new(),
        method_name: String::new(),
        method_desc: String::new(),
        line_number: 0,
        times,
        children: child_ids,
    };
    nodes.insert(tid, node);
    tid
}

fn build_from_flat(
    flat: &[StackTraceNode],
    idx: usize,
    parent: u32,
    next_id: &mut u32,
    nodes: &mut HashMap<u32, Node>,
    parents: &mut HashMap<u32, Vec<u32>>,
) -> Option<u32> {
    let n = flat.get(idx)?;
    let id = *next_id;
    *next_id += 1;
    parents.entry(id).or_default().push(parent);

    let child_ids = if !n.children_refs.is_empty() {
        n.children_refs
            .iter()
            .filter_map(|&r| build_from_flat(flat, r as usize, id, next_id, nodes, parents))
            .collect()
    } else {
        n.children
            .iter()
            .cloned()
            .map(|c| build_owned(c, id, next_id, nodes, parents))
            .collect()
    };

    let times = if n.times.is_empty() {
        vec![n.time]
    } else {
        n.times.clone()
    };

    nodes.insert(
        id,
        Node {
            id,
            is_thread: false,
            name: String::new(),
            class_name: n.class_name.clone(),
            method_name: n.method_name.clone(),
            method_desc: n.method_desc.clone(),
            line_number: n.line_number,
            times,
            children: child_ids,
        },
    );
    Some(id)
}

fn build_owned(
    n: StackTraceNode,
    parent: u32,
    next_id: &mut u32,
    nodes: &mut HashMap<u32, Node>,
    parents: &mut HashMap<u32, Vec<u32>>,
) -> u32 {
    let id = *next_id;
    *next_id += 1;
    parents.entry(id).or_default().push(parent);
    let child_ids = n
        .children
        .into_iter()
        .map(|c| build_owned(c, id, next_id, nodes, parents))
        .collect();
    let times = if n.times.is_empty() {
        vec![n.time]
    } else {
        n.times
    };
    nodes.insert(
        id,
        Node {
            id,
            is_thread: false,
            name: String::new(),
            class_name: n.class_name,
            method_name: n.method_name,
            method_desc: n.method_desc,
            line_number: n.line_number,
            times,
            children: child_ids,
        },
    );
    id
}

fn build_sources(
    nodes: &HashMap<u32, Node>,
    class_sources: &HashMap<String, String>,
    method_sources: &HashMap<String, String>,
    line_sources: &HashMap<String, String>,
) -> (Vec<String>, HashMap<u32, String>) {
    let mut id_to_source = HashMap::new();
    let mut source_set = HashSet::new();
    for s in class_sources.values().chain(method_sources.values()).chain(line_sources.values()) {
        source_set.insert(s.clone());
    }

    for (id, node) in nodes {
        if node.is_thread || node.class_name.is_empty() {
            continue;
        }
        if node
            .class_name
            .starts_with("com.destroystokyo.paper.event.executor.asm.generated.")
        {
            continue;
        }
        let mut source = None;
        if !node.method_name.is_empty() && !node.method_desc.is_empty() {
            let key = format!(
                "{};{};{}",
                node.class_name, node.method_name, node.method_desc
            );
            source = method_sources.get(&key).cloned();
        }
        if source.is_none() && node.line_number != 0 {
            let key = format!("{};{}", node.class_name, node.line_number);
            source = line_sources.get(&key).cloned();
        }
        if source.is_none() {
            source = class_sources.get(&node.class_name).cloned();
        }
        if let Some(s) = source {
            if s != "minecraft" && s != "java" {
                id_to_source.insert(*id, s);
            }
        }
    }
    let mut all: Vec<_> = source_set.into_iter().collect();
    all.sort();
    (all, id_to_source)
}

fn meta_from_sampler(data: &SamplerData) -> MetaSnapshot {
    let md = data.metadata.as_ref();
    let platform = md.and_then(|m| m.platform.as_ref());
    let ps = md.and_then(|m| m.platform_statistics.as_ref());
    let ss = md.and_then(|m| m.system_statistics.as_ref());
    let tps = ps.and_then(|p| p.tps.as_ref());
    let mspt = ps.and_then(|p| p.mspt.as_ref()).and_then(|m| m.last1m.as_ref());
    let heap = ps.and_then(|p| p.memory.as_ref()).and_then(|m| m.heap.as_ref());
    let os = ss.and_then(|s| s.os.as_ref());
    let cpu = ss.and_then(|s| s.cpu.as_ref());
    let java = ss.and_then(|s| s.java.as_ref());
    let start = md.map(|m| m.start_time).unwrap_or(0);
    let end = md.map(|m| m.end_time).unwrap_or(0);
    let plugins: Vec<String> = md
        .map(|m| {
            let mut v: Vec<_> = m.sources.keys().cloned().collect();
            v.sort();
            v
        })
        .unwrap_or_default();

    MetaSnapshot {
        platform_brand: platform.map(|p| p.brand.clone()).unwrap_or_default(),
        platform_name: platform.map(|p| p.name.clone()).unwrap_or_default(),
        platform_version: platform.map(|p| p.version.clone()).unwrap_or_default(),
        minecraft_version: platform
            .map(|p| p.minecraft_version.clone())
            .unwrap_or_default(),
        spark_version: platform.map(|p| p.spark_version).unwrap_or(0),
        platform_type: match platform.map(|p| p.r#type).unwrap_or(0) {
            0 => "SERVER",
            1 => "CLIENT",
            2 => "PROXY",
            3 => "APPLICATION",
            _ => "UNKNOWN",
        }
        .into(),
        user: md
            .and_then(|m| m.user.as_ref())
            .map(|u| u.name.clone())
            .unwrap_or_default(),
        comment: md.map(|m| m.comment.clone()).unwrap_or_default(),
        start_time_ms: start,
        end_time_ms: end,
        duration_ms: end.saturating_sub(start),
        interval_us: md.map(|m| m.interval).unwrap_or(0),
        sampler_mode: match md.map(|m| m.sampler_mode).unwrap_or(0) {
            0 => "EXECUTION",
            1 => "ALLOCATION",
            _ => "UNKNOWN",
        }
        .into(),
        sampler_engine: match md.map(|m| m.sampler_engine).unwrap_or(0) {
            0 => "JAVA",
            1 => "ASYNC",
            _ => "UNKNOWN",
        }
        .into(),
        sampler_engine_version: md
            .map(|m| m.sampler_engine_version.clone())
            .unwrap_or_default(),
        tps_1m: tps.map(|t| t.last1m),
        tps_5m: tps.map(|t| t.last5m),
        tps_15m: tps.map(|t| t.last15m),
        tps_target: tps.map(|t| t.game_target_tps),
        mspt_mean: mspt.map(|m| m.mean),
        mspt_median: mspt.map(|m| m.median),
        mspt_p95: mspt.map(|m| m.percentile95),
        mspt_max: mspt.map(|m| m.max),
        players: ps.map(|p| p.player_count),
        heap_used: heap.map(|h| h.used),
        heap_committed: heap.map(|h| h.committed),
        os: os
            .map(|o| format!("{} {}", o.name, o.version))
            .unwrap_or_default(),
        arch: os.map(|o| o.arch.clone()).unwrap_or_default(),
        cpu_threads: cpu.map(|c| c.threads),
        cpu_model: cpu.map(|c| c.model_name.clone()).unwrap_or_default(),
        cpu_process_1m: cpu.and_then(|c| c.process_usage.as_ref()).map(|u| u.last1m),
        cpu_system_1m: cpu.and_then(|c| c.system_usage.as_ref()).map(|u| u.last1m),
        java: java
            .map(|j| format!("{} {}", j.vendor, j.version))
            .unwrap_or_default(),
        plugins_mods: plugins,
        time_window_ids: data.time_windows.clone(),
    }
}
