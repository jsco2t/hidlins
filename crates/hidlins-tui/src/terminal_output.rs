//! Owned terminal output and its synthetic release-workload measurements.

use std::fs::File;
use std::io::{self, ErrorKind, Write};
use std::os::fd::AsFd;

use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

/// Only this owner receives rendered secret bytes. Unlike the global Stdout
/// line buffer it has a bounded lifetime and erases accepted output on flush.
/// All storage is initialized once; it never grows or changes length.
pub(crate) struct SecretOutput<W: Write> {
    buffer: Zeroizing<Vec<u8>>,
    used: usize,
    written: usize,
    inner: W,
}

// Sensitive bytes live exclusively in the Zeroizing field. File holds only an
// owned descriptor; the private generic constructor also supports test sinks.
impl<W: Write> ZeroizeOnDrop for SecretOutput<W> {}

pub(crate) fn stdout_writer() -> io::Result<SecretOutput<File>> {
    // Duplicate the existing output destination, preserving redirection and
    // close-on-exec behavior without adding a global retained plaintext owner.
    let file = File::from(std::io::stdout().as_fd().try_clone_to_owned()?);
    Ok(SecretOutput::new(file, 8 * 1024))
}

impl<W: Write> SecretOutput<W> {
    fn new(inner: W, capacity: usize) -> Self {
        assert!(capacity > 0);
        Self {
            buffer: Zeroizing::new(vec![0; capacity]),
            used: 0,
            written: 0,
            inner,
        }
    }

    fn discard(&mut self) {
        // The unused suffix is zero at construction and after every flush.
        // `used` includes every byte ever dirtied since the previous discard,
        // including already-written prefixes after a partial I/O failure.
        self.buffer[..self.used].zeroize();
        self.used = 0;
        self.written = 0;
    }

    fn guard_unwind(&mut self) -> io::Result<()> {
        if std::thread::panicking() {
            // Terminal::drop may write/flush a cursor command after the panic
            // hook has restored the normal screen. Never replay queued text.
            self.discard();
            return Err(io::Error::other("terminal output suppressed during panic"));
        }
        Ok(())
    }

    fn flush_pending(&mut self) -> io::Result<()> {
        while self.written < self.used {
            match self.inner.write(&self.buffer[self.written..self.used]) {
                Ok(0) => return Err(ErrorKind::WriteZero.into()),
                Ok(count) => self.written += count,
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        self.discard();
        Ok(())
    }
}

impl<W: Write> Write for SecretOutput<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.guard_unwind()?;
        if bytes.is_empty() {
            return Ok(0);
        }
        if bytes.len() > self.buffer.len() - self.used {
            self.flush_pending()?;
        }
        if bytes.len() >= self.buffer.len() {
            // The caller owns these bytes; write them directly without a
            // growing intermediate allocation. Preserve Write's partial result.
            return self.inner.write(bytes);
        }
        let end = self.used + bytes.len();
        self.buffer[self.used..end].copy_from_slice(bytes);
        self.used = end;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.guard_unwind()?;
        self.flush_pending()?;
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::time::{Duration, Instant};

    use hidlins_core::Zeroizing;
    use ratatui::backend::CrosstermBackend;
    use ratatui::buffer::CellDiffOption;
    use ratatui::layout::Rect;
    use ratatui::widgets::{Paragraph, Wrap};
    use ratatui::{Terminal, TerminalOptions, Viewport};

    fn render_output_workload(writer: impl Write, owner: &str) {
        let text = Zeroizing::new("synthetic secret note é🔒 ".repeat(240));
        let mut terminal = Terminal::with_options(
            CrosstermBackend::new(writer),
            TerminalOptions {
                viewport: Viewport::Fixed(Rect::new(0, 0, 120, 40)),
            },
        )
        .unwrap();
        for redraw in [true, false] {
            let mut samples = Vec::with_capacity(400);
            for index in 0..420 {
                let start = Instant::now();
                terminal
                    .draw(|frame| {
                        frame.render_widget(
                            Paragraph::new(text.as_str()).wrap(Wrap { trim: false }),
                            frame.area(),
                        );
                        if redraw {
                            for cell in &mut frame.buffer_mut().content {
                                cell.set_diff_option(CellDiffOption::AlwaysUpdate);
                            }
                        }
                    })
                    .unwrap();
                if index >= 20 {
                    samples.push(start.elapsed());
                }
            }
            samples.sort_unstable();
            let median = samples[(samples.len() - 1) / 2];
            let p95 = samples[(samples.len() - 1) * 95 / 100];
            // Make redirects only stdout to a nonrecording OS destination;
            // actual terminal payloads never enter this diagnostic stream.
            eprintln!("tui-output owner={owner} redraw={redraw} size=120x40 samples={} median_us={} p95_us={}", samples.len(), median.as_micros(), p95.as_micros());
            assert!(p95 < Duration::from_millis(50), "retain 20-FPS budget");
        }
    }

    #[test]
    #[ignore = "serial release output measurement: make bench-tui-output"]
    fn render_standard_output_baseline() {
        render_output_workload(std::io::stdout(), "std-global");
    }
    #[test]
    #[ignore = "serial release output measurement: make bench-tui-output"]
    fn render_protected_output_workload() {
        render_output_workload(super::stdout_writer().unwrap(), "owned-zeroizing");
    }

    use super::SecretOutput;
    use crate::allocation_observer::DeallocationWatch;
    use std::collections::VecDeque;
    use std::io::{self, ErrorKind};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    enum Action {
        Limit(usize),
        Interrupted,
        Fail,
        Zero,
    }

    struct ScriptedWriter {
        actions: VecDeque<Action>,
        output: Zeroizing<Vec<u8>>,
        writes: usize,
        flush_failure: bool,
    }

    impl ScriptedWriter {
        fn new(actions: impl IntoIterator<Item = Action>) -> Self {
            Self {
                actions: actions.into_iter().collect(),
                output: Zeroizing::new(Vec::with_capacity(4096)),
                writes: 0,
                flush_failure: false,
            }
        }
    }

    impl Write for ScriptedWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            let count = match self.actions.pop_front() {
                Some(Action::Interrupted) => return Err(ErrorKind::Interrupted.into()),
                Some(Action::Fail) => return Err(ErrorKind::BrokenPipe.into()),
                Some(Action::Zero) => return Ok(0),
                Some(Action::Limit(limit)) => bytes.len().min(limit),
                None => bytes.len(),
            };
            assert!(self.output.len() + count <= self.output.capacity());
            self.output.extend_from_slice(&bytes[..count]);
            Ok(count)
        }

        fn flush(&mut self) -> io::Result<()> {
            if self.flush_failure {
                Err(ErrorKind::BrokenPipe.into())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn small_writes_are_batched_and_the_entire_storage_is_clean_after_flush() {
        let mut output = SecretOutput::new(ScriptedWriter::new([]), 1024);
        let pointer = output.buffer.as_ptr();
        for _ in 0..1000 {
            output.write_all(b"x").unwrap();
        }
        assert_eq!(output.inner.writes, 0);
        assert_eq!(output.used, 1000);
        assert!(output.buffer[..1000].iter().all(|byte| *byte == b'x'));
        output.flush().unwrap();
        assert_eq!(output.inner.writes, 1);
        assert_eq!(output.inner.output.len(), 1000);
        assert!(output.buffer.iter().all(|byte| *byte == 0));
        assert_eq!(output.buffer.len(), 1024);
        assert_eq!(output.buffer.as_ptr(), pointer, "fixed storage never grows");
        output.write_all(b"short").unwrap();
        output.flush().unwrap();
        assert!(
            output.buffer.iter().all(|byte| *byte == 0),
            "no old longer suffix"
        );
        assert_eq!(&output.inner.output[1000..], b"short");
    }

    #[test]
    fn partial_interrupted_and_failed_flush_retries_without_duplicate_bytes() {
        let writer = ScriptedWriter::new([Action::Limit(2), Action::Interrupted, Action::Fail]);
        let mut output = SecretOutput::new(writer, 8);
        output.write_all(b"abcdef").unwrap();
        assert_eq!(output.flush().unwrap_err().kind(), ErrorKind::BrokenPipe);
        assert_eq!(output.written, 2);
        assert_eq!(output.inner.output.as_slice(), b"ab");
        output.write_all(b"ghi").unwrap();
        output.flush().unwrap();
        assert_eq!(output.inner.output.as_slice(), b"abcdefghi");
        assert!(output.buffer.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn zero_write_returns_write_zero_and_retained_owner_erases_on_drop() {
        let mut output = SecretOutput::new(ScriptedWriter::new([Action::Zero]), 8);
        output.write_all(b"secret").unwrap();
        let watch = DeallocationWatch::new(&output.buffer[..6]);
        assert_eq!(output.flush().unwrap_err().kind(), ErrorKind::WriteZero);
        assert_eq!(output.used, 6);
        drop(output);
        watch.assert_erased();
    }

    #[test]
    fn failed_underlying_flush_does_not_retain_or_replay_accepted_plaintext() {
        let mut writer = ScriptedWriter::new([]);
        writer.flush_failure = true;
        let mut output = SecretOutput::new(writer, 8);
        output.write_all(b"secret").unwrap();
        assert_eq!(output.flush().unwrap_err().kind(), ErrorKind::BrokenPipe);
        assert_eq!(output.inner.output.as_slice(), b"secret");
        assert!(output.buffer.iter().all(|byte| *byte == 0));
        output.inner.flush_failure = false;
        output.flush().unwrap();
        assert_eq!(output.inner.output.as_slice(), b"secret");
    }

    #[test]
    fn large_borrowed_writes_preserve_order_and_never_grow_fixed_storage() {
        let mut output = SecretOutput::new(
            ScriptedWriter::new([
                Action::Limit(3),
                Action::Limit(3),
                Action::Limit(5),
                Action::Interrupted,
                Action::Limit(4),
            ]),
            8,
        );
        let pointer = output.buffer.as_ptr();
        output.write_all(b"prefix").unwrap();
        output
            .write_all(b"0123456789abcdefghijklmnopqrstuvwxyz")
            .unwrap();
        output.write_all(b"suffix").unwrap();
        output.flush().unwrap();
        assert_eq!(
            output.inner.output.as_slice(),
            b"prefix0123456789abcdefghijklmnopqrstuvwxyzsuffix"
        );
        assert_eq!(output.buffer.as_ptr(), pointer);
        assert_eq!(output.buffer.len(), 8);
        assert!(output.buffer.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn drop_discards_and_erases_pending_output_without_a_flush() {
        let mut output = SecretOutput::new(std::io::sink(), 32);
        output.write_all(b"synthetic secret").unwrap();
        assert_eq!(&output.buffer[..16], b"synthetic secret");
        let watch = DeallocationWatch::new(&output.buffer[..16]);
        drop(output);
        watch.assert_erased();
    }

    #[test]
    fn exact_capacity_write_borrows_directly_without_retaining_plaintext() {
        let mut output = SecretOutput::new(ScriptedWriter::new([]), 8);
        output.write_all(b"12345678").unwrap();
        assert_eq!(output.inner.output.as_slice(), b"12345678");
        assert_eq!(output.inner.writes, 1);
        assert_eq!(output.used, 0);
        assert!(output.buffer.iter().all(|byte| *byte == 0));
    }

    struct CountingWriter(Arc<AtomicUsize>);

    impl Write for CountingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.fetch_add(bytes.len(), Ordering::SeqCst);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn actual_terminal_drop_during_panic_discards_pending_plaintext_after_restore() {
        let count = Arc::new(AtomicUsize::new(0));
        let mut output = SecretOutput::new(CountingWriter(Arc::clone(&count)), 64);
        output.write_all(b"pending secret").unwrap();
        let watch = DeallocationWatch::new(&output.buffer[..14]);
        let mut terminal = Terminal::with_options(
            CrosstermBackend::new(output),
            TerminalOptions {
                viewport: Viewport::Fixed(Rect::new(0, 0, 2, 1)),
            },
        )
        .unwrap();
        terminal
            .draw(|frame| frame.render_widget("ok", frame.area()))
            .unwrap();
        let before = count.load(Ordering::SeqCst);
        assert!(before > 0);
        // Write forwards through the public Backend implementation into the
        // same fixed allocation, repopulating its initialized watched prefix.
        terminal.backend_mut().write_all(b"pending secret").unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _owned = terminal;
            panic!("synthetic unwind");
        }));
        assert!(result.is_err());
        assert_eq!(
            count.load(Ordering::SeqCst),
            before,
            "unwind cannot flush after panic restoration"
        );
        watch.assert_erased();
    }

    #[test]
    fn stdout_factory_preserves_the_actual_redirected_pipe_destination() {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "terminal_output::tests::child_stdout_factory",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("HIDLINS_TEST_STDOUT_FACTORY", "owned-fixture")
            .output()
            .unwrap();
        assert!(result.status.success(), "owned stdout child succeeds");
        let bytes = Zeroizing::new(result.stdout);
        assert_eq!(
            bytes
                .windows(b"synthetic-factory-output".len())
                .filter(|part| *part == b"synthetic-factory-output")
                .count(),
            1
        );
    }

    #[test]
    fn child_stdout_factory() {
        if std::env::var_os("HIDLINS_TEST_STDOUT_FACTORY").is_some() {
            let mut writer = super::stdout_writer().unwrap();
            writer.write_all(b"synthetic-factory-output\n").unwrap();
            writer.flush().unwrap();
        }
    }
}
