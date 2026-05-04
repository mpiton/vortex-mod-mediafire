//! Vortex MediaFire WASM plugin.
//!
//! Implements the plugin contract used by the Vortex plugin host:
//! - `can_handle(url)` → `"true"` / `"false"`
//! - `supports_playlist(url)` → always `"false"` (single-file hoster)
//! - `extract_links(url)` → JSON metadata for the resolved file
//! - `resolve_stream_url(input)` → direct CDN URL
//!
//! Network access is delegated to the host via `http_request`. Parsing is
//! pure (`parser.rs`) so it can be exercised natively without WASM.

pub mod error;
pub mod parser;
pub mod url_matcher;

#[cfg(target_family = "wasm")]
mod plugin_api;

use serde::Serialize;

use crate::error::PluginError;
use crate::parser::ParsedFile;
use crate::url_matcher::UrlKind;

// ── IPC DTOs ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ExtractLinksResponse {
    pub kind: &'static str,
    pub files: Vec<FileLink>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct FileLink {
    pub id: String,
    pub url: String,
    pub filename: Option<String>,
    pub size_bytes: Option<u64>,
    pub direct_url: String,
    pub resumable: bool,
}

// ── Routing helpers ──────────────────────────────────────────────────────────

pub fn handle_can_handle(url: &str) -> String {
    bool_to_string(matches!(url_matcher::classify_url(url), UrlKind::File))
}

pub fn handle_supports_playlist(_url: &str) -> String {
    bool_to_string(false)
}

fn bool_to_string(b: bool) -> String {
    if b {
        "true".into()
    } else {
        "false".into()
    }
}

pub fn ensure_file_url(url: &str) -> Result<(), PluginError> {
    match url_matcher::classify_url(url) {
        UrlKind::File => Ok(()),
        _ => Err(PluginError::UnsupportedUrl(url.to_string())),
    }
}

// ── Response builders ────────────────────────────────────────────────────────

pub fn build_extract_links_response(source_url: &str, parsed: ParsedFile) -> ExtractLinksResponse {
    let id = url_matcher::extract_file_key(source_url).unwrap_or_default();
    let filename = parsed
        .filename
        .or_else(|| url_matcher::extract_filename_hint(source_url));
    let direct_url = parsed.direct_url;
    let link = FileLink {
        id,
        url: source_url.to_string(),
        filename,
        size_bytes: parsed.size_bytes,
        direct_url,
        resumable: true,
    };
    ExtractLinksResponse {
        kind: "file",
        files: vec![link],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_parsed() -> ParsedFile {
        ParsedFile {
            filename: Some("archive.zip".into()),
            size_bytes: Some(2048),
            direct_url: "https://download42.mediafire.com/abc/key/archive.zip".into(),
        }
    }

    // ── Routing ─────────────────────────────────────────────────────────────

    #[test]
    fn can_handle_recognises_file_url() {
        assert_eq!(
            handle_can_handle("https://www.mediafire.com/file/abc123/foo.zip"),
            "true"
        );
    }

    #[test]
    fn can_handle_rejects_folder() {
        assert_eq!(
            handle_can_handle("https://www.mediafire.com/folder/abc123"),
            "false",
            "folders are out of scope for the hoster plugin"
        );
    }

    #[test]
    fn can_handle_rejects_unrelated() {
        assert_eq!(handle_can_handle("https://example.com/file/abc"), "false");
    }

    #[test]
    fn supports_playlist_always_false() {
        assert_eq!(
            handle_supports_playlist("https://www.mediafire.com/file/abc/foo.zip"),
            "false"
        );
    }

    #[test]
    fn ensure_file_url_accepts_file() {
        ensure_file_url("https://www.mediafire.com/file/abc/foo.zip").unwrap();
    }

    #[test]
    fn ensure_file_url_rejects_folder() {
        let err = ensure_file_url("https://www.mediafire.com/folder/abc").unwrap_err();
        assert!(matches!(err, PluginError::UnsupportedUrl(_)));
    }

    // ── Response builder ────────────────────────────────────────────────────

    #[test]
    fn build_extract_links_response_includes_metadata() {
        let r = build_extract_links_response(
            "https://www.mediafire.com/file/abc123/archive.zip",
            sample_parsed(),
        );
        assert_eq!(r.kind, "file");
        assert_eq!(r.files.len(), 1);
        let f = &r.files[0];
        assert_eq!(f.id, "abc123");
        assert_eq!(f.url, "https://www.mediafire.com/file/abc123/archive.zip");
        assert_eq!(f.filename.as_deref(), Some("archive.zip"));
        assert_eq!(f.size_bytes, Some(2048));
        assert_eq!(
            f.direct_url,
            "https://download42.mediafire.com/abc/key/archive.zip"
        );
        assert!(f.resumable);
    }

    #[test]
    fn build_extract_links_response_falls_back_filename_to_url_hint() {
        let parsed = ParsedFile {
            filename: None,
            size_bytes: None,
            direct_url: "https://download1.mediafire.com/x/y/z.dat".into(),
        };
        let r = build_extract_links_response(
            "https://www.mediafire.com/file/abc123/archive.zip/file",
            parsed,
        );
        assert_eq!(
            r.files[0].filename.as_deref(),
            Some("archive.zip"),
            "missing parsed filename should fall back to the hint embedded in the source URL"
        );
    }

    #[test]
    fn extract_links_response_serialises_camelish_kind() {
        let r = build_extract_links_response(
            "https://www.mediafire.com/file/abc/x.zip",
            sample_parsed(),
        );
        let json = serde_json::to_string(&r).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["kind"], "file");
        assert!(parsed["files"][0]["resumable"].as_bool().unwrap_or(false));
    }
}
