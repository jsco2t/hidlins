use super::*;

fn set_override(app: &mut App, name: &str, seconds: u64) {
    app.registry_mut()
        .unwrap()
        .update_registered_extra(name, |extra| {
            VaultLockConfig::apply_idle_timeout(extra, Some(seconds));
        })
        .unwrap();
}

fn switch_to_second(app: &mut App) {
    app.handle_event(&key_ctrl('l'));
    app.handle_event(&key(' '));
    app.handle_event(&key('j'));
    unlock(app, PASSWORD);
    assert_eq!(app.selected_vault.as_deref(), Some("beta"));
}

#[test]
fn switching_to_unconfigured_vault_restores_frontend_deadline() {
    let (_dir, mut app) = fixture_app(&["alpha", "beta"], Duration::from_secs(2));
    set_override(&mut app, "alpha", 900);
    unlock(&mut app, PASSWORD);
    assert_eq!(
        app.controller.config().idle_timeout,
        Duration::from_secs(900)
    );
    switch_to_second(&mut app);
    let now = Instant::now();
    app.tick(now + Duration::from_secs(3));
    app.tick(now + Duration::from_secs(4));
    assert!(matches!(app.phase, Phase::LockScreen));
    assert!(app.vault().is_none());
    assert_eq!(app.current_auto_lock_seconds(), 2);
}

#[test]
fn switching_to_a_different_override_keeps_its_deadline() {
    let (_dir, mut app) = fixture_app(&["alpha", "beta"], Duration::from_secs(2));
    set_override(&mut app, "alpha", 900);
    set_override(&mut app, "beta", 10);
    unlock(&mut app, PASSWORD);
    switch_to_second(&mut app);
    let now = Instant::now();
    app.tick(now + Duration::from_secs(3));
    app.tick(now + Duration::from_secs(4));
    assert!(
        app.vault().is_some(),
        "the second override exceeds the default"
    );
    app.tick(now + Duration::from_secs(11));
    app.tick(now + Duration::from_secs(12));
    assert!(matches!(app.phase, Phase::LockScreen));
    assert!(app.vault().is_none());
}

#[test]
fn startup_failure_honors_an_already_pending_lock() {
    let (_dir, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    app.lock_pending = true;
    integrate(
        &mut app,
        Err(SyncError::NotConfigured),
        SyncTrigger::Startup,
    );
    assert!(matches!(app.phase, Phase::LockScreen));
    assert!(app.vault().is_none());
    assert!(!app.lock_pending);
}

#[test]
fn ordinary_startup_failure_preserves_offline_workspace() {
    let (_dir, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    integrate(
        &mut app,
        Err(SyncError::NotConfigured),
        SyncTrigger::Startup,
    );
    assert!(matches!(app.phase, Phase::Workspace));
    assert!(app.vault().is_some());
    assert!(!app.lock_pending);
}

#[test]
fn successful_startup_sync_honors_an_already_pending_lock() {
    let (_dir, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    app.lock_pending = true;
    integrate(
        &mut app,
        Ok(SyncOutcome::AlreadyInSync),
        SyncTrigger::Startup,
    );
    assert!(matches!(app.phase, Phase::LockScreen));
    assert!(app.vault().is_none());
    assert!(!app.lock_pending);
}
