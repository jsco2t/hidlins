mod common;

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use hidlins_api::api::session::{init_app, AppSession};
use hidlins_api::dto::{AppInitConfig, KeyfileRef, LifecycleStateDto, LockEvent};
use hidlins_api::error::HidlinsApiError;

use common::{
    create_test_vault, create_test_vault_with_keyfile, register_vault, RecordingLockSink, TestEnv,
};

fn session_with_vault(env: &TestEnv, name: &str, password: &str) -> AppSession {
    let vault_path = create_test_vault(env, name, password);
    register_vault(env, name, &vault_path);
    AppSession::for_test(env.paths_clone()).expect("create session")
}

fn session_with_vault_and_sink(
    env: &TestEnv,
    name: &str,
    password: &str,
) -> (AppSession, RecordingLockSink) {
    let session = session_with_vault(env, name, password);
    let (sink, recording) = RecordingLockSink::new();
    session.lock_events_for_test(sink);
    (session, recording)
}

fn unlocked_session(env: &TestEnv, name: &str, password: &str) -> (AppSession, RecordingLockSink) {
    let (session, recording) = session_with_vault_and_sink(env, name, password);
    session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("unlock");
    // Discard the registration snapshot + the Unlocked transition so callers
    // assert only on events their own actions produce.
    let _ = recording.drain();
    (session, recording)
}

// ---------------------------------------------------------------------------
// Keyfile unlock — D-9. Until these landed, every unlock() call in the suite
// passed `None`, so neither KeyfileRef arm had ever executed.
// ---------------------------------------------------------------------------

#[test]
fn unlock_with_keyfile_path_succeeds() {
    let env = TestEnv::new();
    let name = "kf-path-vault";
    let password = "test-pass";

    let (vault_path, keyfile_path, _) = create_test_vault_with_keyfile(&env, name, password);
    register_vault(&env, name, &vault_path);
    let session = AppSession::for_test(env.paths_clone()).expect("session");

    session
        .unlock(
            name.to_string(),
            password.to_string(),
            Some(KeyfileRef::Path(
                keyfile_path.to_string_lossy().into_owned(),
            )),
        )
        .expect("unlock with keyfile path");

    assert!(session.has_vault(), "vault unlocked via keyfile path");
    assert!(
        session.has_credentials(),
        "D-11 retains keyfile credentials"
    );
}

#[test]
fn unlock_with_keyfile_bytes_succeeds() {
    let env = TestEnv::new();
    let name = "kf-bytes-vault";
    let password = "test-pass";

    let (vault_path, _, keyfile_bytes) = create_test_vault_with_keyfile(&env, name, password);
    register_vault(&env, name, &vault_path);
    let session = AppSession::for_test(env.paths_clone()).expect("session");

    session
        .unlock(
            name.to_string(),
            password.to_string(),
            Some(KeyfileRef::Bytes(keyfile_bytes)),
        )
        .expect("unlock with keyfile bytes");

    assert!(session.has_vault(), "vault unlocked via keyfile bytes");
}

#[test]
fn unlock_with_wrong_keyfile_bytes_fails_and_leaves_session_locked() {
    let env = TestEnv::new();
    let name = "kf-wrong-vault";
    let password = "test-pass";

    let (vault_path, _, _) = create_test_vault_with_keyfile(&env, name, password);
    register_vault(&env, name, &vault_path);
    let session = AppSession::for_test(env.paths_clone()).expect("session");

    let err = session
        .unlock(
            name.to_string(),
            password.to_string(),
            Some(KeyfileRef::Bytes(vec![0xAA; 32])),
        )
        .expect_err("wrong keyfile must not unlock");

    assert!(
        matches!(err, HidlinsApiError::AuthenticationFailed),
        "wrong keyfile is indistinct from wrong password: {err:?}"
    );
    assert!(!session.has_vault(), "no partial unlock state");
    assert!(
        !session.has_credentials(),
        "no credentials retained on failure"
    );
}

#[test]
fn unlock_without_required_keyfile_fails() {
    let env = TestEnv::new();
    let name = "kf-missing-vault";
    let password = "test-pass";

    let (vault_path, _, _) = create_test_vault_with_keyfile(&env, name, password);
    register_vault(&env, name, &vault_path);
    let session = AppSession::for_test(env.paths_clone()).expect("session");

    let err = session
        .unlock(name.to_string(), password.to_string(), None)
        .expect_err("keyfile-protected vault must not open without its keyfile");

    assert!(
        matches!(err, HidlinsApiError::AuthenticationFailed),
        "expected AuthenticationFailed, got {err:?}"
    );
    assert!(!session.has_vault());
}

fn mobile_session_with_vault(env: &TestEnv, name: &str, password: &str) -> AppSession {
    let vault_path = create_test_vault(env, name, password);
    register_vault(env, name, &vault_path);
    AppSession::for_test_mobile(env.paths_clone()).expect("create mobile session")
}

fn unlocked_mobile_session(
    env: &TestEnv,
    name: &str,
    password: &str,
) -> (AppSession, RecordingLockSink) {
    let session = mobile_session_with_vault(env, name, password);
    let (sink, recording) = RecordingLockSink::new();
    session.lock_events_for_test(sink);
    session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("unlock");
    // Discard the registration snapshot + the Unlocked transition (see
    // `unlocked_session`).
    let _ = recording.drain();
    (session, recording)
}

// ---------------------------------------------------------------------------
// init_app
// ---------------------------------------------------------------------------

#[test]
fn init_creates_session_with_locked_state() {
    let env = TestEnv::new();
    env.paths().ensure_exists().expect("ensure dir");
    let session = AppSession::for_test(env.paths_clone()).expect("init");
    assert!(!session.has_vault(), "session should start with no vault");
    assert!(
        !session.has_credentials(),
        "session should start with no credentials"
    );
}

#[test]
fn init_app_uses_state_dir_from_config() {
    let env = TestEnv::new();
    env.paths().ensure_exists().expect("ensure dir");
    let cfg = AppInitConfig {
        state_dir: Some(env.paths().state_dir().to_string_lossy().to_string()),
        config_dir: None,
    };
    let session = init_app(cfg).expect("init_app");
    session.shutdown();
}

// ---------------------------------------------------------------------------
// unlock / lock cycle
// ---------------------------------------------------------------------------

#[test]
fn init_unlock_browse_lock_unlock_cycle_maintains_consistent_state() {
    let env = TestEnv::new();
    let name = "test-vault";
    let password = "correct-horse-battery-staple";
    let (session, recording) = session_with_vault_and_sink(&env, name, password);

    assert!(!session.has_vault(), "pre-unlock: no vault");

    let tree = session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("first unlock");
    assert!(session.has_vault(), "post-unlock: vault present");
    assert!(
        session.has_credentials(),
        "post-unlock: credentials retained (D-11)"
    );
    assert!(!tree.root.uuid.is_empty(), "tree has a root group uuid");

    let events = recording.drain();
    assert!(
        events.iter().any(|e| matches!(e, LockEvent::Unlocked)),
        "unlock event should be emitted"
    );

    session.lock_now().expect("lock");
    assert!(!session.has_vault(), "post-lock: vault dropped");
    assert!(!session.has_credentials(), "post-lock: credentials dropped");

    let events = recording.drain();
    assert!(
        events.iter().any(|e| matches!(e, LockEvent::Locked)),
        "locked event should be emitted"
    );

    let tree2 = session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("second unlock");
    assert!(session.has_vault(), "re-unlock: vault present");
    assert!(session.has_credentials(), "re-unlock: credentials retained");
    assert_eq!(tree.root.name, tree2.root.name, "same vault tree root");
}

#[test]
fn wrong_password_returns_authentication_failed_and_keeps_session_locked() {
    let env = TestEnv::new();
    let name = "test-vault";
    let (session, _recording) = session_with_vault_and_sink(&env, name, "correct-password");

    let result = session.unlock(name.to_string(), "wrong-password".to_string(), None);
    assert!(
        matches!(result, Err(HidlinsApiError::AuthenticationFailed)),
        "expected AuthenticationFailed, got: {result:?}"
    );
    assert!(!session.has_vault(), "no vault after failed unlock");
    assert!(
        !session.has_credentials(),
        "no credentials after failed unlock"
    );
}

#[test]
fn unlock_already_unlocked_returns_error() {
    let env = TestEnv::new();
    let name = "test-vault";
    let password = "test-pass";
    let (session, _recording) = unlocked_session(&env, name, password);

    let result = session.unlock(name.to_string(), password.to_string(), None);
    assert!(
        matches!(result, Err(HidlinsApiError::Internal { .. })),
        "double unlock should error: {result:?}"
    );
    assert!(
        session.has_vault(),
        "vault still present after double-unlock attempt"
    );
}

// ---------------------------------------------------------------------------
// Shutdown is terminal (finding #5)
// ---------------------------------------------------------------------------

#[test]
fn unlock_after_shutdown_returns_error() {
    let env = TestEnv::new();
    let name = "test-vault";
    let password = "test-pass";
    let (session, _recording) = unlocked_session(&env, name, password);

    session.shutdown();
    assert!(session.is_dead(), "session marked dead after shutdown");

    let result = session.unlock(name.to_string(), password.to_string(), None);
    assert!(
        matches!(result, Err(HidlinsApiError::Internal { .. })),
        "unlock after shutdown should error: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// Auto-lock via ticker (manual clock injection)
// ---------------------------------------------------------------------------

#[test]
fn tick_past_idle_deadline_drops_vault_and_emits_locked_event() {
    let env = TestEnv::new();
    let name = "test-vault";
    let password = "test-pass";
    let idle_timeout = Duration::from_secs(5);

    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);
    let session =
        AppSession::for_test_with_timeout(env.paths_clone(), idle_timeout).expect("session");
    let (sink, recording) = RecordingLockSink::new();
    session.lock_events_for_test(sink);

    session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("unlock");
    assert!(session.has_vault(), "vault present after unlock");
    // Discard the registration snapshot + Unlocked so the assertion below can
    // only be satisfied by a Locked produced by the idle expiry itself.
    let _ = recording.drain();

    let now = Instant::now();

    // First tick picks up the unlock activity and re-registers it.
    session.drive_tick(now);

    session.drive_tick(now + Duration::from_secs(1));
    assert!(session.has_vault(), "vault still present after 1s");

    session.drive_tick(now + Duration::from_secs(4));
    assert!(session.has_vault(), "vault still present after 4s");

    // Tick at 5s — idle deadline fires: Active → Expired
    session.drive_tick(now + Duration::from_secs(5));
    // Tick at 6s — Expired → Locked: vault dropped
    session.drive_tick(now + Duration::from_secs(6));

    assert!(!session.has_vault(), "vault dropped after idle expiry");
    assert!(
        !session.has_credentials(),
        "credentials dropped after idle expiry"
    );

    let events = recording.drain();
    assert!(
        events.iter().any(|e| matches!(e, LockEvent::Locked)),
        "LockEvent::Locked emitted on idle expiry"
    );
}

// Finding #8: prove activity actually defers the idle deadline.
#[test]
fn report_activity_resets_idle_deadline() {
    let env = TestEnv::new();
    let name = "test-vault";
    let password = "test-pass";
    let idle_timeout = Duration::from_secs(5);

    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);
    let session =
        AppSession::for_test_with_timeout(env.paths_clone(), idle_timeout).expect("session");

    session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("unlock");

    let t0 = Instant::now();
    session.drive_tick(t0); // warm-up

    // At t+4s, vault should still be present (idle < 5s).
    session.drive_tick(t0 + Duration::from_secs(4));
    assert!(session.has_vault(), "vault present at t+4s");

    // Report activity at t+4s — this resets the idle deadline to t+9s.
    session.report_activity();
    session.drive_tick(t0 + Duration::from_secs(4));

    // These two ticks are the ones that discriminate. AutoLockController runs
    // Active -> Expired -> Locked across two ticks, so WITHOUT the reset the
    // deadline is t+5s: the t+6s tick would go Expired and the t+7s tick would
    // drop the vault. Observing at t+8s only (as this test previously did)
    // passes in both worlds, making the assertion vacuous — a no-op
    // report_activity would not have failed it.
    session.drive_tick(t0 + Duration::from_secs(6));
    assert!(
        session.has_vault(),
        "vault present at t+6s — without the activity reset this tick is Expired"
    );
    session.drive_tick(t0 + Duration::from_secs(7));
    assert!(
        session.has_vault(),
        "vault present at t+7s — without the activity reset the vault is already dropped here"
    );

    // At t+8s (4s after activity, not 8s after unlock), vault still present.
    session.drive_tick(t0 + Duration::from_secs(8));
    assert!(
        session.has_vault(),
        "vault present at t+8s (activity at t+4s reset the 5s deadline)"
    );

    // At t+9s — Expired (5s past the t+4s activity)
    session.drive_tick(t0 + Duration::from_secs(9));
    // At t+10s — Locked
    session.drive_tick(t0 + Duration::from_secs(10));
    assert!(
        !session.has_vault(),
        "vault dropped at t+10s (5s past t+4s activity + 1s Expired→Locked)"
    );
}

#[test]
fn report_activity_defers_lock_without_taking_session_mutex() {
    let env = TestEnv::new();
    let name = "test-vault";
    let password = "test-pass";
    let (session, _recording) = unlocked_session(&env, name, password);

    let session = std::sync::Arc::new(session);
    let session2 = std::sync::Arc::clone(&session);

    let _guard = session.hold_mutex_for_test();

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        session2.report_activity();
        tx.send(()).expect("send completion signal");
    });

    rx.recv_timeout(Duration::from_millis(500))
        .expect("report_activity should not block when mutex is held");
}

// ---------------------------------------------------------------------------
// Lifecycle grace (D-12) — mobile sessions only (finding #6)
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_grace_semantics_match_design_table() {
    let env = TestEnv::new();
    let name = "test-vault";
    let password = "test-pass";
    let idle_timeout = Duration::from_secs(600);

    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);
    let session =
        AppSession::for_test_mobile_with_timeout(env.paths_clone(), idle_timeout).expect("session");
    let (sink, recording) = RecordingLockSink::new();
    session.lock_events_for_test(sink);
    session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("unlock");
    // Discard the registration snapshot + Unlocked so the final assertion is
    // satisfied only by the grace-expiry Locked.
    let _ = recording.drain();

    let t0 = Instant::now();

    // Inactive → no action
    session.report_lifecycle_state(LifecycleStateDto::Inactive);
    session.drive_tick(t0 + Duration::from_secs(1));
    assert!(session.has_vault(), "inactive: vault still present");

    // Paused → starts 15s grace
    session.report_lifecycle_state(LifecycleStateDto::Paused);
    session.drive_tick(t0 + Duration::from_secs(2));
    assert!(
        session.has_vault(),
        "paused: vault present (grace just started)"
    );

    // 14s into grace — still alive
    session.drive_tick(t0 + Duration::from_secs(16));
    assert!(session.has_vault(), "paused+14s: vault still present");

    // Resumed → cancels grace
    session.report_lifecycle_state(LifecycleStateDto::Resumed);
    session.drive_tick(t0 + Duration::from_secs(17));
    assert!(
        session.has_vault(),
        "resumed: vault still present, grace cancelled"
    );

    // Start grace again with Hidden
    session.report_lifecycle_state(LifecycleStateDto::Hidden);
    session.drive_tick(t0 + Duration::from_secs(18));
    assert!(session.has_vault(), "hidden: grace started");

    // Finding #7: assert at the exact 15s boundary, not one tick later.
    // Grace started at t0+18; 15s later is t0+33. The >= check fires
    // directly (no Expired intermediate like idle), so the vault
    // should be locked after the t0+33 tick.
    session.drive_tick(t0 + Duration::from_secs(33));

    assert!(
        !session.has_vault(),
        "hidden+15s: vault dropped at exact boundary"
    );
    assert!(
        !session.has_credentials(),
        "hidden+15s: credentials dropped"
    );

    let events = recording.drain();
    assert!(
        events.iter().any(|e| matches!(e, LockEvent::Locked)),
        "locked event emitted on lifecycle grace expiry"
    );
}

#[test]
fn lifecycle_detached_locks_immediately() {
    let env = TestEnv::new();
    let name = "test-vault";
    let password = "test-pass";
    let (session, recording) = unlocked_mobile_session(&env, name, password);

    session.report_lifecycle_state(LifecycleStateDto::Detached);
    session.drive_tick(Instant::now());

    assert!(!session.has_vault(), "detached: vault dropped immediately");
    assert!(
        !session.has_credentials(),
        "detached: credentials dropped immediately"
    );
    assert!(
        recording
            .drain()
            .iter()
            .any(|e| matches!(e, LockEvent::Locked)),
        "locked event on detach"
    );
}

// Finding #6: desktop sessions ignore lifecycle states.
#[test]
fn desktop_session_ignores_lifecycle_paused() {
    let env = TestEnv::new();
    let name = "test-vault";
    let password = "test-pass";
    let (session, _recording) = unlocked_session(&env, name, password);

    session.report_lifecycle_state(LifecycleStateDto::Paused);
    let t0 = Instant::now();
    // Tick well past the 15s grace period.
    session.drive_tick(t0 + Duration::from_secs(30));

    assert!(
        session.has_vault(),
        "desktop: vault still present despite Paused lifecycle (lifecycle_enabled=false)"
    );
}

// ---------------------------------------------------------------------------
// Credentials dropped on every lock path
// ---------------------------------------------------------------------------

#[test]
fn session_credentials_dropped_on_every_lock_path() {
    // Path 1: manual lock
    {
        let env = TestEnv::new();
        let (session, _rec) = unlocked_session(&env, "v1", "p1");
        assert!(session.has_credentials(), "pre-lock: credentials present");
        session.lock_now().expect("manual lock");
        assert!(
            !session.has_credentials(),
            "manual lock: credentials dropped"
        );
    }

    // Path 2: idle auto-lock
    {
        let env = TestEnv::new();
        let vault_path = create_test_vault(&env, "v2", "p2");
        register_vault(&env, "v2", &vault_path);
        let session = AppSession::for_test_with_timeout(env.paths_clone(), Duration::from_secs(2))
            .expect("session");
        session
            .unlock("v2".to_string(), "p2".to_string(), None)
            .expect("unlock");
        assert!(session.has_credentials(), "pre-idle: credentials present");
        let t0 = Instant::now();
        session.drive_tick(t0); // warm-up: registers unlock activity
        session.drive_tick(t0 + Duration::from_secs(2)); // Active → Expired
        session.drive_tick(t0 + Duration::from_secs(3)); // Expired → Locked
        assert!(!session.has_credentials(), "idle lock: credentials dropped");
    }

    // Path 3: lifecycle (detached) — mobile session
    {
        let env = TestEnv::new();
        let (session, _rec) = unlocked_mobile_session(&env, "v3", "p3");
        assert!(
            session.has_credentials(),
            "pre-lifecycle: credentials present"
        );
        session.report_lifecycle_state(LifecycleStateDto::Detached);
        session.drive_tick(Instant::now());
        assert!(
            !session.has_credentials(),
            "lifecycle lock: credentials dropped"
        );
    }

    // Path 4: shutdown
    {
        let env = TestEnv::new();
        let (session, _rec) = unlocked_session(&env, "v4", "p4");
        assert!(
            session.has_credentials(),
            "pre-shutdown: credentials present"
        );
        session.shutdown();
        assert!(!session.has_credentials(), "shutdown: credentials dropped");
    }
}

// ---------------------------------------------------------------------------
// Multiple sessions per process
// ---------------------------------------------------------------------------

#[test]
fn multiple_sessions_per_process_harden_once() {
    let env1 = TestEnv::new();
    env1.paths().ensure_exists().expect("ensure dir 1");
    let cfg1 = AppInitConfig {
        state_dir: Some(env1.paths().state_dir().to_string_lossy().to_string()),
        config_dir: None,
    };
    let session1 = init_app(cfg1).expect("session 1 via init_app");

    let env2 = TestEnv::new();
    env2.paths().ensure_exists().expect("ensure dir 2");
    let cfg2 = AppInitConfig {
        state_dir: Some(env2.paths().state_dir().to_string_lossy().to_string()),
        config_dir: None,
    };
    let session2 = init_app(cfg2).expect("session 2 via init_app");

    assert!(!session1.has_vault(), "session 1 starts locked");
    assert!(!session2.has_vault(), "session 2 starts locked");
    session2.shutdown();
    session1.shutdown();
}

// ---------------------------------------------------------------------------
// Shutdown + Drop (findings #1, #3)
// ---------------------------------------------------------------------------

#[test]
fn shutdown_is_idempotent_and_joins_ticker() {
    let env = TestEnv::new();
    env.paths().ensure_exists().expect("ensure dir");
    let cfg = AppInitConfig {
        state_dir: Some(env.paths().state_dir().to_string_lossy().to_string()),
        config_dir: None,
    };
    let session = init_app(cfg).expect("init_app with ticker");

    assert!(
        !session.ticker_is_joined(),
        "ticker handle present before shutdown"
    );

    session.shutdown();
    assert!(
        session.ticker_is_joined(),
        "ticker handle consumed (joined) after first shutdown"
    );

    session.shutdown();
    assert!(session.ticker_is_joined(), "second shutdown is a no-op");
}

#[test]
fn drop_cleans_up_like_shutdown() {
    let env = TestEnv::new();
    env.paths().ensure_exists().expect("ensure dir");
    let name = "test-vault";
    let password = "test-pass";
    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);

    let cfg = AppInitConfig {
        state_dir: Some(env.paths().state_dir().to_string_lossy().to_string()),
        config_dir: None,
    };
    let session = init_app(cfg).expect("init_app");
    session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("unlock");
    assert!(session.has_vault(), "vault present before drop");
    drop(session);
    // If Drop didn't run shutdown, the ticker thread would leak.
    // No assertion possible after drop, but the test proves Drop
    // runs without panicking on an unlocked session.
}

// ---------------------------------------------------------------------------
// Contention (vault_holder binary)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires vault_holder binary: HIDLINS_VAULT_HOLDER=target/debug/vault_holder (built by hidlins-core)"]
fn contended_vault_surfaces_holder_pid() {
    let env = TestEnv::new();
    let name = "test-vault";
    let password = "test-pass";
    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);

    let held_signal = env.tempdir().join("held");
    let release_signal = env.tempdir().join("release");
    let vault_holder = std::env::var("HIDLINS_VAULT_HOLDER").unwrap_or_else(|_| {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        workspace
            .join("target/debug/vault_holder")
            .to_string_lossy()
            .into_owned()
    });
    // vault_holder takes the password as argv — this is the existing
    // hidlins-core test-binary interface. The value is a test-only
    // constant, never a real secret.
    let mut holder = std::process::Command::new(&vault_holder)
        .arg("hold")
        .arg(&vault_path)
        .arg(password)
        .arg(&held_signal)
        .arg(&release_signal)
        .spawn()
        .expect("spawn vault_holder");

    let _holder_pid = holder.id();

    let deadline = Instant::now() + Duration::from_secs(10);
    while !held_signal.exists() {
        assert!(
            Instant::now() < deadline,
            "vault_holder did not signal 'held' within 10s"
        );
        thread::sleep(Duration::from_millis(50));
    }

    let session = AppSession::for_test(env.paths_clone()).expect("session");
    let result = session.unlock(name.to_string(), password.to_string(), None);

    // Advisory flock does not expose the holder PID at the kernel level,
    // so holder_pid is None. The VaultContended variant itself is the
    // meaningful assertion — it proves the advisory lock blocked the
    // second open. (The vault_holder process PID is `holder_pid` above
    // but we can't cross-check it from flock.)
    assert!(
        matches!(result, Err(HidlinsApiError::VaultContended { .. })),
        "expected VaultContended, got: {result:?}"
    );

    std::fs::write(&release_signal, "release").expect("signal release");
    holder.wait().expect("wait for holder exit");
}

// ---------------------------------------------------------------------------
// Lock / shutdown racing an in-flight unlock. The session mutex is NOT held
// across the KDF (unlock phases 1–3); a lock trigger that fires during the
// KDF must supersede the unlock — the opened vault is discarded, never
// presented, and no `Unlocked` event may reach a subscriber.
// ---------------------------------------------------------------------------

#[test]
fn lock_now_during_inflight_unlock_discards_the_opened_vault() {
    let env = TestEnv::new();
    let name = "race-lock";
    let password = "test-pass";
    let (session, recording) = session_with_vault_and_sink(&env, name, password);
    let _ = recording.drain(); // discard the registration snapshot

    let result = session.unlock_with_hook(name, password, None, || {
        assert!(session.is_unlocking(), "hook runs inside the unlock window");
        // lock_now must not block behind the KDF and must win over the
        // in-flight unlock.
        session
            .lock_now()
            .expect("lock_now during in-flight unlock");
    });

    assert!(
        matches!(result, Err(HidlinsApiError::VaultLocked)),
        "superseded unlock reports VaultLocked: {result:?}"
    );
    assert!(
        !session.has_vault(),
        "opened vault discarded, not committed"
    );
    assert!(
        !session.has_credentials(),
        "credentials discarded (zeroized on drop)"
    );
    assert!(!session.is_unlocking(), "in-flight claim cleared");
    let events = recording.drain();
    assert!(
        !events.iter().any(|e| matches!(e, LockEvent::Unlocked)),
        "no Unlocked may be emitted for a superseded unlock: {events:?}"
    );

    // The pending flag was consumed: a later unlock is unaffected.
    session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("subsequent unlock succeeds");
    assert!(session.has_vault());
}

#[test]
fn shutdown_during_inflight_unlock_discards_the_opened_vault() {
    let env = TestEnv::new();
    let name = "race-shutdown";
    let password = "test-pass";
    let (session, recording) = session_with_vault_and_sink(&env, name, password);
    let _ = recording.drain();

    let result = session.unlock_with_hook(name, password, None, || {
        session.shutdown();
    });

    assert!(
        matches!(result, Err(HidlinsApiError::Internal { .. })),
        "unlock racing shutdown reports the dead session: {result:?}"
    );
    assert!(session.is_dead(), "session dead after shutdown");
    assert!(!session.has_vault(), "opened vault discarded");
    assert!(!session.has_credentials(), "credentials discarded");
    // The event-side assertion is the stream CLOSE: shutdown drops the sink
    // before phase 3 runs, so nothing pushed afterwards could reach the
    // recorder anyway — asserting "no Unlocked arrived" would be vacuous.
    let _ = recording.drain();
    assert!(
        recording.is_closed(),
        "shutdown mid-unlock closes the event stream"
    );
}

#[test]
fn concurrent_unlock_while_one_is_inflight_errors_without_disturbing_it() {
    let env = TestEnv::new();
    let name = "race-double-unlock";
    let password = "test-pass";
    let (session, _recording) = session_with_vault_and_sink(&env, name, password);

    session
        .unlock_with_hook(name, password, None, || {
            let second = session.unlock(name.to_string(), password.to_string(), None);
            assert!(
                matches!(second, Err(HidlinsApiError::Internal { .. })),
                "second unlock during the first's KDF must error: {second:?}"
            );
        })
        .expect("first unlock commits normally");

    assert!(session.has_vault(), "first unlock unaffected by the second");
}

#[test]
fn lifecycle_lock_trigger_during_inflight_unlock_marks_it_pending() {
    let env = TestEnv::new();
    let name = "race-lifecycle";
    let password = "test-pass";
    let session = mobile_session_with_vault(&env, name, password);

    let result = session.unlock_with_hook(name, password, None, || {
        // Detached fires an immediate lifecycle lock while the unlock is in
        // flight; with no vault to drop it must mark the unlock pending.
        session.report_lifecycle_state(LifecycleStateDto::Detached);
        session.drive_tick(Instant::now());
    });

    assert!(
        matches!(result, Err(HidlinsApiError::VaultLocked)),
        "lifecycle-superseded unlock reports VaultLocked: {result:?}"
    );
    assert!(!session.has_vault(), "opened vault discarded");
    assert!(!session.has_credentials(), "credentials discarded");
}

#[test]
fn failed_unlock_consumes_a_pending_lock_and_clears_the_claim() {
    let env = TestEnv::new();
    let name = "race-open-failure";
    let password = "test-pass";
    let (session, _recording) = session_with_vault_and_sink(&env, name, password);

    let result = session.unlock_with_hook(name, "wrong-password", None, || {
        session.lock_now().expect("lock_now during failing unlock");
    });

    // The open failure is the more informative error and wins.
    assert!(
        matches!(result, Err(HidlinsApiError::AuthenticationFailed)),
        "open failure wins over the pending lock: {result:?}"
    );
    assert!(!session.is_unlocking(), "claim cleared on the failure path");

    // The stale pending flag was consumed by the failed attempt — it must
    // not abort this one.
    session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("later unlock unaffected by stale lock_pending");
}

// ---------------------------------------------------------------------------
// Lock-event stream protocol: authoritative snapshot on subscribe, definitive
// close on shutdown / dead-session subscribe.
// ---------------------------------------------------------------------------

#[test]
fn lock_event_subscription_starts_with_an_authoritative_snapshot() {
    let env = TestEnv::new();
    let name = "snapshot-vault";
    let password = "test-pass";
    let (session, recording) = session_with_vault_and_sink(&env, name, password);

    // Locked session → snapshot says Locked.
    assert!(
        matches!(recording.drain().as_slice(), [LockEvent::Locked]),
        "locked-session subscription snapshots Locked"
    );

    session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("unlock");

    // A fresh subscription on an unlocked session → snapshot says Unlocked.
    let (sink2, recording2) = RecordingLockSink::new();
    session.lock_events_for_test(sink2);
    assert!(
        matches!(recording2.drain().as_slice(), [LockEvent::Unlocked]),
        "unlocked-session subscription snapshots Unlocked"
    );

    // Replacement closed the first sink so its listener terminates cleanly
    // (drain the Unlocked it received before the replacement first —
    // `is_closed` is only meaningful on an empty buffer).
    assert!(
        matches!(recording.drain().as_slice(), [LockEvent::Unlocked]),
        "first subscription saw the unlock before being replaced"
    );
    assert!(
        recording.is_closed(),
        "replaced subscription observes end-of-stream"
    );
}

#[test]
fn shutdown_closes_the_lock_event_stream() {
    let env = TestEnv::new();
    let (session, recording) = unlocked_session(&env, "close-vault", "test-pass");

    session.shutdown();

    let events = recording.drain();
    assert!(
        events.iter().any(|e| matches!(e, LockEvent::Locked)),
        "shutdown of an unlocked session emits Locked first: {events:?}"
    );
    assert!(
        recording.is_closed(),
        "shutdown drops the sink → subscriber sees end-of-stream"
    );
}

#[test]
fn subscribing_on_a_dead_session_snapshots_locked_then_closes() {
    let env = TestEnv::new();
    let (session, _recording) = unlocked_session(&env, "dead-vault", "test-pass");
    session.shutdown();

    let (sink, recording) = RecordingLockSink::new();
    session.lock_events_for_test(sink);

    assert!(
        matches!(recording.drain().as_slice(), [LockEvent::Locked]),
        "dead-session subscription still gets the authoritative Locked"
    );
    assert!(
        recording.is_closed(),
        "dead-session subscription is closed immediately"
    );
}

#[test]
fn plain_tick_during_inflight_unlock_does_not_abort_it() {
    // Regression guard for the controller_locked / lifecycle_lock split in
    // tick_inner: with no vault open the controller idles in Locked, so its
    // verdict during an unlock's KDF window is meaningless. If a future
    // change fed that verdict into the unlocking branch, the 1 Hz production
    // ticker would abort essentially every real unlock (the KDF is tuned to
    // ~1s). Only affirmative triggers (lifecycle, drained lock events) may
    // mark an in-flight unlock pending.
    let env = TestEnv::new();
    let name = "race-plain-tick";
    let password = "test-pass";
    let (session, _recording) = session_with_vault_and_sink(&env, name, password);

    session
        .unlock_with_hook(name, password, None, || {
            session.drive_tick(Instant::now());
            session.drive_tick(Instant::now() + Duration::from_secs(1));
        })
        .expect("idle-Locked controller verdict must not supersede the unlock");

    assert!(session.has_vault(), "unlock committed despite plain ticks");
}

/// One-shot event source: sends a single `OsLock`, signals it did, exits.
/// The signal channel makes the test deterministic — the hook blocks on it,
/// so by the time `drive_tick` runs the event is guaranteed to be queued.
struct OneShotLockSource {
    sent_tx: mpsc::Sender<()>,
}

impl hidlins_security::OsLockEventSource for OneShotLockSource {
    fn run(
        self: Box<Self>,
        sender: mpsc::Sender<hidlins_security::SecurityEvent>,
    ) -> Result<(), hidlins_security::SecurityError> {
        sender
            .send(hidlins_security::SecurityEvent::OsLock {
                reason: hidlins_security::OsLockReason::Screensaver,
            })
            .ok();
        let _ = self.sent_tx.send(());
        Ok(())
    }

    fn name(&self) -> &'static str {
        "one-shot-test"
    }

    fn shutdown_handle(&mut self) -> hidlins_security::ShutdownHandle {
        hidlins_security::ShutdownHandle::new(|| {})
    }
}

#[test]
fn drained_os_lock_event_during_inflight_unlock_marks_it_pending() {
    // The controller drains OS lock events inside tick(); while an unlock's
    // KDF is in flight that drain changes no controller-visible state (it is
    // already Locked), so the drained event itself must mark the unlock
    // pending — otherwise a Super+L mid-KDF would be consumed and the vault
    // would commit behind the OS lock screen once T1.8 wires real sources.
    // The flag contract is pinned in hidlins-security
    // (drained_lock_events_are_observable_even_from_locked_state); this
    // covers the hidlins-api wiring end to end through a real event source.
    let env = TestEnv::new();
    let name = "race-os-lock";
    let password = "test-pass";
    let (session, recording) = session_with_vault_and_sink(&env, name, password);
    let _ = recording.drain();

    let (sent_tx, sent_rx) = mpsc::channel();
    let result = session.unlock_with_hook(name, password, None, || {
        session.attach_lock_source_for_test(OneShotLockSource { sent_tx });
        sent_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("source signals after queueing the OsLock");
        session.drive_tick(Instant::now());
    });

    assert!(
        matches!(result, Err(HidlinsApiError::VaultLocked)),
        "drained lock event supersedes the in-flight unlock: {result:?}"
    );
    assert!(!session.has_vault(), "opened vault discarded");
    assert!(!session.has_credentials(), "credentials discarded");
    let events = recording.drain();
    assert!(
        !events.iter().any(|e| matches!(e, LockEvent::Unlocked)),
        "no Unlocked for a superseded unlock: {events:?}"
    );
}

#[test]
fn os_lock_event_while_unlocked_drops_vault_and_credentials() {
    let env = TestEnv::new();
    let name = "os-lock";
    let password = "test-pass";
    let (session, recording) = session_with_vault_and_sink(&env, name, password);
    let _ = recording.drain();

    session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("unlock");
    let _ = recording.drain();

    assert!(
        session.has_vault(),
        "vault must be open before OS lock event"
    );
    assert!(
        session.has_credentials(),
        "credentials must be present before OS lock event"
    );

    let (sent_tx, sent_rx) = mpsc::channel();
    session.attach_lock_source_for_test(OneShotLockSource { sent_tx });
    sent_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("source signals after queueing the OsLock");
    session.drive_tick(Instant::now());

    assert!(
        !session.has_vault(),
        "vault must be dropped after OS lock event"
    );
    assert!(
        !session.has_credentials(),
        "credentials must be dropped after OS lock event"
    );

    let events = recording.drain();
    assert!(
        events.iter().any(|e| matches!(e, LockEvent::Locked)),
        "LockEvent::Locked must be pushed after OS lock event: {events:?}"
    );
}

#[test]
fn phase_2_panic_clears_the_unlock_claim() {
    // The ClearUnlockingOnDrop armed path: frb catches panics on its pool
    // threads, so an unwind out of phase 2 must not leave the `unlocking`
    // claim set — that would brick every later unlock with "already in
    // progress".
    let env = TestEnv::new();
    let name = "race-panic";
    let password = "test-pass";
    let (session, _recording) = session_with_vault_and_sink(&env, name, password);

    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = session.unlock_with_hook(name, password, None, || {
            panic!("simulated phase-2 panic");
        });
    }));
    assert!(unwound.is_err(), "the hook panic must propagate");

    assert!(!session.is_unlocking(), "claim cleared by the drop guard");
    session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("session usable again after the unwound unlock");
}

#[test]
fn shutdown_during_failing_unlock_surfaces_the_open_error() {
    // Phase-3 precedence: the open failure is consumed before the dead-state
    // check, so the caller sees the more informative AuthenticationFailed
    // rather than the generic dead-session error. Pinned because a reorder
    // of two phase-3 blocks would silently change the error a UI shows.
    let env = TestEnv::new();
    let name = "race-shutdown-authfail";
    let password = "test-pass";
    let (session, _recording) = session_with_vault_and_sink(&env, name, password);

    let result = session.unlock_with_hook(name, "wrong-password", None, || {
        session.shutdown();
    });

    assert!(
        matches!(result, Err(HidlinsApiError::AuthenticationFailed)),
        "open failure wins over the dead-session error: {result:?}"
    );
    assert!(session.is_dead());
}

// ---------------------------------------------------------------------------
// Sync API — security marker probe (T1.6): plaintext secret must never
// appear in vaults.toml after configure_sync seals it.
// ---------------------------------------------------------------------------

#[test]
fn configure_sync_seals_secret_and_plaintext_never_reaches_disk() {
    use hidlins_api::dto::S3ConfigDto;

    let env = TestEnv::new();
    let name = "sync-marker";
    let password = "sync-test-pass";
    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);

    let session = AppSession::for_test(env.paths_clone()).expect("create session");
    session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("unlock");

    let marker_secret = "MARKER-s3cret-K3Y-DO-NOT-LEAK";

    let cfg = S3ConfigDto {
        bucket: "test-bucket".to_string(),
        key: "test.kdbx".to_string(),
        region: "us-east-1".to_string(),
        endpoint: None,
        path_style: false,
        access_key_id: "AKIAIOSFODNN7EXAMPLE".to_string(),
        secret_access_key: marker_secret.to_string(),
    };
    session
        .configure_sync(cfg)
        .expect("configure_sync must succeed");

    // Read the on-disk vaults.toml and assert:
    // 1. The plaintext secret is NOT present.
    // 2. The access_key_id IS present (proves the write actually happened).
    let contents =
        std::fs::read_to_string(env.paths().vaults_toml()).expect("vaults.toml must be readable");

    assert!(
        !contents.contains(marker_secret),
        "plaintext secret must not appear in vaults.toml;\n\
         got:\n{contents}"
    );
    assert!(
        contents.contains("AKIAIOSFODNN7EXAMPLE"),
        "access_key_id must be present in vaults.toml (proves the write landed);\n\
         got:\n{contents}"
    );
}

#[test]
fn sync_status_reports_configured_after_configure_sync() {
    use hidlins_api::dto::S3ConfigDto;

    let env = TestEnv::new();
    let name = "sync-status";
    let password = "sync-test-pass";
    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);

    let session = AppSession::for_test(env.paths_clone()).expect("create session");
    session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("unlock");

    // Before configuring: not configured.
    let status = session.sync_status().expect("sync_status");
    assert!(!status.configured, "not configured before configure_sync");
    assert!(!status.in_flight, "in_flight always false in sync version");

    let cfg = S3ConfigDto {
        bucket: "test-bucket".to_string(),
        key: "test.kdbx".to_string(),
        region: "us-east-1".to_string(),
        endpoint: None,
        path_style: false,
        access_key_id: "AKIAIOSFODNN7EXAMPLE".to_string(),
        secret_access_key: "dummy-secret".to_string(),
    };
    session
        .configure_sync(cfg)
        .expect("configure_sync must succeed");

    // After configuring: configured.
    let status = session.sync_status().expect("sync_status");
    assert!(status.configured, "configured after configure_sync");
}

#[test]
fn clear_sync_config_removes_sync_block() {
    use hidlins_api::dto::S3ConfigDto;

    let env = TestEnv::new();
    let name = "sync-clear";
    let password = "sync-test-pass";
    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);

    let session = AppSession::for_test(env.paths_clone()).expect("create session");
    session
        .unlock(name.to_string(), password.to_string(), None)
        .expect("unlock");

    // Configure sync.
    let cfg = S3ConfigDto {
        bucket: "test-bucket".to_string(),
        key: "test.kdbx".to_string(),
        region: "us-east-1".to_string(),
        endpoint: None,
        path_style: false,
        access_key_id: "AKIAIOSFODNN7EXAMPLE".to_string(),
        secret_access_key: "dummy-secret".to_string(),
    };
    session
        .configure_sync(cfg)
        .expect("configure_sync must succeed");
    assert!(
        session.sync_status().expect("status").configured,
        "configured before clear"
    );

    // Clear sync config — does not require an unlocked vault.
    session
        .clear_sync_config(name.to_string())
        .expect("clear_sync_config must succeed");

    // Verify it's gone.
    let status = session.sync_status().expect("sync_status after clear");
    assert!(
        !status.configured,
        "sync config removed after clear_sync_config"
    );

    // Verify the vault entry itself still exists (only the sync block was removed).
    let contents =
        std::fs::read_to_string(env.paths().vaults_toml()).expect("vaults.toml must be readable");
    assert!(
        contents.contains("sync-clear"),
        "vault entry still registered after clearing sync config"
    );
    assert!(
        !contents.contains("[vault.sync]"),
        "sync block removed from vaults.toml"
    );
}

#[test]
fn sync_api_methods_reject_locked_vault() {
    use hidlins_api::dto::S3ConfigDto;

    let env = TestEnv::new();
    let name = "sync-locked";
    let password = "sync-test-pass";
    let vault_path = create_test_vault(&env, name, password);
    register_vault(&env, name, &vault_path);

    let session = AppSession::for_test(env.paths_clone()).expect("create session");
    // Do NOT unlock — vault stays locked.

    let cfg = S3ConfigDto {
        bucket: "b".to_string(),
        key: "k".to_string(),
        region: "r".to_string(),
        endpoint: None,
        path_style: false,
        access_key_id: "A".to_string(),
        secret_access_key: "S".to_string(),
    };
    assert!(
        matches!(
            session.configure_sync(cfg),
            Err(HidlinsApiError::VaultLocked)
        ),
        "configure_sync must reject a locked vault"
    );
    assert!(
        matches!(session.sync_status(), Err(HidlinsApiError::VaultLocked)),
        "sync_status must reject a locked vault"
    );
    assert!(
        matches!(session.sync_now(), Err(HidlinsApiError::VaultLocked)),
        "sync_now must reject a locked vault"
    );
}
