# TUI Verification Replacement Coverage

Revision 4 replaces the duplicated tui-skeleton and tui-enhancements manual
display/screen-reader suites. Task 009 removed those trees and their manual-only
support files. This ledger gives every former test ID or document an explicit
disposition. “Automated” means it runs under the Rust/Make gates; “real
environment” means the compact, non-gating matrix now retained centrally in
`verifications/tui-real-terminal-matrix.md` and
`verifications/tui-screen-readers.md` in the project notebook.

## TUI skeleton display suite

| Former test/document | Replacement evidence | Disposition |
| --- | --- | --- |
| `T01-DISP-SETUP-01`, `T01-DISP-SETUP-02`, `T01-DISP-SETUP-03` | `test_support::populated_app`; core registry/KDBX tests | Automated fixture construction and validation |
| `T02-DISP-UNLOCK-FLOW`, `T02-DISP-BROWSE-INITIAL`, `T02-DISP-MANUAL-LOCK` | `journey_tests::unlock_browse_search_and_lock_journey_renders_each_state`; `app::tests::manual_lock_drops_vault` | Automated state + semantic render journey |
| `T02-DISP-COUNTDOWN-VISIBLE` | status-bar/countdown unit tests; rendered journey status rows | Automated |
| `T02-DISP-IDLE-LOCK-AND-REUNLOCK` | `app::tests::idle_timeout_auto_locks`, `tabs_reset_to_secrets_on_each_unlock`, `auto_lock_then_unlock_different_vault` | Automated fake-clock/state tests |
| `T02-DISP-WRONG-PW-3-ATTEMPTS` | unlock-attempt state-machine tests in `app::tests` | Automated |
| `T02-DISP-BROWSE-NAV` | tree/navigation unit tests plus rendered journeys | Automated |
| `T02-DISP-EXPIRED-DUAL-AFFORDANCE` | tree snapshot, theme carrier tests, `accessibility_contract_tests::important_states_have_textual_carriers_in_review_order` | Automated text + style contract |
| `T02-DISP-SEARCH` | search unit/security tests, search snapshot, rendered journeys | Automated |
| `T02-DISP-HISTORY` | history overlay and navigation tests in `app::tests` | Automated |
| `T02-DISP-ADD`, `T02-DISP-EDIT`, `T02-DISP-GENERATE` | add/edit/generate persistence journeys in `app::tests` | Automated KDBX-backed journeys |
| `T02-DISP-EDIT-CANCEL-ZEROIZE` | edit/password-input zeroization tests | Automated memory-hygiene contract |
| `T02-DISP-COPY`, `T02-DISP-COPY-VERIFY-CLIPBOARD`, `T02-DISP-COPY-VERIFY-CLEAR-AFTER-TTL`, `T02-DISP-COPY-NO-STOMP-MID-TTL`, `T02-DISP-MULTI-COPY` | clipboard seam tests plus `make test-clipboard` (Task 008 live pass) | Automated and real clipboard integration |
| `06-sync-and-settings.md` | T-SYNC matrix, secure credential persistence tests, settings snapshot, MinIO/CLI live pass | Automated and live-service integration |
| `06-sync-placeholder.md` | Already superseded by the real sync implementation | Removed obsolete document |
| `T02-DISP-THEME-MONOCHROME`, `T02-DISP-THEME-ACCESSIBLE-ENV`, `T02-DISP-THEME-16COLOR`, `T02-DISP-THEME-TRUECOLOR` | theme detection/invariant tests plus `semantic_transcript_is_equivalent_across_color_and_accessible_themes` | Automated; emulator color smoke retained in terminal matrix |
| `T02-DISP-HELP-REACH`, `T02-DISP-HELP-ALL-KEYS`, `T02-DISP-HELP-GLOBALS` | palette/registry completeness and rendered discoverability journey | Automated |
| `T09-CLEAN-01` | Real clipboard test teardown | Automated integration teardown |
| `T09-CLEAN-02`, `T09-CLEAN-03`, `T09-CLEAN-04`, `T09-CLEAN-05` | `TempDir`/RAII fixtures and in-process tests | Manual cleanup no longer applicable |

## TUI skeleton screen-reader suite

| Former test | Replacement evidence | Disposition |
| --- | --- | --- |
| `T03-ORCA-SETUP`, `T03-ORCA-UNLOCK`, `T03-ORCA-BROWSE`, `T03-ORCA-HELP`, `T03-ORCA-COPY-TOAST` | semantic buffer contracts for deterministic output; one compact Linux Orca/AT-SPI case | Automated semantics + retained real environment |
| `T03-VO-SETUP`, `T03-VO-UNLOCK-BROWSE-HELP`, `T03-VO-DISABLE` | semantic buffer contracts; one compact macOS VoiceOver case | Automated semantics + retained real environment |

## TUI enhancements display suite

| Former test | Replacement evidence | Disposition |
| --- | --- | --- |
| `TE-DS-DISC-01` | hint-bar registry/unit tests, golden, rendered discoverability journey | Automated |
| `TE-DS-DISC-02` | which-key delay/continuation tests, golden, rendered journey | Automated |
| `TE-DS-DISC-03` | palette registry/filter/dispatch tests, goldens, rendered palette→search journey | Automated |
| `TE-DS-DISC-04` | breadcrumb/tab tests and tab golden; semantic active-tab carrier | Automated |
| `TE-DS-CFG-01` | config generation/save tests in `user_config` and `app` | Automated |
| `TE-DS-KEY-01` | keymap patch/conflict tests and command-registry meta-tests | Automated |
| `TE-DS-KEY-02` | Plain preset keymap/which-key tests | Automated |
| `TE-DS-CFG-02` | corrupt-config fallback and secret-free warning tests | Automated |
| `TE-DS-CFG-03` | CLI precedence, custom config path, legacy-key and `--vault` tests | Automated |
| `TE-DS-SET-01` | settings auto-lock cycle/save/rearm tests and Settings semantic render | Automated |
| `TE-DS-THM-01` | settings theme cycle/persist tests | Automated |
| `TE-DS-THM-02` | user-theme discovery/patch tests | Automated |
| `TE-DS-THM-03` | hostile/semantic theme invariant tests plus color/monochrome transcript equality | Automated |
| `TE-DS-THM-04` | named/light/dark/ANSI16 resolution matrix tests | Automated; terminal color smoke retained |
| `TE-DS-SRCH-01`, `TE-DS-SRCH-02` | fuzzy search/scope tests, snapshot, rendered `[ALL]` contract | Automated |
| `TE-DS-SRCH-03` | `T-SEC-PREVIEW-1` plus secret-canary rendered contract | Automated security gate |
| `TE-DS-SRCH-04` | quick-select/action/restore tests in `app::tests` | Automated |
| `TE-DS-SRCH-05` | search-layout floor tests, snapshot, rendered 60×16 journey | Automated |
| `TE-DS-NAV-01`, `TE-DS-NAV-02` | focus/tree/detail and jump-history tests | Automated |
| `TE-DS-VIS-01`, `TE-DS-VIS-02` | visual/bulk state and single-save tests plus rendered `VISUAL`/mark journey | Automated |
| `TE-DS-MOUSE-01` | mouse hit-map/event tests plus rendered injected-mouse journey | Automated; native selection retained in terminal matrix |
| `TE-DS-MOUSE-02` | mouse-disabled tests and journey parity assertion | Automated |
| `TE-DS-RO-01` | read-only command/persist/mtime tests plus rendered `RO` journey | Automated |

## TUI enhancements screen-reader suite

| Former test | Replacement evidence | Disposition |
| --- | --- | --- |
| `TE-SR-ORCA-01` | unlock/workspace/lock semantic contracts; compact Linux Orca case | Automated semantics + retained real environment |
| `TE-SR-ORCA-02` | hint/which-key/palette/search rendered journeys; compact Linux Orca case | Automated semantics + retained real environment |
| `TE-SR-ORCA-03` | injected mouse/no-mouse parity; raw-control exclusion; terminal/Orca smoke | Automated application output + retained real environment |
| `TE-SR-VO-01` | the same deterministic semantic contracts; compact macOS VoiceOver case | Automated semantics + retained real environment |

## Irreducible retained coverage

Only these observations remain outside automation:

1. Konsole, the terminal actually installed on Pop!_OS, and a recorded macOS
   terminal: raw/alternate-screen restoration, native selection versus mouse
   capture, minimum-size legibility, and platform color rendering.
2. Linux Orca through AT-SPI: one speech-integration pass over the semantic
   journey, including password non-echo and absence of spoken control garbage.
3. macOS VoiceOver: one equivalent platform screen-reader pass.

These are portable real-environment confidence checks, not gates for this work
package. No TestBackend assertion is represented as proof of AT-SPI, speech,
Braille, or subjective usability.

Task 009 did not execute these cases and does not label them as passes. The old
`02-local-display/` and `03-screen-reader/` trees under both features, plus the
tui-enhancements manual fixtures and scratch tree, are absent.
