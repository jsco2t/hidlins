use super::*;
use std::io::{BufRead, Write};
use std::sync::mpsc::{self, Receiver, Sender};

struct OneLock {
    trigger: Receiver<()>,
    shutdown: Sender<()>,
    delivered: Sender<()>,
}

impl hidlins_security::OsLockEventSource for OneLock {
    fn name(&self) -> &'static str {
        "unlock-fixture"
    }

    fn shutdown_handle(&mut self) -> hidlins_security::ShutdownHandle {
        let shutdown = self.shutdown.clone();
        hidlins_security::ShutdownHandle::new(move || {
            let _ = shutdown.send(());
        })
    }

    fn run(
        self: Box<Self>,
        sender: Sender<hidlins_security::SecurityEvent>,
    ) -> Result<(), hidlins_security::SecurityError> {
        let _ = self.trigger.recv();
        let _ = sender.send(hidlins_security::SecurityEvent::OsLock {
            reason: hidlins_security::OsLockReason::Screensaver,
        });
        let _ = self.delivered.send(());
        Ok(())
    }
}

fn one_lock(app: &mut App) -> (Sender<()>, Receiver<()>) {
    let (trigger, receiver) = mpsc::channel();
    let (delivered, observed) = mpsc::channel();
    app.controller
        .attach_event_source(OneLock {
            trigger: receiver,
            shutdown: trigger.clone(),
            delivered,
        })
        .unwrap();
    (trigger, observed)
}

#[test]
fn os_lock_discards_partially_entered_master_input() {
    let (_directory, mut app) = single_vault_app();
    let (trigger, delivered) = one_lock(&mut app);
    select_and_type(&mut app, "synthetic partial input");
    trigger.send(()).unwrap();
    delivered.recv_timeout(Duration::from_secs(1)).unwrap();
    app.tick(Instant::now());
    assert!(matches!(app.phase, Phase::LockScreen));
    assert!(app.vault().is_none());
}

#[test]
fn os_lock_queued_during_unlock_cannot_publish_workspace() {
    let (_directory, mut app) = single_vault_app();
    let (trigger, delivered) = one_lock(&mut app);
    app.before_unlock_test_hook = Some(Box::new(move || {
        trigger.send(()).unwrap();
        delivered.recv_timeout(Duration::from_secs(1)).unwrap();
    }));
    unlock(&mut app, PASSWORD);
    assert!(matches!(app.phase, Phase::LockScreen));
    assert!(app.vault().is_none());
    assert!(!app.is_syncing());
    assert!(app.server.is_none());
}

#[test]
fn ordinary_unlock_stays_available_without_a_lock_event() {
    let (_directory, mut app) = single_vault_app();
    unlock(&mut app, PASSWORD);
    app.tick(Instant::now());
    assert!(matches!(app.phase, Phase::Workspace));
    assert!(app.vault().is_some());
}

fn mark(message: &str) {
    println!("{message}");
    std::io::stdout().flush().unwrap();
}

#[test]
#[ignore = "owned-process helper; outer native tests invoke it with an isolated HOME"]
fn native_process_helper() {
    let Ok(mode) = std::env::var("HIDLINS_TUI_NATIVE_TEST_MODE") else {
        return;
    };
    let mut app = App::new(&Args::default()).unwrap();
    if mode == "prompt" {
        select_and_type(&mut app, "synthetic partial input");
    } else {
        unlock(&mut app, PASSWORD);
        assert!(app.vault().is_some(), "fixture vault must unlock");
    }
    let (sender, commands) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            if sender.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    mark("READY:1");
    let mut generation = 1;
    let mut was_locked = false;
    let mut reported_notice = false;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        app.tick(Instant::now());
        if matches!(app.phase, Phase::LockScreen) && !was_locked {
            assert!(app.vault().is_none());
            mark(&format!("LOCKED:{generation}"));
            was_locked = true;
        }
        if !reported_notice {
            if let Some(message) = app
                .status
                .as_deref()
                .filter(|message| message.contains("OS lock monitoring unavailable"))
            {
                mark(&format!("NOTICE:{message}"));
                reported_notice = true;
            }
        }
        match commands.recv_timeout(Duration::from_millis(5)) {
            Ok(command) if command == "quit" => {
                app.do_quit();
                mark("STOPPED");
                break;
            }
            Ok(command) if command == "unlock" => {
                if matches!(app.phase, Phase::LockScreen) {
                    app.handle_event(&key(' '));
                }
                unlock(&mut app, PASSWORD);
                assert!(app.vault().is_some());
                generation += 1;
                was_locked = false;
                mark(&format!("READY:{generation}"));
            }
            Ok(command) if command == "timeout" => {
                app.set_auto_lock(900);
                mark("CONFIGURED");
            }
            Ok(command) if command == "state" => {
                mark(if matches!(app.phase, Phase::Workspace) {
                    "STATE:workspace"
                } else {
                    "STATE:locked"
                });
            }
            Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                app.do_quit();
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "owned native helper exceeded its lifetime"
        );
    }
    drop(app);
    reader.join().unwrap();
}

#[cfg(unix)]
struct NativeChild {
    child: std::process::Child,
    stdout: PathBuf,
    stderr: PathBuf,
    _directory: TempDir,
}

#[cfg(unix)]
impl NativeChild {
    fn start(mode: &str, bus: Option<&str>) -> Self {
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};

        let directory = tempfile::tempdir().unwrap();
        let paths = HidlinsPaths::with_state_dir(directory.path().join(".local/state/hidlins"));
        let path = directory.path().join("personal.kdbx");
        drop(
            Vault::create(
                &path,
                &MasterPassword::new(PASSWORD.to_string()),
                None,
                fast_kdf(),
                NoRecoveryConfirmed::yes(),
            )
            .unwrap(),
        );
        let mut registry = VaultRegistry::with_paths(paths);
        registry
            .register_and_save(RegisteredVault {
                name: "personal".to_string(),
                path,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                keyfile_path: None,
                extra: toml::Table::new(),
            })
            .unwrap();
        let stdout = directory.path().join("native.stdout");
        let stderr = directory.path().join("native.stderr");
        let missing_bus = format!(
            "unix:path={}",
            directory.path().join("absent-bus").display()
        );
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "app::tests::native_lock_regressions::native_process_helper",
                "--nocapture",
            ])
            .env("HOME", directory.path())
            .env("XDG_CONFIG_HOME", directory.path().join("config"))
            .env_remove("XDG_SESSION_ID")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env("XDG_RUNTIME_DIR", directory.path())
            .env("DBUS_SYSTEM_BUS_ADDRESS", bus.unwrap_or(&missing_bus))
            .env("HIDLINS_TUI_NATIVE_TEST_MODE", mode)
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(std::fs::File::create(&stdout).unwrap())
            .stderr(std::fs::File::create(&stderr).unwrap())
            .spawn()
            .unwrap();
        let mut result = Self {
            child,
            stdout,
            stderr,
            _directory: directory,
        };
        result.wait_for("READY:1", || {});
        result
    }

    fn output(&self) -> String {
        std::fs::read_to_string(&self.stdout).unwrap()
    }

    fn wait_for(&mut self, marker: &str, mut emit: impl FnMut()) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !self.output().contains(marker) {
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "native helper exited: {} {}",
                self.output(),
                std::fs::read_to_string(&self.stderr).unwrap()
            );
            assert!(
                Instant::now() < deadline,
                "missing {marker}: {} {}",
                self.output(),
                std::fs::read_to_string(&self.stderr).unwrap()
            );
            emit();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn command(&mut self, command: &str) {
        writeln!(self.child.stdin.as_mut().unwrap(), "{command}").unwrap();
    }

    fn stop(&mut self) {
        self.command("quit");
        drop(self.child.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success(), "{}", self.output());
                break;
            }
            assert!(
                Instant::now() < deadline,
                "native helper shutdown exceeded budget"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(self.output().contains("STOPPED"));
    }
}

#[cfg(unix)]
impl Drop for NativeChild {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[cfg(unix)]
#[allow(unsafe_code)]
mod owned_process_signals {
    use std::process::Child;

    pub(super) fn signal(child: &Child, signal: libc::c_int) {
        let pid = libc::pid_t::try_from(child.id()).unwrap();
        // SAFETY: only this fixture's unreaped owned child is addressed.
        assert_eq!(unsafe { libc::kill(pid, signal) }, 0);
    }

    pub(super) fn stopped(child: &Child) -> bool {
        // SAFETY: siginfo_t is a plain C record, writable for this call.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // SAFETY: WNOWAIT observes only this owned child's stop event and
        // preserves Child's wait/reaping state, so its PID cannot be reused.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                &raw mut info,
                libc::WNOHANG | libc::WSTOPPED | libc::WNOWAIT,
            )
        };
        result == 0 && info.si_code == libc::CLD_STOPPED
    }
}

#[cfg(unix)]
#[test]
fn actual_terminal_suspend_resume_locks_the_tui_vault() {
    let mut child = NativeChild::start("unlocked", None);
    owned_process_signals::signal(&child.child, libc::SIGTSTP);
    let deadline = Instant::now() + Duration::from_secs(2);
    while !owned_process_signals::stopped(&child.child) {
        assert!(Instant::now() < deadline, "owned TUI did not stop");
        std::thread::sleep(Duration::from_millis(5));
    }
    owned_process_signals::signal(&child.child, libc::SIGCONT);
    child.wait_for("LOCKED:1", || {});
    child.stop();
}

#[cfg(target_os = "linux")]
use super::super::native_test_bus::{self, LogindBus};

#[cfg(target_os = "linux")]
fn emit_lock(bus: &LogindBus) {
    zbus::block_on(bus.owner.emit_signal(
        None::<&str>,
        native_test_bus::SESSION,
        "org.freedesktop.login1.Session",
        "Lock",
        &(),
    ))
    .unwrap();
}

#[cfg(target_os = "linux")]
fn emit_sleep(bus: &LogindBus, sleeping: bool) {
    zbus::block_on(bus.owner.emit_signal(
        None::<&str>,
        native_test_bus::MANAGER,
        "org.freedesktop.login1.Manager",
        "PrepareForSleep",
        &(sleeping,),
    ))
    .unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn actual_private_logind_lock_locks_the_tui_vault() {
    let bus = LogindBus::new();
    let mut child = NativeChild::start("unlocked", Some(&bus.address));
    child.wait_for("LOCKED:1", || emit_lock(&bus));
    child.stop();
}

#[cfg(target_os = "linux")]
#[test]
fn actual_private_logind_sleep_locks_the_tui_vault() {
    let bus = LogindBus::new();
    let mut child = NativeChild::start("unlocked", Some(&bus.address));
    child.wait_for("LOCKED:1", || emit_sleep(&bus, true));
    child.stop();
}

#[cfg(target_os = "linux")]
#[test]
fn actual_private_logind_lock_clears_a_partial_password_prompt() {
    let bus = LogindBus::new();
    let mut child = NativeChild::start("prompt", Some(&bus.address));
    child.wait_for("LOCKED:1", || emit_lock(&bus));
    child.stop();
}

#[cfg(target_os = "linux")]
#[test]
fn actual_private_logind_wake_preserves_an_unlocked_tui() {
    let bus = LogindBus::new();
    let mut child = NativeChild::start("unlocked", Some(&bus.address));
    for _ in 0..10 {
        emit_sleep(&bus, false);
        std::thread::sleep(Duration::from_millis(10));
    }
    child.command("state");
    child.wait_for("STATE:workspace", || {});
    assert!(!child.output().contains("LOCKED:"));
    child.stop();
}

#[cfg(target_os = "linux")]
#[test]
fn native_lock_sources_survive_timeout_change_and_fresh_unlock() {
    let bus = LogindBus::new();
    let mut child = NativeChild::start("unlocked", Some(&bus.address));
    child.command("timeout");
    child.wait_for("CONFIGURED", || {});
    child.wait_for("LOCKED:1", || emit_lock(&bus));
    // Drain already-delivered notifications before a fresh authenticated intent.
    std::thread::sleep(Duration::from_millis(50));
    child.command("unlock");
    child.wait_for("READY:2", || {});
    child.wait_for("LOCKED:2", || emit_lock(&bus));
    child.stop();
}

#[cfg(target_os = "linux")]
#[test]
fn unavailable_native_backend_warns_and_keeps_offline_tui_available() {
    let mut child = NativeChild::start("unlocked", None);
    child.wait_for("NOTICE:OS lock monitoring unavailable (logind)", || {});
    child.command("state");
    child.wait_for("STATE:workspace", || {});
    child.stop();
}

#[cfg(target_os = "linux")]
#[test]
fn stalled_native_bus_authentication_does_not_prevent_tui_shutdown() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("silent-system-bus");
    let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut child = NativeChild::start("unlocked", Some(&format!("unix:path={}", path.display())));
    let deadline = Instant::now() + Duration::from_secs(2);
    let _stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("owned Unix listener failed: {error}"),
        }
        assert!(
            Instant::now() < deadline,
            "native backend never attempted the owned bus"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    child.stop();
}
