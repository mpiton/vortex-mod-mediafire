//! Smoke test: load the compiled `.wasm` via Extism and call the pure
//! `can_handle` / `supports_playlist` exports.
//!
//! `extract_links` and `resolve_stream_url` need a real `http_request`
//! round-trip — exercised by the host's own integration tests, not
//! here. The stub `http_request` returns an HTTP-like JSON envelope so
//! the WASM module loads without unresolved imports.
//!
//! Skipped unless the WASM artifact is present at
//! `target/wasm32-wasip1/release/vortex_mod_mediafire.wasm`. To produce
//! it:
//!
//! ```bash
//! cargo build --target wasm32-wasip1 --release
//! ```

use std::path::PathBuf;

use extism::{Function, UserData, Val, PTR};

const WASM_REL_PATH: &str = "target/wasm32-wasip1/release/vortex_mod_mediafire.wasm";

fn wasm_path() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(WASM_REL_PATH);
    p.exists().then_some(p)
}

fn stub_http_request() -> Function {
    Function::new(
        "http_request",
        [PTR],
        [PTR],
        UserData::<()>::default(),
        |plugin, _inputs, outputs, _user_data: UserData<()>| {
            let body = r#"{"status":200,"headers":{},"body":""}"#;
            let handle = plugin.memory_new(body)?;
            outputs[0] = Val::I64(handle.offset() as i64);
            Ok(())
        },
    )
}

fn load_plugin(path: &PathBuf) -> extism::Plugin {
    let manifest = extism::Manifest::new([extism::Wasm::file(path)]);
    extism::Plugin::new(&manifest, [stub_http_request()], true).expect("load wasm")
}

/// Resolve the WASM artefact path or skip the calling test with a build hint.
macro_rules! require_wasm {
    () => {
        match wasm_path() {
            Some(p) => p,
            None => {
                eprintln!(
                    "skipping: build with `cargo build --target wasm32-wasip1 --release` first"
                );
                return;
            }
        }
    };
}

#[test]
fn wasm_can_handle_recognises_mediafire_file_url() {
    let path = require_wasm!();
    let mut plugin = load_plugin(&path);
    let result: String = plugin
        .call(
            "can_handle",
            "https://www.mediafire.com/file/abc123/foo.zip",
        )
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
        .call(
            "supports_playlist",
            "https://www.mediafire.com/file/abc/foo.zip",
        )
        .expect("supports_playlist call");
    assert_eq!(result.trim(), "false");
}
