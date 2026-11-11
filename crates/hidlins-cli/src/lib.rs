//! Hidlins CLI library entry point.
//!
//! The binary at `src/main.rs` is a thin shim around [`run`]; integration
//! tests link against this library so they can exercise individual
//! modules without forking a subprocess.
//!
//! ## Layered architecture (per design §2.1)
//!
//! 1. `cli` — clap-derive command tree. Pure parsing, no I/O.
//! 2. `exit` — [`CliExit`] enum + exhaustive `From` impls. Pure mapping.
//! 3. `format` — `OutputFormatter` (JSON vs. human). Pure formatting.
//! 4. `prompt` — secure master-password prompt (FR-061).
//! 5. `agent` — `AgentClient` trait + `NoAgentClient` MVP impl.
//! 6. `commands` — implemented per-subcommand dispatch functions.
//! 7. `views` — the shipped per-subcommand serializable output contracts.

// `deny` (not `forbid`): the one production opt-out is the audited
// async-signal-safe echo-restore handler in `prompt::echo_signals`
// (Drop does not run on signal death, so a plain RAII guard cannot
// restore the terminal on ^C). Mirrors hidlins-tui's terminal_guard.
#![deny(unsafe_code)]
#![deny(missing_docs)]

pub mod agent;
pub mod cli;
pub mod commands;
pub mod exit;
pub mod format;
pub mod prompt;
pub mod views;

pub use cli::{Cli, Command, OutputFormat};
pub use exit::CliExit;

use clap::{parser::ValueSource, CommandFactory, FromArgMatches};
use std::process::ExitCode;

use crate::format::OutputFormatter;

/// Library entry point — the binary's `main()` is a one-line call to
/// this function.
///
/// Parses the command line via [`Cli`], dispatches to the matching
/// subcommand handler, and translates the resulting [`CliExit`] into a
/// `std::process::ExitCode`. Errors are routed through
/// [`OutputFormatter::write_error`] so JSON-mode envelopes land on
/// stdout (design §3.4) and human-mode errors land on stderr.
#[must_use]
pub fn run() -> ExitCode {
    // FR-061 defense-in-depth: if HIDLINS_MASTER_PASSWORD is set in the
    // environment when the CLI starts, warn-and-remove it before any
    // subcommand runs (so subprocess invocations cannot inherit the
    // bypass attempt). Done here rather than per-subcommand because the
    // env-var inspection is process-global state.
    prompt::scrub_env_master_password(&mut std::io::stderr());

    let mut command = Cli::command();
    let matches = command.clone().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());
    let cli_mode = cli.command.is_some()
        || cli.force_cli
        || matches.value_source("format") == Some(ValueSource::CommandLine);
    let tui_only = cli.vault.is_some()
        || cli.theme.is_some()
        || cli.no_mouse
        || cli.read_only
        || cli.dump_keys.is_some();
    let config_unused =
        cli.config.is_some() && cli_mode && !matches!(cli.command, Some(Command::Keys(_)));
    if (cli_mode && tui_only) || config_unused {
        command
            .error(
                clap::error::ErrorKind::ArgumentConflict,
                "TUI options cannot be used in CLI mode; --config is also accepted by `keys`",
            )
            .exit();
    }
    if !cli_mode {
        return run_tui(&cli);
    }
    let format = cli.format;
    let result = dispatch(&cli);
    map_result_to_exit_code(result, format)
}

fn dispatch(cli: &Cli) -> Result<(), CliExit> {
    match &cli.command {
        Some(Command::Vault(args)) => commands::vault::run(cli, args),
        Some(Command::Entry(args)) => commands::entry::run(cli, args),
        Some(Command::Gen(args)) => commands::gen::run(cli, args),
        Some(Command::Sync(args)) => commands::sync::run(cli, args),
        Some(Command::Ssh(args)) => commands::ssh::run(cli, args),
        Some(Command::Completions(args)) => commands::completions::run(cli, args),
        Some(Command::Keys(args)) => commands::keys::run(cli, args),
        None => Cli::command()
            .print_help()
            .map_err(|error| CliExit::Internal(format!("failed to write help: {error}"))),
    }
}

#[cfg(feature = "desktop")]
fn run_tui(cli: &Cli) -> ExitCode {
    match hidlins_tui::run(&tui_args(cli)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(feature = "desktop"))]
fn run_tui(_cli: &Cli) -> ExitCode {
    map_result_to_exit_code(Err(desktop_required()), OutputFormat::Human)
}

#[cfg(not(feature = "desktop"))]
fn desktop_required() -> CliExit {
    CliExit::UserError(
        "TUI and keymap operations require a desktop build; use --cli or a CLI subcommand"
            .to_string(),
    )
}

#[cfg(feature = "desktop")]
fn tui_args(cli: &Cli) -> hidlins_tui::Args {
    hidlins_tui::Args {
        registry: cli.registry.clone(),
        vault: cli.vault.clone(),
        theme: cli.theme.clone(),
        config: cli.config.clone(),
        no_mouse: cli.no_mouse,
        read_only: cli.read_only,
        dump_keys: cli.dump_keys.map(|format| match format {
            cli::DumpKeysFormat::Text => hidlins_tui::DumpFormat::Text,
            cli::DumpKeysFormat::Json => hidlins_tui::DumpFormat::Json,
        }),
    }
}

fn map_result_to_exit_code(result: Result<(), CliExit>, format: OutputFormat) -> ExitCode {
    let exit = match result {
        Ok(()) => CliExit::Success,
        Err(e) => e,
    };
    if !matches!(exit, CliExit::Success) {
        let stdout = std::io::stdout();
        let stderr = std::io::stderr();
        let mut formatter = OutputFormatter::new(stdout.lock(), stderr.lock(), format);
        let _ = formatter.write_error(&exit);
    }
    // `i32 -> u8` is safe — all documented codes fit in 0..=255 (see
    // exit.rs documented table).
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    ExitCode::from(exit.code() as u8)
}

#[cfg(all(test, feature = "desktop"))]
mod console_args_tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parsed_tui_settings_cross_the_library_boundary_without_losing_overrides() {
        let cli = Cli::try_parse_from([
            "hidlins",
            "--registry=selected.toml",
            "--vault",
            "work",
            "--theme=slate",
            "--config",
            "custom.toml",
            "--no-mouse",
            "--read-only",
            "--dump-keys=json",
        ])
        .unwrap();
        let args = tui_args(&cli);
        assert_eq!(
            args.registry.as_deref(),
            Some(std::path::Path::new("selected.toml"))
        );
        assert_eq!(args.vault.as_deref(), Some("work"));
        assert_eq!(args.theme.as_deref(), Some("slate"));
        assert_eq!(
            args.config.as_deref(),
            Some(std::path::Path::new("custom.toml"))
        );
        assert!(args.no_mouse);
        assert!(args.read_only);
        assert_eq!(args.dump_keys, Some(hidlins_tui::DumpFormat::Json));

        let defaults = Cli::try_parse_from(["hidlins"]).unwrap();
        assert_eq!(tui_args(&defaults), hidlins_tui::Args::default());
        let text = Cli::try_parse_from(["hidlins", "--vault=personal", "--dump-keys"]).unwrap();
        assert_eq!(
            tui_args(&text).dump_keys,
            Some(hidlins_tui::DumpFormat::Text)
        );
        assert_eq!(tui_args(&text).vault.as_deref(), Some("personal"));
    }
}
