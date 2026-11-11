//! `hidlins keys` — print the effective TUI keymap in the current process.

use crate::cli::{Cli, KeysArgs};
use crate::exit::CliExit;

/// Print the configured keymap without terminal setup or registry access.
/// The global `--format` and `--config` work before or after `keys`.
///
/// # Errors
/// Returns a user error when this build has no desktop UI support, or an
/// internal error when writing the keymap to stdout fails.
pub fn run(cli: &Cli, _args: &KeysArgs) -> Result<(), CliExit> {
    #[cfg(feature = "desktop")]
    {
        let mut args = crate::tui_args(cli);
        args.dump_keys = Some(match cli.format {
            crate::OutputFormat::Human => hidlins_tui::DumpFormat::Text,
            crate::OutputFormat::Json => hidlins_tui::DumpFormat::Json,
        });
        hidlins_tui::run(&args).map_err(|error| {
            CliExit::Internal(format!("failed to write keymap to stdout: {error}"))
        })
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = cli;
        Err(crate::desktop_required())
    }
}
