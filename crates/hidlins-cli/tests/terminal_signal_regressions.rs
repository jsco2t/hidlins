//! Real binary cleanup; every case owns its controlling terminal and child.
#![cfg(all(unix, feature = "desktop"))]

use std::path::Path;
use std::process::Command;

fn run_case(action: &str, redirected: bool) {
    run_startup_case(action, redirected, "options");
}

fn run_startup_case(action: &str, redirected: bool, startup: &str) {
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/tui-terminal-tests/check.py");
    let output = Command::new("python3")
        .arg(script)
        .arg(env!("CARGO_BIN_EXE_hidlins"))
        .arg(action)
        .arg(if redirected { "redirected" } else { "tty" })
        .arg(startup)
        .output()
        .expect("Python stdlib controlling-PTY fixture starts");
    assert!(
        output.status.success(),
        "{action}, redirected={redirected}: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn no_arguments_launches_tui_and_quits_cleanly() {
    run_startup_case("quit", false, "default");
}

#[test]
fn shared_registry_override_launches_tui_and_quits_cleanly() {
    run_startup_case("quit", false, "shared");
}

macro_rules! terminal_case {
    ($name:ident, $action:literal, $redirected:literal) => {
        #[test]
        fn $name() {
            run_case($action, $redirected);
        }
    };
}

terminal_case!(term_restores_tty, "TERM", false);
terminal_case!(
    term_restores_controlling_tty_with_redirected_stdin,
    "TERM",
    true
);
terminal_case!(hup_restores_tty, "HUP", false);
terminal_case!(
    hup_restores_controlling_tty_with_redirected_stdin,
    "HUP",
    true
);
terminal_case!(normal_quit_restores_tty, "quit", false);
terminal_case!(
    normal_quit_restores_redirected_controlling_tty,
    "quit",
    true
);
