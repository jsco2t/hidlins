mod common;

use std::sync::mpsc;
use std::time::Duration;

use hidlins_api::api::session::{init_app, AppSession};
use hidlins_api::dto::{AppInitConfig, LockEvent};
use hidlins_api::event::{EventSink, MpscEventSink};

#[test]
fn application_channel_gets_snapshot_replacement_and_shutdown() {
    let env = common::TestEnv::new();
    let session = init_app(AppInitConfig {
        state_dir: Some(env.paths().state_dir().to_string_lossy().into_owned()),
        config_dir: None,
    })
    .expect("application session");
    let (first, old) = mpsc::channel();
    session.lock_events(Box::new(MpscEventSink::new(first)));
    assert!(matches!(old.recv().unwrap(), LockEvent::Locked));
    let (second, current) = mpsc::channel();
    session.lock_events(Box::new(MpscEventSink::new(second)));
    assert!(matches!(current.recv().unwrap(), LockEvent::Locked));
    assert!(matches!(
        old.recv_timeout(Duration::from_secs(1)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
    session.shutdown();
    while current.try_recv().is_ok() {}
    assert!(matches!(
        current.recv_timeout(Duration::from_secs(1)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
    let (late, closed) = mpsc::channel();
    session.lock_events(Box::new(MpscEventSink::new(late)));
    assert!(matches!(closed.recv().unwrap(), LockEvent::Locked));
    assert!(closed.recv().is_err());
}

#[test]
fn delivery_failures_remain_observable_across_resubscriptions() {
    let env = common::TestEnv::new();
    let session = AppSession::for_test(env.paths_clone()).unwrap();
    let initial = session.dropped_lock_events();
    for expected in 1..=2 {
        let (sink, receiver) = mpsc::channel();
        drop(receiver);
        session.lock_events(Box::new(MpscEventSink::new(sink)));
        assert_eq!(session.dropped_lock_events(), initial + expected);
    }
    let (sink, receiver) = mpsc::channel();
    session.lock_events(Box::new(MpscEventSink::new(sink)));
    assert!(matches!(receiver.recv().unwrap(), LockEvent::Locked));
    assert_eq!(session.dropped_lock_events(), initial + 2);
}

#[test]
fn existing_infallible_sink_implementations_remain_usable() {
    struct Sink(mpsc::Sender<LockEvent>);
    impl EventSink<LockEvent> for Sink {
        fn send(&self, event: LockEvent) {
            self.0.send(event).unwrap();
        }
    }
    let env = common::TestEnv::new();
    let session = AppSession::for_test(env.paths_clone()).unwrap();
    let (sink, receiver) = mpsc::channel();
    session.lock_events(Box::new(Sink(sink)));
    assert!(matches!(receiver.recv().unwrap(), LockEvent::Locked));
    assert_eq!(session.dropped_lock_events(), 0);
}
