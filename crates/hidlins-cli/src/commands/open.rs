//! Common registered-vault unlock boundary with one pre-operation local sync attempt.

use std::{
    io::Write as _,
    time::{Duration, Instant},
};

use hidlins_core::{Keyfile, MasterPassword, RegisteredVault, Vault, VaultError, VaultRegistry};
use hidlins_sync::{
    client,
    config::local::LocalSyncConfig,
    discovery::{self, CandidateCache, ServiceKind},
    SyncOptions,
};

use crate::{
    agent::NoAgentClient,
    cli::Cli,
    commands::resolve_paths,
    exit::CliExit,
    prompt::{master_password, PromptOpts},
};

/// Credentials and registration resolved once for a vault-opening command.
pub(crate) struct OpenContext {
    pub(crate) record: RegisteredVault,
    pub(crate) master: MasterPassword,
    pub(crate) keyfile: Option<Keyfile>,
}

/// Authenticate, consume the command's single startup attempt, then return local-open inputs.
pub(crate) fn prepare(cli: &Cli, name: &str) -> Result<OpenContext, CliExit> {
    let mut registry = VaultRegistry::load(resolve_paths(cli)?).map_err(CliExit::from)?;
    let record = registry
        .get(name)
        .ok_or_else(|| {
            CliExit::from(VaultError::NotRegistered {
                name: name.to_string(),
            })
        })?
        .clone();
    let master = master_password(&PromptOpts {
        vault: name,
        agent: &NoAgentClient,
        prompt_label: "Master password: ",
    })?;
    let keyfile = record.keyfile_path.clone().map(Keyfile::Path);
    let mut vault = Vault::open(&record.path, &master, keyfile.as_ref()).map_err(CliExit::from)?;
    if registry
        .get(name)
        .and_then(LocalSyncConfig::from_vault_entry)
        .is_some_and(|config| config.is_active_client())
    {
        let discovered = trusted_candidates();
        if let Err(error) = client::sync_vault(
            &mut vault,
            name,
            &mut registry,
            &master,
            keyfile.as_ref(),
            discovered,
            SyncOptions::default(),
        ) {
            let _ = writeln!(
                std::io::stderr().lock(),
                "warning: pre-operation local sync failed; continuing with local vault: {error}"
            );
        }
    }
    drop(vault);
    Ok(OpenContext {
        record,
        master,
        keyfile,
    })
}

fn trusted_candidates() -> Vec<hidlins_sync::address::LocalEndpoint> {
    let Ok(mut browser) = discovery::desktop::DesktopBrowser::start() else {
        return Vec::new();
    };
    let mut cache = CandidateCache::new();
    let deadline = Instant::now() + Duration::from_millis(500);
    loop {
        if discovery::poll_into(&mut browser, &mut cache, Instant::now()).is_err() {
            return Vec::new();
        }
        let found = cache.candidates(ServiceKind::Trusted);
        if !found.is_empty() || Instant::now() >= deadline {
            return found;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
