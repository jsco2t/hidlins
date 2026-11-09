//! Ambient native bus configuration must not create non-sync network calls.
#![cfg(all(target_os = "linux", feature = "desktop"))]

mod common;

use std::fs::File;
use std::io::Write;
use std::net::{Ipv4Addr, TcpListener};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::{hidlins_cmd, seed_vault, VaultsToml};
use hidlins_core::{MasterPassword, Vault};

const PASSWORD: &str = "synthetic native bus master";

struct Service {
    child: Child,
    stdout: PathBuf,
    stderr: PathBuf,
}

impl Service {
    fn start(registry: &VaultsToml, address: &str) -> Self {
        let reservation = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = reservation.local_addr().unwrap().port();
        drop(reservation);
        let stdout = registry.tempdir.path().join("native-bus.stdout");
        let stderr = registry.tempdir.path().join("native-bus.stderr");
        let child = hidlins_cmd()
            .args([
                "--registry",
                &registry.registry_arg(),
                "--format",
                "json",
                "sync",
                "serve",
                "--vault",
                "v",
                "--address",
                "127.0.0.1",
                "--port",
                &port.to_string(),
            ])
            .env("DBUS_SYSTEM_BUS_ADDRESS", address)
            .stdin(Stdio::piped())
            .stdout(File::create(&stdout).unwrap())
            .stderr(File::create(&stderr).unwrap())
            .spawn()
            .unwrap();
        let mut service = Self {
            child,
            stdout,
            stderr,
        };
        service
            .child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(format!("{PASSWORD}\n").as_bytes())
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let stdout = std::fs::read_to_string(&service.stdout).unwrap();
            if let Some(ready) = stdout
                .lines()
                .find_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            {
                assert_eq!(ready["status"], "serving", "{}", service.diagnostics());
                return service;
            }
            assert!(
                service.child.try_wait().unwrap().is_none(),
                "local serving must survive native rejection: {}",
                service.diagnostics()
            );
            assert!(
                Instant::now() < deadline,
                "no readiness: {}",
                service.diagnostics()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn diagnostics(&self) -> String {
        format!(
            "stdout={} stderr={}",
            std::fs::read_to_string(&self.stdout).unwrap(),
            std::fs::read_to_string(&self.stderr).unwrap()
        )
    }

    fn stop(&mut self) {
        assert!(Command::new("kill")
            .args(["-INT", &self.child.id().to_string()])
            .status()
            .unwrap()
            .success());
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(
                    status.success(),
                    "normal local stop: {}",
                    self.diagnostics()
                );
                return;
            }
            assert!(
                Instant::now() < deadline,
                "native rejection must not hang shutdown: {}",
                self.diagnostics()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn check_tcp_rejected(nonce: bool) {
    let registry = VaultsToml::new();
    let vault = seed_vault(&registry, "v", PASSWORD);
    let before = std::fs::read(&vault).unwrap();
    let bus = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    bus.set_nonblocking(true).unwrap();
    let port = bus.local_addr().unwrap().port();
    let address = if nonce {
        let path = registry.tempdir.path().join("synthetic-nonce");
        std::fs::write(&path, b"0123456789abcdef").unwrap();
        format!(
            "nonce-tcp:host=127.0.0.1,port={port},family=ipv4,noncefile={}",
            path.display()
        )
    } else {
        format!("tcp:host=127.0.0.1,port={port},family=ipv4")
    };
    let mut service = Service::start(&registry, &address);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match bus.accept() {
            Ok(_) => panic!(
                "ambient native bus must not cause any TCP connection: {}",
                service.diagnostics()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("owned listener failed: {error}"),
        }
        let stderr = std::fs::read_to_string(&service.stderr).unwrap();
        if stderr.contains("local Unix transport") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "transport policy must report native degradation: {}",
            service.diagnostics()
        );
        assert!(
            service.child.try_wait().unwrap().is_none(),
            "local service must remain available"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    service.stop();
    assert!(matches!(bus.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock));
    assert_eq!(std::fs::read(&vault).unwrap(), before);
    drop(Vault::open(&vault, &MasterPassword::new(PASSWORD.to_owned()), None).unwrap());
}

#[test]
fn ambient_tcp_bus_is_rejected_before_connecting() {
    check_tcp_rejected(false);
}

#[test]
fn ambient_nonce_tcp_bus_is_rejected_before_connecting() {
    check_tcp_rejected(true);
}

#[test]
fn missing_unix_bus_preserves_existing_best_effort_degradation() {
    let registry = VaultsToml::new();
    seed_vault(&registry, "v", PASSWORD);
    let address = format!(
        "unix:path={}",
        registry.tempdir.path().join("absent-native-bus").display()
    );
    let mut service = Service::start(&registry, &address);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let stderr = std::fs::read_to_string(&service.stderr).unwrap();
        if stderr.contains("connect to system DBus") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Unix connection failures retain the existing diagnostic: {}",
            service.diagnostics()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    service.stop();
}
