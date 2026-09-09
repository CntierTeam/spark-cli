use crate::cli::FilterPat;

#[derive(Debug, Clone)]
pub enum WindowSelect {
    All,
    /// Match `timeWindows` ids (web Refine ids)
    Ids(Vec<i32>),
    /// Explicit indices into `times[]` / `timeWindows`
    Indices(Vec<usize>),
    /// Inclusive index range into `timeWindows`
    Range(usize, usize),
}

#[derive(Clone)]
pub struct FilterOpts {
    pub search: Option<String>,
    pub thread: Option<FilterPat>,
    pub includes: Vec<FilterPat>,
    pub excludes: Vec<FilterPat>,
    pub min_percent: f64,
    pub depth: usize,
    pub top: usize,
    pub windows: WindowSelect,
    pub top_threads: usize,
    pub min_thread_percent: f64,
}

impl FilterOpts {
    pub fn node_text_allowed(&self, class: &str, method: &str, source: Option<&str>) -> bool {
        let hay_parts = [class, method, source.unwrap_or("")];
        if !self.includes.is_empty() {
            let ok = self.includes.iter().any(|p| {
                hay_parts.iter().any(|h| p.is_match(h))
            });
            if !ok {
                return false;
            }
        }
        for p in &self.excludes {
            if hay_parts.iter().any(|h| p.is_match(h)) {
                return false;
            }
        }
        true
    }
}
