//! MediaFire HTML page parsing + HTTP envelope.
//!
//! The plugin host marshals each network call as a JSON-encoded
//! [`HttpRequest`] / [`HttpResponse`] pair through the `http_request`
//! host function. The pure parsing in this module makes it testable
//! natively without touching the host.

use std::collections::HashMap;
use std::sync::OnceLock;

use base64::Engine;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::PluginError;

const USER_AGENT: &str = "Mozilla/5.0 (Vortex/1.0; +https://vortex-app.com) MediaFirePlugin/1.0";

// ── HTTP envelope ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: String,
}

/// Reject pages larger than this so a malicious server can't make every
/// regex scan a multi-megabyte buffer. Real MediaFire landing pages weigh
/// well under 500 KB, so 2 MiB is a generous ceiling.
pub const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

impl HttpResponse {
    pub fn into_success_body(self) -> Result<String, PluginError> {
        if (200..300).contains(&self.status) {
            if self.body.len() > MAX_BODY_BYTES {
                return Err(PluginError::HttpStatus {
                    status: self.status,
                    message: format!("body exceeds {MAX_BODY_BYTES} bytes"),
                });
            }
            Ok(self.body)
        } else if self.status == 404 || self.status == 410 {
            Err(PluginError::Offline(format!("status {}", self.status)))
        } else {
            Err(PluginError::HttpStatus {
                status: self.status,
                message: truncate(&self.body, 256),
            })
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut cut = max;
        while !s.is_char_boundary(cut) && cut > 0 {
            cut -= 1;
        }
        format!("{}…", &s[..cut])
    }
}

pub fn parse_http_response(raw: &str) -> Result<HttpResponse, PluginError> {
    serde_json::from_str(raw).map_err(|e| PluginError::HostResponse(e.to_string()))
}

// ── Page request ─────────────────────────────────────────────────────────────

pub fn build_file_page_request(url: &str) -> Result<String, PluginError> {
    let mut headers = HashMap::new();
    headers.insert("User-Agent".to_string(), USER_AGENT.to_string());
    headers.insert(
        "Accept".to_string(),
        "text/html,application/xhtml+xml".to_string(),
    );
    let req = HttpRequest {
        method: "GET".into(),
        url: url.to_string(),
        headers,
        body: None,
    };
    serde_json::to_string(&req).map_err(PluginError::SerdeJson)
}

// ── Parsed file ──────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
pub struct ParsedFile {
    pub filename: Option<String>,
    pub size_bytes: Option<u64>,
    pub direct_url: String,
}

/// Decode a `data-scrambled-url` value.
///
/// MediaFire base64-encodes the direct download URL in this attribute
/// to slow down naive scrapers. We only accept decoded URLs whose host
/// belongs to MediaFire's `download*.mediafire.com` CDN — anything else
/// could be an attacker-controlled redirect target.
pub fn decode_scrambled_url(scrambled: &str) -> Option<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(scrambled.trim())
        .ok()?;
    let url = String::from_utf8(bytes).ok()?;
    if is_safe_download_url(&url) {
        Some(url)
    } else {
        None
    }
}

fn is_safe_download_url(url: &str) -> bool {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return false;
    }
    let after_scheme = match url.find("://") {
        Some(i) => &url[i + 3..],
        None => return false,
    };
    let host_end = after_scheme.find('/').unwrap_or(after_scheme.len());
    let host = &after_scheme[..host_end];
    // Allow download<digits>.mediafire.com or plain mediafire.com download CDNs
    download_host_regex().is_match(host)
}

fn download_host_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"^download[0-9]*\.mediafire\.com$")
            .expect("download_host_regex: compile-time constant must compile")
    })
}

// ── HTML parsing ─────────────────────────────────────────────────────────────

pub fn parse_file_page(html: &str) -> Result<ParsedFile, PluginError> {
    let direct_url = locate_direct_url(html).ok_or(PluginError::NoDirectLink)?;
    let filename = locate_filename(html).or_else(|| filename_from_url(&direct_url));
    let size_bytes = locate_size_text(html).and_then(|s| parse_size_bytes(&s));
    Ok(ParsedFile {
        filename,
        size_bytes,
        direct_url,
    })
}

fn locate_direct_url(html: &str) -> Option<String> {
    if let Some(decoded) =
        capture(html, scrambled_attr_regex()).and_then(|s| decode_scrambled_url(&s))
    {
        return Some(decoded);
    }
    // `plain_href_regex` already anchors the host to `download[0-9]*.mediafire.com`,
    // so no second `is_safe_download_url` check is needed on this branch.
    capture(html, plain_href_regex())
}

fn locate_filename(html: &str) -> Option<String> {
    capture(html, filename_title_regex())
}

fn locate_size_text(html: &str) -> Option<String> {
    capture(html, size_text_regex())
}

fn filename_from_url(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let last = path.rsplit('/').next()?;
    if last.is_empty() {
        return None;
    }
    Some(last.to_string())
}

fn capture(html: &str, re: &Regex) -> Option<String> {
    re.captures(html)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

fn scrambled_attr_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        // (?i) case-insensitive; non-greedy attribute capture
        Regex::new(r#"(?i)data-scrambled-url\s*=\s*"([^"]+)""#)
            .expect("scrambled_attr_regex: compile-time constant must compile")
    })
}

fn plain_href_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r#"(?i)href\s*=\s*"(https?://download[0-9]*\.mediafire\.com/[^"]+)""#)
            .expect("plain_href_regex: compile-time constant must compile")
    })
}

fn filename_title_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r#"(?i)class="dl-btn-label"\s+title="([^"]+)""#)
            .expect("filename_title_regex: compile-time constant must compile")
    })
}

fn size_text_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        // Match "Download (1.50 MB)" — case-insensitive
        Regex::new(r"(?i)Download\s*\(([^)]+)\)")
            .expect("size_text_regex: compile-time constant must compile")
    })
}

// ── Size parsing ─────────────────────────────────────────────────────────────

pub fn parse_size_bytes(text: &str) -> Option<u64> {
    let re = size_value_regex();
    let caps = re.captures(text)?;
    let value: f64 = caps.get(1)?.as_str().parse().ok()?;
    let unit = caps.get(2)?.as_str().to_ascii_uppercase();
    let multiplier: f64 = match unit.as_str() {
        "B" => 1.0,
        "KB" => 1024.0,
        "MB" => 1024.0 * 1024.0,
        "GB" => 1024.0 * 1024.0 * 1024.0,
        "TB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    let bytes = (value * multiplier).round();
    if bytes.is_finite() && bytes >= 0.0 {
        Some(bytes as u64)
    } else {
        None
    }
}

fn size_value_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)([0-9]+(?:\.[0-9]+)?)\s*(B|KB|MB|GB|TB)\b")
            .expect("size_value_regex: compile-time constant must compile")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── HTTP envelope ───────────────────────────────────────────────────────

    #[test]
    fn parse_http_response_round_trips_success() {
        let raw = r#"{"status":200,"headers":{},"body":"ok"}"#;
        let resp = parse_http_response(raw).unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, "ok");
    }

    #[test]
    fn into_success_body_passes_2xx() {
        let resp = HttpResponse {
            status: 200,
            headers: HashMap::new(),
            body: "<html>".into(),
        };
        assert_eq!(resp.into_success_body().unwrap(), "<html>");
    }

    #[test]
    fn into_success_body_maps_404_to_offline() {
        let resp = HttpResponse {
            status: 404,
            headers: HashMap::new(),
            body: "".into(),
        };
        let err = resp.into_success_body().unwrap_err();
        assert!(matches!(err, PluginError::Offline(_)));
    }

    #[test]
    fn into_success_body_maps_410_to_offline() {
        let resp = HttpResponse {
            status: 410,
            headers: HashMap::new(),
            body: "".into(),
        };
        let err = resp.into_success_body().unwrap_err();
        assert!(matches!(err, PluginError::Offline(_)));
    }

    #[test]
    fn into_success_body_rejects_oversized_2xx_payload() {
        let resp = HttpResponse {
            status: 200,
            headers: HashMap::new(),
            body: "x".repeat(MAX_BODY_BYTES + 1),
        };
        let err = resp.into_success_body().unwrap_err();
        assert!(matches!(err, PluginError::HttpStatus { status: 200, .. }));
    }

    #[test]
    fn into_success_body_maps_500_to_http_status() {
        let resp = HttpResponse {
            status: 500,
            headers: HashMap::new(),
            body: "boom".into(),
        };
        let err = resp.into_success_body().unwrap_err();
        assert!(matches!(err, PluginError::HttpStatus { status: 500, .. }));
    }

    // ── Page request builder ────────────────────────────────────────────────

    #[test]
    fn build_file_page_request_emits_get_with_user_agent() {
        let json = build_file_page_request("https://www.mediafire.com/file/abc/foo.zip").unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["method"], "GET");
        assert_eq!(v["url"], "https://www.mediafire.com/file/abc/foo.zip");
        assert!(
            v["headers"]
                .as_object()
                .and_then(|h| h.get("User-Agent"))
                .is_some(),
            "request must carry a User-Agent so MediaFire returns the JS-rendered page"
        );
    }

    // ── Scrambled URL decoder ───────────────────────────────────────────────

    #[test]
    fn decode_scrambled_url_returns_inner_url() {
        let plain = "https://download1234.mediafire.com/abc/myfile.zip";
        let scrambled = base64::engine::general_purpose::STANDARD.encode(plain.as_bytes());
        assert_eq!(decode_scrambled_url(&scrambled).as_deref(), Some(plain));
    }

    #[test]
    fn decode_scrambled_url_returns_none_for_garbage() {
        assert_eq!(decode_scrambled_url("not%base64!"), None);
    }

    #[test]
    fn decode_scrambled_url_rejects_non_download_target() {
        let bad = base64::engine::general_purpose::STANDARD.encode(b"https://evil.example/payload");
        assert_eq!(decode_scrambled_url(&bad), None);
    }

    // ── Page parser ─────────────────────────────────────────────────────────

    #[test]
    fn parse_file_page_extracts_plain_href() {
        let html = r#"
            <html><body>
              <a class="input popsok" aria-label="Download file"
                 href="https://download2261.mediafire.com/abc/key/myfile.zip">
                <span class="dl-btn-label" title="myfile.zip">Download (1.50 MB)</span>
              </a>
            </body></html>
        "#;
        let parsed = parse_file_page(html).unwrap();
        assert_eq!(
            parsed.direct_url,
            "https://download2261.mediafire.com/abc/key/myfile.zip"
        );
        assert_eq!(parsed.filename.as_deref(), Some("myfile.zip"));
        assert_eq!(parsed.size_bytes, Some(1_572_864));
    }

    #[test]
    fn parse_file_page_extracts_scrambled_href() {
        let plain = "https://download2261.mediafire.com/abc/key/scrambled.zip";
        let scrambled = base64::engine::general_purpose::STANDARD.encode(plain.as_bytes());
        let html = format!(
            r##"
            <html><body>
              <a class="input popsok" aria-label="Download file"
                 data-scrambled-url="{scrambled}" href="#">
                <span class="dl-btn-label" title="scrambled.zip">Download (10.00 KB)</span>
              </a>
            </body></html>
            "##
        );
        let parsed = parse_file_page(&html).unwrap();
        assert_eq!(parsed.direct_url, plain);
        assert_eq!(parsed.filename.as_deref(), Some("scrambled.zip"));
        assert_eq!(parsed.size_bytes, Some(10_240));
    }

    #[test]
    fn parse_file_page_no_link_returns_error() {
        let html = "<html><body>no download here</body></html>";
        let err = parse_file_page(html).unwrap_err();
        assert!(matches!(err, PluginError::NoDirectLink));
    }

    #[test]
    fn parse_file_page_size_can_be_missing() {
        let html = r#"
            <html><body>
              <a class="popsok" aria-label="Download file"
                 href="https://download1.mediafire.com/x/y/z.bin">
                <span class="dl-btn-label" title="z.bin">Download</span>
              </a>
            </body></html>
        "#;
        let parsed = parse_file_page(html).unwrap();
        assert_eq!(parsed.size_bytes, None);
        assert_eq!(parsed.filename.as_deref(), Some("z.bin"));
    }

    #[test]
    fn parse_file_page_filename_can_be_missing_falls_back_to_url_segment() {
        let html = r#"
            <html><body>
              <a class="popsok" aria-label="Download file"
                 href="https://download42.mediafire.com/abc/key/fallback.dat">
              </a>
            </body></html>
        "#;
        let parsed = parse_file_page(html).unwrap();
        assert_eq!(parsed.filename.as_deref(), Some("fallback.dat"));
    }

    // ── Size parsing ────────────────────────────────────────────────────────

    #[test]
    fn parse_size_bytes_recognises_units() {
        assert_eq!(parse_size_bytes("1.50 MB"), Some(1_572_864));
        assert_eq!(parse_size_bytes("10.00 KB"), Some(10_240));
        assert_eq!(parse_size_bytes("1 GB"), Some(1_073_741_824));
        assert_eq!(parse_size_bytes("123 B"), Some(123));
    }

    #[test]
    fn parse_size_bytes_returns_none_for_garbage() {
        assert_eq!(parse_size_bytes("nope"), None);
        assert_eq!(parse_size_bytes(""), None);
    }
}
