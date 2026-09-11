//! Local-network sync, pairing, import, peer management, and foreground serving.

use std::{
    io::{BufRead as _, Write as _},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use hidlins_core::{HidlinsPaths, Keyfile, MasterPassword, Vault, VaultError, VaultRegistry};
use hidlins_sync::{
    address::LocalEndpoint,
    client::{self, ClientPairingSession, PendingPairingStore},
    config::local::LocalSyncConfig,
    discovery::{self, CandidateCache, ServiceKind},
    identity::{PublicIdentity, SyncRole},
    server::{
        AuthoritativeVault, HostQueue, PairingAuthority, PairingAuthorityError, ServerController,
    },
    trust::{PeerRecord, PeerStatus, TrustError},
    SyncOptions,
};
use serde::Serialize;

use crate::{
    agent::NoAgentClient,
    cli::{
        Cli, OutputFormat, SyncArgs, SyncImportArgs, SyncNowArgs, SyncPairArgs, SyncPeerListArgs,
        SyncPeerRenameArgs, SyncPeerRevokeArgs, SyncPeersVerb, SyncServeArgs, SyncStatusArgs,
        SyncVerb,
    },
    commands::{resolve_paths, write_json_success},
    exit::CliExit,
    prompt::{master_password, PromptOpts},
    views::sync::SyncView,
};

const DISCOVERY_WAIT: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(20);

/// Dispatch a local-network sync operation.
pub fn run(cli: &Cli, args: &SyncArgs) -> Result<(), CliExit> {
    match args.verb.as_ref() {
        Some(SyncVerb::Now(args)) => run_now(cli, args),
        Some(SyncVerb::Serve(args)) => run_serve(cli, args),
        Some(SyncVerb::Pair(args)) => run_pair(cli, args),
        Some(SyncVerb::Import(args)) => run_import(cli, args),
        Some(SyncVerb::Status(args)) => run_status(cli, args),
        Some(SyncVerb::Peers(args)) => match args.verb.as_ref() {
            Some(SyncPeersVerb::List(args)) => run_peers_list(cli, args),
            Some(SyncPeersVerb::Rename(args)) => run_peer_rename(cli, args),
            Some(SyncPeersVerb::Revoke(args)) => run_peer_revoke(cli, args),
            None => Err(CliExit::UserError("missing sync peers operation".into())),
        },
        None => Err(CliExit::UserError("missing sync operation".into())),
    }
}

fn run_now(cli: &Cli, args: &SyncNowArgs) -> Result<(), CliExit> {
    let paths = resolve_paths(cli)?;
    let mut registry = VaultRegistry::load(paths).map_err(CliExit::from)?;
    let name = resolve_vault_name(&registry, args.vault.as_deref())?;
    let (mut vault, master, keyfile) = unlock(&registry, &name)?;
    let candidates = candidates(args.address.as_deref(), args.port, ServiceKind::Trusted)?;
    let started = Instant::now();
    let outcome = client::sync_vault(
        &mut vault,
        &name,
        &mut registry,
        &master,
        keyfile.as_ref(),
        candidates,
        SyncOptions::default(),
    )
    .map_err(|error| attach_vault_name(CliExit::from(error), &name))?;
    if matches!(cli.format, OutputFormat::Json) {
        write_json_success(&SyncView::from(&outcome))
    } else {
        writeln!(
            std::io::stderr().lock(),
            "{}",
            hidlins_sync::sync::format_outcome(&outcome, started.elapsed())
        )
        .map_err(output_error)
    }
}

fn run_pair(cli: &Cli, args: &SyncPairArgs) -> Result<(), CliExit> {
    let paths = resolve_paths(cli)?;
    let mut registry = VaultRegistry::load(paths).map_err(CliExit::from)?;
    let name = resolve_vault_name(&registry, args.vault.as_deref())?;
    let (_, master, _) = unlock(&registry, &name)?;
    let mut config = match registry
        .get(&name)
        .and_then(LocalSyncConfig::from_vault_entry)
    {
        Some(config) if config.role() == SyncRole::Client && !config.is_active_client() => config,
        Some(_) => {
            return Err(CliExit::UserError(
                "vault is already paired or configured as an authority".into(),
            ))
        }
        None => LocalSyncConfig::configure(&mut registry, &name, SyncRole::Client, &master)
            .map_err(|error| CliExit::from(hidlins_sync::SyncError::from(error)))?,
    };
    let recovering = config.provisional().is_some();
    let routes = candidates(
        args.address.as_deref(),
        args.port,
        if recovering {
            ServiceKind::Trusted
        } else {
            ServiceKind::Pairing
        },
    )?;
    if recovering {
        client::recover_pairing(
            &mut config,
            &name,
            &master,
            routes,
            epoch_seconds()?,
            |active| active.persist(&mut registry, &name),
        )
        .map_err(|error| CliExit::from(hidlins_sync::SyncError::from(error)))?;
        return success(
            cli,
            &PairView {
                status: "paired",
                vault: &name,
            },
        );
    }
    let identity = config
        .identity()
        .unlock(&name, SyncRole::Client, &master)
        .map_err(|_| CliExit::VaultLocked("local sync identity could not be unlocked".into()))?;
    let session = ClientPairingSession::begin(&identity, routes)
        .map_err(|error| CliExit::from(hidlins_sync::SyncError::from(error)))?;
    let endpoint = session.endpoint();
    confirm_sas(&session.sas().to_string())?;
    let now = epoch_seconds()?;
    session
        .confirm(&mut config, args.name.clone(), now, |prepared| {
            prepared.persist(&mut registry, &name)
        })
        .map_err(|error| CliExit::from(hidlins_sync::SyncError::from(error)))?;
    config.set_routing_hint(Some(endpoint));
    config
        .persist(&mut registry, &name)
        .map_err(|error| CliExit::from(hidlins_sync::SyncError::from(error)))?;
    success(
        cli,
        &PairView {
            status: "paired",
            vault: &name,
        },
    )
}

fn run_import(cli: &Cli, args: &SyncImportArgs) -> Result<(), CliExit> {
    let paths = resolve_paths(cli)?;
    let mut registry = VaultRegistry::load(paths).map_err(CliExit::from)?;
    if registry.get(&args.id).is_some() {
        return Err(CliExit::UserError(format!(
            "vault already registered: {}",
            args.id
        )));
    }
    let target = import_target(&registry, args)?;
    let master = prompt_master(&args.id)?;
    let keyfile = args.keyfile.clone().map(Keyfile::Path);
    let pending_store = PendingPairingStore::new(registry.paths().state_dir(), &args.id);
    let pending = pending_store
        .load()
        .map_err(|error| CliExit::from(hidlins_sync::SyncError::from(error)))?;
    let mut config = match pending {
        Some(config) => config,
        None => LocalSyncConfig::create(&args.id, SyncRole::Client, &master)
            .map_err(|error| CliExit::from(hidlins_sync::SyncError::from(error)))?,
    };
    let recovering = config.provisional().is_some();
    let routes = candidates(
        args.address.as_deref(),
        args.port,
        if recovering {
            ServiceKind::Trusted
        } else {
            ServiceKind::Pairing
        },
    )?;
    if recovering {
        client::recover_pairing(
            &mut config,
            &args.id,
            &master,
            routes.clone(),
            epoch_seconds()?,
            |active| pending_store.save(active),
        )
        .map_err(|error| CliExit::from(hidlins_sync::SyncError::from(error)))?;
    }
    if config.is_active_client() {
        complete_import(
            args,
            &master,
            keyfile.as_ref(),
            config,
            &routes,
            &mut registry,
            &pending_store,
        )?;
        return success(
            cli,
            &ImportView {
                status: "imported",
                vault: &args.id,
                path: target.display().to_string(),
            },
        );
    }
    let identity = config
        .identity()
        .unlock(&args.id, SyncRole::Client, &master)
        .map_err(|_| CliExit::VaultLocked("local sync identity could not be unlocked".into()))?;
    let session = ClientPairingSession::begin(&identity, routes.clone())
        .map_err(|error| CliExit::from(hidlins_sync::SyncError::from(error)))?;
    let endpoint = session.endpoint();
    confirm_sas(&session.sas().to_string())?;
    session
        .confirm(
            &mut config,
            args.name.clone(),
            epoch_seconds()?,
            |prepared| pending_store.save(prepared),
        )
        .map_err(|error| CliExit::from(hidlins_sync::SyncError::from(error)))?;
    config.set_routing_hint(Some(endpoint));
    complete_import(
        args,
        &master,
        keyfile.as_ref(),
        config,
        &routes,
        &mut registry,
        &pending_store,
    )?;
    success(
        cli,
        &ImportView {
            status: "imported",
            vault: &args.id,
            path: target.display().to_string(),
        },
    )
}

fn complete_import(
    args: &SyncImportArgs,
    master: &MasterPassword,
    keyfile: Option<&Keyfile>,
    mut config: LocalSyncConfig,
    routes: &[LocalEndpoint],
    registry: &mut VaultRegistry,
    pending_store: &PendingPairingStore,
) -> Result<(), CliExit> {
    let target = args.path.clone().unwrap_or_else(|| {
        registry
            .paths()
            .state_dir()
            .join(format!("{}.kdbx", args.id))
    });
    // The authority sends PairActivated before its pairing worker has fully
    // released the connection slot. Retry only this immediate, idempotent
    // encrypted fetch within a small fixed bound.
    let mut fetched = None;
    let mut last_error = hidlins_sync::client::LanError::Unreachable;
    for _ in 0..3 {
        match client::fetch_paired_vault(&args.id, master, &config, routes.iter().copied()) {
            Ok(value) => {
                fetched = Some(value);
                break;
            }
            Err(error) => {
                last_error = error;
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    let (bytes, version) =
        fetched.ok_or_else(|| CliExit::from(hidlins_sync::SyncError::from(last_error)))?;
    config.set_sync_versions(Some(version), Some(version));
    client::import_paired_vault(
        &bytes, &target, &args.id, master, keyfile, &config, registry,
    )
    .map_err(|error| CliExit::UserError(error.to_string()))?;
    pending_store
        .clear()
        .map_err(|error| CliExit::from(hidlins_sync::SyncError::from(error)))?;
    Ok(())
}

fn import_target(
    registry: &VaultRegistry,
    args: &SyncImportArgs,
) -> Result<std::path::PathBuf, CliExit> {
    let target = args.path.clone().unwrap_or_else(|| {
        registry
            .paths()
            .state_dir()
            .join(format!("{}.kdbx", args.id))
    });
    if target.exists() {
        return Err(CliExit::UserError(format!(
            "path already exists: {}",
            target.display()
        )));
    }
    Ok(target)
}

fn run_serve(cli: &Cli, args: &SyncServeArgs) -> Result<(), CliExit> {
    let paths = resolve_paths(cli)?;
    let mut registry = VaultRegistry::load(paths.clone()).map_err(CliExit::from)?;
    let name = resolve_vault_name(&registry, args.vault.as_deref())?;
    let (mut vault, master, keyfile) = unlock(&registry, &name)?;
    let config = match registry
        .get(&name)
        .and_then(LocalSyncConfig::from_vault_entry)
    {
        Some(config) if config.role() == SyncRole::Server => config,
        Some(_) => {
            return Err(CliExit::UserError(
                "a paired client vault cannot serve".into(),
            ))
        }
        None => LocalSyncConfig::configure(&mut registry, &name, SyncRole::Server, &master)
            .map_err(|error| CliExit::from(hidlins_sync::SyncError::from(error)))?,
    };
    let endpoint = if let Some(address) = args.address.as_deref() {
        manual_endpoint(address, args.port)?
    } else {
        hidlins_sync::discovery::desktop::allowed_interface_endpoints(args.port)
            .map_err(|_| CliExit::UserError("no active local interface is available".into()))?
            .into_iter()
            .next()
            .ok_or_else(|| CliExit::UserError("no active local interface is available".into()))?
    };
    let identity = Arc::new(
        config
            .identity()
            .unlock(&name, SyncRole::Server, &master)
            .map_err(|_| {
                CliExit::VaultLocked("local sync identity could not be unlocked".into())
            })?,
    );
    let (queue, mut processor) = HostQueue::new();
    let authority: Arc<dyn PairingAuthority> =
        Arc::new(CliPairingAuthority::new(paths, name.clone()));
    let mut controller = ServerController::start_with_pairing_authority(
        endpoint,
        identity,
        active_peer_keys(&config),
        queue,
        Some(authority),
    )
    .map_err(|error| CliExit::Internal(error.to_string()))?;
    controller.replace_pairing_recovery(
        config
            .provisional()
            .map(|record| record.peer_key().into_bytes()),
    );
    if args.pairing_window {
        controller
            .open_pairing()
            .map_err(|error| CliExit::Internal(error.to_string()))?;
    }
    let bound = controller.endpoint();
    success(
        cli,
        &ServeView {
            status: "serving",
            endpoint: bound.to_string(),
            pairing_window: args.pairing_window,
        },
    )?;
    let signals = signals::SignalGuard::install()?;
    let mut host = AuthoritativeVault::new(&mut vault, &master, keyfile.as_ref());
    while !signals.interrupted() {
        match processor.process_one(&mut host) {
            Ok(true) => {}
            Ok(false) => std::thread::sleep(POLL),
            Err(hidlins_sync::server::ServerError::Stopped) => break,
            Err(error) => return Err(CliExit::Internal(error.to_string())),
        }
        let _ = controller.advertisements();
        if let Ok(latest) = VaultRegistry::load(registry.paths().clone()) {
            if let Some(config) = latest
                .get(&name)
                .and_then(LocalSyncConfig::from_vault_entry)
            {
                controller.replace_trusted(active_peer_keys(&config));
            }
        }
    }
    processor.shutdown();
    controller.stop();
    Ok(())
}

fn run_status(cli: &Cli, args: &SyncStatusArgs) -> Result<(), CliExit> {
    let registry = VaultRegistry::load(resolve_paths(cli)?).map_err(CliExit::from)?;
    let name = resolve_vault_name(&registry, args.vault.as_deref())?;
    let config = local_config(&registry, &name)?;
    let status = config.status();
    success(
        cli,
        &StatusView {
            vault: &name,
            role: role_name(config.role()),
            paired: status.paired,
            active_clients: status.active_clients,
        },
    )
}

fn run_peers_list(cli: &Cli, args: &SyncPeerListArgs) -> Result<(), CliExit> {
    let registry = VaultRegistry::load(resolve_paths(cli)?).map_err(CliExit::from)?;
    let name = resolve_vault_name(&registry, args.vault.as_deref())?;
    let config = local_config(&registry, &name)?;
    let peers = peer_refs(&config)
        .into_iter()
        .enumerate()
        .map(|(index, peer)| PeerView {
            peer: format!("peer-{index}"),
            name: peer.display_name().to_string(),
            revoked: peer.status() == PeerStatus::Revoked,
        })
        .collect();
    success(
        cli,
        &PeersView {
            vault: &name,
            peers,
        },
    )
}

fn run_peer_rename(cli: &Cli, args: &SyncPeerRenameArgs) -> Result<(), CliExit> {
    update_peer(cli, args.vault.as_deref(), &args.peer, |config, key| {
        config.rename_peer(key, args.name.clone())
    })?;
    success(
        cli,
        &PeerChangeView {
            status: "renamed",
            peer: &args.peer,
        },
    )
}

fn run_peer_revoke(cli: &Cli, args: &SyncPeerRevokeArgs) -> Result<(), CliExit> {
    update_peer(
        cli,
        args.vault.as_deref(),
        &args.peer,
        LocalSyncConfig::revoke_peer,
    )?;
    success(
        cli,
        &PeerChangeView {
            status: "revoked",
            peer: &args.peer,
        },
    )
}

fn update_peer(
    cli: &Cli,
    requested: Option<&str>,
    id: &str,
    update: impl FnOnce(&mut LocalSyncConfig, PublicIdentity) -> Result<(), TrustError>,
) -> Result<(), CliExit> {
    let mut registry = VaultRegistry::load(resolve_paths(cli)?).map_err(CliExit::from)?;
    let name = resolve_vault_name(&registry, requested)?;
    let index = parse_peer_id(id)?;
    LocalSyncConfig::transactional_update(&mut registry, &name, |config| {
        let key = peer_refs(config)
            .get(index)
            .map(|peer| peer.public_key())
            .ok_or(TrustError::NotActive)?;
        update(config, key)
    })
    .map_err(|_| CliExit::UserError(format!("peer not found or update rejected: {id}")))
}

fn local_config(registry: &VaultRegistry, name: &str) -> Result<LocalSyncConfig, CliExit> {
    registry
        .get(name)
        .and_then(LocalSyncConfig::from_vault_entry)
        .ok_or_else(|| CliExit::UserError("local sync is not configured for this vault".into()))
}

fn peer_refs(config: &LocalSyncConfig) -> Vec<&PeerRecord> {
    match config.role() {
        SyncRole::Server => config.trusted_peers().iter().collect(),
        SyncRole::Client => config.pinned_server().into_iter().collect(),
    }
}

fn parse_peer_id(value: &str) -> Result<usize, CliExit> {
    value
        .strip_prefix("peer-")
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| CliExit::UserError("peer must be an opaque peer-N handle".into()))
}

fn unlock(
    registry: &VaultRegistry,
    name: &str,
) -> Result<(Vault, MasterPassword, Option<Keyfile>), CliExit> {
    let record = registry.get(name).ok_or_else(|| {
        CliExit::from(VaultError::NotRegistered {
            name: name.to_string(),
        })
    })?;
    let master = prompt_master(name)?;
    let keyfile = record.keyfile_path.clone().map(Keyfile::Path);
    let vault = Vault::open(&record.path, &master, keyfile.as_ref()).map_err(CliExit::from)?;
    Ok((vault, master, keyfile))
}

fn prompt_master(name: &str) -> Result<MasterPassword, CliExit> {
    master_password(&PromptOpts {
        vault: name,
        agent: &NoAgentClient,
        prompt_label: "Master password: ",
    })
}

fn resolve_vault_name(
    registry: &VaultRegistry,
    requested: Option<&str>,
) -> Result<String, CliExit> {
    if let Some(name) = requested {
        return Ok(name.to_string());
    }
    let mut names = registry.list().map(|record| record.name.clone());
    match (names.next(), names.next()) {
        (Some(name), None) => Ok(name),
        (None, _) => Err(CliExit::UserError("no vaults registered".into())),
        _ => Err(CliExit::UserError(
            "multiple vaults registered; use --vault".into(),
        )),
    }
}

fn candidates(
    address: Option<&str>,
    port: Option<u16>,
    kind: ServiceKind,
) -> Result<Vec<LocalEndpoint>, CliExit> {
    if let (Some(address), Some(port)) = (address, port) {
        return Ok(vec![manual_endpoint(address, port)?]);
    }
    let mut browser = discovery::desktop::DesktopBrowser::start().map_err(|_| {
        CliExit::UserError(
            "local discovery is unavailable; use the restricted --address/--port fallback".into(),
        )
    })?;
    let mut cache = CandidateCache::new();
    let found =
        discovery::collect_candidates(&mut browser, &mut cache, kind, DISCOVERY_WAIT, POLL, || {
            false
        })
        .map_err(|_| CliExit::UserError("local discovery failed".into()))?;
    if found.is_empty() {
        Err(CliExit::UserError(
            "no local sync server was discovered".into(),
        ))
    } else {
        Ok(found)
    }
}

fn manual_endpoint(address: &str, port: u16) -> Result<LocalEndpoint, CliExit> {
    let literal = if address.contains(':') {
        format!("[{address}]:{port}")
    } else {
        format!("{address}:{port}")
    };
    literal
        .parse()
        .map_err(|_| CliExit::UserError("address must be an allowed non-public IP literal".into()))
}

fn confirm_sas(sas: &str) -> Result<(), CliExit> {
    write_sas_prompt(sas)?;
    read_confirmation()
}

fn write_sas_prompt(sas: &str) -> Result<(), CliExit> {
    let mut stderr = std::io::stderr().lock();
    writeln!(stderr, "Pairing SAS: {sas}").map_err(output_error)?;
    write!(stderr, "Confirm the same SAS on both devices? [y/N]: ").map_err(output_error)?;
    stderr.flush().map_err(output_error)
}

fn read_confirmation() -> Result<(), CliExit> {
    let mut answer = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut answer)
        .map_err(|error| CliExit::UserError(format!("failed to read confirmation: {error}")))?;
    if matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        Ok(())
    } else {
        Err(CliExit::UserError("pairing rejected".into()))
    }
}

fn epoch_seconds() -> Result<u64, CliExit> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .map_err(|_| CliExit::Internal("system clock unavailable".into()))
}

fn active_peer_keys(config: &LocalSyncConfig) -> Vec<[u8; 32]> {
    config
        .trusted_peers()
        .iter()
        .filter(|peer| peer.status() == PeerStatus::Active)
        .map(|peer| peer.public_key().into_bytes())
        .collect()
}

fn role_name(role: SyncRole) -> &'static str {
    match role {
        SyncRole::Server => "server",
        SyncRole::Client => "client",
    }
}
#[allow(
    clippy::needless_pass_by_value,
    reason = "used directly as Result::map_err callback"
)]
fn output_error(error: std::io::Error) -> CliExit {
    CliExit::Internal(format!("failed to write output: {error}"))
}

fn attach_vault_name(mut exit: CliExit, name: &str) -> CliExit {
    if let CliExit::SyncConflict { vault, .. } = &mut exit {
        if vault.is_empty() {
            *vault = name.to_string();
        }
    }
    exit
}

fn success<T: Serialize + HumanLine>(cli: &Cli, value: &T) -> Result<(), CliExit> {
    if matches!(cli.format, OutputFormat::Json) {
        write_json_success(value)
    } else {
        writeln!(std::io::stderr().lock(), "{}", value.human()).map_err(output_error)
    }
}

trait HumanLine {
    fn human(&self) -> String;
}

#[derive(Serialize)]
struct PairView<'a> {
    status: &'static str,
    vault: &'a str,
}
impl HumanLine for PairView<'_> {
    fn human(&self) -> String {
        format!("paired vault '{}'", self.vault)
    }
}
#[derive(Serialize)]
struct ImportView<'a> {
    status: &'static str,
    vault: &'a str,
    path: String,
}
impl HumanLine for ImportView<'_> {
    fn human(&self) -> String {
        format!("imported vault '{}' to {}", self.vault, self.path)
    }
}
#[derive(Serialize)]
struct ServeView {
    status: &'static str,
    endpoint: String,
    pairing_window: bool,
}
impl HumanLine for ServeView {
    fn human(&self) -> String {
        format!(
            "local sync server ready at {}{}",
            self.endpoint,
            if self.pairing_window {
                " (pairing open)"
            } else {
                ""
            }
        )
    }
}
#[derive(Serialize)]
struct StatusView<'a> {
    vault: &'a str,
    role: &'static str,
    paired: bool,
    active_clients: usize,
}
impl HumanLine for StatusView<'_> {
    fn human(&self) -> String {
        format!(
            "{}: role={}, paired={}, active_clients={}",
            self.vault, self.role, self.paired, self.active_clients
        )
    }
}
#[derive(Serialize)]
struct PeerView {
    peer: String,
    name: String,
    revoked: bool,
}
#[derive(Serialize)]
struct PeersView<'a> {
    vault: &'a str,
    peers: Vec<PeerView>,
}
impl HumanLine for PeersView<'_> {
    fn human(&self) -> String {
        if self.peers.is_empty() {
            "no peers".into()
        } else {
            self.peers
                .iter()
                .map(|peer| {
                    format!(
                        "{}\t{}\t{}",
                        peer.peer,
                        peer.name,
                        if peer.revoked { "revoked" } else { "active" }
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}
#[derive(Serialize)]
struct PeerChangeView<'a> {
    status: &'static str,
    peer: &'a str,
}
impl HumanLine for PeerChangeView<'_> {
    fn human(&self) -> String {
        format!("{} {}", self.status, self.peer)
    }
}

struct CliPairingAuthority {
    paths: HidlinsPaths,
    vault: String,
    prompt: Mutex<()>,
    stopped: AtomicBool,
    cancellation: AtomicUsize,
}
impl CliPairingAuthority {
    fn new(paths: HidlinsPaths, vault: String) -> Self {
        Self {
            paths,
            vault,
            prompt: Mutex::new(()),
            stopped: AtomicBool::new(false),
            cancellation: AtomicUsize::new(0),
        }
    }
}
impl PairingAuthority for CliPairingAuthority {
    fn confirm(
        &self,
        _peer: PublicIdentity,
        sas: hidlins_sync::pairing::SasCode,
        timeout: Duration,
    ) -> Result<String, PairingAuthorityError> {
        let _guard = self
            .prompt
            .lock()
            .map_err(|_| PairingAuthorityError::Canceled)?;
        if self.stopped.load(Ordering::Acquire) {
            return Err(PairingAuthorityError::Canceled);
        }
        let cancellation = self.cancellation.load(Ordering::Acquire);
        write_sas_prompt(&sas.to_string()).map_err(|_| PairingAuthorityError::Canceled)?;
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("hidlins-cli-pairing-confirmation".into())
            .spawn(move || {
                let _ = sender.try_send(read_confirmation().is_ok());
            })
            .map_err(|_| PairingAuthorityError::Canceled)?;
        let deadline = Instant::now() + timeout;
        loop {
            if self.stopped.load(Ordering::Acquire)
                || self.cancellation.load(Ordering::Acquire) != cancellation
            {
                return Err(PairingAuthorityError::Canceled);
            }
            match receiver.recv_timeout(Duration::from_millis(50)) {
                Ok(true) => break,
                Ok(false) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(PairingAuthorityError::Canceled)
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if Instant::now() >= deadline {
                return Err(PairingAuthorityError::Canceled);
            }
        }
        Ok("client".into())
    }
    fn prepare(
        &self,
        transaction: &hidlins_sync::pairing::PairingTransaction,
        confirmation: &hidlins_sync::pairing::ConfirmedPairing,
        display_name: String,
    ) -> Result<(), PairingAuthorityError> {
        let mut registry = VaultRegistry::load(self.paths.clone())
            .map_err(|_| PairingAuthorityError::Persistence)?;
        let now = epoch_seconds().map_err(|_| PairingAuthorityError::Persistence)?;
        LocalSyncConfig::transactional_update(&mut registry, &self.vault, |config| {
            config.prepare_client(
                transaction,
                confirmation,
                display_name,
                now,
                now.saturating_add(hidlins_sync::protocol::PAIRING_WINDOW.as_secs()),
            )
        })
        .map_err(|_| PairingAuthorityError::Persistence)
    }
    fn activate(
        &self,
        transaction: &hidlins_sync::pairing::PairingTransaction,
    ) -> Result<(), PairingAuthorityError> {
        let mut registry = VaultRegistry::load(self.paths.clone())
            .map_err(|_| PairingAuthorityError::Persistence)?;
        let now = epoch_seconds().map_err(|_| PairingAuthorityError::Persistence)?;
        LocalSyncConfig::transactional_update(&mut registry, &self.vault, |config| {
            config.activate_client(transaction, now)
        })
        .map_err(|_| PairingAuthorityError::Persistence)
    }
    fn recover(
        &self,
        peer: PublicIdentity,
    ) -> Result<hidlins_sync::pairing::PairingTransaction, PairingAuthorityError> {
        let registry = VaultRegistry::load(self.paths.clone())
            .map_err(|_| PairingAuthorityError::Persistence)?;
        let config = registry
            .get(&self.vault)
            .and_then(LocalSyncConfig::from_vault_entry)
            .ok_or(PairingAuthorityError::Persistence)?;
        let now = epoch_seconds().map_err(|_| PairingAuthorityError::Persistence)?;
        config
            .recover_pairing_transaction(peer, now)
            .map_err(|_| PairingAuthorityError::Persistence)
    }
    fn shutdown(&self) {
        self.stopped.store(true, Ordering::Release);
    }

    fn cancel_pending(&self) {
        self.cancellation.fetch_add(1, Ordering::AcqRel);
    }
}

#[cfg(unix)]
#[allow(clippy::borrow_as_ptr, clippy::unused_self)]
mod signals {
    use crate::exit::CliExit;
    use std::sync::atomic::{AtomicBool, Ordering};
    static INTERRUPTED: AtomicBool = AtomicBool::new(false);
    extern "C" fn handler(_: libc::c_int) {
        INTERRUPTED.store(true, Ordering::Release);
    }
    pub(super) struct SignalGuard {
        old_int: libc::sigaction,
        old_term: libc::sigaction,
    }
    impl SignalGuard {
        pub(super) fn install() -> Result<Self, CliExit> {
            INTERRUPTED.store(false, Ordering::Release);
            // SAFETY: sigaction is initialized before use; handler performs only an atomic store.
            #[allow(unsafe_code)]
            unsafe {
                let mut action: libc::sigaction = std::mem::zeroed();
                action.sa_sigaction = handler as *const () as usize;
                libc::sigemptyset(&mut action.sa_mask);
                let mut old_int = std::mem::zeroed();
                let mut old_term = std::mem::zeroed();
                if libc::sigaction(libc::SIGINT, &action, &mut old_int) != 0
                    || libc::sigaction(libc::SIGTERM, &action, &mut old_term) != 0
                {
                    return Err(CliExit::Internal(
                        "could not install foreground signal handler".into(),
                    ));
                }
                Ok(Self { old_int, old_term })
            }
        }
        pub(super) fn interrupted(&self) -> bool {
            INTERRUPTED.load(Ordering::Acquire)
        }
    }
    impl Drop for SignalGuard {
        fn drop(&mut self) {
            // SAFETY: both actions were returned by successful sigaction calls.
            #[allow(unsafe_code)]
            unsafe {
                libc::sigaction(libc::SIGINT, &self.old_int, std::ptr::null_mut());
                libc::sigaction(libc::SIGTERM, &self.old_term, std::ptr::null_mut());
            }
        }
    }
}
