//! MediaFire URL detection and parsing.
//!
//! Recognised shapes:
//! - File: `https://www.mediafire.com/file/<key>[/<filename>][/file]`
//! - Folder: `https://www.mediafire.com/folder/<key>[/<name>]` (out of scope here — folder enumeration is a crawler concern)
//!
//! The matcher only accepts http(s) and the canonical hosts
//! `www.mediafire.com`, `mediafire.com`, `m.mediafire.com`. Anything
//! else falls through to [`UrlKind::Unknown`].

use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UrlKind {
    /// Single file link: `mediafire.com/file/<key>/...`
    File,
    /// Folder link: `mediafire.com/folder/<key>/...`
    Folder,
    /// Anything else.
    Unknown,
}

pub fn classify_url(url: &str) -> UrlKind {
    let Some((host, path)) = validate_and_split(url) else {
        return UrlKind::Unknown;
    };
    if !is_mediafire_host(&host) {
        return UrlKind::Unknown;
    }
    let path = normalize_path(path);
    if file_regex().is_match(path) {
        return UrlKind::File;
    }
    if folder_regex().is_match(path) {
        return UrlKind::Folder;
    }
    UrlKind::Unknown
}

/// Extract the file key (`<key>`) from a recognised file URL.
pub fn extract_file_key(url: &str) -> Option<String> {
    let (host, path) = validate_and_split(url)?;
    if !is_mediafire_host(&host) {
        return None;
    }
    let path = normalize_path(path);
    file_regex()
        .captures(path)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

/// Extract the optional filename hint from the file URL path.
///
/// MediaFire file URLs often carry the original filename as the third
/// segment: `/file/<key>/<filename>[/file]`. The hint is best-effort —
/// the authoritative filename comes from the parsed download page.
/// Returns `None` when the second segment is the literal `file` suffix
/// or absent.
pub fn extract_filename_hint(url: &str) -> Option<String> {
    let (host, path) = validate_and_split(url)?;
    if !is_mediafire_host(&host) {
        return None;
    }
    let path = normalize_path(path);
    let caps = file_regex().captures(path)?;
    let raw = caps.get(2)?.as_str();
    if raw.is_empty() || raw == "file" {
        return None;
    }
    Some(raw.to_string())
}

fn is_mediafire_host(host: &str) -> bool {
    matches!(
        host,
        "mediafire.com" | "www.mediafire.com" | "m.mediafire.com"
    )
}

fn normalize_path(path: &str) -> &str {
    let no_frag = path.split('#').next().unwrap_or("");
    let no_query = no_frag.split('?').next().unwrap_or("");
    no_query.trim_end_matches('/')
}

fn file_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        // /file/<key>(/<filename>)?(/file)?  — `<filename>` cannot itself be the literal `file`
        // segment when followed by nothing, but we accept it ambiguously and the caller
        // resolves it via [`extract_filename_hint`].
        Regex::new(r"^/file/([A-Za-z0-9]+)(?:/([^/]+))?(?:/file)?$")
            .expect("file_regex: compile-time constant regex must compile")
    })
}

fn folder_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"^/folder/([A-Za-z0-9]+)(?:/[^/]+)?$")
            .expect("folder_regex: compile-time constant regex must compile")
    })
}

fn validate_and_split(url: &str) -> Option<(String, &str)> {
    let (scheme, rest) = url.split_once("://")?;
    if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") {
        return None;
    }
    let (authority, path_and_query) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, ""),
    };
    let authority_no_user = authority.rsplit('@').next().unwrap_or(authority);
    let host = authority_no_user
        .split(':')
        .next()
        .unwrap_or(authority_no_user);
    if host.is_empty() {
        return None;
    }
    Some((host.to_ascii_lowercase(), path_and_query))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("https://www.mediafire.com/file/abc123def456", UrlKind::File)]
    #[case("https://mediafire.com/file/abc123def456/file", UrlKind::File)]
    #[case(
        "https://www.mediafire.com/file/abc123/myarchive.zip/file",
        UrlKind::File
    )]
    #[case("https://www.mediafire.com/file/abc123/myarchive.zip", UrlKind::File)]
    #[case("https://m.mediafire.com/file/xyz789", UrlKind::File)]
    #[case("https://www.mediafire.com/folder/foo123", UrlKind::Folder)]
    #[case("https://example.com/file/abc", UrlKind::Unknown)]
    #[case("ftp://www.mediafire.com/file/abc", UrlKind::Unknown)]
    #[case("not a url", UrlKind::Unknown)]
    fn classify_url_recognises_shapes(#[case] url: &str, #[case] expected: UrlKind) {
        assert_eq!(classify_url(url), expected);
    }

    #[test]
    fn classify_handles_query_and_fragment() {
        assert_eq!(
            classify_url("https://www.mediafire.com/file/abc123/?foo=bar#x"),
            UrlKind::File
        );
    }

    #[test]
    fn extract_file_key_from_short_path() {
        assert_eq!(
            extract_file_key("https://www.mediafire.com/file/abc123def456"),
            Some("abc123def456".into())
        );
    }

    #[test]
    fn extract_file_key_from_full_path_with_filename() {
        assert_eq!(
            extract_file_key("https://www.mediafire.com/file/abc123/myfile.zip/file"),
            Some("abc123".into())
        );
    }

    #[test]
    fn extract_file_key_from_folder_returns_none() {
        assert_eq!(
            extract_file_key("https://www.mediafire.com/folder/abc123"),
            None
        );
    }

    #[test]
    fn extract_file_key_from_other_host_returns_none() {
        assert_eq!(extract_file_key("https://example.com/file/abc123"), None);
    }

    #[test]
    fn extract_filename_hint_picks_segment_after_key() {
        assert_eq!(
            extract_filename_hint("https://www.mediafire.com/file/abc123/myfile.zip/file"),
            Some("myfile.zip".into())
        );
    }

    #[test]
    fn extract_filename_hint_none_when_absent() {
        assert_eq!(
            extract_filename_hint("https://www.mediafire.com/file/abc123"),
            None
        );
    }

    #[test]
    fn classify_rejects_malformed_key() {
        assert_eq!(
            classify_url("https://www.mediafire.com/file/abc-typo"),
            UrlKind::Unknown
        );
    }
}
