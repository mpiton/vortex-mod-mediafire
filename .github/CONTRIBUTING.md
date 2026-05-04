# Contributing to vortex-mod-mediafire

Thanks for taking the time to contribute! This crate is a WASM plugin for the
[Vortex download manager](https://github.com/mpiton/vortex). It targets
`wasm32-wasip1` via Extism PDK and is loaded by the Vortex host at runtime.

## How to Contribute

### Reporting Bugs

1. Check if the bug has already been reported in [Issues](https://github.com/mpiton/vortex-mod-mediafire/issues)
2. If not, create a new issue using the **Bug Report** template
3. Include the MediaFire URL shape (without sensitive parts), the `vortex --version`,
   and the plugin version

### Suggesting Features

1. Check existing [Feature Requests](https://github.com/mpiton/vortex-mod-mediafire/issues?q=label%3Aenhancement)
2. Open a new issue using the **Feature Request** template
3. Describe the MediaFire URL shape or capability and the use case

### Pull Requests

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/your-feature`)
3. Add a **failing test first** — see existing fixtures in `tests/fixtures/*.html`
4. Implement the change in `src/parser.rs` / `src/url_matcher.rs` / `src/lib.rs`
5. Run `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`
6. Build the WASM artefact (`cargo build --target wasm32-wasip1 --release`)
   and check `wasm_smoke.rs` still passes
7. Commit using [Conventional Commits](https://www.conventionalcommits.org/)
8. Push to your fork and open a Pull Request

### Commit Message Format

```
<type>(<scope>): <description>

[optional body]
```

Types: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `chore`, `ci`
Scopes: `parser`, `url-matcher`, `plugin-api`, `error`, `tests`, `build`

Example: `fix(parser): handle data-scrambled-url with whitespace padding`

## Development Setup

```bash
# Prerequisites
rustup target add wasm32-wasip1

# Clone
git clone https://github.com/mpiton/vortex-mod-mediafire.git
cd vortex-mod-mediafire

# Native unit tests + parser fixtures + WASM smoke
cargo test

# Lint + format
cargo clippy --all-targets -- -D warnings
cargo fmt --check

# Build WASM release artefact (1.1 MB)
cargo build --target wasm32-wasip1 --release
# → target/wasm32-wasip1/release/vortex_mod_mediafire.wasm
```

## Adding a fixture

Real MediaFire pages drift over time. To add a new variant:

1. Save the relevant HTML snippet to `tests/fixtures/NN_<short_name>.html`
   (only the `<a>` download button + label is needed — strip everything else)
2. Add a `#[case]` row to `parser_fixtures.rs::parses_recognised_fixture`
3. Run `cargo test` — RED first, then make the parser pass without breaking
   any existing fixture
4. Update `fixture_count_matches_acceptance_criterion` if you removed any

## Security

MediaFire's `data-scrambled-url` attribute is base64-encoded. The decoder
in `parser.rs::decode_scrambled_url` enforces a `download<n>.mediafire.com`
host allow-list — **do not relax this check**. An attacker controlling
the scramble payload could otherwise redirect the host download engine
to an arbitrary URL.

For sensitive vulnerability reports, see [SECURITY.md](SECURITY.md).

## Code of Conduct

This project follows the upstream
[Vortex Code of Conduct](https://github.com/mpiton/vortex/blob/main/CODE_OF_CONDUCT.md).
By participating, you agree to uphold it.

## Questions

Open a [Discussion](https://github.com/mpiton/vortex-mod-mediafire/discussions)
or file an issue using the **Question** template.
