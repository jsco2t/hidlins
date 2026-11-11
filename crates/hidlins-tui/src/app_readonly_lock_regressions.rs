use super::*;
use hidlins_core::{Keyfile, VaultReadOnly};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command as ProcessCommand, Stdio};

fn marker(message: &str) {
    println!("LOCK:{message}");
    std::io::stdout().flush().unwrap();
}

#[test]
#[ignore = "owned-process helper invoked explicitly by the shared-lock tests"]
fn core_lock_holder() {
    let Some(path) = std::env::var_os("HIDLINS_TUI_LOCK_TEST_PATH") else {
        return;
    };
    let keyfile = std::env::var_os("HIDLINS_TUI_LOCK_TEST_KEYFILE")
        .map(PathBuf::from)
        .map(Keyfile::Path);
    let master = MasterPassword::new(PASSWORD.to_string());
    let wait = || {
        marker("acquired");
        let mut input = Vec::new();
        std::io::stdin().read_to_end(&mut input).unwrap();
    };
    match std::env::var("HIDLINS_TUI_LOCK_TEST_MODE")
        .unwrap()
        .as_str()
    {
        "reader" => match VaultReadOnly::open(Path::new(&path), &master, keyfile.as_ref()) {
            Ok(_vault) => wait(),
            Err(VaultError::Contended { .. }) => marker("contended"),
            Err(error) => panic!("invalid reader fixture: {error}"),
        },
        "writer" => match Vault::open(Path::new(&path), &master, keyfile.as_ref()) {
            Ok(_vault) => wait(),
            Err(VaultError::Contended { .. }) => marker("contended"),
            Err(error) => panic!("invalid writer fixture: {error}"),
        },
        _ => panic!("unknown owned lock fixture mode"),
    }
}

struct LockPeer {
    child: Child,
    acquired: bool,
    _directory: TempDir,
}

impl LockPeer {
    fn start(path: &Path, keyfile: Option<&Path>, mode: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let stdout = directory.path().join("lock.stdout");
        let stderr = directory.path().join("lock.stderr");
        let mut command = ProcessCommand::new(std::env::current_exe().unwrap());
        command
            .args([
                "--ignored",
                "--exact",
                "app::tests::readonly_lock_regressions::core_lock_holder",
                "--nocapture",
            ])
            .env("HIDLINS_TUI_LOCK_TEST_PATH", path)
            .env("HIDLINS_TUI_LOCK_TEST_MODE", mode)
            .env_remove("HIDLINS_TUI_LOCK_TEST_KEYFILE")
            .stdin(Stdio::piped())
            .stdout(std::fs::File::create(&stdout).unwrap())
            .stderr(std::fs::File::create(&stderr).unwrap());
        if let Some(keyfile) = keyfile {
            command.env("HIDLINS_TUI_LOCK_TEST_KEYFILE", keyfile);
        }
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let acquired = loop {
            let output = std::fs::read_to_string(&stdout).unwrap();
            if output.contains("LOCK:acquired") {
                break true;
            }
            if output.contains("LOCK:contended") {
                break false;
            }
            assert!(
                child.try_wait().unwrap().is_none(),
                "lock helper exited without a result: {} {}",
                output,
                std::fs::read_to_string(&stderr).unwrap()
            );
            assert!(
                Instant::now() < deadline,
                "owned lock helper did not report acquisition"
            );
            std::thread::sleep(Duration::from_millis(2));
        };
        Self {
            child,
            acquired,
            _directory: directory,
        }
    }
}

impl Drop for LockPeer {
    fn drop(&mut self) {
        drop(self.child.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(2));
                }
                _ => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return;
                }
            }
        }
    }
}

#[test]
fn existing_process_reader_does_not_prevent_readonly_tui_unlock() {
    let (_directory, mut app) = populated_app();
    let path = app.registry().get("personal").unwrap().path.clone();
    let before = std::fs::read(&path).unwrap();
    let reader = LockPeer::start(&path, None, "reader");
    assert!(reader.acquired);
    app.read_only = true;
    unlock(&mut app, PASSWORD);
    assert!(
        app.vault().is_some(),
        "read-only TUI must coexist with a reader that opened first"
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn readonly_tui_allows_later_reader_and_excludes_writers_until_all_readers_close() {
    let (_directory, mut app) = populated_app();
    let path = app.registry().get("personal").unwrap().path.clone();
    let before = std::fs::read(&path).unwrap();
    app.read_only = true;
    unlock(&mut app, PASSWORD);
    let reader = LockPeer::start(&path, None, "reader");
    assert!(
        reader.acquired,
        "another process must share the read-only TUI lock"
    );
    assert!(!LockPeer::start(&path, None, "writer").acquired);
    app.lock_app();
    assert!(!LockPeer::start(&path, None, "writer").acquired);
    drop(reader);
    assert!(LockPeer::start(&path, None, "writer").acquired);
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn two_readonly_tuis_can_inspect_the_same_vault() {
    let (_directory, mut first) = populated_app();
    let registry = VaultRegistry::load(first.paths.clone()).unwrap();
    let mut second =
        App::from_registry(registry, first.paths.clone(), AutoLockConfig::default()).unwrap();
    first.read_only = true;
    second.read_only = true;
    unlock(&mut first, PASSWORD);
    unlock(&mut second, PASSWORD);
    assert!(first.vault().is_some());
    assert!(second.vault().is_some());
}

#[test]
fn writable_tui_keeps_exclusive_ownership() {
    let (_directory, mut app) = populated_app();
    let path = app.registry().get("personal").unwrap().path.clone();
    unlock(&mut app, PASSWORD);
    assert!(!LockPeer::start(&path, None, "reader").acquired);
    assert!(!LockPeer::start(&path, None, "writer").acquired);
    app.lock_app();
    assert!(LockPeer::start(&path, None, "writer").acquired);
}

#[test]
fn readonly_handle_cannot_supply_mutation_or_worker_ownership() {
    let (_directory, mut app) = populated_app();
    app.read_only = true;
    unlock(&mut app, PASSWORD);
    assert!(
        app.vault_mut().is_none(),
        "shared ownership must not expose a mutable Vault"
    );
    assert!(
        app.session.begin_sync().is_none(),
        "a read-only handle cannot move into a mutating worker"
    );
    assert!(app.vault().is_some());
}

#[test]
fn readonly_queries_render_history_clipboard_and_ui_state_preserve_ciphertext() {
    let (_directory, mut app) = populated_app();
    let path = app.registry().get("personal").unwrap().path.clone();
    let master = MasterPassword::new(PASSWORD.to_string());
    let mut writer = Vault::open(&path, &master, None).unwrap();
    let uuid = writer.entry_uuids_by_title("Root1")[0];
    writer
        .update_entry(uuid, |entry| {
            entry.set_password("updated synthetic");
            Ok(())
        })
        .unwrap();
    writer.save().unwrap();
    drop(writer);
    let before = std::fs::read(&path).unwrap();
    app.read_only = true;
    unlock(&mut app, PASSWORD);
    app.tree.select(uuid);
    let entry = app.vault().unwrap().get_entry(uuid).unwrap();
    assert_eq!(entry.history().len(), 1);
    assert_eq!(
        app.vault()
            .unwrap()
            .search(SearchOptions::new("Root1"))
            .unwrap()[0]
            .uuid,
        uuid
    );
    let rendered = crate::test_support::render_at(&app, 100, 30, Instant::now());
    assert!(rendered.transcript.contains("Root1"));
    assert!(!rendered.transcript.contains("updated synthetic"));
    let sink = crate::clipboard::RecordingClipboard::new();
    let log = sink.log.clone();
    app.set_clipboard(Box::new(sink));
    app.do_copy(CopyField::Password);
    assert_eq!(log.borrow().last.as_ref().unwrap().0, "updated synthetic");
    assert_eq!(app.recents.rank(uuid), Some(0));
    app.toggle_pin_selected();
    assert!(app
        .ui_config
        .vault_state("personal")
        .unwrap()
        .pinned
        .contains(&uuid));
    app.execute_command(Command::DeleteEntry, None);
    assert!(app.vault().unwrap().get_entry(uuid).is_ok());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn readonly_keyfile_vault_shares_its_lock_and_releases_it_on_idle_lock() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("keyed.kdbx");
    let keyfile = directory.path().join("fixture.key");
    std::fs::write(&keyfile, b"synthetic read-only fixture keyfile").unwrap();
    drop(
        Vault::create(
            &path,
            &MasterPassword::new(PASSWORD.to_string()),
            Some(&Keyfile::Path(keyfile.clone())),
            fast_kdf(),
            NoRecoveryConfirmed::yes(),
        )
        .unwrap(),
    );
    let paths = HidlinsPaths::with_state_dir(directory.path().join("state"));
    let mut registry = VaultRegistry::with_paths(paths.clone());
    registry
        .register_and_save(RegisteredVault {
            name: "personal".to_string(),
            path: path.clone(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            keyfile_path: Some(keyfile.clone()),
            extra: toml::Table::new(),
        })
        .unwrap();
    let mut app = App::from_registry(
        registry,
        paths,
        AutoLockConfig {
            idle_timeout: Duration::from_secs(1),
        },
    )
    .unwrap();
    app.read_only = true;
    unlock(&mut app, PASSWORD);
    let reader = LockPeer::start(&path, Some(&keyfile), "reader");
    assert!(reader.acquired);
    let now = Instant::now();
    app.tick(now + Duration::from_secs(2));
    app.tick(now + Duration::from_secs(3));
    assert!(matches!(app.phase, Phase::LockScreen));
    drop(reader);
    assert!(LockPeer::start(&path, Some(&keyfile), "writer").acquired);
}
