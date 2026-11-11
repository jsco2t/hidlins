//! Spawned-process local-sync command-tree and stable error-path tests.

mod common;

use common::{run_with_stdin, seed_vault, VaultsToml};

#[test]
fn local_sync_command_tree_is_complete() {
    let reg = VaultsToml::new();
    for args in [
        &["sync", "now", "--help"][..],
        &["sync", "serve", "--help"][..],
        &["sync", "pair", "--help"][..],
        &["sync", "import", "--help"][..],
        &["sync", "status", "--help"][..],
        &["sync", "peers", "--help"][..],
    ] {
        let (code, _, stderr) = run_with_stdin(&reg, args, "");
        assert_eq!(code, 0, "missing local-sync command {args:?}: {stderr}");
    }
}

/// Syncing a vault with no `[vault.sync]` config is a clean user error
/// (exit 1) — the most common mistake. `sync_now` reports `NotConfigured`
/// before building a transport.
#[test]
fn cli_sync_unconfigured_exits_1() {
    let reg = VaultsToml::new();
    seed_vault(&reg, "personal", "master-pw");
    // Unlock succeeds (correct password), then sync_now → NotConfigured → 1.
    let (code, _out, stderr) = run_with_stdin(
        &reg,
        &[
            "sync",
            "now",
            "--vault",
            "personal",
            "--address",
            "127.0.0.1",
            "--port",
            "9",
        ],
        "master-pw\n",
    );
    assert_eq!(
        code, 1,
        "unconfigured sync should be a user error; stderr: {stderr}"
    );
}

/// A wrong master password short-circuits at unlock (exit 2) before any
/// transport call.
#[test]
fn cli_sync_wrong_master_password_exits_2() {
    let reg = VaultsToml::new();
    seed_vault(&reg, "personal", "correct-master");
    let (code, _out, _err) = run_with_stdin(
        &reg,
        &[
            "sync",
            "now",
            "--vault",
            "personal",
            "--address",
            "127.0.0.1",
            "--port",
            "9",
        ],
        "WRONG-master\n",
    );
    assert_eq!(code, 2, "wrong master password should exit 2");
}

/// No `--vault` with multiple registered vaults is an ambiguity user error
/// (exit 1), surfaced before any prompt.
#[test]
fn cli_sync_ambiguous_vault_exits_1() {
    let reg = VaultsToml::new();
    seed_vault(&reg, "a", "pw");
    seed_vault(&reg, "b", "pw");
    let (code, _out, stderr) = run_with_stdin(&reg, &["sync", "status"], "");
    assert_eq!(
        code, 1,
        "ambiguous vault (no --vault, >1 registered) should exit 1; stderr: {stderr}"
    );
}

/// Syncing an unregistered vault is a user error (exit 1).
#[test]
fn cli_sync_unknown_vault_exits_1() {
    let reg = VaultsToml::new();
    seed_vault(&reg, "personal", "pw");
    let (code, _out, _err) = run_with_stdin(&reg, &["sync", "status", "--vault", "ghost"], "");
    assert_eq!(code, 1, "unknown vault should exit 1");
}
