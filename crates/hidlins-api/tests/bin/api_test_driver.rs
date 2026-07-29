//! Headless test driver for the `hidlins-api` boundary crate.
//!
//! Used by the interop shell scripts (`tools/interop-tests/`) to
//! exercise the API surface from the command line. Runs with fast-KDF
//! parameters and exits non-zero on any failure.
//!
//! Subcommands:
//!   create-vault <dir> <name> <password>
//!   add-corpus   <dir> <name> <password>
//!   edit-entry   <dir> <name> <entry-uuid> <field>
//!                 Reads two lines from stdin: password, then value.

use std::process::ExitCode;

use hidlins_api::fixtures;
use hidlins_core::HidlinsPaths;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: api-test-driver <subcommand> [args...]");
        eprintln!("subcommands: create-vault, add-corpus, edit-entry");
        return ExitCode::from(1);
    }

    let result = match args[1].as_str() {
        "create-vault" => cmd_create_vault(&args[2..]),
        "add-corpus" => cmd_add_corpus(&args[2..]),
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

fn cmd_create_vault(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 3 {
        return Err("usage: create-vault <dir> <name> <password>".into());
    }
    let paths = HidlinsPaths::with_state_dir(args[0].clone().into());
    paths.ensure_exists()?;
    let path = fixtures::create_test_vault(&paths, &args[1], &args[2]);
    println!("created: {}", path.display());
    Ok(())
}

fn cmd_add_corpus(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 3 {
        return Err("usage: add-corpus <dir> <name> <password>".into());
    }
    let paths = HidlinsPaths::with_state_dir(args[0].clone().into());
    fixtures::add_corpus_entries(&paths, &args[1], &args[2]);
    println!("corpus added to {}", args[1]);
    Ok(())
}

fn cmd_edit_entry(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 4 {
        return Err("usage: edit-entry <dir> <name> <entry-uuid> <field>\n\
                     reads two lines from stdin: password, then value"
            .into());
    }
    let paths = HidlinsPaths::with_state_dir(args[0].clone().into());

    let mut password = String::new();
    std::io::stdin().read_line(&mut password)?;
    let password = password.trim_end().to_string();
    let master = hidlins_core::MasterPassword::new(password);

    let vault_path = paths.state_dir().join(format!("{}.kdbx", &args[1]));
    let mut vault = hidlins_core::Vault::open(&vault_path, &master, None)?;

    let uuid: hidlins_core::Uuid = args[2].parse().map_err(|_| "invalid UUID")?;
    let field = &args[3];

    let mut value = String::new();
    std::io::stdin().read_line(&mut value)?;
    let value = value.trim_end().to_string();

    vault.update_entry(uuid, |view| {
        match field.as_str() {
            "username" => view.set_username(&value),
            "password" => view.set_password(&value),
            "url" => view.set_url(&value),
            "notes" => view.set_notes(&value),
            "title" => view.set_title(&value),
            other => {
                view.set_custom_field(other, &value, false);
            }
        }
        Ok(())
    })?;

    vault.save()?;
    println!("edited {uuid} field={field}");
    Ok(())
}
