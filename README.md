# vortex-mod-mediafire

MediaFire WASM plugin for [Vortex](https://github.com/mpiton/vortex). Resolves
free MediaFire `https://www.mediafire.com/file/...` links to their direct
CDN URL on `download<n>.mediafire.com`.

## Features

- Plain `href` extraction from the public download landing page
- `data-scrambled-url` (base64) decoding for the obfuscated variant —
  decoded URL must resolve to a `download*.mediafire.com` host or it is
  rejected (defensive against attacker-controlled scramble payloads)
- Filename + size parsing from the `<span class="dl-btn-label">` label
- Falls back to the URL's filename hint and finally the path tail when
  the page omits an explicit label
- No wait, no captcha — the plugin returns the direct URL on first
  resolve

## URL shapes recognised

- `https://www.mediafire.com/file/<key>`
- `https://www.mediafire.com/file/<key>/file`
- `https://www.mediafire.com/file/<key>/<filename>`
- `https://www.mediafire.com/file/<key>/<filename>/file`
- `https://m.mediafire.com/file/<key>...` (mobile shorthand)

Folder links (`mediafire.com/folder/...`) are **not** in scope here —
folder enumeration is a crawler concern and lives in a separate plugin.

## Plugin contract

The plugin exports the standard Vortex plugin contract:

| Function                   | Input               | Output                          |
|---------------------------|---------------------|---------------------------------|
| `can_handle`              | URL string          | `"true"` / `"false"`            |
| `supports_playlist`       | URL string          | always `"false"`                |
| `extract_links`           | URL string          | JSON `ExtractLinksResponse`     |
| `resolve_stream_url`      | JSON `{url}`        | direct CDN URL string           |

`ExtractLinksResponse` mirrors the `LinkStatus::Online` shape used by the
host's link-check pipeline: each `files[]` entry carries `filename`,
`size_bytes`, `direct_url`, and `resumable: true`.

## Build

```bash
rustup target add wasm32-wasip1
cargo build --target wasm32-wasip1 --release
```

Resulting WASM: `target/wasm32-wasip1/release/vortex_mod_mediafire.wasm`.

## Install (development)

```bash
PLUGIN_DIR="$HOME/.local/share/dev.vortex.app/plugins/vortex-mod-mediafire"
mkdir -p "$PLUGIN_DIR"
cp plugin.toml "$PLUGIN_DIR/plugin.toml"
cp target/wasm32-wasip1/release/vortex_mod_mediafire.wasm \
   "$PLUGIN_DIR/vortex-mod-mediafire.wasm"
```

Vortex picks up the new plugin via the file watcher; no restart needed.

## Tests

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --target x86_64-unknown-linux-gnu -- -D warnings
cargo clippy --locked --lib --target wasm32-wasip1 -- -D warnings
cargo build --locked --lib --target wasm32-wasip1 --release
cargo test --locked --all-targets --target x86_64-unknown-linux-gnu
```

The HTML fixtures live in `tests/fixtures/*.html` — ten variants covering
plain href, large/small sizes, missing size text, missing filename,
extra attributes, and an offline page that triggers `NoDirectLink`.

## License

GPL-3.0 — same as Vortex core.
