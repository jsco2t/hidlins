# TUI Screen-Reader Confidence Cases

**Last Updated:** 2026-08-31
**Status:** Optional, non-gating real-environment acceptance

The automated accessibility contracts verify reading-order text, textual state
carriers, password canary absence, theme-independent semantics, and absence of
raw terminal controls. These two cases cover only the platform accessibility
stack and speech integration. They are not represented as executed.

Use the disposable-vault setup from
[the real-terminal matrix](tui-real-terminal-matrix.md). The terminal under
test must expose reviewable terminal text through the platform accessibility
stack; no terminal brand or desktop shell is required.

## Case 1 — Linux Orca through AT-SPI

Prerequisites: an active graphical Linux login, a working AT-SPI accessibility
bus, speech output, and Orca configured for the user session.

1. Start Orca, then launch Hidlins in the same graphical session using the
   isolated `HOME`.
2. Traverse the vault list, master-password prompt, unlocked workspace, command
   palette or help, search, and locked screen.
3. Record whether important labels and current states are spoken in usable
   reading order.
4. Enter the disposable master password and confirm none of its characters are
   spoken or exposed in reviewable terminal text.
5. Confirm speech contains no raw escape-sequence names, control-code noise, or
   repeated cursor-positioning garbage.

Pass means all five observations succeed. Record OS, Orca version, terminal
name/version, commit, date, and result.

## Case 2 — macOS VoiceOver

Prerequisites: an active macOS login, VoiceOver enabled, speech output, and a
terminal that exposes its text through the macOS accessibility APIs.

1. Launch Hidlins with the isolated `HOME` and navigate the same vault-list,
   password-prompt, workspace, command/help, search, and lock surfaces.
2. Record whether important labels and current states are spoken in usable
   reading order.
3. Enter the disposable master password and confirm none of its characters are
   spoken or exposed in reviewable terminal text.
4. Confirm speech contains no raw escape-sequence names, control-code noise, or
   repeated cursor-positioning garbage.

Pass means all four observations succeed. Record macOS version, VoiceOver
version, terminal name/version, commit, date, and result.
