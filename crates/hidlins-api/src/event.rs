//! Framework-independent event subscriptions. A sink must return promptly;
//! callbacks run while the session state is locked and must not reenter it.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};

pub trait EventSink<T: Send>: Send + Sync + 'static {
    fn send(&self, event: T);

    /// Return false if the event could not be delivered. Existing sinks which
    /// cannot fail need only implement `send`.
    fn try_send(&self, event: T) -> bool {
        self.send(event);
        true
    }
}

pub(crate) struct CountedEventSink<T: Send> {
    sink: Box<dyn EventSink<T>>,
    dropped: Arc<AtomicU64>,
}

impl<T: Send> CountedEventSink<T> {
    pub(crate) fn new(sink: Box<dyn EventSink<T>>, dropped: Arc<AtomicU64>) -> Self {
        Self { sink, dropped }
    }
}

impl<T: Send + 'static> EventSink<T> for CountedEventSink<T> {
    fn send(&self, event: T) {
        if !self.sink.try_send(event) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// An owned standard-library channel adapter for application subscriptions.
pub struct MpscEventSink<T> {
    tx: mpsc::Sender<T>,
}

impl<T: Send> MpscEventSink<T> {
    pub fn new(tx: mpsc::Sender<T>) -> Self {
        Self { tx }
    }
}

impl<T: Send + 'static> EventSink<T> for MpscEventSink<T> {
    fn send(&self, event: T) {
        let _ = self.try_send(event);
    }

    fn try_send(&self, event: T) -> bool {
        self.tx.send(event).is_ok()
    }
}
