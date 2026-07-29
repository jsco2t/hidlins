use std::fmt::Write as _;

use super::session::AppSession;
use crate::dto::UiPrefs;
use crate::error::HidlinsApiError;

#[allow(clippy::needless_pass_by_value)] // frb FFI passes owned DTOs
impl AppSession {
    /// Load UI preferences from the state directory.
    ///
    /// Returns defaults when the file is missing or corrupt — prefs are
    /// never a blocking failure.
    pub fn get_prefs(&self) -> Result<UiPrefs, HidlinsApiError> {
        let path = self.paths.state_dir().join("ui-prefs.toml");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Ok(UiPrefs::default());
        };
        Ok(parse_prefs(&content))
    }

    /// Persist UI preferences to the state directory.
    pub fn set_prefs(&self, prefs: UiPrefs) -> Result<(), HidlinsApiError> {
        let path = self.paths.state_dir().join("ui-prefs.toml");
        let content = serialize_prefs(&prefs);
        hidlins_core::atomic::write_atomic(&path, content.as_bytes())?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Hand-rolled TOML for the flat UiPrefs struct.
//
// The struct has seven optional primitive fields — no nesting, no arrays,
// no tables. A toml crate dependency would pull serde + serde_derive +
// the full TOML parser for this trivial case; hand-rolling follows the
// project's supply-chain minimisation rule.
// ---------------------------------------------------------------------------

fn serialize_prefs(prefs: &UiPrefs) -> String {
    let mut out = String::new();
    if let Some(ref v) = prefs.theme_mode {
        let _ = writeln!(out, "theme_mode = \"{}\"", escape_toml_string(v));
    }
    if let Some(v) = prefs.window_x {
        let _ = writeln!(out, "window_x = {v}");
    }
    if let Some(v) = prefs.window_y {
        let _ = writeln!(out, "window_y = {v}");
    }
    if let Some(v) = prefs.window_width {
        let _ = writeln!(out, "window_width = {v}");
    }
    if let Some(v) = prefs.window_height {
        let _ = writeln!(out, "window_height = {v}");
    }
    if let Some(ref v) = prefs.last_vault {
        let _ = writeln!(out, "last_vault = \"{}\"", escape_toml_string(v));
    }
    if let Some(v) = prefs.list_pane_width {
        let _ = writeln!(out, "list_pane_width = {v}");
    }
    out
}

fn parse_prefs(text: &str) -> UiPrefs {
    let mut prefs = UiPrefs::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "theme_mode" => prefs.theme_mode = unquote_toml_string(value),
                "window_x" => prefs.window_x = value.parse().ok(),
                "window_y" => prefs.window_y = value.parse().ok(),
                "window_width" => prefs.window_width = value.parse().ok(),
                "window_height" => prefs.window_height = value.parse().ok(),
                "last_vault" => prefs.last_vault = unquote_toml_string(value),
                "list_pane_width" => prefs.list_pane_width = value.parse().ok(),
                _ => {} // Unknown keys are silently ignored (forward-compat).
            }
        }
    }
    prefs
}

/// Escape special characters for a TOML basic string (double-quoted).
fn escape_toml_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

/// Extract the content of a TOML basic string ("...").
fn unquote_toml_string(s: &str) -> Option<String> {
    let s = s.trim();
    let inner = s.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next()? {
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                _ => return None, // Invalid escape — treat as corrupt.
            }
        } else {
            out.push(ch);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_full_prefs() {
        let prefs = UiPrefs {
            theme_mode: Some("dark".to_string()),
            window_x: Some(100),
            window_y: Some(200),
            window_width: Some(800),
            window_height: Some(600),
            last_vault: Some("my-vault".to_string()),
            list_pane_width: Some(360.5),
        };
        let serialized = serialize_prefs(&prefs);
        let parsed = parse_prefs(&serialized);
        assert_eq!(parsed.theme_mode.as_deref(), Some("dark"));
        assert_eq!(parsed.window_x, Some(100));
        assert_eq!(parsed.window_y, Some(200));
        assert_eq!(parsed.window_width, Some(800));
        assert_eq!(parsed.window_height, Some(600));
        assert_eq!(parsed.last_vault.as_deref(), Some("my-vault"));
        assert_eq!(parsed.list_pane_width, Some(360.5));
    }

    #[test]
    fn empty_defaults() {
        let prefs = parse_prefs("");
        assert!(prefs.theme_mode.is_none());
        assert!(prefs.window_x.is_none());
        assert!(prefs.last_vault.is_none());
        assert!(prefs.list_pane_width.is_none());
    }

    #[test]
    fn corrupt_values_fall_back_to_none() {
        let text = "window_x = not-a-number\ntheme_mode = unquoted\nlist_pane_width = invalid\n";
        let prefs = parse_prefs(text);
        assert!(prefs.window_x.is_none());
        assert!(prefs.theme_mode.is_none());
        assert!(prefs.list_pane_width.is_none());
    }

    #[test]
    fn string_escaping_round_trips() {
        let prefs = UiPrefs {
            theme_mode: Some("has \"quotes\" and \\backslash".to_string()),
            last_vault: Some("new\nline".to_string()),
            ..UiPrefs::default()
        };
        let serialized = serialize_prefs(&prefs);
        let parsed = parse_prefs(&serialized);
        assert_eq!(
            parsed.theme_mode.as_deref(),
            Some("has \"quotes\" and \\backslash")
        );
        assert_eq!(parsed.last_vault.as_deref(), Some("new\nline"));
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let text = "future_key = \"value\"\ntheme_mode = \"light\"\n";
        let prefs = parse_prefs(text);
        assert_eq!(prefs.theme_mode.as_deref(), Some("light"));
    }

    #[test]
    fn comments_and_blanks_are_skipped() {
        let text = "# comment\n\ntheme_mode = \"dark\"\n  # another\n";
        let prefs = parse_prefs(text);
        assert_eq!(prefs.theme_mode.as_deref(), Some("dark"));
    }
}
