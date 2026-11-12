use super::*;
use hidlins_sync::client::LanTransport;
use hidlins_sync::noise::NoiseKeypair;
use hidlins_sync::transport::SyncTransport;
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

struct ReleasedCapture(Arc<AtomicBool>);

impl Drop for ReleasedCapture {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

struct HostFixture {
    app: Option<App>,
    release: Option<Sender<()>>,
    releaser: Option<JoinHandle<()>>,
    client: Option<JoinHandle<bool>>,
    client_finished: Arc<AtomicBool>,
    capture_dropped: Arc<AtomicBool>,
    path: PathBuf,
    before: Vec<u8>,
    _directory: TempDir,
}

impl HostFixture {
    fn release_after_network_stops(&mut self) {
        let release = self.release.take().unwrap();
        let finished = Arc::clone(&self.client_finished);
        self.releaser = Some(std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(3);
            while !finished.load(Ordering::Acquire) {
                assert!(
                    Instant::now() < deadline,
                    "teardown did not close the owned client socket"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            // Give a detached owner time to return, while a joining owner must
            // wait for this release. Positive assertions depend on ownership,
            // not a maximum operation duration or the scheduler's speed.
            std::thread::sleep(Duration::from_millis(50));
            let _ = release.send(());
        }));
    }

    fn assert_released(&self) {
        assert!(
            self.capture_dropped.load(Ordering::Acquire),
            "actual host-work capture must drop before teardown returns"
        );
        assert_eq!(std::fs::read(&self.path).unwrap(), self.before);
        drop(
            Vault::open(&self.path, &MasterPassword::new(PASSWORD.to_string()), None)
                .expect("host worker must release vault ownership before teardown returns"),
        );
    }
}

impl Drop for HostFixture {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
        drop(self.app.take());
        if let Some(releaser) = self.releaser.take() {
            let _ = releaser.join();
        }
        if let Some(client) = self.client.take() {
            let _ = client.join();
        }
    }
}

fn authorize_client(app: &mut App, identity: &NoiseKeypair) -> [u8; 32] {
    use hidlins_sync::identity::PublicIdentity;
    use hidlins_sync::pairing::PairingWindow;

    let master = MasterPassword::new(PASSWORD.to_string());
    let mut server = LocalSyncConfig::configure(
        app.registry_mut().unwrap(),
        "personal",
        SyncRole::Server,
        &master,
    )
    .unwrap();
    let peer = PublicIdentity::new(identity.public_key());
    let now = Instant::now();
    let mut window = PairingWindow::open_at(now);
    let sas = window.begin_candidate(peer, [47; 32], now).unwrap();
    let confirmation = window.confirm(sas, now).unwrap();
    let transaction = confirmation
        .transaction(server.identity().public_key(), peer)
        .unwrap();
    server
        .prepare_client(
            &transaction,
            &confirmation,
            "host lifetime fixture".to_string(),
            1000,
            1180,
        )
        .unwrap();
    server.activate_client(&transaction, 1001).unwrap();
    server
        .persist(app.registry_mut().unwrap(), "personal")
        .unwrap();
    app.start_server(master).unwrap();
    server.identity().public_key().into_bytes()
}

fn host_fixture(paused: bool) -> HostFixture {
    let (directory, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    let path = app.vault().unwrap().backing_path().to_path_buf();
    let before = std::fs::read(&path).unwrap();
    let identity = NoiseKeypair::generate().unwrap();
    let server_key = authorize_client(&mut app, &identity);
    let endpoint = app.server.as_ref().unwrap().endpoint().parse().unwrap();
    let (release, gate) = mpsc::channel();
    let (started, entered) = mpsc::channel();
    let capture_dropped = Arc::new(AtomicBool::new(false));
    if paused {
        let dropped = Arc::clone(&capture_dropped);
        app.server_work_test_hook = Some(Box::new(move || {
            let _capture = ReleasedCapture(dropped);
            started.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(5))
                .expect("owned host probe must be released");
        }));
    }
    let finished = Arc::new(AtomicBool::new(false));
    let client_finished = Arc::clone(&finished);
    let client = std::thread::spawn(move || {
        let result = LanTransport::new(identity, server_key, Some(endpoint), [])
            .unwrap()
            .head();
        client_finished.store(true, Ordering::Release);
        result.is_ok()
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.tick(Instant::now());
        if (paused && entered.try_recv().is_ok())
            || (!paused && client.is_finished() && !app.is_syncing())
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "real authenticated request did not reach App host worker"
        );
        std::thread::yield_now();
    }
    HostFixture {
        app: Some(app),
        release: Some(release),
        releaser: None,
        client: Some(client),
        client_finished: finished,
        capture_dropped,
        path,
        before,
        _directory: directory,
    }
}

#[test]
fn quit_waits_for_actual_incoming_host_work_and_releases_vault() {
    let mut fixture = host_fixture(true);
    fixture.release_after_network_stops();
    fixture.app.as_mut().unwrap().do_quit();
    assert!(fixture.app.as_ref().unwrap().should_quit);
    fixture.assert_released();
}

#[test]
fn app_drop_waits_for_actual_incoming_host_work_and_releases_vault() {
    let mut fixture = host_fixture(true);
    fixture.release_after_network_stops();
    drop(fixture.app.take());
    fixture.assert_released();
}

#[test]
fn server_stop_waits_for_host_work_and_restores_offline_workspace() {
    let mut fixture = host_fixture(true);
    fixture.release_after_network_stops();
    let app = fixture.app.as_mut().unwrap();
    app.stop_server_by_user();
    assert!(fixture.capture_dropped.load(Ordering::Acquire));
    assert!(matches!(app.phase, Phase::Workspace));
    assert!(app.vault().is_some());
    assert!(!app.is_syncing());
    assert!(app.server.is_none());
    assert_eq!(std::fs::read(&fixture.path).unwrap(), fixture.before);
}

#[test]
fn normal_actual_host_request_returns_resources_to_workspace() {
    let mut fixture = host_fixture(false);
    assert!(fixture.client.take().unwrap().join().unwrap());
    let app = fixture.app.as_ref().unwrap();
    assert!(matches!(app.phase, Phase::Workspace));
    assert!(app.vault().is_some());
    assert!(!app.is_syncing());
    assert_eq!(std::fs::read(&fixture.path).unwrap(), fixture.before);
}
