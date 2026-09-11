//! Top-level command tree.
//!
//! Defines [`Cli`], the complete [`Command`] subcommand enum, and each
//! subcommand's arguments and verbs.
//!
//! ## ASCII-art header
//!
//! The `--help` output is prefixed with a `Hidlins` ASCII-art banner via
//! clap's `before_help`. The banner is preserved verbatim from the
//! project owner's source — see [`BANNER`].

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// ASCII-art banner shown above every `hidlins --help` output.
pub const BANNER: &str = r"
 _   _ ___ ____  _     ___ _   _ ____
| | | |_ _|  _ \| |   |_ _| \ | / ___|
| |_| || || | | | |    | ||  \| \___ \
|  _  || || |_| | |___ | || |\  |___) |
|_| |_|___|____/|_____|___|_| \_|____/

";

/// Master-password environment variable: detected at startup and ignored
/// with a stderr warning. Documented as reserved.
pub const MASTER_PASSWORD_ENV_VAR: &str = "HIDLINS_MASTER_PASSWORD";

const AFTER_HELP: &str = "\
Master password is collected via a secure stdin prompt (no echo). The \
HIDLINS_MASTER_PASSWORD environment variable, if set, is ignored and \
removed from the process environment at startup. There is no \
--master-password flag by design.";

/// Top-level `hidlins` command.
#[derive(Parser, Debug)]
#[command(
    name = "hidlins",
    version,
    about = "Hidlins — keeper of secrets. Offline-first KDBX secrets manager.",
    before_help = BANNER,
    after_help = AFTER_HELP,
)]
pub struct Cli {
    /// The subcommand to dispatch.
    #[command(subcommand)]
    pub command: Command,

    /// Output format. `human` (default) is line-oriented for terminals;
    /// `json` is the stable machine-readable schema for scripts.
    #[arg(long, value_enum, global = true, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,

    /// Path to the vault registry file. Defaults to
    /// `$HOME/.local/state/hidlins/vaults.toml` (per
    /// `hidlins_core::HidlinsPaths`).
    #[arg(long, global = true)]
    pub registry: Option<PathBuf>,
}

/// Top-level subcommand selector.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Vault lifecycle and registration commands.
    Vault(VaultArgs),
    /// Secret-entry CRUD, search, and TOTP commands.
    Entry(EntryArgs),
    /// Password and passphrase generation.
    Gen(GenArgs),
    /// Pair and synchronize vaults over the local network.
    Sync(SyncArgs),
    /// SSH key entry management.
    ///
    /// MVP slot — body returns exit 11 (`not.implemented`). The real
    /// implementation arrives with `features/ssh-keys/`; flag surface
    /// is declared here as the forward-compat contract.
    Ssh(SshArgs),
    /// Generate shell completion scripts.
    ///
    /// Supported shells: `bash`, `zsh`, `fish` (documented), plus
    /// `powershell` and `elvish` (accepted; not in the Phase-0 support
    /// matrix). Typical use: `hidlins completions bash > ~/.hidlins-completions.bash`
    /// then `source ~/.hidlins-completions.bash` from your shell rc.
    /// The `make completions` Makefile target writes pre-generated
    /// scripts into `shell-completions/` for packaging.
    Completions(CompletionsArgs),
    /// Print the TUI's effective keymap (command name, keys, description, group).
    ///
    /// Relays `hidlins-tui --dump-keys` — locate the TUI binary on `$PATH` (or
    /// via `$HIDLINS_TUI_BIN`). Spawns a subprocess but passes NO secret
    /// material: keymap data only. Closes tui-skeleton sibling contract #6.
    Keys(KeysArgs),
}

/// Flags for `hidlins keys`.
#[derive(Args, Debug)]
pub struct KeysArgs {
    /// Output format: `human` (default) or `json`.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

/// `--format` selector. Default `human`.
#[derive(ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// Line-oriented human-readable output (default).
    #[default]
    Human,
    /// Stable JSON schema — see `views/` for per-subcommand shapes.
    Json,
}

// ---------------------------------------------------------------------------
// Per-subcommand argument structs. These types are the shipped command-line
// contract, so help text, completions, and parsing tests keep them stable.
// ---------------------------------------------------------------------------

/// `hidlins vault` — vault lifecycle.
#[derive(Args, Debug)]
pub struct VaultArgs {
    /// Verb to execute.
    #[command(subcommand)]
    pub verb: Option<VaultVerb>,
}

/// Verbs accepted by `hidlins vault`.
#[derive(Subcommand, Debug)]
pub enum VaultVerb {
    /// Create a new vault and register it.
    Create(VaultCreateArgs),
    /// Authenticate and register an existing KDBX vault.
    Register(VaultRegisterArgs),
    /// Probe vault unlock with the given master password.
    ///
    /// In Phase 0 MVP this is a one-shot probe: prompts for the master
    /// password, attempts unlock, prints success or maps to exit code 2
    /// on auth failure, then exits. When the hidlins agent ships
    /// (post-MVP), this command caches the unlocked vault for
    /// subsequent commands.
    Open(VaultOpenArgs),
    /// List registered vaults.
    List(VaultListArgs),
    /// Configure the per-vault idle-lock timeout.
    SetLock(VaultSetLockArgs),
}

/// Flags for `hidlins vault create`.
#[derive(Args, Debug)]
pub struct VaultCreateArgs {
    /// Registry name for the new vault (unique).
    #[arg(long)]
    pub id: String,
    /// Absolute or relative path where the `.kdbx` file will be created.
    #[arg(long)]
    pub path: std::path::PathBuf,
    /// Optional keyfile required to unlock this vault.
    #[arg(long)]
    pub keyfile: Option<std::path::PathBuf>,
    /// Acknowledge the no-recovery warning. Required — there is no
    /// master-password recovery in Hidlins.
    #[arg(long)]
    pub no_recovery_warning: bool,
}

/// Flags for `hidlins vault register`.
#[derive(Args, Debug)]
pub struct VaultRegisterArgs {
    /// Registry name for the existing vault (unique).
    #[arg(long)]
    pub id: String,
    /// Path to an existing KDBX vault.
    #[arg(long)]
    pub path: std::path::PathBuf,
    /// Optional keyfile required to unlock this vault.
    #[arg(long)]
    pub keyfile: Option<std::path::PathBuf>,
}

/// Flags for `hidlins vault open`.
#[derive(Args, Debug)]
pub struct VaultOpenArgs {
    /// Registry name of the vault to probe.
    #[arg(long)]
    pub id: String,
}

/// Flags for `hidlins vault list` (none today; struct exists for
/// forward-compat with future `--filter`-style flags).
#[derive(Args, Debug)]
pub struct VaultListArgs {}

/// Flags for `hidlins vault set-lock`.
///
/// `--timeout <seconds>` and `--clear` are mutually exclusive; clap
/// enforces this at parse time via `conflicts_with`.
#[derive(Args, Debug)]
pub struct VaultSetLockArgs {
    /// Registry name of the vault to configure.
    #[arg(long)]
    pub id: String,
    /// Idle-timeout in seconds before the vault auto-locks. Must be
    /// at least 1.
    #[arg(long, conflicts_with = "clear")]
    pub timeout: Option<u64>,
    /// Remove the per-vault override and fall back to the default.
    #[arg(long, conflicts_with = "timeout")]
    pub clear: bool,
}

/// `hidlins entry` — secret-entry CRUD + search.
#[derive(Args, Debug)]
pub struct EntryArgs {
    /// Verb to execute.
    #[command(subcommand)]
    pub verb: Option<EntryVerb>,
}

/// Verbs accepted by `hidlins entry`.
#[derive(Subcommand, Debug)]
pub enum EntryVerb {
    /// Add a new entry to a vault.
    Add(EntryAddArgs),
    /// Get an entry by UUID or title.
    Get(EntryGetArgs),
    /// Edit an existing entry.
    Edit(EntryEditArgs),
    /// Remove an entry (move to Recycle Bin by default).
    Rm(EntryRmArgs),
    /// List entries in a vault.
    List(EntryListArgs),
    /// Search entries.
    Search(EntrySearchArgs),
}

/// Password-class selection flags shared by `entry add --generate` and
/// `gen password`. Negative flags so the defaults (all four classes on)
/// don't need to be repeated for every invocation.
#[derive(Args, Debug, Clone, Copy, Default)]
#[allow(clippy::struct_excessive_bools)] // matches the four CharSet classes 1:1
pub struct PasswordClassFlags {
    /// Disable lowercase letters in the generated password.
    #[arg(long)]
    pub no_lowercase: bool,
    /// Disable uppercase letters in the generated password.
    #[arg(long)]
    pub no_uppercase: bool,
    /// Disable digits in the generated password.
    #[arg(long)]
    pub no_digits: bool,
    /// Disable symbols in the generated password.
    #[arg(long)]
    pub no_symbols: bool,
    /// Exclude visually ambiguous characters (`0/O/o/1/l/I/|/backtick`).
    #[arg(long)]
    pub exclude_ambiguous: bool,
}

/// Flags for `hidlins entry add`.
#[derive(Args, Debug)]
pub struct EntryAddArgs {
    /// Registry name of the vault to add into.
    #[arg(long)]
    pub vault: String,
    /// Title of the new entry (required).
    #[arg(long)]
    pub title: String,
    /// Optional username.
    #[arg(long)]
    pub username: Option<String>,
    /// Optional URL.
    #[arg(long)]
    pub url: Option<String>,
    /// Optional notes.
    #[arg(long)]
    pub notes: Option<String>,
    /// Read the entry's password from stdin (no-echo when stdin is a
    /// TTY). Mutually exclusive with `--generate`.
    #[arg(long, conflicts_with = "generate")]
    pub password_stdin: bool,
    /// Generate a fresh password via `hidlins-genpw`. Mutually
    /// exclusive with `--password-stdin`. Honours the `PasswordClassFlags`
    /// + `--length` flags.
    #[arg(long, conflicts_with = "password_stdin")]
    pub generate: bool,
    /// Generated-password length (only consulted with `--generate`).
    /// Default: 20.
    #[arg(long, default_value_t = 20)]
    pub length: usize,
    /// Tag to attach to the new entry. Repeat for multiple tags.
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    /// Include the generated/captured password in the output view (JSON
    /// + human). Default: omit.
    #[arg(long)]
    pub show_password: bool,
    /// Character-class controls for `--generate`.
    #[command(flatten)]
    pub class_flags: PasswordClassFlags,
}

/// Flags for `hidlins entry get`.
#[derive(Args, Debug)]
pub struct EntryGetArgs {
    /// Registry name of the vault.
    #[arg(long)]
    pub vault: String,
    /// UUID of the entry. Mutually exclusive with `--title`.
    #[arg(long, conflicts_with = "title")]
    pub uuid: Option<String>,
    /// Title of the entry; case-insensitive exact match. If multiple
    /// entries share the title the command exits 1 listing the
    /// candidate UUIDs.
    #[arg(long, conflicts_with = "uuid")]
    pub title: Option<String>,
    /// Include the password value in the output. Mutually exclusive
    /// with `--copy` (copying redundantly with showing is suspicious).
    #[arg(long, conflicts_with = "copy")]
    pub show_password: bool,
    /// Compute and include the current TOTP code (HMAC-SHA1; RFC 6238).
    #[arg(long)]
    pub show_totp: bool,
    /// Copy the password to the system clipboard with auto-clear.
    /// Mutually exclusive with `--show-password`.
    #[arg(long, conflicts_with = "show_password")]
    pub copy: bool,
}

/// Flags for `hidlins entry edit`.
#[derive(Args, Debug)]
pub struct EntryEditArgs {
    /// Registry name of the vault.
    #[arg(long)]
    pub vault: String,
    /// UUID of the entry to edit (required).
    #[arg(long)]
    pub uuid: String,
    /// New title.
    #[arg(long)]
    pub title: Option<String>,
    /// New username.
    #[arg(long)]
    pub username: Option<String>,
    /// New URL.
    #[arg(long)]
    pub url: Option<String>,
    /// New notes.
    #[arg(long)]
    pub notes: Option<String>,
    /// Read a new password from stdin (no-echo when stdin is a TTY).
    #[arg(long)]
    pub password_stdin: bool,
    /// Tag to add. Repeat for multiple.
    #[arg(long = "add-tag")]
    pub add_tags: Vec<String>,
    /// Tag to remove (silent no-op if not present). Repeat for multiple.
    #[arg(long = "rm-tag")]
    pub rm_tags: Vec<String>,
}

/// Flags for `hidlins entry rm`.
#[derive(Args, Debug)]
pub struct EntryRmArgs {
    /// Registry name of the vault.
    #[arg(long)]
    pub vault: String,
    /// UUID of the entry to remove (required).
    #[arg(long)]
    pub uuid: String,
    /// Permanently delete the entry, bypassing the recycle bin.
    #[arg(long)]
    pub permanent: bool,
}

/// Flags for `hidlins entry list`.
#[derive(Args, Debug)]
pub struct EntryListArgs {
    /// Registry name of the vault.
    #[arg(long)]
    pub vault: String,
    /// Filter to entries carrying every supplied tag (intersect).
    /// Repeat the flag for multiple tags.
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    /// Include expired entries. Default: omit.
    #[arg(long)]
    pub include_expired: bool,
    /// Optional pagination — max rows to emit.
    #[arg(long)]
    pub limit: Option<usize>,
    /// Optional pagination — rows to skip from the start.
    #[arg(long)]
    pub offset: Option<usize>,
}

/// `--mode` selector for `hidlins entry search`. Default `substring` (backward
/// compatible — the CLI previously had no mode flag, so this also surfaces the
/// existing-but-hidden wildcard matcher and the new fuzzy matcher).
#[derive(ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchModeArg {
    /// Case-insensitive substring match (default).
    #[default]
    Substring,
    /// Whole-field wildcard match (`*` any run, `?` one char).
    Wildcard,
    /// fzf-style fuzzy match; JSON output gains `score` + `matched_indices`.
    Fuzzy,
}

/// Flags for `hidlins entry search`.
#[derive(Args, Debug)]
pub struct EntrySearchArgs {
    /// Registry name of the vault.
    #[arg(long)]
    pub vault: String,
    /// Search query (positional).
    pub query: String,
    /// Matching mode: substring (default), wildcard, or fuzzy.
    #[arg(long, value_enum, default_value_t = SearchModeArg::Substring)]
    pub mode: SearchModeArg,
    /// Restrict the search: `group:<name>` (the group subtree) or `tag:<tag>`.
    #[arg(long)]
    pub scope: Option<String>,
    /// Optional cap on returned matches.
    #[arg(long)]
    pub limit: Option<usize>,
    /// Include entries in the recycle bin. Default: exclude.
    #[arg(long)]
    pub include_recycled: bool,
}

/// `hidlins gen` — password and passphrase generation.
#[derive(Args, Debug)]
pub struct GenArgs {
    /// Verb to execute.
    #[command(subcommand)]
    pub verb: Option<GenVerb>,
}

/// Verbs accepted by `hidlins gen`.
#[derive(Subcommand, Debug)]
pub enum GenVerb {
    /// Generate a random password with selectable character classes.
    Password(GenPasswordArgs),
    /// Generate an EFF-large-wordlist diceware passphrase.
    Passphrase(GenPassphraseArgs),
}

/// Flags for `hidlins gen password`.
///
/// `--copy` hands the generated value to `hidlins-security`'s clipboard
/// with a 30s auto-clear timer. The CLI blocks on the timer's expiry
/// before exit (required on Wayland so the timer thread can actually
/// clear the buffer). `--show` is mutually exclusive with `--copy`.
#[derive(Args, Debug)]
pub struct GenPasswordArgs {
    /// Password length (characters). Default: 20.
    #[arg(long, default_value_t = 20)]
    pub length: usize,
    /// Character-class controls.
    #[command(flatten)]
    pub class_flags: PasswordClassFlags,
    /// Copy the value to the clipboard with auto-clear. Mutually
    /// exclusive with `--show` (copy implies do-not-print).
    #[arg(long, conflicts_with = "show")]
    pub copy: bool,
    /// In JSON mode, include the generated value in the output. Default
    /// JSON output omits the value; human-mode output always prints the
    /// value to stdout regardless of this flag (mirror `pbcopy` style).
    #[arg(long, conflicts_with = "copy")]
    pub show: bool,
}

/// Flags for `hidlins gen passphrase`.
///
/// Same `--copy` semantics as [`GenPasswordArgs`]: clipboard hand-off
/// with a 30s auto-clear timer; blocks on the timer's expiry.
#[derive(Args, Debug)]
pub struct GenPassphraseArgs {
    /// Number of words. Default: 6.
    #[arg(long, default_value_t = 6)]
    pub word_count: usize,
    /// Separator inserted between words. Default: `-`.
    #[arg(long, default_value = "-")]
    pub separator: String,
    /// Copy the value to the clipboard with auto-clear. Mutually
    /// exclusive with `--show`.
    #[arg(long, conflicts_with = "show")]
    pub copy: bool,
    /// In JSON mode, include the generated value in the output.
    #[arg(long, conflicts_with = "copy")]
    pub show: bool,
}

/// `hidlins sync` — local-network synchronization and serving.
#[derive(Args, Debug)]
pub struct SyncArgs {
    /// Local-sync operation.
    #[command(subcommand)]
    pub verb: Option<SyncVerb>,
}

/// Local-sync operations.
#[derive(Subcommand, Debug)]
pub enum SyncVerb {
    /// Synchronize a paired client vault now.
    Now(SyncNowArgs),
    /// Run an authoritative sync server in the foreground until Ctrl+C.
    Serve(SyncServeArgs),
    /// Pair an existing vault with an authoritative server.
    Pair(SyncPairArgs),
    /// Pair and import a complete encrypted vault.
    Import(SyncImportArgs),
    /// Show local-sync role, pairing, and server status.
    Status(SyncStatusArgs),
    /// List, rename, or revoke paired peers.
    Peers(SyncPeersArgs),
}

/// Common existing-vault selector for a one-shot sync.
#[derive(Args, Debug)]
pub struct SyncNowArgs {
    /// Vault registry name. Defaults to the sole registered vault.
    #[arg(long)]
    pub vault: Option<String>,
    /// Restricted IP-literal diagnostic fallback when discovery is unavailable.
    #[arg(long, requires = "port")]
    pub address: Option<String>,
    /// Port paired with `--address`.
    #[arg(long, requires = "address")]
    pub port: Option<u16>,
}

/// Foreground authoritative server options.
#[derive(Args, Debug)]
pub struct SyncServeArgs {
    /// Vault registry name. Defaults to the sole registered vault.
    #[arg(long)]
    pub vault: Option<String>,
    /// Restricted IP literal to bind; otherwise an allowed active interface is selected.
    #[arg(long)]
    pub address: Option<String>,
    /// Listener port.
    #[arg(long, default_value_t = 37371)]
    pub port: u16,
    /// Open the bounded pairing window immediately after startup.
    #[arg(long)]
    pub pairing_window: bool,
}

/// Existing-vault pairing options.
#[derive(Args, Debug)]
pub struct SyncPairArgs {
    /// Vault registry name. Defaults to the sole registered vault.
    #[arg(long)]
    pub vault: Option<String>,
    /// Restricted IP-literal diagnostic fallback.
    #[arg(long, requires = "port")]
    pub address: Option<String>,
    /// Port paired with `--address`.
    #[arg(long, requires = "address")]
    pub port: Option<u16>,
    /// Local display name for the authority.
    #[arg(long, default_value = "authority")]
    pub name: String,
}

/// Pair-and-import options. Passwords remain secure stdin-only.
#[derive(Args, Debug)]
pub struct SyncImportArgs {
    /// New local registry name.
    #[arg(long)]
    pub id: String,
    /// Destination KDBX path. Defaults to the Hidlins state directory.
    #[arg(long)]
    pub path: Option<PathBuf>,
    /// Optional keyfile required by the remote vault.
    #[arg(long)]
    pub keyfile: Option<PathBuf>,
    /// Restricted IP-literal diagnostic fallback.
    #[arg(long, requires = "port")]
    pub address: Option<String>,
    /// Port paired with `--address`.
    #[arg(long, requires = "address")]
    pub port: Option<u16>,
    /// Local display name for the authority.
    #[arg(long, default_value = "authority")]
    pub name: String,
}

/// Local-sync status options.
#[derive(Args, Debug)]
pub struct SyncStatusArgs {
    /// Vault registry name. Defaults to the sole registered vault.
    #[arg(long)]
    pub vault: Option<String>,
}

/// Peer-management command group.
#[derive(Args, Debug)]
pub struct SyncPeersArgs {
    /// Peer operation.
    #[command(subcommand)]
    pub verb: Option<SyncPeersVerb>,
}

/// Peer-management operations.
#[derive(Subcommand, Debug)]
pub enum SyncPeersVerb {
    /// List configured peers.
    List(SyncPeerListArgs),
    /// Rename a peer's local display label.
    Rename(SyncPeerRenameArgs),
    /// Revoke a peer immediately.
    Revoke(SyncPeerRevokeArgs),
}

/// Peer-list options.
#[derive(Args, Debug)]
pub struct SyncPeerListArgs {
    /// Vault registry name. Defaults to the sole registered vault.
    #[arg(long)]
    pub vault: Option<String>,
}

/// Peer-rename options.
#[derive(Args, Debug)]
pub struct SyncPeerRenameArgs {
    /// Vault registry name. Defaults to the sole registered vault.
    #[arg(long)]
    pub vault: Option<String>,
    /// Opaque `peer-N` handle from `sync peers list`.
    #[arg(long)]
    pub peer: String,
    /// New local display name.
    #[arg(long)]
    pub name: String,
}

/// Peer-revocation options.
#[derive(Args, Debug)]
pub struct SyncPeerRevokeArgs {
    /// Vault registry name. Defaults to the sole registered vault.
    #[arg(long)]
    pub vault: Option<String>,
    /// Opaque `peer-N` handle from `sync peers list`.
    #[arg(long)]
    pub peer: String,
}

/// `hidlins ssh` — slot only.
#[derive(Args, Debug)]
pub struct SshArgs {
    /// Verb to execute.
    #[command(subcommand)]
    pub verb: Option<SshVerb>,
}

/// Verbs accepted by `hidlins ssh` (all slots). All three bodies return
/// `CliExit::NotImplemented`; the flag surfaces are the forward-compat
/// contract with `features/ssh-keys/`.
#[derive(Subcommand, Debug)]
pub enum SshVerb {
    /// Add an SSH-key entry to a vault. (Slot — see `features/ssh-keys/`.)
    Add(SshAddArgs),
    /// Load an SSH key into ssh-agent with TTL. (Slot — see `features/ssh-keys/`.)
    Load(SshLoadArgs),
    /// Generate a new SSH keypair and store the private key. (Slot — see `features/ssh-keys/`.)
    Generate(SshGenerateArgs),
}

/// Flags for `hidlins ssh add` (slot — body returns exit 11).
#[derive(Args, Debug)]
pub struct SshAddArgs {
    /// Registry name of the destination vault.
    #[arg(long)]
    pub vault: Option<String>,
    /// Path to the existing private key to import.
    #[arg(long)]
    pub key_path: Option<PathBuf>,
    /// Optional comment to attach to the entry.
    #[arg(long)]
    pub comment: Option<String>,
}

/// Flags for `hidlins ssh load` (slot — body returns exit 11).
#[derive(Args, Debug)]
pub struct SshLoadArgs {
    /// Registry name of the source vault.
    #[arg(long)]
    pub vault: Option<String>,
    /// UUID of the SSH-key entry to load.
    #[arg(long)]
    pub uuid: Option<String>,
    /// TTL in seconds before ssh-agent expires the key.
    #[arg(long)]
    pub ttl: Option<u64>,
}

/// Flags for `hidlins ssh generate` (slot — body returns exit 11).
#[derive(Args, Debug)]
pub struct SshGenerateArgs {
    /// Registry name of the destination vault.
    #[arg(long)]
    pub vault: Option<String>,
    /// Algorithm (`ed25519` or `rsa`). Defaults to `ed25519` when implemented.
    #[arg(long)]
    pub algorithm: Option<String>,
    /// Optional comment to attach to the public key.
    #[arg(long)]
    pub comment: Option<String>,
}

/// `hidlins completions <shell>` — emit a shell completion script.
///
/// `<shell>` is required (no default). Supported values are the
/// `clap_complete::Shell` variants: `bash`, `zsh`, `fish`,
/// `powershell`, `elvish`. The CLI documents the first three as the
/// supported targets; the other two are accepted for users who already
/// rely on them.
#[derive(Args, Debug)]
pub struct CompletionsArgs {
    /// Target shell to generate a completion script for.
    pub shell: Option<clap_complete::Shell>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn banner_is_the_complete_hidlins_wordmark() {
        const EXPECTED: &str = r"
 _   _ ___ ____  _     ___ _   _ ____
| | | |_ _|  _ \| |   |_ _| \ | / ___|
| |_| || || | | | |    | ||  \| \___ \
|  _  || || |_| | |___ | || |\  |___) |
|_| |_|___|____/|_____|___|_| \_|____/

";
        assert_eq!(BANNER, EXPECTED);
        assert!(!BANNER.contains("/_/    \\__,_/"));
    }

    #[test]
    fn cli_command_has_all_subcommands() {
        let cmd = Cli::command();
        let names: Vec<_> = cmd.get_subcommands().map(clap::Command::get_name).collect();
        for expected in [
            "vault",
            "entry",
            "gen",
            "sync",
            "ssh",
            "completions",
            "keys",
        ] {
            assert!(
                names.contains(&expected),
                "missing subcommand {expected}; have {names:?}"
            );
        }
    }

    #[test]
    fn entry_search_mode_flag_parses_and_defaults() {
        // Absent → substring (backward compatible).
        let cli = Cli::try_parse_from(["hidlins", "entry", "search", "--vault", "v", "q"]).unwrap();
        let Some(Command::Entry(EntryArgs {
            verb: Some(EntryVerb::Search(args)),
        })) = Some(cli.command)
        else {
            panic!("expected entry search");
        };
        assert_eq!(args.mode, SearchModeArg::Substring, "default is substring");
        assert!(args.scope.is_none());

        // Each mode value maps.
        for (flag, want) in [
            ("substring", SearchModeArg::Substring),
            ("wildcard", SearchModeArg::Wildcard),
            ("fuzzy", SearchModeArg::Fuzzy),
        ] {
            let cli = Cli::try_parse_from([
                "hidlins", "entry", "search", "--vault", "v", "--mode", flag, "q",
            ])
            .unwrap();
            let Command::Entry(EntryArgs {
                verb: Some(EntryVerb::Search(args)),
            }) = cli.command
            else {
                panic!("expected entry search");
            };
            assert_eq!(args.mode, want, "--mode {flag}");
        }
    }

    #[test]
    fn entry_search_scope_flag_parses() {
        let cli = Cli::try_parse_from([
            "hidlins",
            "entry",
            "search",
            "--vault",
            "v",
            "--scope",
            "group:Banking",
            "q",
        ])
        .unwrap();
        let Command::Entry(EntryArgs {
            verb: Some(EntryVerb::Search(args)),
        }) = cli.command
        else {
            panic!("expected entry search");
        };
        assert_eq!(args.scope.as_deref(), Some("group:Banking"));
    }

    #[test]
    fn no_master_password_flag_exists_anywhere() {
        // FR-061 structural gate: no clap flag named --master-password,
        // --password, or similar lives anywhere in the tree. Phase 1
        // does not yet have password-shaped flags by design; this test
        // is the canary against accidental future addition.
        let cmd = Cli::command();
        assert_args_have_no_master_password_flag(&cmd, "hidlins");
    }

    fn assert_args_have_no_master_password_flag(cmd: &clap::Command, path: &str) {
        for arg in cmd.get_arguments() {
            let long = arg.get_long().unwrap_or("");
            assert_ne!(
                long, "master-password",
                "found forbidden --master-password flag at {path}"
            );
        }
        for sub in cmd.get_subcommands() {
            let sub_path = format!("{path} {}", sub.get_name());
            assert_args_have_no_master_password_flag(sub, &sub_path);
        }
    }

    #[test]
    fn output_format_default_is_human() {
        assert_eq!(OutputFormat::default(), OutputFormat::Human);
    }

    #[test]
    fn master_password_env_var_name_is_documented_constant() {
        // Single source of truth so the env-var name matches in tests,
        // docs, and runtime detection.
        assert_eq!(MASTER_PASSWORD_ENV_VAR, "HIDLINS_MASTER_PASSWORD");
    }
}
