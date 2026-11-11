//! US-052: production LogindSource/controller lock transitions over real D-Bus.
//!
//! Requires dbus-daemon on PATH, with no root, display, active logind session,
//! host-bus policy changes or live OS signals. A private service owns login1;
//! signals must originate on that owning connection to pass production filters.
//! Run via `make test-os-events`. This verifies the adapter contract, not the
//! host desktop's delivery of physical sleep/screensaver events.
#![cfg(all(target_os = "linux", feature = "logind"))]

#[path = "common/logind_bus.rs"]
#[allow(clippy::duplicate_mod)] // Also compiled independently by the immutable regression wrapper.
mod logind_bus;

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn helper_lines(child: &mut Child) -> Receiver<String> {
    let stdout = child.stdout.take().expect("helper stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if tx.send(line.expect("read helper stdout")).is_err() {
                break;
            }
        }
    });
    rx
}

fn assert_locks_from_owner(path: &str, interface: &str, member: &str) {
    let bus = logind_bus::LogindBus::new();
    let mut child = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_logind_helper"))
            .env("DBUS_SYSTEM_BUS_ADDRESS", &bus.address)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn logind_helper"),
    );
    let lines = helper_lines(&mut child.0);
    let ready = lines
        .recv_timeout(Duration::from_secs(5))
        .expect("ready marker");
    assert_eq!(ready, format!("ready PID={}", child.0.id()));

    // Ready means the worker was spawned, not that it subscribed. Repeating
    // the same idempotent lock event bounds setup without a timing assumption.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        zbus::block_on(async {
            if member == "PrepareForSleep" {
                bus.owner
                    .emit_signal(None::<&str>, path, interface, member, &true)
                    .await
                    .unwrap();
            } else {
                bus.owner
                    .emit_signal(None::<&str>, path, interface, member, &())
                    .await
                    .unwrap();
            }
        });
        match lines.recv_timeout(Duration::from_millis(20)) {
            Ok(line) => {
                assert_eq!(line, "locked");
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                assert!(
                    Instant::now() < deadline,
                    "owner signal must lock the helper"
                );
            }
            Err(error) => panic!("helper exited before locking: {error}"),
        }
    }
    drop(child.0.stdin.take());
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(
                status.success(),
                "helper exits after clean controller shutdown"
            );
            break;
        }
        assert!(Instant::now() < deadline, "helper shutdown must finish");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "isolated D-Bus contract; run via make test-os-events"]
fn prepare_for_sleep_signal_transitions_to_locked() {
    assert_locks_from_owner(
        logind_bus::MANAGER,
        "org.freedesktop.login1.Manager",
        "PrepareForSleep",
    );
}

#[test]
#[ignore = "isolated D-Bus contract; run via make test-os-events"]
fn session_lock_signal_transitions_to_locked() {
    assert_locks_from_owner(
        logind_bus::SESSION,
        "org.freedesktop.login1.Session",
        "Lock",
    );
}
