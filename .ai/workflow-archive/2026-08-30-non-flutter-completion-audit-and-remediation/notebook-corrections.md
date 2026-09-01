# External Notebook Corrections

Recorded: 2026-08-31

Tasks 001–008 recorded these corrections while the project notebook was outside
their approved write scope. Revision 4 and Task 009 later authorized and applied
the TUI verification/status corrections directly to the authoritative notebook.
The earlier observations remain below as dated provenance.

## Bugs index and bug workspaces

1. `bugs/index.md` and `bugs/merge-attachment-propagation/`
   - Current text says **Planning Complete**.
   - Correct disposition: **Fixed in current source**. Both-side attachment
     add, replace, and remove propagation are covered by merge semantic and
     property tests. The implementation predates this workflow; Task 004
     revalidated it while repairing history attachments.
2. `bugs/index.md` and `bugs/merge-history-attachment-loss/`
   - Current text says only rolling three-generation backups are planned and
     the root-cause fix is deferred upstream.
   - Correct disposition: **Root cause fixed by this workflow, not deferred**.
     Task 004 added an exact-version audited Keepass API patch, zeroizing pool
     snapshots, pool-safe history repair, fail-closed ambiguity handling,
     encrypted save/reopen regressions, and KeePassXC attachment interop. The
     implemented safety net remains one `.kdbx.bak`; the planned rolling-backup
     workaround should be marked superseded rather than implemented.
3. `bugs/index.md` and `bugs/rstcred1-password-change/`
   - Current text says the gap is recorded/latent, no frontend exposes password
     change, and Flutter implementation has not landed.
   - Correct disposition: **Fixed in the implemented API boundary**.
     `hidlins-api::AppSession::change_master_password` re-encrypts stored S3
     credentials before registry persistence and has success, wrong-password,
     sync-exclusion, ordering, and re-encryption tests. CLI/TUI still do not
     expose password change, and completion of the Flutter UI itself remains
     outside this non-Flutter package.

## Feature status rollups

1. `features/index.md`, `features/tui-skeleton/index.md`, and
   `features/tui-skeleton/tasks/index.md`
   - The current-status prose still cites 193 TUI tests.
   - Current evidence: 405 default TUI tests pass, four display/environment
     mode tests pass serially, and all 13 snapshots are current. The former
     31/32/Orca-gate statement was superseded by Revision 4: Task 008 added
     deterministic automation and Task 009 reconciled the notebook to 32/32,
     retaining Orca/VoiceOver only as optional unexecuted platform confidence.
2. `features/tui-enhancements/index.md` and its task index
   - **Implementation complete, verification pending** remains accurate at the
     end of Task 007. This note is historical: Revision 4 superseded the manual
     suite with Task 008 automation, and Task 009 reconciled T5.3 as complete
     while retaining optional non-gating Orca/VoiceOver platform cases.
3. `features/vault-core/index.md` and its task index
   - **External macOS/CI/M1 verification pending** remains an honest external
     evidence gap on this Linux host. The repository has a macOS+Linux CI
     matrix and Linux equivalents pass, but this workflow will not fabricate a
     CI run or M1 benchmark result. Task 008 records the final external
     disposition.

## Items intentionally unchanged

- `ssh-keys`, the optional `agent`, and Flutter UI completion are roadmap or
  excluded work, not false completion claims repaired by this package.
- Historical phase-by-phase changelogs may retain what was true at the time;
  only their current-status summaries need the corrections above.
