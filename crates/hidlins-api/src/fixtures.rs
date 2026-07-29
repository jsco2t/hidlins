//! Test fixture builders for the `test-fixtures` feature.
//!
//! Exposes helpers that integration tests and the `api-test-driver`
//! binary use to create vaults with known content. Reuses
//! `hidlins-core`'s fast-KDF parameters so tests are fast.

use std::path::PathBuf;

use hidlins_core::{
    EntryBuilder, HidlinsPaths, KdfParams, MasterPassword, NoRecoveryConfirmed, Vault,
    VaultRegistry,
};

/// Fast KDF parameters for tests (vs. the ~1s production Argon2id).
pub const FAST_KDF: KdfParams = KdfParams {
    memory_kib: 64,
    iterations: 1,
    parallelism: 1,
};

/// Self-cleaning temporary directory with `HidlinsPaths` configured.
pub struct TestEnv {
    /// The temporary directory (cleaned up on drop).
    pub dir: tempfile::TempDir,
    /// Paths pointing into the temporary directory.
    pub paths: HidlinsPaths,
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl TestEnv {
    /// Create a fresh test environment in a temp directory.
    ///
    /// # Panics
    ///
    /// Panics if the temp directory or state subdirectory cannot be created.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("create test tmpdir");
        let paths = HidlinsPaths::with_state_dir(dir.path().to_path_buf());
        paths.ensure_exists().expect("create state dir");
        Self { dir, paths }
    }

    /// Borrow the paths.
    pub fn paths(&self) -> &HidlinsPaths {
        &self.paths
    }
}

/// Convenience: a master password for test vaults.
pub fn master() -> MasterPassword {
    MasterPassword::new("test-password".to_string())
}

/// Create a vault file and register it, using fast-KDF.
///
/// # Panics
///
/// Panics on any I/O or registry error.
pub fn create_test_vault(paths: &HidlinsPaths, name: &str, password: &str) -> PathBuf {
    let vault_path = paths.state_dir().join(format!("{name}.kdbx"));
    let master = MasterPassword::new(password.to_string());
    let mut vault = Vault::create(
        &vault_path,
        &master,
        None,
        FAST_KDF,
        NoRecoveryConfirmed::yes(),
    )
    .expect("create test vault");

    let mut registry = VaultRegistry::load(paths.clone()).expect("load registry");
    registry
        .register(hidlins_core::RegisteredVault {
            name: name.to_string(),
            path: vault_path.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            keyfile_path: None,
            extra: toml::Table::default(),
        })
        .expect("register vault");
    registry.save().expect("save registry");

    vault.save().expect("save vault");
    vault_path
}

/// Add the standard 4-entry corpus to an existing vault.
///
/// # Panics
///
/// Panics on any I/O or entry-add error.
pub fn add_corpus_entries(paths: &HidlinsPaths, name: &str, password: &str) {
    let vault_path = paths.state_dir().join(format!("{name}.kdbx"));
    let master = MasterPassword::new(password.to_string());
    let mut vault = Vault::open(&vault_path, &master, None).expect("open vault");

    let root = vault.root_group_uuid();
    let entries = [
        EntryBuilder::credential("GitHub")
            .username("octocat")
            .password("gh-secret-123")
            .url("https://github.com"),
        EntryBuilder::credential("Bank")
            .username("banker")
            .password("bank-secret-456")
            .url("https://bank.test"),
        EntryBuilder::credential("Email")
            .username("user@test.com")
            .password("email-secret-789"),
        EntryBuilder::secure_note("Recovery Codes"),
    ];

    for builder in entries {
        vault.add_entry(root, builder.build()).expect("add entry");
    }

    vault.save().expect("save vault with corpus");
}
