//! TUI startup settings and effective keymap formatting.
//!
//! The console crate owns argument parsing and help. This library accepts
//! typed settings; CLI > config.toml > compiled-default precedence is resolved
//! by [`Args::effective`].

use std::fmt::Write as _;
use std::path::PathBuf;

use crate::command::keymap::Keymap;
use crate::command::registry::COMMANDS;
use crate::user_config::UserConfig;

/// Output format for `--dump-keys`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DumpFormat {
    /// Human-readable, tab-separated columns grouped like the palette.
    Text,
    /// A JSON array of command objects (stable field names — `hidlins keys`
    /// (T4.3) and user scripts consume it).
    Json,
}

/// The parsed CLI arguments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Args {
    /// Optional shared vault registry path.
    pub registry: Option<PathBuf>,
    /// `--vault NAME`: skip the vault list and open this vault's unlock prompt.
    pub vault: Option<String>,
    /// `--theme NAME`: override the configured theme selection.
    pub theme: Option<String>,
    /// `--config PATH`: load `config.toml` from this path instead of the default.
    pub config: Option<PathBuf>,
    /// Disable mouse capture even when `mouse = true` in config.toml.
    pub no_mouse: bool,
    /// Open the vault without permitting any KDBX mutation or sync.
    pub read_only: bool,
    /// `--dump-keys[=json]`: print the effective keymap and exit before terminal
    /// setup.
    pub dump_keys: Option<DumpFormat>,
}

/// CLI-over-config-over-default resolution of the settings that both a flag and
/// `config.toml` can set. `config` path resolution is separate (it selects
/// *which* config to load, before this runs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Effective {
    /// The theme name to resolve (CLI `--theme`, else `None` → the resolver
    /// falls through to `config.theme`).
    pub theme: Option<String>,
    /// Whether mouse capture is enabled (`config.mouse` unless `--no-mouse`).
    pub mouse: bool,
    /// Whether this is a read-only session.
    pub read_only: bool,
}

impl Args {
    /// Resolve the shared registry override before any TUI I/O.
    pub(crate) fn paths(&self) -> Result<hidlins_core::HidlinsPaths, hidlins_core::VaultError> {
        let defaults = hidlins_core::HidlinsPaths::from_env()?;
        match &self.registry {
            Some(path) => Ok(hidlins_core::HidlinsPaths::with_registry_file(path.clone())
                .with_config_dir(defaults.config_dir().to_path_buf())),
            None => Ok(defaults),
        }
    }

    /// Resolve CLI-over-config settings (see [`Effective`]).
    pub(crate) fn effective(&self, cfg: &UserConfig) -> Effective {
        Effective {
            theme: self.theme.clone(),
            mouse: cfg.mouse && !self.no_mouse,
            read_only: self.read_only,
        }
    }

    // A stable seam: future flags land as "reserved" here before their owning
    // feature ships. All Phase-4 flags (`--read-only` T4.7, `--no-mouse` T4.6)
    // are now live, so nothing is deferred.
    #[allow(clippy::unused_self)]
    pub(crate) fn deferred_option_error(&self) -> Option<String> {
        None
    }
}

/// Render the effective keymap as `--dump-keys` output. Context-free: every
/// command in the registry with its bound key(s), grouped like the palette.
pub(crate) fn dump_keys(keymap: &Keymap, format: DumpFormat) -> String {
    match format {
        DumpFormat::Text => dump_keys_text(keymap),
        DumpFormat::Json => dump_keys_json(keymap),
    }
}

/// Commands sorted for display: by group rank, then by their position in the
/// `COMMANDS` table (stable, matches the palette).
fn sorted_specs() -> Vec<&'static crate::command::registry::CommandSpec> {
    let mut specs: Vec<_> = COMMANDS.iter().collect();
    specs.sort_by_key(|s| s.group.rank());
    specs
}

fn dump_keys_text(keymap: &Keymap) -> String {
    let mut out = String::new();
    for spec in sorted_specs() {
        let keys = keymap
            .rendered_keys(spec.id)
            .unwrap_or_else(|| "(unbound)".to_string());
        // group \t name \t keys \t desc
        out.push_str(spec.group.label());
        out.push('\t');
        out.push_str(spec.name);
        out.push('\t');
        out.push_str(&keys);
        out.push('\t');
        out.push_str(spec.desc);
        out.push('\n');
    }
    out
}

fn dump_keys_json(keymap: &Keymap) -> String {
    let mut out = String::from("[\n");
    let specs = sorted_specs();
    for (i, spec) in specs.iter().enumerate() {
        let keys = keymap.rendered_key_parts(spec.id);
        let keys_json = keys
            .iter()
            .map(|k| format!("\"{}\"", json_escape(k)))
            .collect::<Vec<_>>()
            .join(", ");
        let contexts_json = spec
            .contexts
            .names()
            .iter()
            .map(|c| format!("\"{}\"", json_escape(c)))
            .collect::<Vec<_>>()
            .join(", ");
        // Writing to a `String` is infallible; the `write!` results are ignored.
        let _ = write!(
            out,
            "  {{\"name\": \"{}\", \"keys\": [{keys_json}], \"desc\": \"{}\", \
             \"group\": \"{}\", \"contexts\": [{contexts_json}]}}",
            json_escape(spec.name),
            json_escape(spec.desc),
            json_escape(spec.group.label()),
        );
        if i + 1 < specs.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("]\n");
    out
}

/// Escape a string for embedding in a JSON double-quoted value. Only `"` and
/// `\` occur in our static ASCII names/descs; escaped defensively (AC-8: no
/// `serde_json` dependency).
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_settings_override_config() {
        let mut cfg = UserConfig {
            mouse: true,
            ..UserConfig::default()
        };
        let args = Args {
            theme: Some("paper".into()),
            no_mouse: true,
            read_only: true,
            ..Args::default()
        };
        let effective = args.effective(&cfg);
        assert_eq!(effective.theme.as_deref(), Some("paper"));
        assert!(!effective.mouse);
        assert!(effective.read_only);
        let defaults = Args::default().effective(&cfg);
        assert!(defaults.theme.is_none());
        assert!(defaults.mouse);
        assert!(!defaults.read_only);
        cfg.mouse = false;
        assert!(!Args::default().effective(&cfg).mouse);
    }

    #[test]
    fn dump_keys_text_lists_effective_keymap() {
        use crate::command::keymap::{KeymapPatch, Preset};
        let (km, _) = Keymap::from_patch(&KeymapPatch {
            preset: Some(Preset::Vim),
            bindings: std::collections::BTreeMap::new(),
        });
        let text = dump_keys(&km, DumpFormat::Text);
        // Every command appears with its tab-separated columns.
        for spec in COMMANDS {
            assert!(
                text.lines().any(|l| {
                    let mut cols = l.split('\t');
                    cols.next() == Some(spec.group.label()) && cols.next() == Some(spec.name)
                }),
                "text dump missing command {}",
                spec.name
            );
        }
        // A known binding is rendered.
        assert!(
            text.lines()
                .any(|l| l.contains("copy-password") && l.contains('y')),
            "copy-password should show its 'y' binding somewhere:\n{text}"
        );
    }

    #[test]
    fn dump_keys_json_is_stable_and_complete() {
        use crate::command::keymap::{BindValue, KeymapPatch, Preset};
        // Rebind copy-password to a key with a JSON metacharacter would be ideal,
        // but keys are constrained; assert escaping via a hypothetical instead.
        let mut bindings = std::collections::BTreeMap::new();
        bindings.insert("copy-password".to_string(), BindValue::One("y".to_string()));
        let (km, warnings) = Keymap::from_patch(&KeymapPatch {
            preset: Some(Preset::Vim),
            bindings,
        });
        assert!(warnings.is_empty(), "{warnings:?}");

        let json = dump_keys(&km, DumpFormat::Json);
        // Every command name appears as a JSON field.
        for spec in COMMANDS {
            assert!(
                json.contains(&format!("\"name\": \"{}\"", spec.name)),
                "json dump missing command {}",
                spec.name
            );
        }
        // The rebind is reflected and the stable fields are present.
        assert!(json.contains("\"name\": \"copy-password\""));
        assert!(
            json.contains("\"keys\": [\"y\"]"),
            "rebound key reflected:\n{json}"
        );
        assert!(json.contains("\"desc\":"));
        assert!(json.contains("\"group\":"));
        assert!(json.contains("\"contexts\":"));
    }

    #[test]
    fn json_escape_handles_metacharacters() {
        assert_eq!(json_escape(r#"a"b\c"#), r#"a\"b\\c"#);
        assert_eq!(json_escape("tab\tnl\n"), "tab\\tnl\\n");
        assert_eq!(json_escape("plain"), "plain");
    }
}
