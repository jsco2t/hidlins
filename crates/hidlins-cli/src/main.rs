//! Unified `hidlins` console binary — TUI by default, CLI for commands/--cli.

fn main() -> std::process::ExitCode {
    hidlins_cli::run()
}
