//! Native lock-source lifetime and UI-facing degradation notices.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
    Arc,
};

use hidlins_security::{
    AutoLockController, OsLockEventSource, SecurityError, SecurityEvent, ShutdownHandle,
};

pub(crate) struct NativeEvents {
    failures: Receiver<SourceFailure>,
    report: Sender<SourceFailure>,
    stopping: Arc<AtomicBool>,
}

impl NativeEvents {
    pub(crate) fn start(controller: &mut AutoLockController) -> Self {
        let (report, failures) = mpsc::channel();
        let monitor = Self {
            failures,
            report,
            stopping: Arc::default(),
        };
        #[cfg(unix)]
        monitor.attach(
            controller,
            "sigstop",
            hidlins_security::SigstopSource::new(),
        );
        #[cfg(target_os = "linux")]
        monitor.attach(controller, "logind", hidlins_security::LogindSource::new());
        #[cfg(target_os = "macos")]
        monitor.attach(controller, "iokit", hidlins_security::IoKitSource::new());
        monitor
    }

    fn attach<S: OsLockEventSource + 'static>(
        &self,
        controller: &mut AutoLockController,
        name: &'static str,
        source: Result<S, SecurityError>,
    ) {
        let result = source.and_then(|source| {
            controller.attach_event_source(ReportingSource {
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

    pub(crate) fn notices(&self) -> Vec<String> {
        let mut notices = Vec::new();
        while let Ok(failure) = self.failures.try_recv() {
            notices.push(format!(
                "OS lock monitoring unavailable ({}): {}",
                failure.name, failure.detail
            ));
        }
        notices
    }

    pub(crate) fn shutdown(&self, controller: &mut AutoLockController) {
        self.stopping.store(true, Ordering::Release);
        controller.shutdown_sources();
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
            detail: "event-source thread failed unexpectedly".into(),
        };
        let result = Box::new(source).run(sender);
        notice.detail = match &result {
            Ok(()) => "event-source thread stopped unexpectedly".into(),
            Err(error) => error.to_string(),
        };
        result
    }
}
