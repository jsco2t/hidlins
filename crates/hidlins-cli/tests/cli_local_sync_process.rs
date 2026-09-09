//! Real spawned-process CLI journey for local serving, pairing, import, and cleanup.

mod common;

use std::{
    io::{BufRead as _, BufReader, Write as _},
    process::Stdio,
};

use common::{hidlins_cmd, run_with_stdin, seed_vault, VaultsToml};
use hidlins_core::{HidlinsPaths, RegisteredVault, VaultRegistry};

fn sync_address() -> String {
    std::env::var("HIDLINS_TEST_SYNC_ADDRESS").unwrap_or_else(|_| "127.0.0.1".to_string())
}

fn register_copy(registry: &VaultsToml, source: &std::path::Path, name: &str) {
    let path = registry.tempdir.path().join(format!("{name}.kdbx"));
    std::fs::copy(source, &path).expect("copy vault");
    let mut records = VaultRegistry::load(HidlinsPaths::with_registry_file(
        registry.vaults_toml.clone(),
    ))
    .expect("load client registry");
    records
        .register(RegisteredVault {
            name: name.into(),
            path,
            created_at: "2026-01-01T00:00:00Z".into(),
            keyfile_path: None,
            extra: toml::Table::new(),
        })
        .expect("register copy");
    records.save().expect("save client registry");
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one ordered multi-process lifecycle journey"
)]
fn spawned_cli_local_sync_journey_and_signal_cleanup() {
    // LNS-PROCESS-001: this test is replayed in Linux network namespaces on
    // private IPv4 and IPv6 ULA addresses by tools/local-sync-tests/.
    let address = sync_address();
    let server = VaultsToml::new();
    let client = VaultsToml::new();
    let imported = VaultsToml::new();
    let server_vault = seed_vault(&server, "authority", "test-master");
    register_copy(&client, &server_vault, "client");

    let port = 40_000 + u16::try_from(std::process::id() % 20_000).expect("bounded process id");
    let mut command = hidlins_cmd();
    command
        .args([
            "--registry",
            &server.registry_arg(),
            "--format",
            "json",
            "sync",
            "serve",
            "--vault",
            "authority",
            "--address",
            &address,
            "--port",
            &port.to_string(),
            "--pairing-window",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut service = command.spawn().expect("spawn foreground service");
    service
        .stdin
        .as_mut()
        .expect("server stdin")
        .write_all(b"test-master\nyes\nyes\n")
        .expect("preload secure prompt and bilateral confirmations");
    let mut readiness = String::new();
    BufReader::new(service.stdout.take().expect("server stdout"))
        .read_line(&mut readiness)
        .expect("read readiness");
    let ready: serde_json::Value = serde_json::from_str(&readiness).expect("JSON readiness");
    assert_eq!(
        ready["status"], "serving",
        "unexpected readiness: {readiness}"
    );
    let expected_endpoint = if address.contains(':') {
        format!("[{address}]:{port}")
    } else {
        format!("{address}:{port}")
    };
    assert_eq!(ready["endpoint"], expected_endpoint);

    let port_text = port.to_string();
    let pair = run_with_stdin(
        &client,
        &[
            "--format",
            "json",
            "sync",
            "pair",
            "--vault",
            "client",
            "--address",
            &address,
            "--port",
            &port_text,
        ],
        "test-master\nyes\n",
    );
    assert_eq!(pair.0, 0, "pair failed: {} {}", pair.1, pair.2);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&pair.1).unwrap()["status"],
        "paired"
    );
    assert!(
        service.try_wait().expect("server status").is_none(),
        "server exited after pair"
    );
    let first_sync = run_with_stdin(
        &client,
        &[
            "sync",
            "now",
            "--vault",
            "client",
            "--address",
            &address,
            "--port",
            &port_text,
        ],
        "test-master\n",
    );
    assert_eq!(
        first_sync.0, 0,
        "first paired sync failed: {} {}",
        first_sync.1, first_sync.2
    );

    let peers = run_with_stdin(
        &server,
        &[
            "--format",
            "json",
            "sync",
            "peers",
            "list",
            "--vault",
            "authority",
        ],
        "",
    );
    assert_eq!(peers.0, 0, "peer list failed: {}", peers.2);
    assert!(peers.1.contains("peer-0"));
    assert!(!peers.1.contains("public_key"));

    let import_path = imported.tempdir.path().join("imported.kdbx");
    let import = run_with_stdin(
        &imported,
        &[
            "--format",
            "json",
            "sync",
            "import",
            "--id",
            "imported",
            "--path",
            import_path.to_str().unwrap(),
            "--address",
            &address,
            "--port",
            &port_text,
        ],
        "test-master\nyes\n",
    );
    assert!(
        service.try_wait().expect("server status").is_none(),
        "server exited during import: {} {}",
        import.1,
        import.2
    );
    assert_eq!(import.0, 0, "import failed: {} {}", import.1, import.2);
    assert!(import_path.exists());

    let now = run_with_stdin(
        &imported,
        &[
            "--format",
            "json",
            "sync",
            "now",
            "--vault",
            "imported",
            "--address",
            &address,
            "--port",
            &port_text,
        ],
        "test-master\n",
    );
    assert_eq!(now.0, 0, "manual sync failed: {} {}", now.1, now.2);

    let rename = run_with_stdin(
        &server,
        &[
            "sync",
            "peers",
            "rename",
            "--vault",
            "authority",
            "--peer",
            "peer-0",
            "--name",
            "laptop",
        ],
        "",
    );
    assert_eq!(rename.0, 0, "rename failed: {}", rename.2);
    let revoke = run_with_stdin(
        &server,
        &[
            "sync",
            "peers",
            "revoke",
            "--vault",
            "authority",
            "--peer",
            "peer-0",
        ],
        "",
    );
    assert_eq!(revoke.0, 0, "revoke failed: {}", revoke.2);

    let signal = std::process::Command::new("kill")
        .args(["-INT", &service.id().to_string()])
        .status()
        .expect("send SIGINT to foreground service");
    assert!(signal.success());
    let status = service.wait().expect("wait for foreground service");
    assert!(status.success(), "service did not exit cleanly: {status}");
    assert!(std::net::TcpStream::connect((address.as_str(), port)).is_err());

    let offline_mutation = run_with_stdin(
        &client,
        &[
            "entry",
            "add",
            "--vault",
            "client",
            "--title",
            "offline-local",
        ],
        "test-master\n",
    );
    assert_eq!(
        offline_mutation.0, 0,
        "offline local mutation failed: {}",
        offline_mutation.2
    );
    assert_eq!(
        offline_mutation
            .2
            .matches("warning: pre-operation local sync failed")
            .count(),
        1,
        "startup must run once and never after save: {}",
        offline_mutation.2
    );
    let offline_read = run_with_stdin(
        &client,
        &["entry", "list", "--vault", "client"],
        "test-master\n",
    );
    assert_eq!(
        offline_read.0, 0,
        "offline local read failed: {}",
        offline_read.2
    );
    assert_eq!(
        offline_read
            .2
            .matches("warning: pre-operation local sync failed")
            .count(),
        1
    );
}

#[test]
fn public_literal_is_rejected_before_network_use() {
    let reg = VaultsToml::new();
    seed_vault(&reg, "authority", "test-master");
    let result = run_with_stdin(
        &reg,
        &[
            "sync",
            "serve",
            "--vault",
            "authority",
            "--address",
            "8.8.8.8",
            "--port",
            "37371",
        ],
        "test-master\n",
    );
    assert_eq!(result.0, 1, "public address accepted: {}", result.2);
    assert!(result.2.contains("non-public IP literal"));
}
