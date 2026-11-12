//! Keymap output errors retain stable process statuses without panicking.
#![cfg(all(unix, feature = "desktop"))]

mod common;

use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::Stdio;

#[test]
fn broken_keymap_sinks_return_errors_without_panicking() {
    let home = tempfile::tempdir().unwrap();
    for (args, expected) in [
        (vec!["keys"], 10),
        (vec!["keys", "--format=json"], 10),
        (vec!["--dump-keys"], 1),
        (vec!["--dump-keys=json"], 1),
    ] {
        let (reader, writer) = UnixStream::pair().unwrap();
        drop(reader);
        let writer: OwnedFd = writer.into();
        let output = common::hidlins_cmd()
            .args(&args)
            .env("HOME", home.path())
            .env("XDG_CONFIG_HOME", home.path().join("config"))
            .stdin(Stdio::null())
            .stdout(Stdio::from(writer))
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(output.status.code(), Some(expected), "{args:?}: {stderr}");
        assert!(!stderr.contains("panicked"), "{args:?}: {stderr}");
    }
}
