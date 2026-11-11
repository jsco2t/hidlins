//! Phase-1 integration tests: `--help`, `--version`, and the
//! ASCII-art banner are all stable and discoverable.

mod common;

use common::run_args;
use hidlins_cli::cli::BANNER;

const EXPECTED_HIDLINS_BANNER: &str = r"
 _   _ ___ ____  _     ___ _   _ ____
| | | |_ _|  _ \| |   |_ _| \ | / ___|
| |_| || || | | | |    | ||  \| \___ \
|  _  || || |_| | |___ | || |\  |___) |
|_| |_|___|____/|_____|___|_| \_|____/

";

#[test]
fn top_level_help_exits_zero_and_lists_every_subcommand() {
    let (code, stdout, stderr) = run_args(&["--help"]);
    assert_eq!(code, 0, "stderr was: {stderr}");
    for expected in ["vault", "entry", "gen", "sync", "ssh", "completions"] {
        assert!(
            stdout.contains(expected),
            "--help missing subcommand {expected}; stdout:\n{stdout}"
        );
    }
}

#[test]
fn top_level_help_contains_ascii_art_banner() {
    assert_eq!(BANNER, EXPECTED_HIDLINS_BANNER);
    // clap trims the leading newline from `before_help`, but preserves the
    // complete five-row wordmark and its trailing separation from the body.
    let emitted_banner = EXPECTED_HIDLINS_BANNER.trim_start_matches('\n');
    for flag in ["-h", "--help"] {
        let (code, stdout, stderr) = run_args(&[flag]);
        assert_eq!(code, 0, "stderr was: {stderr}");
        assert!(
            stdout.starts_with(emitted_banner),
            "expected complete HIDLINS banner before {flag} output; got:\n{stdout}"
        );
        assert!(
            !stdout.contains("/_/    \\__,_/"),
            "legacy Falach banner leaked into {flag} output:\n{stdout}"
        );
    }
}

#[test]
fn version_flag_exits_zero() {
    let (code, stdout, _stderr) = run_args(&["--version"]);
    assert_eq!(code, 0);
    // Cargo's `version` substitutes the workspace version.
    assert!(stdout.contains("hidlins"), "version output: {stdout:?}");
}

#[test]
fn each_subcommand_exposes_help() {
    // Every subcommand listed in `Command` must accept `--help` and
    // exit 0. Phase 1 stub bodies never run because `--help` short-
    // circuits inside clap.
    for sub in ["vault", "entry", "gen", "sync", "ssh", "completions"] {
        let (code, stdout, stderr) = run_args(&[sub, "--help"]);
        assert_eq!(
            code, 0,
            "`hidlins {sub} --help` failed; stderr:\n{stderr}\nstdout:\n{stdout}"
        );
    }
}

#[test]
fn after_help_documents_master_password_policy() {
    let (code, stdout, _stderr) = run_args(&["--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("HIDLINS_MASTER_PASSWORD"),
        "--help should document the env-var policy:\n{stdout}"
    );
    // Per FR-061: the env var is "ignored". Pattern is documented in
    // both the after_help epilog and the runtime warning.
    assert!(stdout.contains("ignored"), "missing 'ignored' wording");
}
