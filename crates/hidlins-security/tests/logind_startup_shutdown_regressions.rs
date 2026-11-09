//! Cancellation must cover bus authentication, before signal streams exist.
#![cfg(all(target_os = "linux", feature = "logind"))]

use std::io::{BufRead, Read, Write};
use std::os::unix::net::UnixListener;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use hidlins_security::{AutoLockConfig, AutoLockController, LogindSource};

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn controller_drop_cancels_pending_bus_authentication() {
    let directory = tempfile::tempdir().unwrap();
    let socket = directory.path().join("silent-bus");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut child = ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "startup_shutdown_child",
                "--ignored",
                "--nocapture",
            ])
            .env("HIDLINS_STALLED_BUS_CHILD", "1")
            .env(
                "DBUS_SYSTEM_BUS_ADDRESS",
                format!("unix:path={}", socket.display()),
            )
            .stdin(Stdio::piped())
            .spawn()
            .unwrap(),
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut peer = loop {
        match listener.accept() {
            Ok((peer, _)) => break peer,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "source must connect to the fixture bus"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("accept fixture connection: {error}"),
        }
    };
    peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut first_byte = [0];
    peer.read_exact(&mut first_byte).unwrap();
    // No authentication response is sent: the real zbus connection remains
    // pending. Only cancellation, not a bus error, can release controller drop.
    child.0.stdin.take().unwrap().write_all(b"drop\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(
                status.success(),
                "child must complete controller drop cleanly"
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "controller drop must cancel pending logind startup"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(peer);
}

#[test]
#[ignore = "subprocess entry point; invoked by the parent regression"]
fn startup_shutdown_child() {
    if std::env::var_os("HIDLINS_STALLED_BUS_CHILD").is_none() {
        return; // Only the parent owns the bus, stdin and child environment.
    }
    assert!(std::env::var_os("HIDLINS_STALLED_BUS_CHILD").is_some());
    let mut controller = AutoLockController::new(AutoLockConfig::default()).unwrap();
    controller
        .attach_event_source(LogindSource::new().unwrap())
        .unwrap();
    let mut instruction = String::new();
    std::io::stdin().lock().read_line(&mut instruction).unwrap();
    assert_eq!(instruction, "drop\n");
    drop(controller);
}
