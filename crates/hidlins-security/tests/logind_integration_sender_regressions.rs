//! Exercise the maintained acceptance tests against a valid isolated service.
//! The old driver emits from an unrelated sender and cannot lock the helper.
#![cfg(all(target_os = "linux", feature = "logind"))]

#[path = "common/logind_bus.rs"]
mod logind_bus;

// The acceptance test itself is the code under repair. Keep its unchanged
// bodies in this executable so their actual transport behavior is exercised.
#[allow(clippy::manual_assert)]
#[path = "us_052_post_mvp_logind.rs"]
mod existing;

use std::process::{Child, Command};
use std::time::{Duration, Instant};

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn maintained_logind_acceptance_cases_use_the_service_owner() {
    let bus = logind_bus::LogindBus::new();
    assert!(bus.owner.unique_name().is_some());
    let mut child = ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args(["existing::", "--ignored", "--nocapture", "--test-threads=1"])
            .env("DBUS_SYSTEM_BUS_ADDRESS", &bus.address)
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(
                status.success(),
                "maintained logind acceptance cases must observe real owner signals"
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "acceptance subprocess must finish within its bound"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
