//! Real authenticated incoming commits must revoke stale OTP before change events.
use std::net::TcpListener;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use hidlins_core::{EntryBuilder, HidlinsPaths, Vault, VaultRegistry};
use hidlins_sync::client::{LanCancellation, LanTransport};
use hidlins_sync::pairing::PairingWindow;
use hidlins_sync::SyncTransport;

use super::*;
use crate::event::EventSink;
use crate::fixtures::create_test_vault;

const MASTER: &str = "incoming commit fixture master";
const OLD_URI: &str = "otpauth://totp/Example:old?secret=JBSWY3DPEHPK3PXP&issuer=Example";
const NEW_URI: &str = "otpauth://totp/Example:new?secret=KRSXG5DSNFXGOIDB&issuer=Example";
const WORKER: &str = "api::sync::incoming_commit_regressions::fault_worker";
fn paths(directory: &Path) -> HidlinsPaths {
    HidlinsPaths::with_state_dir(directory.join("state"))
}

struct Sink {
    session: Weak<AppSession>,
    otp: String,
    sender: mpsc::Sender<(SyncEvent, bool)>,
}
impl EventSink<SyncEvent> for Sink {
    fn send(&self, event: SyncEvent) {
        // totp_now is cache-only and never takes the held SessionState guard.
        let revoked = !matches!(event, SyncEvent::Done(_))
            || self
                .session
                .upgrade()
                .is_some_and(|s| s.totp_now(self.otp.clone()).is_err());
        let _ = self.sender.send((event, revoked));
    }
}
struct Fixture {
    session: Arc<AppSession>,
    client: LocalSyncConfig,
    server_key: [u8; 32],
    endpoint: LocalEndpoint,
    path: PathBuf,
    otp: String,
    upload: Vec<u8>,
    events: mpsc::Receiver<(SyncEvent, bool)>,
}
impl Fixture {
    fn new(directory: &Path) -> Self {
        let paths = paths(directory);
        let path = create_test_vault(&paths, "vault", MASTER);
        let master = MasterPassword::new(MASTER.into());
        let mut vault = Vault::open(&path, &master, None).unwrap();
        let otp = vault
            .add_entry(
                vault.root_group_uuid(),
                EntryBuilder::totp("old title", OLD_URI).unwrap().build(),
            )
            .unwrap();
        vault.save().unwrap();
        drop(vault);
        let remote_path = directory.join("remote.kdbx");
        std::fs::copy(&path, &remote_path).unwrap();
        let mut remote = Vault::open(&remote_path, &master, None).unwrap();
        remote
            .update_entry(otp, |view| {
                view.set_title("incoming title");
                view.set_custom_field(hidlins_core::fields::OTP, NEW_URI, true);
                Ok(())
            })
            .unwrap();
        remote.save().unwrap();
        drop(remote);
        let upload = std::fs::read(remote_path).unwrap();
        let client = LocalSyncConfig::create("client", SyncRole::Client, &master).unwrap();
        let mut registry = VaultRegistry::load(paths.clone()).unwrap();
        let mut server =
            LocalSyncConfig::configure(&mut registry, "vault", SyncRole::Server, &master).unwrap();
        let peer = client.identity().public_key();
        let now = Instant::now();
        let mut window = PairingWindow::open_at(now);
        let sas = window.begin_candidate(peer, [79; 32], now).unwrap();
        let confirmation = window.confirm(sas, now).unwrap();
        let transaction = confirmation
            .transaction(server.identity().public_key(), peer)
            .unwrap();
        server
            .prepare_client(
                &transaction,
                &confirmation,
                "confirmed client".into(),
                1000,
                1180,
            )
            .unwrap();
        server.activate_client(&transaction, 1001).unwrap();
        server.persist(&mut registry, "vault").unwrap();
        let server_key = server.identity().public_key().into_bytes();
        let session = Arc::new(AppSession::for_test(paths).unwrap());
        session.unlock("vault".into(), MASTER.into(), None).unwrap();
        let endpoint = (0..8)
            .find_map(|_| {
                let probe = TcpListener::bind("127.0.0.1:0").unwrap();
                let port = probe.local_addr().unwrap().port();
                drop(probe);
                session
                    .start_sync_server(LocalEndpointDto {
                        address: "127.0.0.1".into(),
                        port,
                        scope_id: 0,
                    })
                    .ok()
            })
            .expect("actual API server started")
            .checked()
            .unwrap();
        let otp = otp.to_string();
        session.entry_detail(otp.clone()).unwrap();
        assert!(session.totp_now(otp.clone()).is_ok());
        let (sender, events) = mpsc::channel();
        session.sync_events_for_test(Box::new(Sink {
            session: Arc::downgrade(&session),
            otp: otp.clone(),
            sender,
        }));
        Self {
            session,
            client,
            server_key,
            endpoint,
            path,
            otp,
            upload,
            events,
        }
    }
    fn request(&self, upload: Option<Vec<u8>>) -> bool {
        let identity = self
            .client
            .identity()
            .unlock(
                "client",
                SyncRole::Client,
                &MasterPassword::new(MASTER.into()),
            )
            .unwrap();
        let endpoint = self.endpoint;
        let key = self.server_key;
        let cancellation = LanCancellation::default();
        let worker_cancel = cancellation.clone();
        let worker = thread::spawn(move || {
            let mut transport = LanTransport::new(identity, key, None, vec![endpoint])
                .unwrap()
                .with_cancellation(worker_cancel);
            let head = transport
                .head()
                .unwrap()
                .expect("authority already has vault");
            upload.is_none_or(|bytes| transport.commit_conditional(&bytes, Some(&head)).is_ok())
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while !worker.is_finished() && Instant::now() < deadline {
            self.session.pump_sync_server_for_test();
            thread::yield_now();
        }
        let completed = worker.is_finished();
        if !completed {
            cancellation.cancel();
        }
        let result = worker.join().unwrap();
        assert!(
            completed,
            "real authenticated host request completed; fixture deadline is not a behavioral red"
        );
        result
    }
    fn assert_change_boundary(&self, changed: bool) {
        assert_eq!(
            self.session.totp_now(self.otp.clone()).is_err(),
            changed,
            "old OTP cache revoked exactly when canonical replacement installed"
        );
        let done: Vec<_> = self
            .events
            .try_iter()
            .filter(|(event, _)| matches!(event, SyncEvent::Done(_)))
            .collect();
        assert_eq!(
            done.len(),
            usize::from(changed),
            "one established change event per installed replacement"
        );
        if changed {
            assert!(matches!(
                done[0].0,
                SyncEvent::Done(SyncOutcomeDto::FastReplaced)
            ));
            assert!(done[0].1, "old OTP was revoked before publishing Done");
        }
        let detail = self.session.entry_detail(self.otp.clone()).unwrap();
        assert_eq!(
            detail.title,
            if changed {
                "incoming title"
            } else {
                "old title"
            }
        );
        let cache = self.session.totp_cache.read().unwrap();
        assert!(
            cache.get(&self.otp.parse().unwrap()).unwrap().secret_uri()
                == if changed { NEW_URI } else { OLD_URI }
        );
        drop(cache);
        assert!(
            self.session.totp_now(self.otp.clone()).is_ok(),
            "refill uses current canonical secret"
        );
        for _ in 0..5 {
            self.session.pump_sync_server_for_test();
        }
        assert!(
            self.events
                .try_iter()
                .all(|(event, _)| !matches!(event, SyncEvent::Done(_))),
            "ordinary polls do not repeat change events"
        );
    }
}
#[test]
fn incoming_noise_commit_replaces_entry_and_revokes_old_otp_before_done() {
    let directory = tempfile::tempdir().unwrap();
    let fixture = Fixture::new(directory.path());
    assert!(fixture.request(Some(fixture.upload.clone())));
    fixture.assert_change_boundary(true);
}
#[test]
fn head_and_empty_polls_keep_valid_cache_without_change_events() {
    let directory = tempfile::tempdir().unwrap();
    let fixture = Fixture::new(directory.path());
    let before = std::fs::read(&fixture.path).unwrap();
    assert!(fixture.request(None));
    fixture.assert_change_boundary(false);
    assert!(std::fs::read(&fixture.path).unwrap() == before);
}
#[test]
fn rejected_authenticated_upload_keeps_canonical_cache_without_change_events() {
    let directory = tempfile::tempdir().unwrap();
    let fixture = Fixture::new(directory.path());
    let before = std::fs::read(&fixture.path).unwrap();
    assert!(!fixture.request(Some(b"invalid encrypted KDBX object".to_vec())));
    fixture.assert_change_boundary(false);
    assert!(std::fs::read(&fixture.path).unwrap() == before);
}
#[test]
#[ignore = "isolated atomic fault worker; outer scenarios execute it explicitly"]
fn fault_worker() {
    let Some(directory) = std::env::var_os("HIDLINS_INCOMING_FIXTURE") else {
        return;
    };
    let directory = Path::new(&directory);
    let phase = std::env::var("HIDLINS_INCOMING_PHASE").unwrap();
    let fixture = Fixture::new(directory);
    let before = std::fs::read(&fixture.path).unwrap();
    let signal = directory.join("signals").join(&phase);
    // Only the canonical vault's setup save has touched these signal files.
    std::fs::remove_file(&signal).unwrap();
    std::fs::create_dir(&signal).unwrap();
    assert!(
        !fixture.request(Some(fixture.upload.clone())),
        "original storage error stays visible on wire"
    );
    let changed = phase == "rename_done";
    assert_eq!(
        std::fs::read(&fixture.path).unwrap() != before,
        changed,
        "actual atomic rename boundary"
    );
    fixture.assert_change_boundary(changed);
}
fn run_phase(phase: &str) {
    let directory = tempfile::tempdir().unwrap();
    let signals = directory.path().join("signals");
    std::fs::create_dir(&signals).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", WORKER, "--ignored", "--nocapture"])
        .env("HIDLINS_INCOMING_FIXTURE", directory.path())
        .env("HIDLINS_INCOMING_PHASE", phase)
        .env(
            "HIDLINS_ATOMIC_FAULT_TARGET",
            paths(directory.path()).state_dir().join("vault.kdbx"),
        )
        .env("HIDLINS_ATOMIC_FAULT_SIGNAL_DIR", signals)
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("running 1 test"));
    assert!(
        output.status.success(),
        "isolated incoming commit assertion failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn pre_fsync_error_emits_no_false_change() {
    run_phase("temp_written");
}
#[test]
fn pre_rename_error_emits_no_false_change() {
    run_phase("fsync_done");
}
#[test]
fn post_rename_error_revokes_cache_and_announces_installed_change() {
    run_phase("rename_done");
}
