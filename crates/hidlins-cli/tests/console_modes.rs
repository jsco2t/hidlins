//! Process-level routing contracts for the single console application.
#![cfg(feature = "desktop")]

mod common;

use std::process::{Output, Stdio};

fn run(home: &std::path::Path, args: &[&str]) -> Output {
    common::hidlins_cmd()
        .args(args)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env_remove("HIDLINS_TUI_THEME")
        .env_remove("NO_COLOR")
        .stdin(Stdio::null())
        .output()
        .expect("run isolated console")
}

fn success(output: &Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.stdout.contains(&0x1b),
        "no terminal setup for introspection"
    );
    String::from_utf8(output.stdout.clone()).expect("UTF-8 output")
}

#[test]
fn no_arguments_reach_tui_registry_loading_before_terminal_setup() {
    let home = tempfile::tempdir().unwrap();
    let state = home.path().join(".local/state/hidlins");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("vaults.toml"), "not valid TOML [").unwrap();
    let output = run(home.path(), &[]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.contains(&0x1b));
}

#[test]
fn registry_alone_is_shared_and_used_by_tui() {
    let home = tempfile::tempdir().unwrap();
    let registry = home.path().join("custom.toml");
    std::fs::write(&registry, "not valid TOML [").unwrap();
    let output = run(home.path(), &["--registry", registry.to_str().unwrap()]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.contains(&0x1b));
}

#[test]
fn explicit_cli_without_command_prints_help_successfully() {
    let home = tempfile::tempdir().unwrap();
    for args in [
        vec!["--cli"],
        vec!["--format", "human"],
        vec!["--format=json"],
        vec!["--cli", "--registry", "missing.toml"],
    ] {
        let text = success(&run(home.path(), &args));
        assert!(text.contains("Usage:"), "{args:?}: {text}");
        assert!(text.contains("vault"));
    }
}

#[test]
fn forcing_cli_preserves_one_shot_command_and_format_in_both_positions() {
    let home = tempfile::tempdir().unwrap();
    for args in [
        vec![
            "--cli", "gen", "password", "--length", "19", "--show", "--format", "json",
        ],
        vec![
            "gen", "password", "--cli", "--length", "19", "--show", "--format", "json",
        ],
    ] {
        let text = success(&run(home.path(), &args));
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["password"].as_str().unwrap().len(), 19);
    }
}

#[test]
fn tui_keymap_dump_honors_custom_config_and_all_tui_options() {
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join("gen");
    std::fs::write(
        &config,
        "[keymap]\npreset = \"plain\"\n[keymap.bindings]\ncopy-password = \"ctrl+y\"\n",
    )
    .unwrap();
    let text = success(&run(
        home.path(),
        &[
            "--config",
            config.to_str().unwrap(),
            "--theme",
            "gen",
            "--vault",
            "sync",
            "--no-mouse",
            "--read-only",
            "--dump-keys=json",
        ],
    ));
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let copy = value
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "copy-password")
        .unwrap();
    assert!(
        copy["keys"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str().unwrap().to_lowercase() == "ctrl+y"),
        "{copy}"
    );
}

#[test]
fn option_values_do_not_select_cli_mode() {
    let home = tempfile::tempdir().unwrap();
    for args in [
        vec!["--vault=--cli"],
        vec!["--vault", "gen"],
        vec!["--vault", "keys"],
    ] {
        let output = run(home.path(), &args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{args:?}: {stderr}");
        assert!(stderr.contains("vault"), "{stderr}");
        assert!(
            !stderr.contains("Usage:"),
            "should reach TUI unknown-vault handling: {stderr}"
        );
    }
}

#[test]
fn keymap_cli_is_in_process_and_uses_global_format_in_both_positions() {
    let home = tempfile::tempdir().unwrap();
    for args in [
        vec!["--format", "json", "keys"],
        vec!["keys", "--format=json"],
        vec!["--cli", "keys", "--format", "json"],
    ] {
        let output = common::hidlins_cmd()
            .args(&args)
            .env("HOME", home.path())
            .env("PATH", "")
            .env("HIDLINS_TUI_BIN", home.path().join("does-not-exist"))
            .output()
            .unwrap();
        let text = success(&output);
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(value
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["name"] == "copy-password"));
    }
    let text = success(&run(home.path(), &["keys"]));
    assert!(text.contains("copy-password"));
    assert!(!text.starts_with('['));
}

#[test]
fn keys_config_is_accepted_before_and_after_command() {
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join("custom.toml");
    std::fs::write(&config, "[keymap.bindings]\ncopy-password = false\n").unwrap();
    for args in [
        vec![
            "--config",
            config.to_str().unwrap(),
            "keys",
            "--format=json",
        ],
        vec![
            "keys",
            "--config",
            config.to_str().unwrap(),
            "--format=json",
        ],
    ] {
        let text = success(&run(home.path(), &args));
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let copy = value
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["name"] == "copy-password")
            .unwrap();
        assert_eq!(copy["keys"], serde_json::json!([]));
    }
}

#[test]
fn unused_tui_settings_are_rejected_in_cli_mode() {
    let home = tempfile::tempdir().unwrap();
    for args in [
        vec!["--no-mouse", "vault", "list"],
        vec!["--read-only", "--cli"],
        vec!["--theme", "slate", "--format", "json"],
        vec!["--vault", "work", "entry", "list", "--vault", "other"],
        vec!["--config", "prefs.toml", "vault", "list"],
        vec!["--config", "prefs.toml", "--cli"],
        vec!["--dump-keys", "keys"],
    ] {
        let output = run(home.path(), &args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.stderr.contains(&0x1b));
    }
}

#[test]
fn help_and_version_are_terminal_free_and_describe_unified_interface() {
    let home = tempfile::tempdir().unwrap();
    let help = success(&run(home.path(), &["--help"]));
    for flag in [
        "--cli",
        "--vault",
        "--registry",
        "--config",
        "--theme",
        "--read-only",
        "--no-mouse",
        "--dump-keys",
    ] {
        assert!(help.contains(flag), "help missing {flag}");
    }
    assert!(help.contains("TUI"));
    let version = success(&run(home.path(), &["--version"]));
    assert!(version.starts_with("hidlins "));
}

#[test]
fn malformed_options_fail_before_runtime() {
    let home = tempfile::tempdir().unwrap();
    for args in [
        vec!["--unknown"],
        vec!["--vault"],
        vec!["--theme"],
        vec!["--registry"],
        vec!["--cli=true"],
        vec!["--no-mouse=true"],
        vec!["--dump-keys=xml"],
        vec!["--format=xml"],
        vec!["--", "--cli"],
    ] {
        let output = run(home.path(), &args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(!output.stderr.contains(&0x1b));
    }
}

#[test]
fn manifest_has_only_one_console_executable() {
    // Static delivery contract independent of stale files in target/.
    let manifest = include_str!("../../hidlins-tui/Cargo.toml");
    assert!(
        manifest.contains("autobins = false"),
        "TUI library must not auto-discover a second console binary"
    );
}

#[test]
fn all_generated_completion_shells_include_new_options() {
    let home = tempfile::tempdir().unwrap();
    for shell in ["bash", "zsh", "fish"] {
        let text = success(&run(home.path(), &["completions", shell]));
        for flag in [
            "cli",
            "dump-keys",
            "no-mouse",
            "read-only",
            "theme",
            "config",
        ] {
            assert!(text.contains(flag), "{shell}: missing {flag}");
        }
    }
}
