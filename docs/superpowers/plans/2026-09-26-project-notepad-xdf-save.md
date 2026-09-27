# Project Notepad and XDF Save Confirmation

## Task 1: Explicit XDF overwrite confirmation

- [x] Preserve create-new behavior without confirmation.
- [x] Use the Save dialog's overwrite confirmation or NookLink's existing
  challenge as the sole authorization, and label NookLink overwrites clearly.
- [x] Stage and reparse the XDF before replacing an existing destination.
- [x] Test overwrite, no-confirm refusal, invalid staged data, and
  clean-close-after-save behavior.

## Task 2: Project Notepad utility window

- [x] Add backward-compatible per-project note text and Stay on top settings.
- [x] Add Tools and dock entry points; use shared window focus/geometry handling.
- [x] Test legacy/default preference behavior, persistence, UI exposure, and
  unchanged BIN data.

## Task 3: Verification and handoff

- [x] Run formatting, app tests, workspace tests, and release build within the
  repository's test-repeat limit.
- [ ] Complete the release GUI WM_CLOSE smoke check; see the ledger for the
  current process-close failure.
- [x] Update `AGENTS.md`, the execution ledger, and final source snapshots.
