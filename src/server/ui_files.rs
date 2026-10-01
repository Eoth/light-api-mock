//! Where the UI's files come from.
//!
//! A release binary carries the built UI (embedded by build.rs), so that one file is a complete Mimicway.
//! `STATIC_DIR` serves a directory instead: for UI development, or a customized UI. A binary built without the UI
//! falls back to `./frontend/dist`, as it did before the UI was embedded.
use axum::body::Body;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use std::path::PathBuf;

include!(concat!(env!("OUT_DIR"), "/embedded_ui.rs"));

/// Files as (path, content) pairs, sorted by path.
pub type FileTable = &'static [(&'static str, &'static [u8])];

#[derive(Debug, Clone)]
pub enum UiSource {
    Directory(PathBuf),
    Embedded(FileTable),
}

impl UiSource {
    /// `STATIC_DIR` when set, else the UI embedded in this binary, else `./frontend/dist`.
    pub fn from_env() -> Self {
        Self::choose(std::env::var("STATIC_DIR").ok(), EMBEDDED_UI)
    }

    fn choose(static_dir: Option<String>, embedded: FileTable) -> Self {
        match static_dir.filter(|dir| !dir.trim().is_empty()) {
            Some(dir) => UiSource::Directory(PathBuf::from(dir)),
            None if !embedded.is_empty() => UiSource::Embedded(embedded),
            None => UiSource::Directory(PathBuf::from("./frontend/dist")),
        }
    }

    /// For the startup log.
    pub fn describe(&self) -> String {
        match self {
            UiSource::Directory(dir) => format!("directory {}", dir.display()),
            UiSource::Embedded(files) => format!("embedded in the binary ({} files)", files.len()),
        }
    }
}

/// Answers a request for a UI file from `files`: `/` and directory paths give their `index.html`, other paths must
/// match a file exactly (so no path can lead outside the table). Fingerprinted files under `assets/` never change
/// under the same name and may be cached for good; anything else is revalidated, so a new release shows at once.
pub fn serve(files: FileTable, method: &Method, uri: &Uri) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() || path.ends_with('/') {
        format!("{path}index.html")
    } else {
        path.to_string()
    };
    let Ok(index) = files.binary_search_by(|(name, _)| (*name).cmp(path.as_str())) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content = files[index].1;
    let cache = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(content)
    };
    (
        [(CONTENT_TYPE, content_type(&path)), (CACHE_CONTROL, cache)],
        body,
    )
        .into_response()
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests;
