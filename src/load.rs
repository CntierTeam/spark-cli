use std::fs;
use std::path::Path;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentKind {
    Sampler,
    Heap,
    Health,
}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("http {status}: {body}")]
    Http { status: u16, body: String },
    #[error("{0}")]
    Msg(String),
}

pub fn load_bytes(input: &str) -> Result<Vec<u8>, LoadError> {
    let p = Path::new(input);
    if p.is_file() {
        return Ok(fs::read(p)?);
    }
    // bytebin code
    let base = std::env::var("BYTEBIN_URL").unwrap_or_else(|_| "https://bytebin.lucko.me/".into());
    let url = if input.starts_with("http://") || input.starts_with("https://") {
        input.to_string()
    } else {
        format!("{base}{input}")
    };
    // tiny std-only HTTP via `curl` fallback if no TLS crate — use ureq would need dep.
    // Prefer native: spawn curl for portability in standalone builds without openssl.
    let output = std::process::Command::new("curl")
        .args(["-fsSL", "-A", "spark-dump", &url])
        .output()
        .map_err(|e| LoadError::Msg(format!("curl failed to start ({e}); pass a local file path")))?;
    if !output.status.success() {
        return Err(LoadError::Http {
            status: 0,
            body: String::from_utf8_lossy(&output.stderr).into(),
        });
    }
    Ok(output.stdout)
}

pub fn detect_kind(
    input: &str,
    forced: Option<ContentKind>,
    _bytes: &[u8],
) -> Result<ContentKind, LoadError> {
    if let Some(k) = forced {
        return Ok(k);
    }
    let path = Path::new(input);
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("sparkprofile") => Ok(ContentKind::Sampler),
        Some("sparkheap") => Ok(ContentKind::Heap),
        Some("sparkhealth") => Ok(ContentKind::Health),
        _ => {
            // Heuristic: try sampler first by magic — CONSOLE often near start for sampler.
            // Default sampler for opaque bytebin codes (most common).
            Ok(ContentKind::Sampler)
        }
    }
}
