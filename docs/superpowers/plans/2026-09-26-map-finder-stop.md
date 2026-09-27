# Map Finder Stop implementation plan

**Goal:** Add responsive cancellation to the current background map scan and
measure CPU-worker feasibility separately.

**Spec:** `docs/superpowers/specs/2026-09-26-map-finder-stop.md`

## Task 1: Stop and cancellation lifecycle

Files: `map_search.rs`, `operations.rs`, `lib.rs`, `tests.rs`,
`docs/tuner-ui-ipc.md`, `AGENTS.md`.

- [x] Add a failing scanner cancellation test proving partial candidates are
  discarded and cancellation is reported.
- [x] Add failing UI and NookLink tests for Stop, busy/idle behavior, and status.
- [x] Add a shared atomic token checked at bounded intervals; return an explicit
  canceled result through the worker and operation coordinator.
- [x] Add Stop/Stopping controls and the `cancel_map_search` UI IPC action.
- [x] Document the action, update project notes, and verify no BIN/XDF mutation.
- [x] Run fmt, app tests, workspace tests, release build, and GUI smoke.

## Task 2: CPU worker feasibility spike (throwaway only)

- [x] Compare serial and shape-split CPU scan timings on the same bounded fixture
  and scan configuration; report measured speedup and overhead.
- [x] Do not retain parallel-scanner code or add dependencies in this task.

## Follow-up: Stop from the background-operation popup

Files: `lib.rs`, `tests.rs`, this spec/plan, and the SDD ledger.

- [x] Add a UI regression for the amber popup Stop action and non-scan hiding.
- [x] Place Stop/Stopping beside the popup progress bar and reuse the existing
  `request_map_search_cancel` path.
- [x] Re-run formatting and the project verification battery; record snapshots.

Follow-up verification (2026-09-26): fmt clean; app tests 407/407; workspace
tests 489/489; release build succeeds; `scripts/smoke-test.sh --app` passes
21/21 through Git Bash, including graceful GUI close and unchanged fixture BIN.
Snapshots: `.superpowers/sdd/2026-09-26-map-finder-stop/snapshots/popup-followup-final/`.

## Constraints

- No GPU backend/dependency.
- Cancellation is cooperative and bounded by periodic worker checks; it must not
  block the UI thread.
- User-selected approval covers Stop and the separate throwaway CPU benchmark,
  not production multi-worker scanning.
