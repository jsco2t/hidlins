//! A shared registry override must not redirect TUI preferences or themes.
#![cfg(feature = "desktop")]

mod common;

fn command(home: &std::path::Path) -> std::process::Command {
    let mut command = common::hidlins_cmd();
    command
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("xdg"));
    command
}

#[test]
fn registry_override_keeps_xdg_preferences_for_both_keymap_routes() {
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join("xdg/hidlins/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "[keymap.bindings]\ncopy-password = false\n").unwrap();
    let alternate = home.path().join("registry");
    std::fs::create_dir(&alternate).unwrap();
    // A neighboring config is unrelated to the selected registry. It must
    // never take precedence over the user's actual XDG preferences.
    let neighbor = alternate.join("config.toml");
    std::fs::write(&neighbor, "[keymap.bindings]\ncopy-password = \"y\"\n").unwrap();
    let registry = alternate.join("vaults.toml");
    for args in [vec!["--dump-keys=json"], vec!["keys", "--format=json"]] {
        let output = command(home.path())
            .arg("--registry")
            .arg(&registry)
            .args(&args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let copy = value
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["name"] == "copy-password")
            .unwrap();
        assert_eq!(
            copy["keys"],
            serde_json::json!([]),
            "{args:?}: must use XDG keymap"
        );
    }
    assert_eq!(
        std::fs::read_to_string(neighbor).unwrap(),
        "[keymap.bindings]\ncopy-password = \"y\"\n"
    );
}

#[test]
fn registry_override_creates_startup_preferences_in_xdg_directory() {
    let home = tempfile::tempdir().unwrap();
    let alternate = home.path().join("registry");
    std::fs::create_dir(&alternate).unwrap();
    let output = command(home.path())
        .arg("--registry")
        .arg(alternate.join("vaults.toml"))
        .args(["--vault", "not-registered"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("not-registered"));
    assert!(
        home.path().join("xdg/hidlins/config.toml").is_file(),
        "startup should generate preferences at the normal XDG path"
    );
    assert!(
        !alternate.join("config.toml").exists(),
        "a registry override must not create a neighboring preference file"
    );
}
