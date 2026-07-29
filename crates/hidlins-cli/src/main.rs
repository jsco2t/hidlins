//! `hidlins` binary — thin shim over [`hidlins_cli::run`].

fn main() -> std::process::ExitCode {
    hidlins_cli::run()
}
