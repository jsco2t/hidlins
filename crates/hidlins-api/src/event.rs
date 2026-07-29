use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[cfg(any(test, feature = "test-fixtures"))]
use std::sync::mpsc;

use crate::dto::{LockEvent, SyncEvent};

pub trait EventSink<T: Send>: Send + Sync + 'static {
    fn send(&self, event: T);
}

pub(crate) struct StreamSinkAdapter<T> {
    sink: crate::frb_generated::StreamSink<T>,
    dropped: Arc<AtomicU64>,
}

impl<T> StreamSinkAdapter<T> {
    pub(crate) fn new(sink: crate::frb_generated::StreamSink<T>, dropped: Arc<AtomicU64>) -> Self {
        Self { sink, dropped }
    }
}

impl EventSink<LockEvent> for StreamSinkAdapter<LockEvent> {
    fn send(&self, event: LockEvent) {
        if self.sink.add(event).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl EventSink<SyncEvent> for StreamSinkAdapter<SyncEvent> {
    fn send(&self, event: SyncEvent) {
        if self.sink.add(event).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
pub struct MpscEventSink<T> {
    tx: mpsc::Sender<T>,
}

#[cfg(any(test, feature = "test-fixtures"))]
impl<T: Send> MpscEventSink<T> {
    pub fn new(tx: mpsc::Sender<T>) -> Self {
        Self { tx }
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
impl<T: Send + 'static> EventSink<T> for MpscEventSink<T> {
    fn send(&self, event: T) {
        let _ = self.tx.send(event);
    }
}
