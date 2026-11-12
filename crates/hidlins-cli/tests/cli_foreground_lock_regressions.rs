//! Actual foreground lock/sleep lifetime over owned processes and a private bus.
#![cfg(all(unix, feature = "desktop"))]

mod common;
#[cfg(target_os = "linux")]
#[path = "../../hidlins-security/tests/common/logind_bus.rs"]
mod logind_bus;

use std::fs::File;
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use common::{hidlins_cmd, seed_vault, VaultsToml};
use hidlins_core::{MasterPassword, Vault};
use hidlins_security::{AutoLockConfig, AutoLockController, LockState};

const PASSWORD: &str = "synthetic foreground master";
const STOP_BUDGET: Duration = Duration::from_secs(2);

struct Service {
    child: Child,
    stdout: PathBuf,
    stderr: PathBuf,
    endpoint: SocketAddr,
    vault: PathBuf,
    before: Vec<u8>,
}

impl Service {
    fn start(registry: &VaultsToml, bus_address: Option<&str>) -> Self {
        let vault = seed_vault(registry, "v", PASSWORD);
        let before = std::fs::read(&vault).unwrap();
        let reservation = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let endpoint = reservation.local_addr().unwrap();
        drop(reservation);
        let stdout = registry.tempdir.path().join("foreground.stdout");
        let stderr = registry.tempdir.path().join("foreground.stderr");
        let missing_bus = format!(
            "unix:path={}",
            registry.tempdir.path().join("absent-bus").display()
        );
        let mut command = hidlins_cmd();
        command
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
                &endpoint.port().to_string(),
                "--pairing-window",
            ])
            .env(
                "DBUS_SYSTEM_BUS_ADDRESS",
                bus_address.unwrap_or(&missing_bus),
            )
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(File::create(&stdout).unwrap())
            .stderr(File::create(&stderr).unwrap());
        let child = command.spawn().unwrap();
        let mut service = Self {
            child,
            stdout,
            stderr,
            endpoint,
            vault,
            before,
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
            let output = std::fs::read_to_string(&service.stdout).unwrap();
            if let Some(ready) = output
                .lines()
                .find_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            {
                assert_eq!(ready["status"], "serving", "{}", service.diagnostics());
                assert_eq!(ready["endpoint"], service.endpoint.to_string());
                return service;
            }
            assert!(
                service.child.try_wait().unwrap().is_none(),
                "service exited before readiness: {}",
                service.diagnostics()
            );
            assert!(
                Instant::now() < deadline,
                "service never reported readiness: {}",
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

    fn signal(&self, signal: libc::c_int) {
        process_status::signal(&self.child, signal);
    }

    fn require_stopped(&mut self, mut emit: impl FnMut()) {
        let deadline = Instant::now() + STOP_BUDGET;
        let status: ExitStatus = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "foreground service must end after observed lock: {}",
                self.diagnostics()
            );
            emit();
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(
            status.success(),
            "clean stop required: {}",
            self.diagnostics()
        );
        assert!(
            TcpStream::connect_timeout(&self.endpoint, Duration::from_millis(100)).is_err(),
            "listener must be closed"
        );
        assert_eq!(
            std::fs::read(&self.vault).unwrap(),
            self.before,
            "lock must not rewrite ciphertext"
        );
        drop(
            Vault::open(&self.vault, &MasterPassword::new(PASSWORD.to_owned()), None)
                .expect("process must release the exclusive vault lock"),
        );
        assert!(!std::fs::read_to_string(&self.stderr)
            .unwrap()
            .contains(PASSWORD));
        let output = std::fs::read_to_string(&self.stdout).unwrap();
        assert_eq!(
            output.lines().count(),
            1,
            "one readiness document, no restart: {output}"
        );
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            process_status::signal(&self.child, libc::SIGCONT);
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[allow(unsafe_code)]
mod process_status {
    use std::process::Child;

    pub(super) fn signal(child: &Child, signal: libc::c_int) {
        let pid = libc::pid_t::try_from(child.id()).unwrap();
        // SAFETY: only this fixture's unreaped owned child PID is addressed;
        // no pointers or process-global signal dispositions are changed here.
        let _ = unsafe { libc::kill(pid, signal) };
    }

    pub(super) fn stopped(child: &Child) -> bool {
        // SAFETY: siginfo_t is a plain C record; zero initialization is valid.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // SAFETY: the writable record is live for this call. WNOWAIT preserves
        // the owned Child's wait/reaping state; only stop events are queried.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                &raw mut info,
                libc::WNOHANG | libc::WSTOPPED | libc::WNOWAIT,
            )
        };
        result == 0 && info.si_code == libc::CLD_STOPPED
    }
}

#[test]
fn sigint_still_stops_and_releases_the_foreground_service() {
    let registry = VaultsToml::new();
    let mut service = Service::start(&registry, None);
    service.signal(libc::SIGINT);
    service.require_stopped(|| {});
}

#[test]
fn terminal_stop_then_resume_ends_the_unlocked_foreground_service() {
    let registry = VaultsToml::new();
    let mut service = Service::start(&registry, None);
    service.signal(libc::SIGTSTP);
    let deadline = Instant::now() + STOP_BUDGET;
    while !process_status::stopped(&service.child) {
        assert!(
            service.child.try_wait().unwrap().is_none(),
            "service must actually stop before resume: {}",
            service.diagnostics()
        );
        assert!(
            Instant::now() < deadline,
            "SIGTSTP did not stop the owned service: {}",
            service.diagnostics()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    service.signal(libc::SIGCONT);
    service.require_stopped(|| {});
}

#[test]
fn event_only_controller_has_no_implicit_idle_deadline() {
    let mut controller = AutoLockController::new(AutoLockConfig::default()).unwrap();
    let future = Instant::now() + Duration::from_secs(172_800);
    assert_eq!(controller.tick(future), LockState::Active);
    assert_eq!(
        controller.tick(future + Duration::from_secs(172_800)),
        LockState::Active
    );
}

#[cfg(target_os = "linux")]
fn lock(bus: &logind_bus::LogindBus) {
    zbus::block_on(bus.owner.emit_signal(
        None::<&str>,
        logind_bus::SESSION,
        "org.freedesktop.login1.Session",
        "Lock",
        &(),
    ))
    .unwrap();
}

#[cfg(target_os = "linux")]
fn sleep(bus: &logind_bus::LogindBus, sleeping: bool) {
    zbus::block_on(bus.owner.emit_signal(
        None::<&str>,
        logind_bus::MANAGER,
        "org.freedesktop.login1.Manager",
        "PrepareForSleep",
        &sleeping,
    ))
    .unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn logind_lock_stops_even_with_a_stalled_handshake() {
    let bus = logind_bus::LogindBus::new();
    let registry = VaultsToml::new();
    let mut service = Service::start(&registry, Some(&bus.address));
    let mut stalled = TcpStream::connect(service.endpoint).unwrap();
    stalled.write_all(b"H").unwrap();
    service.require_stopped(|| lock(&bus));
}

#[cfg(target_os = "linux")]
#[test]
fn logind_sleep_stops_the_foreground_service() {
    let bus = logind_bus::LogindBus::new();
    let registry = VaultsToml::new();
    let mut service = Service::start(&registry, Some(&bus.address));
    service.require_stopped(|| sleep(&bus, true));
}

#[cfg(target_os = "linux")]
#[test]
fn logind_wake_does_not_stop_serving() {
    let bus = logind_bus::LogindBus::new();
    let registry = VaultsToml::new();
    let mut service = Service::start(&registry, Some(&bus.address));
    for _ in 0..10 {
        sleep(&bus, false);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        service.child.try_wait().unwrap().is_none(),
        "wake must not lock: {}",
        service.diagnostics()
    );
    service.signal(libc::SIGINT);
    service.require_stopped(|| {});
}

#[cfg(target_os = "linux")]
#[test]
fn unavailable_logind_warns_but_keeps_local_foreground_serving() {
    let registry = VaultsToml::new();
    let mut service = Service::start(&registry, None);
    let deadline = Instant::now() + STOP_BUDGET;
    loop {
        let stderr = std::fs::read_to_string(&service.stderr).unwrap();
        if stderr.contains("OS lock monitoring unavailable (logind)") {
            break;
        }
        assert!(
            service.child.try_wait().unwrap().is_none(),
            "unavailable optional hook must preserve serving: {}",
            service.diagnostics()
        );
        assert!(
            Instant::now() < deadline,
            "native hook degradation must be reported: {}",
            service.diagnostics()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    service.signal(libc::SIGINT);
    service.require_stopped(|| {});
}

#[cfg(target_os = "linux")]
#[test]
fn shutdown_cancels_stalled_logind_setup() {
    use std::os::unix::net::UnixListener;
    let registry = VaultsToml::new();
    let path = registry.tempdir.path().join("stalled-bus");
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = format!("unix:path={}", path.display());
    let mut service = Service::start(&registry, Some(&address));
    let deadline = Instant::now() + STOP_BUDGET;
    let _connection = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("owned bus listener failed: {error}"),
        }
        assert!(
            Instant::now() < deadline,
            "CLI OS adapter never connected to its configured private bus: {}",
            service.diagnostics()
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    service.signal(libc::SIGINT);
    service.require_stopped(|| {});
}

#[cfg(target_os = "linux")]
#[test]
fn lock_cancels_a_pairing_confirmation_waiting_for_stdin() {
    use hidlins_sync::{address::LocalEndpoint, client::ClientPairingSession, noise::NoiseKeypair};
    let bus = logind_bus::LogindBus::new();
    let registry = VaultsToml::new();
    let mut service = Service::start(&registry, Some(&bus.address));
    let identity = NoiseKeypair::generate().unwrap();
    let endpoint =
        LocalEndpoint::new(Ipv4Addr::LOCALHOST.into(), service.endpoint.port(), 0).unwrap();
    let _pairing = ClientPairingSession::begin(&identity, [endpoint]).unwrap();
    let deadline = Instant::now() + STOP_BUDGET;
    while !std::fs::read_to_string(&service.stderr)
        .unwrap()
        .contains("Pairing SAS:")
    {
        assert!(
            Instant::now() < deadline,
            "authority never reached confirmation: {}",
            service.diagnostics()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    service.require_stopped(|| lock(&bus));
}
