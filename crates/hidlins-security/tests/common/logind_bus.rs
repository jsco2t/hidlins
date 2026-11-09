//! Real private D-Bus transport with a deliberately small logind service.
//! Nothing connects to, emits on, or changes the host's system bus.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

pub const SERVICE: &str = "org.freedesktop.login1";
pub const MANAGER: &str = "/org/freedesktop/login1";
pub const SESSION: &str = "/org/freedesktop/login1/session/hidlins_fixture";

struct Manager;

#[zbus::interface(name = "org.freedesktop.login1.Manager")]
#[allow(clippy::unused_self)] // D-Bus interface methods require a receiver.
impl Manager {
    #[zbus(name = "GetSessionByPID")]
    fn get_session_by_pid(&self, pid: u32) -> zbus::OwnedObjectPath {
        assert!(pid > 0, "the fixture expects a real process ID");
        SESSION.try_into().unwrap()
    }
}

struct Session;

#[zbus::interface(name = "org.freedesktop.login1.Session")]
#[allow(clippy::unused_self)] // D-Bus interface methods require a receiver.
impl Session {
    fn activate(&self) {}
}

struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub struct LogindBus {
    pub address: String,
    pub owner: zbus::Connection,
    _daemon: Daemon,
}

impl LogindBus {
    pub fn new() -> Self {
        let mut daemon = Daemon(
            Command::new("dbus-daemon")
                .args(["--session", "--nofork", "--print-address=1"])
                .stdout(Stdio::piped())
                .spawn()
                .expect("install dbus-daemon to run the isolated logind contracts"),
        );
        let mut address = String::new();
        BufReader::new(daemon.0.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let address = address.trim().to_owned();
        assert!(
            !address.is_empty(),
            "private daemon must publish its address"
        );
        let owner = zbus::block_on(async {
            zbus::connection::Builder::address(address.as_str())
                .name(SERVICE)
                .serve_at(MANAGER, Manager)
                .serve_at(SESSION, Session)
                .build()
                .await
                .unwrap()
        });
        // Prove that the fixture's real owner and method dispatch are ready.
        let path: zbus::OwnedObjectPath = zbus::block_on(async {
            let probe =
                zbus::Proxy::new(&owner, SERVICE, MANAGER, "org.freedesktop.login1.Manager")
                    .await
                    .unwrap();
            probe
                .call("GetSessionByPID", &std::process::id())
                .await
                .unwrap()
        });
        assert_eq!(path.as_str(), SESSION);
        Self {
            address,
            owner,
            _daemon: daemon,
        }
    }
}
