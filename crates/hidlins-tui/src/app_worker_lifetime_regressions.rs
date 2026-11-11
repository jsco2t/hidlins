use super::*;
use std::io::Read;
use std::net::{Ipv4Addr, TcpListener};
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;

struct StalledPeer {
    endpoint: hidlins_sync::address::LocalEndpoint,
    entered: Receiver<()>,
    stopping: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl StalledPeer {
    fn start() -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let endpoint = listener.local_addr().unwrap().to_string().parse().unwrap();
        listener.set_nonblocking(true).unwrap();
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&stopping);
        let (sender, entered) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_millis(20)))
                            .unwrap();
                        let mut buffer = [0_u8; 512];
                        let mut announced = false;
                        while !stop.load(Ordering::Acquire) {
                            match stream.read(&mut buffer) {
                                Ok(0) => return,
                                Ok(_) if !announced => {
                                    let _ = sender.send(());
                                    announced = true;
                                }
                                Ok(_) => {}
                                Err(error)
                                    if matches!(
                                        error.kind(),
                                        std::io::ErrorKind::WouldBlock
                                            | std::io::ErrorKind::TimedOut
                                    ) => {}
                                Err(_) => return,
                            }
                        }
                        return;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => panic!("owned fixture listener failed: {error}"),
                }
            }
        });
        Self {
            endpoint,
            entered,
            stopping,
            worker: Some(worker),
        }
    }
}

impl Drop for StalledPeer {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}

struct ClientFixture {
    app: Option<App>,
    _peer: StalledPeer,
    path: PathBuf,
    before: Vec<u8>,
    _directory: TempDir,
}

fn stalled_client() -> ClientFixture {
    use hidlins_sync::pairing::PairingWindow;

    let (directory, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    let path = app.vault().unwrap().backing_path().to_path_buf();
    let before = std::fs::read(&path).unwrap();
    let master = MasterPassword::new(PASSWORD.to_string());
    let mut client = LocalSyncConfig::configure(
        app.registry_mut().unwrap(),
        "personal",
        SyncRole::Client,
        &master,
    )
    .unwrap();
    let server = LocalSyncConfig::create("fixture-server", SyncRole::Server, &master).unwrap();
    let now = Instant::now();
    let mut window = PairingWindow::open_at(now);
    let sas = window
        .begin_candidate(server.identity().public_key(), [43; 32], now)
        .unwrap();
    let confirmation = window.confirm(sas, now).unwrap();
    let transaction = confirmation
        .transaction(
            server.identity().public_key(),
            client.identity().public_key(),
        )
        .unwrap();
    client
        .prepare_server(
            &transaction,
            &confirmation,
            "stalled fixture server".to_string(),
            1000,
            1180,
        )
        .unwrap();
    client.activate_server(&transaction, 1001).unwrap();
    let peer = StalledPeer::start();
    client.set_routing_hint(Some(peer.endpoint));
    client
        .persist(app.registry_mut().unwrap(), "personal")
        .unwrap();
    app.start_sync(SyncTrigger::Manual, master);
    peer.entered
        .recv_timeout(Duration::from_secs(5))
        .expect("real client reached the owned peer");
    ClientFixture {
        app: Some(app),
        _peer: peer,
        path,
        before,
        _directory: directory,
    }
}

fn assert_released(fixture: &ClientFixture) {
    assert_eq!(std::fs::read(&fixture.path).unwrap(), fixture.before);
    drop(
        Vault::open(
            &fixture.path,
            &MasterPassword::new(PASSWORD.to_string()),
            None,
        )
        .expect("teardown must release the worker-owned vault before returning"),
    );
}

#[test]
fn quit_joins_real_client_and_releases_vault_before_returning() {
    let mut fixture = stalled_client();
    let app = fixture.app.as_mut().unwrap();
    app.do_quit();
    assert!(app.should_quit);
    assert_released(&fixture);
}

#[test]
fn app_drop_joins_real_client_and_releases_vault_before_returning() {
    let mut fixture = stalled_client();
    drop(fixture.app.take());
    assert_released(&fixture);
}

#[test]
fn event_loop_unwind_joins_real_client_and_releases_vault() {
    let mut fixture = stalled_client();
    let app = fixture.app.take().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _owned_app = app;
        panic!("synthetic event-loop failure");
    }));
    assert!(result.is_err());
    assert_released(&fixture);
}

#[test]
fn manual_lock_cancels_real_client_before_the_handshake_deadline() {
    let mut fixture = stalled_client();
    let app = fixture.app.as_mut().unwrap();
    app.handle_event(&key_ctrl('l'));
    // The production handshake timeout is five seconds. A one-second bound
    // distinguishes owner cancellation while leaving ample scheduling margin.
    let deadline = Instant::now() + Duration::from_secs(1);
    while !matches!(app.phase, Phase::LockScreen) && Instant::now() < deadline {
        app.tick(Instant::now());
        std::thread::yield_now();
    }
    assert!(matches!(app.phase, Phase::LockScreen));
    assert!(!app.is_syncing());
    assert_released(&fixture);
}

#[test]
fn ordinary_worker_completion_returns_the_owned_vault_to_workspace() {
    let (_directory, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    configure_target(&mut app, "personal");
    app.set_sync_engine(Arc::new(AlreadyInSyncEngine));
    app.start_sync(
        SyncTrigger::Manual,
        MasterPassword::new(PASSWORD.to_string()),
    );
    let deadline = Instant::now() + Duration::from_secs(1);
    while app.is_syncing() && Instant::now() < deadline {
        app.tick(Instant::now());
        std::thread::yield_now();
    }
    assert!(matches!(app.phase, Phase::Workspace));
    assert!(app.vault().is_some());
    assert!(!app.is_syncing());
}
