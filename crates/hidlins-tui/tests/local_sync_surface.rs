//! Static contract for the reference TUI's local-sync-only surface.

#[test]
fn active_tui_sources_have_no_optional_automatic_sync_toggles() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for relative in [
        "app.rs",
        "user_config.rs",
        "screens/settings.rs",
        "overlay/mod.rs",
        "sync_runtime.rs",
    ] {
        let source = std::fs::read_to_string(root.join(relative)).expect("read TUI source");
        for forbidden in ["sync_on_unlock", "sync_on_lock_quit", "OnLock", "OnQuit"] {
            assert!(
                !source.contains(forbidden),
                "{relative} retains forbidden legacy sync surface `{forbidden}`"
            );
        }
    }
}

#[test]
fn local_sync_controls_are_keyboard_registered_and_accessibly_named() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let surface = ["app.rs", "screens/settings.rs", "overlay/local_sync.rs"]
        .into_iter()
        .map(|path| std::fs::read_to_string(root.join(path)).expect("read TUI surface"))
        .collect::<String>();
    for required in [
        "Local sync",
        "Pair this vault",
        "Import paired vault",
        "Allow pairing for 3 minutes",
        "Start sync server",
        "Stop sync server",
        "Revoke peer",
    ] {
        assert!(
            surface.contains(required),
            "missing TUI action/status `{required}`"
        );
    }
}
