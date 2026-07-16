//! Real ABI smoke tests for every runtime export of the release WASM artifact.
//! A MediaFire page fixture exercises extraction and resolution through the
//! same Extism `http_request` boundary as Vortex.
//!
//! Requires the WASM artifact at
//! `target/wasm32-wasip1/release/vortex_mod_mediafire.wasm`. To produce
//! it:
//!
//! ```bash
//! cargo build --target wasm32-wasip1 --release
//! ```

use std::path::PathBuf;

use extism::{Function, UserData, Val, PTR};
use serde_json::{json, Value};

const WASM_REL_PATH: &str = "target/wasm32-wasip1/release/vortex_mod_mediafire.wasm";
const FILE_URL: &str = "https://www.mediafire.com/file/abc123/archive.zip";
const DIRECT_URL: &str = "https://download1.mediafire.com/abc123/archive.zip";
const FILE_PAGE: &str = r#"<a href="https://download1.mediafire.com/abc123/archive.zip"><span class="dl-btn-label" title="archive.zip">Download (1.50 MB)</span></a>"#;

fn wasm_path() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(WASM_REL_PATH);
    assert!(
        path.is_file(),
        "missing release WASM artifact at {}; run `cargo build --target wasm32-wasip1 --release` first",
        path.display()
    );
    path
}

fn stub_http_request() -> Function {
    Function::new(
        "http_request",
        [PTR],
        [PTR],
        UserData::<()>::default(),
        |plugin, _inputs, outputs, _user_data: UserData<()>| {
            let response = json!({ "status": 200, "headers": {}, "body": FILE_PAGE }).to_string();
            let handle = plugin.memory_new(&response)?;
            outputs[0] = Val::I64(handle.offset() as i64);
            Ok(())
        },
    )
}

fn load_plugin(path: &PathBuf) -> extism::Plugin {
    let manifest = extism::Manifest::new([extism::Wasm::file(path)]);
    extism::Plugin::new(&manifest, [stub_http_request()], true).expect("load wasm")
}

/// Require the release WASM artefact and report how to build it when missing.
macro_rules! require_wasm {
    () => {
        wasm_path()
    };
}

#[test]
fn wasm_can_handle_recognises_mediafire_file_url() {
    let path = require_wasm!();
    let mut plugin = load_plugin(&path);
    let result: String = plugin
        .call("can_handle", FILE_URL)
        .expect("can_handle call");
    assert_eq!(result.trim(), "true");
}

#[test]
fn wasm_can_handle_rejects_unrelated_url() {
    let path = require_wasm!();
    let mut plugin = load_plugin(&path);
    let result: String = plugin
        .call("can_handle", "https://example.com/some/page")
        .expect("can_handle call");
    assert_eq!(result.trim(), "false");
}

#[test]
fn wasm_supports_playlist_always_false() {
    let path = require_wasm!();
    let mut plugin = load_plugin(&path);
    let result: String = plugin
        .call("supports_playlist", FILE_URL)
        .expect("supports_playlist call");
    assert_eq!(result.trim(), "false");
}

#[test]
fn wasm_extraction_and_resolution_exports_are_callable() {
    let path = require_wasm!();
    let mut plugin = load_plugin(&path);

    let links: String = plugin
        .call("extract_links", FILE_URL)
        .expect("extract_links call");
    let links: Value = serde_json::from_str(&links).expect("extract_links JSON");
    assert_eq!(links["kind"], "file");
    assert_eq!(links["files"][0]["filename"], "archive.zip");
    assert_eq!(links["files"][0]["direct_url"], DIRECT_URL);

    let direct_url: String = plugin
        .call("resolve_stream_url", json!({ "url": FILE_URL }).to_string())
        .expect("resolve_stream_url call");
    assert_eq!(direct_url, DIRECT_URL);
}
