mod common;

use hidlins_core::{Keyfile, NoRecoveryConfirmed, Vault, VaultError, VaultReadOnly};

use common::{fast_kdf, master, TestEnv};

#[test]
fn open_with_wrong_password_returns_auth_failed() {
    let env = TestEnv::new();
    let path = env.tempdir().join("personal.kdbx");

    drop(
        Vault::create(
            &path,
            &master("correct"),
            None,
            fast_kdf(),
            NoRecoveryConfirmed::yes(),
        )
        .expect("create"),
    );

    assert!(matches!(
        Vault::open(&path, &master("wrong"), None),
        Err(VaultError::AuthenticationFailed)
    ));
    assert!(matches!(
        VaultReadOnly::open(&path, &master("wrong"), None),
        Err(VaultError::AuthenticationFailed)
    ));
}

#[test]
fn open_with_wrong_keyfile_returns_auth_failed() {
    let env = TestEnv::new();
    let path = env.tempdir().join("keyed.kdbx");
    let correct_keyfile = Keyfile::Bytes(b"correct keyfile".to_vec());
    let wrong_keyfile = Keyfile::Bytes(b"wrong keyfile".to_vec());

    drop(
        Vault::create(
            &path,
            &master("correct"),
            Some(&correct_keyfile),
            fast_kdf(),
            NoRecoveryConfirmed::yes(),
        )
        .expect("create"),
    );

    let Err(err) = Vault::open(&path, &master("correct"), Some(&wrong_keyfile)) else {
        panic!("wrong keyfile should fail");
    };
    assert!(matches!(err, VaultError::AuthenticationFailed));

    let display = err.to_string().to_ascii_lowercase();
    let debug = format!("{err:?}").to_ascii_lowercase();
    assert!(!display.contains("keyfile"));
    assert!(!debug.contains("keyfile"));
}

#[test]
fn equivalent_legacy_and_xml_v2_hex_keyfiles_unlock_the_same_vault() {
    let env = TestEnv::new();
    let path = env.tempdir().join("hex-keyed.kdbx");
    let legacy = Keyfile::Bytes(
        b"000102030405060708090A0B0C0D0E0F101112131415161718191a1b1c1d1e1f".to_vec(),
    );
    let xml_v2 = Keyfile::Bytes(
        br#"<?xml version="1.0" encoding="utf-8"?>
<KeyFile>
  <Meta><Version>2.0</Version></Meta>
  <Key><Data Hash="00000000">
    00010203 04050607 08090A0B 0C0D0E0F
    10111213 14151617 18191a1b 1c1d1e1f
  </Data></Key>
</KeyFile>"#
            .to_vec(),
    );

    drop(
        Vault::create(
            &path,
            &master("correct"),
            Some(&legacy),
            fast_kdf(),
            NoRecoveryConfirmed::yes(),
        )
        .expect("create with legacy hexadecimal keyfile"),
    );

    Vault::open(&path, &master("correct"), Some(&xml_v2))
        .expect("equivalent XML v2 hexadecimal keyfile should unlock the vault");
}
