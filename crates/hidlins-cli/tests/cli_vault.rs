//! End-to-end vault lifecycle integration tests.
//!
//! Each test exercises the full `create → list → open → set-lock`
//! sequence against a tempdir-isolated registry. The tests live here
//! rather than in `cli_exit_codes.rs` because they verify
//! happy-path behaviour (state on disk, sequencing) rather than
//! individual exit codes.

mod common;

use common::{run_with_stdin, VaultsToml};

/// An existing KDBX can be authenticated, registered, and immediately used by
/// the CLI without creating a replacement vault or modifying its bytes.
#[test]
fn vault_register_existing_vault_enables_id_based_commands() {
    let reg = VaultsToml::new();
    let path = common::create_unregistered_vault(&reg, "kitchen-sink", "fixture-password");
    let before = std::fs::read(&path).expect("read fixture before registration");

    let (code, stdout, stderr) = run_with_stdin(
        &reg,
        &[
            "--format",
            "json",
            "vault",
            "register",
            "--id",
            "demo",
            "--path",
            path.to_str().unwrap(),
        ],
        "fixture-password\n",
    );
    assert_eq!(code, 0, "register stderr:\n{stderr}");
    let result: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(result["id"], "demo");
    assert_eq!(result["status"], "registered");
    assert!(
        result.get("keyfile").is_none(),
        "an omitted keyfile must not serialize as null"
    );
    assert_eq!(
        result["path"],
        std::fs::canonicalize(&path)
            .expect("canonical fixture path")
            .display()
            .to_string()
    );
    assert_eq!(
        std::fs::read(&path).expect("read fixture after registration"),
        before,
        "registration must not modify the KDBX"
    );

    let (open_code, _, open_stderr) = run_with_stdin(
        &reg,
        &["vault", "open", "--id", "demo"],
        "fixture-password\n",
    );
    assert_eq!(open_code, 0, "open stderr:\n{open_stderr}");

    let (entry_code, _, entry_stderr) = run_with_stdin(
        &reg,
        &["entry", "list", "--vault", "demo"],
        "fixture-password\n",
    );
    assert_eq!(entry_code, 0, "entry list stderr:\n{entry_stderr}");

    let (list_code, list_stdout, list_stderr) =
        run_with_stdin(&reg, &["--format", "json", "vault", "list"], "");
    assert_eq!(list_code, 0, "list stderr:\n{list_stderr}");
    let list: serde_json::Value = serde_json::from_str(list_stdout.trim()).unwrap();
    assert_eq!(list["vaults"][0]["path"], result["path"]);
}

#[test]
fn vault_register_keyfile_vault_persists_canonical_paths_and_reopens() {
    let reg = VaultsToml::new();
    let (path, keyfile) =
        common::create_unregistered_vault_with_keyfile(&reg, "protected", "correct-password");

    let (code, stdout, stderr) = run_with_stdin(
        &reg,
        &[
            "--format",
            "json",
            "vault",
            "register",
            "--id",
            "protected",
            "--path",
            path.to_str().unwrap(),
            "--keyfile",
            keyfile.to_str().unwrap(),
        ],
        "correct-password\n",
    );
    assert_eq!(code, 0, "register stderr:\n{stderr}");
    let result: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(
        result["path"],
        std::fs::canonicalize(&path)
            .expect("canonical vault")
            .display()
            .to_string()
    );
    assert_eq!(
        result["keyfile"],
        std::fs::canonicalize(&keyfile)
            .expect("canonical keyfile")
            .display()
            .to_string()
    );

    let (open_code, _, open_stderr) = run_with_stdin(
        &reg,
        &["vault", "open", "--id", "protected"],
        "correct-password\n",
    );
    assert_eq!(open_code, 0, "open stderr:\n{open_stderr}");
}

#[test]
fn vault_register_wrong_password_leaves_vault_and_registry_unchanged() {
    let reg = VaultsToml::new();
    let path = common::create_unregistered_vault(&reg, "wrong-password", "correct-password");
    let before = std::fs::read(&path).expect("read vault before failed registration");

    let (code, stdout, stderr) = run_with_stdin(
        &reg,
        &[
            "vault",
            "register",
            "--id",
            "wrong-password",
            "--path",
            path.to_str().unwrap(),
        ],
        "incorrect-password\n",
    );
    assert_eq!(code, 2, "stderr:\n{stderr}");
    assert!(
        stdout.is_empty(),
        "failed registration wrote stdout: {stdout}"
    );
    assert!(
        !stderr.contains("incorrect-password") && !stderr.contains("correct-password"),
        "registration error leaked password material: {stderr}"
    );
    assert!(
        !reg.vaults_toml.exists(),
        "failed authentication must not create a registry"
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn vault_register_missing_invalid_and_bad_keyfile_never_write_registry() {
    let cases = ["missing", "invalid", "bad-keyfile"];
    for case in cases {
        let reg = VaultsToml::new();
        let (path, keyfile) = match case {
            "missing" => (reg.tempdir.path().join("missing.kdbx"), None),
            "invalid" => {
                let path = reg.tempdir.path().join("invalid.kdbx");
                std::fs::write(&path, b"synthetic invalid KDBX marker").unwrap();
                (path, None)
            }
            "bad-keyfile" => {
                let (path, _) = common::create_unregistered_vault_with_keyfile(
                    &reg,
                    "bad-keyfile",
                    "correct-password",
                );
                let wrong = reg.tempdir.path().join("wrong.key");
                std::fs::write(&wrong, b"wrong synthetic keyfile").unwrap();
                (path, Some(wrong))
            }
            _ => unreachable!(),
        };
        let before = path.exists().then(|| std::fs::read(&path).unwrap());
        let mut args = vec![
            "vault",
            "register",
            "--id",
            case,
            "--path",
            path.to_str().unwrap(),
        ];
        if let Some(keyfile) = keyfile.as_ref() {
            args.extend(["--keyfile", keyfile.to_str().unwrap()]);
        }
        let (code, stdout, stderr) = run_with_stdin(&reg, &args, "correct-password\n");
        assert_ne!(code, 0, "{case} unexpectedly registered");
        assert!(stdout.is_empty(), "{case} wrote stdout: {stdout}");
        assert!(
            !stderr.contains("correct-password"),
            "{case} leaked password: {stderr}"
        );
        assert!(
            !reg.vaults_toml.exists(),
            "{case} must not create a registry"
        );
        if let Some(before) = before {
            assert_eq!(
                std::fs::read(&path).unwrap(),
                before,
                "{case} modified KDBX"
            );
        }
    }
}

#[test]
fn vault_register_duplicate_id_fails_before_password_prompt_or_registry_change() {
    let reg = VaultsToml::new();
    let first = common::create_unregistered_vault(&reg, "first", "first-password");
    let second = common::create_unregistered_vault(&reg, "second", "second-password");
    let (first_code, _, first_stderr) = run_with_stdin(
        &reg,
        &[
            "vault",
            "register",
            "--id",
            "duplicate",
            "--path",
            first.to_str().unwrap(),
        ],
        "first-password\n",
    );
    assert_eq!(first_code, 0, "first register stderr:\n{first_stderr}");
    let registry_before = std::fs::read(&reg.vaults_toml).unwrap();
    let second_before = std::fs::read(&second).unwrap();

    let (code, stdout, stderr) = run_with_stdin(
        &reg,
        &[
            "vault",
            "register",
            "--id",
            "duplicate",
            "--path",
            second.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.contains("already registered"), "stderr:\n{stderr}");
    assert!(
        !stderr.contains("Master password"),
        "duplicate ID prompted before rejection: {stderr}"
    );
    assert_eq!(std::fs::read(&reg.vaults_toml).unwrap(), registry_before);
    assert_eq!(std::fs::read(&second).unwrap(), second_before);
}

/// Full happy path: create → list (one entry) → open (probe) →
/// set-lock 300 → list (timeout=300) → set-lock --clear → list
/// (no timeout).
#[test]
fn vault_lifecycle_create_open_setlock_clear() {
    let reg = VaultsToml::new();
    let path = reg.tempdir.path().join("life.kdbx");
    let path_arg = path.to_str().unwrap();

    // 1. create
    let (code, _stdout, stderr) = run_with_stdin(
        &reg,
        &[
            "vault",
            "create",
            "--id",
            "life",
            "--path",
            path_arg,
            "--no-recovery-warning",
        ],
        "pw\npw\n",
    );
    assert_eq!(code, 0, "create stderr:\n{stderr}");
    assert!(path.exists(), "KDBX file should be on disk at {path_arg}");

    // 2. list shows the new vault
    let (code, stdout, _) = run_with_stdin(&reg, &["--format", "json", "vault", "list"], "");
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(parsed["vaults"][0]["id"], "life");

    // 3. open with the correct password — exit 0
    let (code, _stdout, stderr) = run_with_stdin(&reg, &["vault", "open", "--id", "life"], "pw\n");
    assert_eq!(code, 0, "open stderr:\n{stderr}");

    // 4. set-lock 300
    let (code, _stdout, _) = run_with_stdin(
        &reg,
        &["vault", "set-lock", "--id", "life", "--timeout", "300"],
        "",
    );
    assert_eq!(code, 0);

    // 5. list shows idle_timeout_seconds: 300
    let (code, stdout, _) = run_with_stdin(&reg, &["--format", "json", "vault", "list"], "");
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(
        parsed["vaults"][0]["idle_timeout_seconds"], 300,
        "timeout should round-trip through registry; got {parsed}"
    );

    // 6. set-lock --clear
    let (code, _stdout, _) =
        run_with_stdin(&reg, &["vault", "set-lock", "--id", "life", "--clear"], "");
    assert_eq!(code, 0);

    // 7. list no longer surfaces the override
    let (code, stdout, _) = run_with_stdin(&reg, &["--format", "json", "vault", "list"], "");
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(
        parsed["vaults"][0].get("idle_timeout_seconds").is_none(),
        "cleared override should remove the key; got {parsed}"
    );
}

/// The global `--registry <path>` flag must honor the exact file the
/// user named — not silently substitute `<parent>/vaults.toml`.
/// Regression test for the path-resolution bug where only
/// `path.parent()` was kept.
#[test]
fn registry_flag_honors_custom_file_name() {
    let reg = VaultsToml::new();
    let custom_registry = reg.tempdir.path().join("registry-2026.toml");
    let custom_arg = custom_registry.display().to_string();
    let vault_path = reg.tempdir.path().join("custom.kdbx");

    let (code, _stdout, stderr) = {
        let mut cmd = common::hidlins_cmd();
        cmd.args([
            "--registry",
            &custom_arg,
            "vault",
            "create",
            "--id",
            "custom",
            "--path",
            vault_path.to_str().unwrap(),
            "--no-recovery-warning",
        ]);
        common::run_cmd_with_stdin(&mut cmd, "pw\npw\n")
    };
    assert_eq!(code, 0, "create stderr:\n{stderr}");
    assert!(
        custom_registry.exists(),
        "registration must land in the file the user named"
    );
    assert!(
        !reg.tempdir.path().join("vaults.toml").exists(),
        "no vaults.toml must be silently created next to the named file"
    );

    // And the same flag reads it back.
    let (code, stdout, _) = {
        let mut cmd = common::hidlins_cmd();
        cmd.args([
            "--registry",
            &custom_arg,
            "--format",
            "json",
            "vault",
            "list",
        ]);
        common::run_cmd_with_stdin(&mut cmd, "")
    };
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(parsed["vaults"][0]["id"], "custom");
}

/// Password mismatch on first try, then matching pair on second try.
/// stdin = first-mismatch + retry-match.
#[test]
fn vault_create_password_mismatch_then_match_recovers() {
    let reg = VaultsToml::new();
    let path = reg.tempdir.path().join("retry.kdbx");
    let (code, _stdout, stderr) = run_with_stdin(
        &reg,
        &[
            "vault",
            "create",
            "--id",
            "retry",
            "--path",
            path.to_str().unwrap(),
            "--no-recovery-warning",
        ],
        // attempt 1: a / b (mismatch). attempt 2: c / c (match).
        "a\nb\nc\nc\n",
    );
    assert_eq!(code, 0, "should recover; stderr:\n{stderr}");
    assert!(
        stderr.contains("did not match"),
        "stderr should mention the mismatch:\n{stderr}"
    );
    assert!(path.exists());
}

/// Three full mismatches → exit 1, no vault file created, no registry
/// entry.
#[test]
fn vault_create_three_mismatch_strikes_exits_user_error() {
    let reg = VaultsToml::new();
    let path = reg.tempdir.path().join("strikes.kdbx");
    let (code, _stdout, stderr) = run_with_stdin(
        &reg,
        &[
            "vault",
            "create",
            "--id",
            "strikes",
            "--path",
            path.to_str().unwrap(),
            "--no-recovery-warning",
        ],
        // Three attempts, all mismatched.
        "a\nb\nc\nd\ne\nf\n",
    );
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert!(
        stderr.contains("did not match"),
        "stderr should mention the mismatch failure:\n{stderr}"
    );
    assert!(
        !path.exists(),
        "no KDBX file should land on disk when password collection fails"
    );

    // Registry should also be empty (we collect the password BEFORE
    // touching disk, so a failed collection leaves no trace).
    let (list_code, stdout, _) = run_with_stdin(&reg, &["--format", "json", "vault", "list"], "");
    assert_eq!(list_code, 0);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(parsed["vaults"], serde_json::json!([]));
}

/// Re-registering an existing id fails — even when the path differs.
#[test]
fn vault_create_duplicate_id_rejected() {
    let reg = VaultsToml::new();
    let path1 = reg.tempdir.path().join("a.kdbx");
    let path2 = reg.tempdir.path().join("b.kdbx");

    // First registration succeeds.
    let (code1, _, _) = run_with_stdin(
        &reg,
        &[
            "vault",
            "create",
            "--id",
            "dup",
            "--path",
            path1.to_str().unwrap(),
            "--no-recovery-warning",
        ],
        "x\nx\n",
    );
    assert_eq!(code1, 0);

    // Second registration with the same id fails.
    let (code2, _stdout, stderr) = run_with_stdin(
        &reg,
        &[
            "vault",
            "create",
            "--id",
            "dup",
            "--path",
            path2.to_str().unwrap(),
            "--no-recovery-warning",
        ],
        "y\ny\n",
    );
    assert_eq!(code2, 1, "duplicate id should fail; stderr:\n{stderr}");
    assert!(
        stderr.contains("already registered"),
        "stderr should explain why:\n{stderr}"
    );
}

/// `vault set-lock` against a vault that doesn't exist surfaces
/// `NotRegistered` (exit 1) — not an internal error.
#[test]
fn vault_set_lock_unknown_vault_exits_user_error() {
    let reg = VaultsToml::new();
    let (code, _stdout, stderr) = run_with_stdin(
        &reg,
        &["vault", "set-lock", "--id", "ghost", "--timeout", "60"],
        "",
    );
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert!(
        stderr.contains("ghost"),
        "stderr should name the missing vault:\n{stderr}"
    );
}

/// `vault set-lock` accepting either `--timeout` or `--clear` is
/// mutually exclusive at parse time (clap enforces). Both → clap
/// parse error (exit 2).
#[test]
fn vault_set_lock_timeout_and_clear_mutually_exclusive() {
    let reg = VaultsToml::new();
    // Register so the dispatch would otherwise reach the validation.
    let path = reg.tempdir.path().join("m.kdbx");
    run_with_stdin(
        &reg,
        &[
            "vault",
            "create",
            "--id",
            "m",
            "--path",
            path.to_str().unwrap(),
            "--no-recovery-warning",
        ],
        "x\nx\n",
    );
    let (code, _stdout, stderr) = run_with_stdin(
        &reg,
        &[
            "vault",
            "set-lock",
            "--id",
            "m",
            "--timeout",
            "60",
            "--clear",
        ],
        "",
    );
    assert_eq!(
        code, 2,
        "clap should reject conflicting flags with exit 2; stderr:\n{stderr}"
    );
}

/// `vault set-lock` with neither `--timeout` nor `--clear` is a
/// `UserError` — we want a clear "tell me what to do" message rather
/// than a no-op.
#[test]
fn vault_set_lock_neither_timeout_nor_clear_exits_user_error() {
    let reg = VaultsToml::new();
    let path = reg.tempdir.path().join("n.kdbx");
    run_with_stdin(
        &reg,
        &[
            "vault",
            "create",
            "--id",
            "n",
            "--path",
            path.to_str().unwrap(),
            "--no-recovery-warning",
        ],
        "x\nx\n",
    );
    let (code, _stdout, stderr) = run_with_stdin(&reg, &["vault", "set-lock", "--id", "n"], "");
    assert_eq!(code, 1, "stderr:\n{stderr}");
    assert!(
        stderr.contains("--timeout"),
        "stderr should suggest --timeout:\n{stderr}"
    );
    assert!(
        stderr.contains("--clear"),
        "stderr should suggest --clear:\n{stderr}"
    );
}
