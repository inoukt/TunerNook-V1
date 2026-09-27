# Table Geometry and Smoke Safety Implementation Plan

> **For agentic workers:** Implement inline in this task, test-first, and record each phase in `.superpowers/sdd/2026-09-25-table-geometry-and-smoke-safety/progress.md`.

**Goal:** Restore table-window geometry consistently and make GUI smoke cleanup PID-specific.

**Architecture:** Reuse `TableWindowMemory` and project-scoped recent-table order. Auto-fit geometry follows zoom changes live; exact XDF-fingerprint keys remain authoritative, with a missing-key fallback to the most recently used matching table in the active BIN project. For smoke testing, PowerShell captures the launched process object/PID and performs both graceful close and any fallback kill through that same object.

**Tech Stack:** Rust 2021, egui/eframe 0.36.2, Bash, PowerShell; no new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-25-table-geometry-and-smoke-safety.md`

## Global Constraints

- Keep typed table-window memory as the single saved-geometry source.
- Preserve explicit user sizing and canvas clamping; scroll when the usable canvas cannot fit a table.
- Add no dependencies or duplicate persistence stores.
- Smoke cleanup may target only the exact GUI PID launched by this run.
- Preserve existing BIN/XDF bytes and other running app instances.
- Workspace has no `.git`; record before/final snapshots and progress in `.superpowers/sdd/2026-09-25-table-geometry-and-smoke-safety/`.
- Do not repeat an identical focused test workflow more than 10 times.

## Review Focus

1. An auto-fit table zoom change must persist the new content-fit geometry before close/reopen; a manually sized table must retain its chosen border.
2. An XDF edit changes its normalized fingerprint but preserves a table semantic ID; its saved geometry should still be reused within the same BIN project.
3. Multiple older definitions for that semantic ID must use recent-table order, while an exact current key takes precedence.
4. A same-semantic memory from another BIN project must never leak into the active project's layout, and small displays must keep scroll access after clamping.
5. When another `tuner-app.exe` is already running, `scripts/smoke-test.sh --app` must leave it alive and close only its own process.

---

### Task 1: Keep saved and live table geometry consistent

**Files:**
- Modify: `crates/tuner-app/src/tests.rs`
- Modify only if the test reproduces: `crates/tuner-app/src/lib.rs`

**Interfaces:** Reuse `show_table_window`, `open_table_internal`, `TableWindowMemory`, and project-scoped `recent_tables`. The full table key remains `XDF fingerprint | semantic ID`.

- [x] Add a rendered-UI regression that changes zoom on an auto-fit table and asserts the live border matches the saved-zoom content-fit size; confirm a manually sized window is not resized by zoom.
- [x] Run the focused zoom regression and verify it fails because the current border retains its prior dimensions (846×428 observed vs. 762×386 expected at 90% zoom).
- [x] Recompute and request the content-fit size on zoom changes only when `fit_to_content` is true; keep position and manual size intact.
- [x] Add a regression where two older XDF fingerprints retain the same semantic ID but different geometries; opening the new definition must use the most recent old geometry.
- [x] Run `cargo test -p tuner-app table_reuses_recent_project_geometry_after_xdf_revision -- --nocapture`; verify it fails because only exact keys are currently consulted (429×240 default vs. 920×610 saved geometry).
- [x] Reuse the most recent semantic-ID match only when the active BIN project has no exact key. Keep exact key precedence and project isolation.
- [x] Run the zoom and fingerprint regressions plus `project_state_restores_open_table_layout`, manual resize, and small-canvas geometry tests.

### Task 2: Make GUI smoke cleanup PID-specific

**Files:**
- Modify: `scripts/smoke-test.sh`

**Interfaces:** Keep `scripts/smoke-test.sh [--app]`; allow `TUNERNOOK_BIN_DIR` to override the release executable directory for validation.

- [x] Launch via PowerShell `Start-Process -PassThru`, capture its Windows PID, wait on the returned process object, request close with `CloseMainWindow`, and use `Kill` only on that same process object if it does not exit.
- [x] Remove all image-name-wide termination and accept a configurable binary directory without changing the default.
- [x] Run CLI/API smoke and then `scripts/smoke-test.sh --app` with a pre-existing TunerNook PID; verify the pre-existing process stays alive and the launched smoke PID exits normally.

## Final Verification

Run once after all tasks: `cargo fmt --all -- --check`, `cargo test -p tuner-app`, `cargo test --workspace`, `cargo build --release -p tuner-app`, and `bash scripts/smoke-test.sh --app` from Git Bash. Confirm every recorded PID cleanup targeted only the smoke-launched process.
