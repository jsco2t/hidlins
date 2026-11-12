//! EOF is missing input, distinct from an explicitly supplied empty password.
mod common;

use std::io::{Cursor, ErrorKind};

use common::{run_with_stdin, seed_vault, VaultsToml};
use hidlins_cli::prompt::{new_master_password_confirmed, read_password_no_echo};
use hidlins_core::{EntryBuilder, MasterPassword, Vault};

#[test]
fn empty_stream_is_not_a_password() {
    let result = read_password_no_echo("Password: ", &mut Cursor::new(b""), &mut Vec::new());
    assert_eq!(
        result.expect_err("EOF must reject missing input").kind(),
        ErrorKind::UnexpectedEof
    );
}

#[test]
fn confirmation_rejects_exhausted_or_truncated_input() {
    for input in [b"".as_slice(), b"first\n", b"first\nother\n"] {
        let result = new_master_password_confirmed(&mut Cursor::new(input), &mut Vec::new());
        assert!(
            result.is_err(),
            "exhausted confirmation must not become an empty password"
        );
    }
}

#[test]
fn explicit_empty_lines_and_unterminated_nonempty_passwords_remain_valid() {
    let empty =
        read_password_no_echo("Password: ", &mut Cursor::new(b"\n"), &mut Vec::new()).unwrap();
    assert!(empty.is_empty());
    let password = read_password_no_echo(
        "Password: ",
        &mut Cursor::new(b"synthetic"),
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(password, "synthetic");
    let confirmed =
        new_master_password_confirmed(&mut Cursor::new(b"\n\n"), &mut Vec::new()).unwrap();
    assert!(confirmed.as_bytes().is_empty());
    let confirmed =
        new_master_password_confirmed(&mut Cursor::new(b"same\nsame"), &mut Vec::new()).unwrap();
    assert_eq!(confirmed.as_bytes(), b"same");
}

#[test]
fn create_with_missing_confirmation_never_creates_or_registers_a_vault() {
    for input in ["", "first\n", "first\nother\n"] {
        let registry = VaultsToml::new();
        let path = registry.tempdir.path().join("new.kdbx");
        let (code, _, stderr) = run_with_stdin(
            &registry,
            &[
                "vault",
                "create",
                "--id",
                "new",
                "--path",
                path.to_str().unwrap(),
                "--no-recovery-warning",
            ],
            input,
        );
        assert_eq!(code, 1, "missing password input must fail: {stderr}");
        assert!(!path.exists(), "missing input must not create ciphertext");
        assert!(
            !registry.vaults_toml.exists(),
            "missing input must not create registration"
        );
    }
}

#[test]
fn entry_edit_eof_preserves_the_existing_ciphertext_and_password() {
    let registry = VaultsToml::new();
    let path = seed_vault(&registry, "v", "master");
    let master = MasterPassword::new("master".to_owned());
    let mut vault = Vault::open(&path, &master, None).unwrap();
    let id = vault
        .add_entry(
            vault.root_group_uuid(),
            EntryBuilder::credential("preserve")
                .password("synthetic entry value")
                .build(),
        )
        .unwrap();
    vault.save().unwrap();
    drop(vault);
    let before = std::fs::read(&path).unwrap();
    let (code, _, stderr) = run_with_stdin(
        &registry,
        &[
            "entry",
            "edit",
            "--vault",
            "v",
            "--uuid",
            &id.to_string(),
            "--password-stdin",
        ],
        "master\n",
    );
    assert_eq!(code, 1, "missing replacement must fail: {stderr}");
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "EOF must not mutate the vault"
    );
    let vault = Vault::open(&path, &master, None).unwrap();
    assert_eq!(
        vault.get_entry(id).unwrap().password(),
        "synthetic entry value"
    );
}
