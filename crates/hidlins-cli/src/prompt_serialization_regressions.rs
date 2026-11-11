//! Guard lifetimes cannot overlap ownership of process-global signal state.

use std::os::fd::AsFd;
use std::sync::mpsc;
use std::time::Duration;

use nix::sys::termios::{LocalFlags, SetArg};

fn terminal() -> nix::pty::OpenptyResult {
    let pair = nix::pty::openpty(None, None).unwrap();
    let mut attrs = super::tcgetattr(pair.slave.as_fd()).unwrap();
    attrs.local_flags.insert(LocalFlags::ECHO);
    super::tcsetattr(pair.slave.as_fd(), SetArg::TCSANOW, &attrs).unwrap();
    pair
}

#[test]
fn second_terminal_guard_waits_until_first_owner_restores_and_disarms() {
    let first = terminal();
    let second = terminal();
    let first_guard = super::EchoGuard::disable(first.slave.as_fd()).unwrap();
    let (attempted_tx, attempted_rx) = mpsc::channel();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        attempted_tx.send(()).unwrap();
        let guard = super::EchoGuard::disable(second.slave.as_fd()).unwrap();
        entered_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        drop(guard);
        assert!(super::tcgetattr(second.slave.as_fd())
            .unwrap()
            .local_flags
            .contains(LocalFlags::ECHO));
    });
    attempted_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let entered_early = entered_rx.recv_timeout(Duration::from_millis(100)).is_ok();
    if entered_early {
        // Old guards overlap, but channel handoffs explicitly order every raw
        // static write: second acquired, second drops/joins, then first drops.
        release_tx.send(()).unwrap();
        worker.join().unwrap();
        drop(first_guard);
    } else {
        drop(first_guard);
        let entered = entered_rx.recv_timeout(Duration::from_secs(5));
        let _ = release_tx.send(());
        worker.join().unwrap();
        entered.expect("waiting guard must acquire once the first owner releases");
    }
    assert!(super::tcgetattr(first.slave.as_fd())
        .unwrap()
        .local_flags
        .contains(LocalFlags::ECHO));
    assert!(
        !entered_early,
        "a second guard must not replace live process-global signal state"
    );
}

#[test]
fn unwind_restores_echo_and_next_guard_can_acquire_after_poison() {
    let pair = terminal();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = super::EchoGuard::disable(pair.slave.as_fd()).unwrap();
        panic!("synthetic prompt unwind");
    }));
    assert!(result.is_err());
    assert!(super::tcgetattr(pair.slave.as_fd())
        .unwrap()
        .local_flags
        .contains(LocalFlags::ECHO));
    {
        let _guard = super::EchoGuard::disable(pair.slave.as_fd()).unwrap();
        assert!(!super::tcgetattr(pair.slave.as_fd())
            .unwrap()
            .local_flags
            .contains(LocalFlags::ECHO));
    }
    assert!(super::tcgetattr(pair.slave.as_fd())
        .unwrap()
        .local_flags
        .contains(LocalFlags::ECHO));
}
