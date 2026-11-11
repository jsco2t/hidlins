//! The emulator authority deliberately builds without the desktop UI.
#![cfg(not(feature = "desktop"))]

mod common;

#[test]
fn cli_only_build_keeps_help_and_commands() {
    for args in [
        vec!["--help"],
        vec!["--version"],
        vec!["--cli"],
        vec!["gen", "password", "--length", "12"],
    ] {
        let (code, stdout, stderr) = common::run_args(&args);
        assert_eq!(code, 0, "{args:?}: {stderr}");
        assert!(!stdout.is_empty());
        assert!(!stdout.contains('\u{1b}'));
    }
}

#[test]
fn cli_only_build_explains_unavailable_interactive_and_keymap_operations() {
    for args in [
        vec![],
        vec!["--vault", "work"],
        vec!["--dump-keys"],
        vec!["keys"],
        vec!["keys", "--format=json"],
    ] {
        let (code, stdout, stderr) = common::run_args(&args);
        assert_eq!(code, 1, "{args:?}: {stderr}{stdout}");
        assert!(format!("{stdout}{stderr}").contains("desktop build"));
        assert!(!stdout.contains('\u{1b}'));
    }
}
