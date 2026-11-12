//! Event-only foreground lifetime: OS lock ends serving, with no idle policy.

use std::io::Write;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
    Arc,
};
use std::time::Instant;

use hidlins_security::{
    AutoLockConfig, AutoLockController, LockState, OsLockEventSource, SecurityError, SecurityEvent,
    ShutdownHandle,
};

use crate::exit::CliExit;

pub(super) struct ForegroundLock {
    controller: AutoLockController,
    failures: Receiver<SourceFailure>,
    report: Sender<SourceFailure>,
    stopping: Arc<AtomicBool>,
}

impl ForegroundLock {
    fn new() -> Result<Self, CliExit> {
        let (report, failures) = mpsc::channel();
        Ok(Self {
            controller: AutoLockController::new(AutoLockConfig::default())
                .map_err(|error| CliExit::Internal(error.to_string()))?,
            failures,
            report,
            stopping: Arc::default(),
        })
    }

    pub(super) fn start() -> Result<Self, CliExit> {
        let mut monitor = Self::new()?;
        #[cfg(unix)]
        monitor.attach("sigstop", hidlins_security::SigstopSource::new());
        #[cfg(target_os = "linux")]
        monitor.attach("logind", hidlins_security::LogindSource::new());
        #[cfg(target_os = "macos")]
        monitor.attach("iokit", hidlins_security::IoKitSource::new());
        Ok(monitor)
    }

    fn attach<S: OsLockEventSource + 'static>(
        &mut self,
        name: &'static str,
        source: Result<S, SecurityError>,
    ) {
        let result = source.and_then(|source| {
            self.controller.attach_event_source(ReportingSource {
                source,
                report: self.report.clone(),
                stopping: Arc::clone(&self.stopping),
            })
        });
        if let Err(error) = result {
            let _ = self.report.send(SourceFailure {
                name,
                detail: error.to_string(),
            });
        }
    }

    pub(super) fn is_locked(&mut self) -> bool {
        self.poll_at(Instant::now(), &mut std::io::stderr().lock())
    }

    pub(super) fn check_before_serve(&mut self) -> Result<(), CliExit> {
        if self.is_locked() {
            Err(CliExit::VaultLocked(
                "system lock received while starting foreground sync".into(),
            ))
        } else {
            Ok(())
        }
    }

    fn poll_at(&mut self, now: Instant, stderr: &mut dyn Write) -> bool {
        while let Ok(failure) = self.failures.try_recv() {
            let _ = writeln!(
                stderr,
                "warning: OS lock monitoring unavailable ({}): {}",
                failure.name, failure.detail
            );
        }
        // Never seed activity/unlock: last_activity stays None, so this
        // controller consumes OS events without starting an idle deadline.
        self.controller.tick(now) == LockState::Locked
    }
}

impl Drop for ForegroundLock {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        self.controller.shutdown_sources();
    }
}

struct SourceFailure {
    name: &'static str,
    detail: String,
}

struct ReportingSource<S> {
    source: S,
    report: Sender<SourceFailure>,
    stopping: Arc<AtomicBool>,
}

struct SourceExit {
    name: &'static str,
    report: Sender<SourceFailure>,
    stopping: Arc<AtomicBool>,
    detail: String,
}

impl Drop for SourceExit {
    fn drop(&mut self) {
        if !self.stopping.load(Ordering::Acquire) {
            let _ = self.report.send(SourceFailure {
                name: self.name,
                detail: std::mem::take(&mut self.detail),
            });
        }
    }
}

impl<S: OsLockEventSource + 'static> OsLockEventSource for ReportingSource<S> {
    fn name(&self) -> &'static str {
        self.source.name()
    }

    fn shutdown_handle(&mut self) -> ShutdownHandle {
        self.source.shutdown_handle()
    }

    fn run(self: Box<Self>, sender: Sender<SecurityEvent>) -> Result<(), SecurityError> {
        let Self {
            source,
            report,
            stopping,
        } = *self;
        let mut notice = SourceExit {
            name: source.name(),
            report,
            stopping,
            detail: "event source stopped unexpectedly".to_owned(),
        };
        let result = Box::new(source).run(sender);
        if let Err(error) = &result {
            notice.detail = error.to_string();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hidlins_security::OsLockReason;
    use std::time::Duration;

    #[derive(Clone, Copy)]
    enum Mode {
        Lock,
        Error,
        Panic,
        Exit,
        Park,
    }

    struct Source {
        mode: Mode,
        ready: Sender<()>,
        stopped: Arc<AtomicBool>,
    }

    impl OsLockEventSource for Source {
        fn name(&self) -> &'static str {
            "fixture"
        }

        fn shutdown_handle(&mut self) -> ShutdownHandle {
            let stopped = Arc::clone(&self.stopped);
            ShutdownHandle::new(move || stopped.store(true, Ordering::Release))
        }

        fn run(self: Box<Self>, sender: Sender<SecurityEvent>) -> Result<(), SecurityError> {
            if matches!(self.mode, Mode::Lock) {
                sender
                    .send(SecurityEvent::OsLock {
                        reason: OsLockReason::Screensaver,
                    })
                    .unwrap();
            }
            self.ready.send(()).unwrap();
            match self.mode {
                Mode::Error => {
                    return Err(SecurityError::EventSourceStart {
                        name: "fixture",
                        detail: "synthetic run failure".into(),
                    })
                }
                Mode::Panic => panic!("synthetic source panic"),
                Mode::Exit => return Ok(()),
                Mode::Lock | Mode::Park => {}
            }
            while !self.stopped.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(1));
            }
            Ok(())
        }
    }

    fn source(mode: Mode) -> (Source, Receiver<()>) {
        let (ready, receiver) = mpsc::channel();
        (
            Source {
                mode,
                ready,
                stopped: Arc::default(),
            },
            receiver,
        )
    }

    #[test]
    fn queued_lock_is_observed_before_serving() {
        let mut monitor = ForegroundLock::new().unwrap();
        let (source, ready) = source(Mode::Lock);
        monitor.attach("fixture", Ok(source));
        ready.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(monitor.poll_at(Instant::now(), &mut Vec::new()));
        assert!(monitor.poll_at(Instant::now(), &mut Vec::new()));
    }

    #[test]
    fn no_event_monitor_never_acquires_an_idle_deadline() {
        let mut monitor = ForegroundLock::new().unwrap();
        let future = Instant::now() + Duration::from_secs(172_800);
        assert!(!monitor.poll_at(future, &mut Vec::new()));
        assert!(!monitor.poll_at(future + Duration::from_secs(172_800), &mut Vec::new()));
    }

    #[test]
    fn construction_failure_warns_once_and_preserves_local_use() {
        let mut monitor = ForegroundLock::new().unwrap();
        monitor.attach::<Source>(
            "fixture",
            Err(SecurityError::EventSourceStart {
                name: "fixture",
                detail: "synthetic construction failure".into(),
            }),
        );
        let mut output = Vec::new();
        assert!(!monitor.poll_at(Instant::now(), &mut output));
        assert!(String::from_utf8_lossy(&output).contains("synthetic construction failure"));
        let count = output.len();
        assert!(!monitor.poll_at(Instant::now(), &mut output));
        assert_eq!(output.len(), count);
    }

    #[test]
    fn run_error_panic_and_unexpected_exit_each_warn_once() {
        for mode in [Mode::Error, Mode::Panic, Mode::Exit] {
            let mut monitor = ForegroundLock::new().unwrap();
            let (source, ready) = source(mode);
            monitor.attach("fixture", Ok(source));
            ready.recv_timeout(Duration::from_secs(1)).unwrap();
            let mut output = Vec::new();
            let deadline = Instant::now() + Duration::from_secs(1);
            while output.is_empty() {
                assert!(!monitor.poll_at(Instant::now(), &mut output));
                assert!(
                    Instant::now() < deadline,
                    "source termination must be reported"
                );
                std::thread::yield_now();
            }
            assert!(String::from_utf8_lossy(&output)
                .contains("OS lock monitoring unavailable (fixture)"));
            let count = output.len();
            assert!(!monitor.poll_at(Instant::now(), &mut output));
            assert_eq!(output.len(), count);
        }
    }

    #[test]
    fn shutdown_joins_source_without_a_degradation_warning() {
        let mut monitor = ForegroundLock::new().unwrap();
        let (source, ready) = source(Mode::Park);
        let stopped = Arc::clone(&source.stopped);
        monitor.attach("fixture", Ok(source));
        ready.recv_timeout(Duration::from_secs(1)).unwrap();
        let (_, unused_receiver) = mpsc::channel();
        let failures = std::mem::replace(&mut monitor.failures, unused_receiver);
        drop(monitor);
        assert!(stopped.load(Ordering::Acquire));
        assert!(failures.try_recv().is_err());
    }
}
