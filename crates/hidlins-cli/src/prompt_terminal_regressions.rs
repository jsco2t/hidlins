//! Test-only fault injection never alters the host terminal or production builds.

use std::cell::RefCell;
use std::io::Cursor;
use std::os::fd::{AsFd, BorrowedFd};

use nix::errno::Errno;
use nix::sys::termios::{LocalFlags, SetArg, Termios};

enum Fault {
    Query(Errno),
    Set { original: Termios, error: Errno },
}

thread_local! {
    static FAULT: RefCell<Option<Fault>> = const { RefCell::new(None) };
}

struct ResetFault;

impl Drop for ResetFault {
    fn drop(&mut self) {
        FAULT.with(|fault| *fault.borrow_mut() = None);
    }
}

fn inject(fault: Fault) -> ResetFault {
    FAULT.with(|slot| {
        assert!(slot.borrow().is_none());
        *slot.borrow_mut() = Some(fault);
    });
    ResetFault
}

pub(super) fn get_attrs(fd: BorrowedFd<'_>) -> nix::Result<Termios> {
    FAULT.with(|fault| match fault.borrow().as_ref() {
        Some(Fault::Query(error)) => Err(*error),
        Some(Fault::Set { original, .. }) => Ok(original.clone()),
        None => super::tcgetattr(fd),
    })
}

pub(super) fn set_attrs(fd: BorrowedFd<'_>, action: SetArg, attrs: &Termios) -> nix::Result<()> {
    FAULT.with(|fault| match fault.borrow().as_ref() {
        Some(Fault::Set { error, .. }) => Err(*error),
        _ => super::tcsetattr(fd, action, attrs),
    })
}

fn check_query_failure(error: Errno) {
    let _reset = inject(Fault::Query(error));
    let mut input = Cursor::new(b"synthetic secret\n");
    let result = super::read_password_no_echo("Password: ", &mut input, &mut Vec::new());
    assert_eq!(
        result
            .expect_err("terminal query failure must reject secret input")
            .raw_os_error(),
        Some(error as i32)
    );
    assert_eq!(
        input.position(),
        0,
        "terminal failure must not consume a secret"
    );
}

#[test]
fn permission_error_querying_terminal_never_reads_input() {
    check_query_failure(Errno::EPERM);
}

#[test]
fn io_error_querying_terminal_never_reads_input() {
    check_query_failure(Errno::EIO);
}

#[test]
fn failed_echo_disable_never_reads_input() {
    let pair = nix::pty::openpty(None, None).unwrap();
    let original = super::tcgetattr(pair.slave.as_fd()).unwrap();
    let _reset = inject(Fault::Set {
        original,
        error: Errno::EIO,
    });
    let mut input = Cursor::new(b"synthetic secret\n");
    let result = super::read_password_no_echo("Password: ", &mut input, &mut Vec::new());
    assert_eq!(
        result
            .expect_err("failed echo suppression must reject secret input")
            .raw_os_error(),
        Some(Errno::EIO as i32)
    );
    assert_eq!(
        input.position(),
        0,
        "failed echo suppression must not consume a secret"
    );
}

#[test]
fn nonterminal_pipe_input_remains_supported() {
    let _reset = inject(Fault::Query(Errno::ENOTTY));
    let mut input = Cursor::new(b"synthetic pipe\n");
    assert_eq!(
        super::read_password_no_echo("Password: ", &mut input, &mut Vec::new()).unwrap(),
        "synthetic pipe"
    );
}

#[test]
fn actual_owned_terminal_disables_and_restores_echo() {
    let pair = nix::pty::openpty(None, None).unwrap();
    let mut attrs = super::tcgetattr(pair.slave.as_fd()).unwrap();
    attrs.local_flags.insert(LocalFlags::ECHO);
    super::tcsetattr(pair.slave.as_fd(), SetArg::TCSANOW, &attrs).unwrap();
    {
        let _guard = super::EchoGuard::disable(pair.slave.as_fd());
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
