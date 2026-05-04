//! Fixture-driven integration tests for the MediaFire HTML parser.
//!
//! Each fixture in `tests/fixtures/*.html` mirrors a shape we have
//! observed on real MediaFire pages. The acceptance criteria for
//! task 34 mandates ≥ 10 distinct variants — see
//! `vortex/.claude/output/sprints/prd-v2-roadmap/tasks/34-plugin-mediafire.md`.

use std::fs;
use std::path::Path;

use rstest::rstest;
use vortex_mod_mediafire::error::PluginError;
use vortex_mod_mediafire::parser::{parse_file_page, parse_size_bytes, ParsedFile};

const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn load_fixture(name: &str) -> String {
    let path = Path::new(FIXTURES_DIR).join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[rstest]
#[case(
    "01_plain_href.html",
    "https://download2261.mediafire.com/abc123def456g/archive.zip",
    Some("archive.zip"),
    Some(1_572_864)
)]
#[case(
    "02_filename_with_spaces.html",
    "https://download01.mediafire.com/key/My%20Document.pdf",
    Some("My Document.pdf"),
    Some(843_213) // 823.45 KB → round(823.45 * 1024)
)]
#[case(
    "03_large_gigabyte_file.html",
    "https://download42.mediafire.com/big/giant.iso",
    Some("giant.iso"),
    Some(4_509_715_661) // 4.20 GB
)]
#[case(
    "04_small_kilobyte_file.html",
    "https://download7.mediafire.com/x/notes.txt",
    Some("notes.txt"),
    Some(2_150) // 2.10 KB
)]
#[case(
    "05_no_filename_label.html",
    "https://download3.mediafire.com/k/key/data.bin",
    Some("data.bin"),
    None
)]
#[case(
    "06_no_size_text.html",
    "https://download10.mediafire.com/k/key/silent.bin",
    Some("silent.bin"),
    None
)]
#[case(
    "07_byte_size.html",
    "https://download21.mediafire.com/k/tiny.txt",
    Some("tiny.txt"),
    Some(512)
)]
#[case(
    "08_extra_attributes.html",
    "https://download108.mediafire.com/abc/photo.jpg",
    Some("photo.jpg"),
    Some(3_292_528) // 3.14 MB
)]
#[case(
    "10_terabyte_size.html",
    "https://download500.mediafire.com/k/huge.tar",
    Some("huge.tar"),
    Some(1_099_511_627_776) // 1 TB
)]
fn parses_recognised_fixture(
    #[case] fixture: &str,
    #[case] expected_url: &str,
    #[case] expected_filename: Option<&str>,
    #[case] expected_size: Option<u64>,
) {
    let html = load_fixture(fixture);
    let parsed = parse_file_page(&html).expect("fixture must parse");
    assert_eq!(parsed.direct_url, expected_url, "fixture: {fixture}");
    assert_eq!(
        parsed.filename.as_deref(),
        expected_filename,
        "fixture: {fixture}"
    );
    // Size parsing is float-rounded; allow ±2 byte tolerance for the
    // higher-precision GB/TB cases where the fixture's text uses
    // truncated decimals.
    match (parsed.size_bytes, expected_size) {
        (Some(got), Some(want)) => {
            let diff = got.abs_diff(want);
            assert!(
                diff <= 2,
                "fixture {fixture}: expected ~{want} bytes, got {got} ({diff} delta)"
            );
        }
        (None, None) => {}
        (got, want) => panic!("fixture {fixture}: expected size {want:?}, got {got:?}"),
    }
}

#[test]
fn offline_fixture_returns_no_direct_link() {
    let html = load_fixture("09_offline_404.html");
    let err = parse_file_page(&html).unwrap_err();
    assert!(
        matches!(err, PluginError::NoDirectLink),
        "removed-file pages must surface NoDirectLink, got: {err:?}"
    );
}

#[test]
fn fixture_count_matches_acceptance_criterion() {
    let entries = fs::read_dir(FIXTURES_DIR)
        .expect("fixtures dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("html"))
        .count();
    assert!(
        entries >= 10,
        "task 34 acceptance criteria requires ≥10 HTML fixtures, found {entries}"
    );
}

#[test]
fn parse_size_bytes_handles_each_supported_unit() {
    let cases = [
        ("123 B", 123u64),
        ("1.50 KB", 1_536),
        ("2.00 MB", 2_097_152),
        ("3.00 GB", 3_221_225_472),
        ("1.00 TB", 1_099_511_627_776),
    ];
    for (text, expected) in cases {
        let got = parse_size_bytes(text)
            .unwrap_or_else(|| panic!("parse_size_bytes('{text}') returned None"));
        let diff = got.abs_diff(expected);
        assert!(diff <= 2, "{text}: expected {expected}, got {got}");
    }
}

#[test]
fn parsed_file_struct_is_consistent() {
    // Sanity-check: building a `ParsedFile` and reading its fields back
    // works — keeps coverage on the public field set so the struct
    // definition can't silently lose fields without a test failure.
    let p = ParsedFile {
        filename: Some("a.zip".into()),
        size_bytes: Some(10),
        direct_url: "https://download1.mediafire.com/x/y/a.zip".into(),
    };
    assert_eq!(p.filename.as_deref(), Some("a.zip"));
    assert_eq!(p.size_bytes, Some(10));
    assert!(p.direct_url.starts_with("https://"));
}
