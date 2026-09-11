//! `hidlins vault {create, register, open, list, set-lock}` dispatcher.
//!
//! ## State directory resolution
//!
//! The global `--registry <path>` flag is the path to the registry
//! file itself — `crate::commands::resolve_paths` honors the exact
//! file name via [`hidlins_core::HidlinsPaths::with_registry_file`].
//! When the flag is omitted we fall back to `HidlinsPaths::from_env`
//! (the `$HOME/.local/state/hidlins/vaults.toml` path).
//!
//! ## `[vault.lock]` schema
//!
//! The `[vault.lock] idle_timeout_seconds` on-disk schema is owned by
//! `hidlins-security` ([`VaultLockConfig`]); this module reads and
//! writes it exclusively through that crate's helpers so the CLI's
//! write path and security-behaviors' read path cannot drift.

use hidlins_core::{
    KdfParams, Keyfile, NoRecoveryConfirmed, RegisteredVault, Vault, VaultError, VaultRegistry,
};
use hidlins_security::VaultLockConfig;

use crate::agent::NoAgentClient;
use crate::cli::{
    Cli, VaultArgs, VaultCreateArgs, VaultOpenArgs, VaultRegisterArgs, VaultSetLockArgs, VaultVerb,
};
use crate::commands::{resolve_paths, write_success};
use crate::exit::CliExit;
use crate::prompt::{master_password, new_master_password_confirmed, PromptOpts};
use crate::views::vault::{
    VaultCreateKdfView, VaultCreateView, VaultListEntry, VaultListView, VaultOpenView,
    VaultRegisterView, VaultSetLockView,
};

/// Phase 2 entry point — dispatches to the verb handler.
///
/// # Errors
///
/// Any [`CliExit`] returned by the per-verb handlers.
pub fn run(cli: &Cli, args: &VaultArgs) -> Result<(), CliExit> {
    match &args.verb {
        Some(VaultVerb::Create(create)) => run_create(cli, create),
        Some(VaultVerb::Register(register)) => run_register(cli, register),
        Some(VaultVerb::Open(open)) => run_open(cli, open),
        Some(VaultVerb::List(_)) => run_list(cli),
        Some(VaultVerb::SetLock(setlock)) => run_set_lock(cli, setlock),
        None => Err(CliExit::UserError(
            "missing subcommand verb (try `hidlins vault --help`)".to_string(),
        )),
    }
}

// ---------------------------------------------------------------------------
// vault register
// ---------------------------------------------------------------------------

fn run_register(cli: &Cli, args: &VaultRegisterArgs) -> Result<(), CliExit> {
    let paths = resolve_paths(cli)?;
    let mut registry = VaultRegistry::load(paths).map_err(CliExit::from)?;
    if registry.get(&args.id).is_some() {
        return Err(CliExit::from(VaultError::AlreadyRegistered {
            name: args.id.clone(),
        }));
    }

    let agent = NoAgentClient;
    let opts = PromptOpts {
        vault: &args.id,
        agent: &agent,
        prompt_label: "Master password: ",
    };
    let master = master_password(&opts)?;
    let keyfile = args.keyfile.clone().map(Keyfile::Path);

    // Authentication deliberately precedes all registry mutation. Opening an
    // existing vault is read-only; drop its file lock before taking the
    // independent registry write lock below.
    let vault = Vault::open(&args.path, &master, keyfile.as_ref()).map_err(CliExit::from)?;
    drop(vault);

    // Persist stable absolute paths so later ID-based commands do not depend on
    // the working directory used for this one registration invocation.
    let vault_path = canonicalize_registration_path(&args.path)?;
    let keyfile_path = args
        .keyfile
        .as_deref()
        .map(canonicalize_registration_path)
        .transpose()?;

    registry
        .register_and_save(RegisteredVault {
            name: args.id.clone(),
            path: vault_path.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            keyfile_path: keyfile_path.clone(),
            extra: toml::Table::new(),
        })
        .map_err(CliExit::from)?;

    let view = VaultRegisterView {
        id: &args.id,
        path: &vault_path,
        keyfile: keyfile_path.as_deref(),
        status: "registered",
    };
    write_success(cli, &view)
}

fn canonicalize_registration_path(path: &std::path::Path) -> Result<std::path::PathBuf, CliExit> {
    std::fs::canonicalize(path).map_err(|source| {
        CliExit::from(VaultError::Io {
            source,
            path: path.to_path_buf(),
        })
    })
}

// ---------------------------------------------------------------------------
// vault create
// ---------------------------------------------------------------------------

fn run_create(cli: &Cli, args: &VaultCreateArgs) -> Result<(), CliExit> {
    if !args.no_recovery_warning {
        return Err(CliExit::UserError(
            "vault create requires --no-recovery-warning: there is no master-password recovery \
             in Hidlins (data lost on forgotten password)"
                .to_string(),
        ));
    }

    // Load the registry FIRST so a duplicate-id error surfaces before
    // we run the expensive Argon2id KDF + write a .kdbx file that the
    // user can't easily clean up. Belt-and-suspenders: the
    // `VaultRegistry::register` call below still re-checks for
    // duplicates, so a race between two concurrent `vault create`
    // invocations can't silently succeed.
    let paths = resolve_paths(cli)?;
    let mut registry = VaultRegistry::load(paths).map_err(CliExit::from)?;
    if registry.get(&args.id).is_some() {
        return Err(CliExit::from(hidlins_core::VaultError::AlreadyRegistered {
            name: args.id.clone(),
        }));
    }

    // Collect the password BEFORE touching disk so a user who Ctrl-Cs
    // out of the prompt doesn't leave a half-registered vault behind.
    let stdin = std::io::stdin();
    let mut stdin = stdin.lock();
    let mut stderr = std::io::stderr().lock();
    let master = new_master_password_confirmed(&mut stdin, &mut stderr)?;
    let keyfile = args.keyfile.clone().map(Keyfile::Path);
    let kdf = KdfParams::default();

    let _vault = Vault::create(
        &args.path,
        &master,
        keyfile.as_ref(),
        kdf,
        NoRecoveryConfirmed::yes(),
    )
    .map_err(CliExit::from)?;
    // Drop the Vault handle here — Phase 2's `vault create` returns
    // immediately after registration; the file is on disk and the
    // registry will be updated below. The handle's exclusive lock is
    // released as it drops.

    registry
        .register(RegisteredVault {
            name: args.id.clone(),
            path: args.path.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            keyfile_path: args.keyfile.clone(),
            extra: toml::Table::new(),
        })
        .map_err(CliExit::from)?;
    registry.save().map_err(CliExit::from)?;

    let view = VaultCreateView {
        id: &args.id,
        path: &args.path,
        keyfile: args.keyfile.as_deref(),
        kdf: VaultCreateKdfView {
            algorithm: "argon2id",
            memory_kib: kdf.memory_kib,
            iterations: kdf.iterations,
            parallelism: kdf.parallelism,
        },
    };
    write_success(cli, &view)
}

// ---------------------------------------------------------------------------
// vault open (probe)
// ---------------------------------------------------------------------------

fn run_open(cli: &Cli, args: &VaultOpenArgs) -> Result<(), CliExit> {
    // The shared boundary authenticates and consumes this process's one
    // configured pre-operation sync attempt before returning locally.
    let _opened = super::open::prepare(cli, &args.id)?;

    let view = VaultOpenView {
        id: &args.id,
        status: "unlocked-ok",
    };
    write_success(cli, &view)
}

// ---------------------------------------------------------------------------
// vault list
// ---------------------------------------------------------------------------

fn run_list(cli: &Cli) -> Result<(), CliExit> {
    let paths = resolve_paths(cli)?;
    let registry = VaultRegistry::load(paths).map_err(CliExit::from)?;

    let entries: Vec<VaultListEntry<'_>> = registry
        .list()
        .map(|r| VaultListEntry {
            id: &r.name,
            path: &r.path,
            keyfile: r.keyfile_path.as_deref(),
            created_at: &r.created_at,
            // Lenient read: `vault list` must not fail on a hand-edited
            // registry; malformed lock data displays as "no override".
            idle_timeout_seconds: VaultLockConfig::idle_timeout_seconds_from_extra(&r.extra),
        })
        .collect();
    let view = VaultListView { vaults: entries };
    write_success(cli, &view)
}

// ---------------------------------------------------------------------------
// vault set-lock
// ---------------------------------------------------------------------------

fn run_set_lock(cli: &Cli, args: &VaultSetLockArgs) -> Result<(), CliExit> {
    // Exactly one of --timeout / --clear is meaningful. clap's
    // `conflicts_with` blocks both being present; we enforce
    // "exactly one" here (neither set is also a user error).
    if args.timeout.is_none() && !args.clear {
        return Err(CliExit::UserError(
            "vault set-lock requires either --timeout <seconds> or --clear".to_string(),
        ));
    }
    if let Some(0) = args.timeout {
        // When security-behaviors lands it owns the canonical
        // validation. Until then, reject 0 at the CLI layer rather
        // than writing an obviously-broken value to the registry.
        return Err(CliExit::UserError(
            "--timeout must be at least 1 second".to_string(),
        ));
    }

    let paths = resolve_paths(cli)?;
    let mut registry = VaultRegistry::load(paths).map_err(CliExit::from)?;

    // The registry's API returns `&RegisteredVault`. We need owning
    // mutation. Take ownership of the vector via `into_records` is not
    // available; instead, re-read by name, mutate a clone, deregister,
    // re-register. That round-trip preserves all `extra` keys.
    let original = registry
        .get(&args.id)
        .ok_or_else(|| {
            CliExit::from(VaultError::NotRegistered {
                name: args.id.clone(),
            })
        })?
        .clone();

    let mut updated = original;
    VaultLockConfig::apply_idle_timeout(&mut updated.extra, args.timeout);

    registry
        .deregister(&args.id, false)
        .map_err(CliExit::from)?;
    registry.register(updated).map_err(CliExit::from)?;
    registry.save().map_err(CliExit::from)?;

    let view = VaultSetLockView {
        id: &args.id,
        idle_timeout_seconds: args.timeout,
    };
    write_success(cli, &view)
}
