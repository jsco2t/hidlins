//! Headless test driver for the `hidlins-api` boundary crate.
//!
//! Used by the interop shell scripts (`tools/interop-tests/`) to
//! exercise the public `AppSession` surface from the command line. The
//! corpus-seeding helpers used only for performance/timer setup retain
//! direct fixture access; interop create/read/write commands do not.
//!
//! Subcommands:
//!   create-vault <dir> <name>
//!   add-corpus   <dir> <name>
//!   seed-search-corpus <dir> <name> <count>
//!   find-entry   <dir> <name> <title>
//!   set-lock-timeout <dir> <name> <seconds>
//!   edit-entry   <dir> <name> <entry-uuid> <field>
//!
//! Every password-bearing command reads its password from stdin. `edit-entry`
//! reads a second line containing the new value.

use std::process::ExitCode;

use hidlins_api::api::session::{init_app, AppSession};
use hidlins_api::dto::{AppInitConfig, EntryDraftDto, EntryEditDto, EntryKindDto};
use hidlins_core::HidlinsPaths;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: api-test-driver <subcommand> [args...]");
        eprintln!(
            "subcommands: create-vault, add-corpus, seed-search-corpus, \
             find-entry, set-lock-timeout, edit-entry"
        );
        return ExitCode::from(1);
    }

    let result = match args[1].as_str() {
        "create-vault" => cmd_create_vault(&args[2..]),
        "add-corpus" => cmd_add_corpus(&args[2..]),
        "seed-search-corpus" => cmd_seed_search_corpus(&args[2..]),
        "find-entry" => cmd_find_entry(&args[2..]),
        "set-lock-timeout" => cmd_set_lock_timeout(&args[2..]),
        "edit-entry" => cmd_edit_entry(&args[2..]),
        other => {
            eprintln!("unknown subcommand: {other}");
            Err("unknown subcommand".into())
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_seed_search_corpus(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 3 {
        return Err("usage: seed-search-corpus <dir> <name> <count>".into());
    }
    let count: usize = args[2].parse()?;
    let paths = HidlinsPaths::with_state_dir(args[0].clone().into());
    let master = hidlins_core::MasterPassword::new(read_stdin_line()?);
    let vault_path = paths.state_dir().join(format!("{}.kdbx", &args[1]));
    let mut vault = hidlins_core::Vault::open(&vault_path, &master, None)?;
    let root = vault.root_group_uuid();
    for index in 0..count {
        let entry = hidlins_core::EntryBuilder::credential(format!("Search Entry {index:05}"))
            .username(format!("user-{index:05}"))
            .password(format!("test-secret-{index:05}"))
            .build();
        vault.add_entry(root, entry)?;
    }
    vault.save()?;
    println!("seeded {count} search entries in {}", args[1]);
    Ok(())
}

fn cmd_find_entry(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 3 {
        return Err("usage: find-entry <dir> <name> <title>".into());
    }
    let session = unlocked_session(&args[0], &args[1], read_stdin_line()?)?;
    let uuid = session
        .vault_tree()?
        .entries
        .into_iter()
        .find(|entry| entry.title == args[2])
        .map(|entry| entry.uuid)
        .ok_or("entry title not found")?;
    println!("{uuid}");
    session.shutdown();
    Ok(())
}

fn cmd_set_lock_timeout(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 3 {
        return Err("usage: set-lock-timeout <dir> <name> <seconds>".into());
    }
    let seconds: u64 = args[2].parse()?;
    if seconds == 0 {
        return Err("seconds must be at least 1".into());
    }
    let paths = HidlinsPaths::with_state_dir(args[0].clone().into());
    let mut registry = hidlins_core::VaultRegistry::load(paths)?;
    registry.update_registered_extra(&args[1], |extra| {
        hidlins_security::VaultLockConfig::apply_idle_timeout(extra, Some(seconds));
    })?;
    println!("set {} idle timeout to {seconds}s", args[1]);
    Ok(())
}

fn cmd_create_vault(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        return Err("usage: create-vault <dir> <name>".into());
    }
    let session = test_session(&args[0])?;
    let summary = session.create_vault(args[1].clone(), read_stdin_line()?, None, None, true)?;
    println!("created: {}", summary.path);
    session.shutdown();
    Ok(())
}

fn cmd_add_corpus(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        return Err("usage: add-corpus <dir> <name>".into());
    }
    let session = unlocked_session(&args[0], &args[1], read_stdin_line()?)?;
    let root = session.vault_tree()?.root.uuid;
    let entries = [
        credential(
            "GitHub",
            Some("octocat"),
            Some("gh-secret-123"),
            Some("https://github.com"),
        ),
        credential(
            "Bank",
            Some("banker"),
            Some("bank-secret-456"),
            Some("https://bank.test"),
        ),
        credential(
            "Email",
            Some("user@test.com"),
            Some("email-secret-789"),
            None,
        ),
        EntryDraftDto {
            kind: EntryKindDto::SecureNote,
            title: "Recovery Codes".to_string(),
            username: None,
            password: None,
            url: None,
            notes: None,
            tags: Vec::new(),
            custom_fields: Vec::new(),
            totp_uri: None,
        },
    ];
    for draft in entries {
        session.create_entry(root.clone(), draft)?;
    }
    println!("corpus added to {}", args[1]);
    session.shutdown();
    Ok(())
}

fn cmd_edit_entry(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 4 {
        return Err("usage: edit-entry <dir> <name> <entry-uuid> <field>\n\
                     reads two lines from stdin: password, then value"
            .into());
    }
    let session = unlocked_session(&args[0], &args[1], read_stdin_line()?)?;

    // The boundary performs UUID validation; do not pre-parse it here.
    let uuid = args[2].clone();
    let field = &args[3];

    let value = read_stdin_line()?;
    let mut edit = EntryEditDto {
        title: None,
        username: None,
        password: None,
        url: None,
        notes: None,
        tags: None,
        custom_fields: None,
        totp_uri: None,
    };
    match field.as_str() {
        "username" => edit.username = Some(value),
        "password" => edit.password = Some(value),
        "url" => edit.url = Some(value),
        "notes" => edit.notes = Some(value),
        "title" => edit.title = Some(value),
        other => return Err(format!("unsupported API-boundary field: {other}").into()),
    }
    session.update_entry(uuid.clone(), edit)?;
    println!("edited {uuid} field={field}");
    session.shutdown();
    Ok(())
}

fn read_stdin_line() -> Result<String, Box<dyn std::error::Error>> {
    let mut value = String::new();
    if std::io::stdin().read_line(&mut value)? == 0 {
        return Err("expected a line on stdin".into());
    }
    value.truncate(value.trim_end().len());
    Ok(value)
}

fn test_session(dir: &str) -> Result<AppSession, Box<dyn std::error::Error>> {
    Ok(init_app(AppInitConfig {
        state_dir: Some(dir.to_string()),
        config_dir: None,
    })?)
}

fn unlocked_session(
    dir: &str,
    name: &str,
    password: String,
) -> Result<AppSession, Box<dyn std::error::Error>> {
    let session = test_session(dir)?;
    session.unlock(name.to_string(), password, None)?;
    Ok(session)
}

fn credential(
    title: &str,
    username: Option<&str>,
    password: Option<&str>,
    url: Option<&str>,
) -> EntryDraftDto {
    EntryDraftDto {
        kind: EntryKindDto::Credential,
        title: title.to_string(),
        username: username.map(str::to_string),
        password: password.map(str::to_string),
        url: url.map(str::to_string),
        notes: None,
        tags: Vec::new(),
        custom_fields: Vec::new(),
        totp_uri: None,
    }
}
