//! A failed completion sink returns a CLI error rather than a generator panic.
#![cfg(unix)]

mod common;

use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::Stdio;

use common::{hidlins_cmd, run_args};

fn check_broken_output(shell: &str, json: bool) {
    let (reader, writer) = UnixStream::pair().unwrap();
    drop(reader);
    let writer: OwnedFd = writer.into();
    let mut command = hidlins_cmd();
    if json {
        command.args(["--format", "json"]);
    }
    let output = command
        .args(["completions", shell])
        .stdin(Stdio::null())
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        output.status.code(),
        Some(10),
        "completion sink errors need the documented internal exit: {stderr}"
    );
    assert!(
        !stderr.contains("panicked"),
        "sink errors must not panic: {stderr}"
    );
    if !json {
        assert!(stderr.contains("completions") && stderr.contains("stdout"));
    }
}

#[test]
fn broken_bash_completion_output_returns_an_error() {
    check_broken_output("bash", false);
}

#[test]
fn broken_zsh_completion_output_returns_an_error() {
    check_broken_output("zsh", false);
}

#[test]
fn broken_fish_completion_output_returns_an_error() {
    check_broken_output("fish", false);
}

#[test]
fn broken_json_completion_output_returns_an_error_without_panicking() {
    check_broken_output("bash", true);
}

#[test]
fn valid_completion_outputs_remain_nonempty_scripts() {
    for shell in ["bash", "zsh", "fish"] {
        let (code, stdout, stderr) = run_args(&["completions", shell]);
        assert_eq!(code, 0, "{shell}: {stderr}");
        assert!(stdout.contains("hidlins"));
        assert!(!stderr.contains("panicked"));
    }
}
