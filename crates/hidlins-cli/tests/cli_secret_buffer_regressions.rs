//! Observe actual owned synthetic password bytes before allocation release.

#[path = "../../hidlins-core/tests/common/allocation_observer.rs"]
mod allocation_observer;

use std::collections::VecDeque;
use std::io::{self, BufRead, Read, Write};

use allocation_observer::DeallocationWatch;
use hidlins_cli::prompt::{new_master_password_confirmed, read_password_no_echo};

struct OwnedLines {
    lines: VecDeque<String>,
    fail_after_write: bool,
}

impl OwnedLines {
    fn new(lines: Vec<String>) -> Self {
        Self {
            lines: lines.into(),
            fail_after_write: false,
        }
    }
}

impl Read for OwnedLines {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        unreachable!("the prompt must use the overridden line reader")
    }
}

impl BufRead for OwnedLines {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        unreachable!("the prompt must use the overridden line reader")
    }

    fn consume(&mut self, _: usize) {
        unreachable!("the prompt must use the overridden line reader")
    }

    fn read_line(&mut self, output: &mut String) -> io::Result<usize> {
        let Some(line) = self.lines.pop_front() else {
            return Ok(0);
        };
        *output = line;
        if self.fail_after_write && self.lines.is_empty() {
            return Err(io::Error::other("synthetic partial read failure"));
        }
        Ok(output.len())
    }
}

#[test]
fn matching_confirmation_duplicate_is_erased() {
    let duplicate = String::from("synthetic matching value\n");
    let watch = DeallocationWatch::new(duplicate.as_bytes());
    let mut input = OwnedLines::new(vec!["synthetic matching value\n".into(), duplicate]);
    let password = new_master_password_confirmed(&mut input, &mut Vec::new()).unwrap();
    assert_eq!(password.as_bytes(), b"synthetic matching value");
    drop(password);
    watch.assert_erased();
}

#[test]
fn consumed_matching_source_is_also_erased() {
    let first = String::from("synthetic matching source\n");
    let watch = DeallocationWatch::new(first.as_bytes());
    let mut input = OwnedLines::new(vec![first, "synthetic matching source\n".into()]);
    drop(new_master_password_confirmed(&mut input, &mut Vec::new()).unwrap());
    watch.assert_erased();
}

fn check_mismatch_owner(first_candidate: bool) {
    let first = String::from("synthetic rejected first\n");
    let second = String::from("synthetic rejected second\n");
    let watch = DeallocationWatch::new(if first_candidate {
        first.as_bytes()
    } else {
        second.as_bytes()
    });
    let mut input = OwnedLines::new(vec![
        first,
        second,
        "synthetic accepted\n".into(),
        "synthetic accepted\n".into(),
    ]);
    let password = new_master_password_confirmed(&mut input, &mut Vec::new()).unwrap();
    assert_eq!(password.as_bytes(), b"synthetic accepted");
    drop(password);
    watch.assert_erased();
}

#[test]
fn mismatching_first_candidate_is_erased() {
    check_mismatch_owner(true);
}

#[test]
fn mismatching_second_candidate_is_erased() {
    check_mismatch_owner(false);
}

#[test]
fn first_candidate_is_erased_when_confirmation_read_fails() {
    let first = String::from("synthetic abandoned confirmation\n");
    let watch = DeallocationWatch::new(first.as_bytes());
    let mut input = OwnedLines::new(vec![first, "synthetic partial confirmation\n".into()]);
    input.fail_after_write = true;
    assert!(new_master_password_confirmed(&mut input, &mut Vec::new()).is_err());
    watch.assert_erased();
}

#[test]
fn partially_read_password_is_erased_on_input_error() {
    let partial = String::from("synthetic partial input");
    let watch = DeallocationWatch::new(partial.as_bytes());
    let mut input = OwnedLines::new(vec![partial]);
    input.fail_after_write = true;
    assert!(read_password_no_echo("Password: ", &mut input, &mut Vec::new()).is_err());
    watch.assert_erased();
}

struct FailingNewline;

impl Write for FailingNewline {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes == b"\n" {
            Err(io::Error::other("synthetic newline output failure"))
        } else {
            Ok(bytes.len())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn captured_password_is_erased_when_prompt_newline_write_fails() {
    let line = String::from("synthetic captured input\n");
    let watch = DeallocationWatch::new(line.as_bytes());
    let mut input = OwnedLines::new(vec![line]);
    assert!(read_password_no_echo("Password: ", &mut input, &mut FailingNewline).is_err());
    watch.assert_erased();
}
