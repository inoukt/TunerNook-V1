# Per-BIN Workspace Persistence and Scrollable Frames Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make new BIN projects open with collapsed categories, restore each BIN's saved desktop workspace, and keep panels/tables usable through independent scrollable frames.

**Architecture:** Keep global appearance and shortcut settings in `AppPreferences`, and add a versioned `ProjectPreferences` record for workspace state. Project records are stored atomically in an app-data `projects` directory under a filename derived from the normalized BIN path, so the source BIN/XDF files remain unchanged. `TunerApp` loads the record when a BIN becomes active, restores compatible table state when its XDF is available, and stages continuous UI changes for idle autosave.

**Tech Stack:** Rust, Cargo workspace, `serde`/`serde_json`, `eframe`/`egui`, existing `tuner-core` SHA-256 helper, headless Rust unit tests, Windows native egui application.

**Spec:** `docs/superpowers/specs/2026-09-13-floating-workspace-design.md`

## Global Constraints

- Do not modify `tuner-core` or `tuner-xdf` for this presentation-state feature.
- Never store BIN bytes, staged transactions, or source-file reopen instructions in project preferences.
- Use the normalized full BIN path as project identity and verify the identity stored in a loaded record.
- Preserve the existing no-overwrite Save As behavior and BIN edit transaction boundaries.
- Keep headless tests from reading or writing the real `%APPDATA%\\TunerNook\\settings.json`.
- Use `apply_patch` for source and documentation edits; this workspace has no usable Git repository, so verification replaces commit checkpoints.

---

### Task 1: Add project preference data and atomic persistence helpers

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` near `AppPreferences`, preference parsing, and save helpers.
- Test: `crates/tuner-app/src/lib.rs` in the existing `#[cfg(test)]` module.

**Interfaces:**
- Produces `ProjectPreferences`, `PROJECT_SETTINGS_VERSION`, `project_identity_for_path`, `project_settings_path`, `load_project_preferences`, and `save_project_preferences` for later tasks.
- `ProjectPreferences` owns `bin_identity`, project layout/panel visibility, browser filter/category state, browser/inspector drawer state, diagnostics height, favorites/recents, table memories, tab orders, and the open/active table keys.

- [x] **Step 1: Write the failing project preference round-trip test**

  Add a test that builds a non-default `ProjectPreferences`, saves it to a temporary JSON path, loads it for the matching identity, and asserts that category state, drawer state, table memory, tab order, and open/active keys survive the round trip. Also assert that a load with a different identity returns safe defaults instead of the other project's state.

- [x] **Step 2: Run the focused test and verify the expected red failure**

  Run `cargo test -p tuner-app project_preferences_round_trip_is_identity_scoped`.

  Expected result: compilation/test failure because `ProjectPreferences` and its load/save API do not exist yet.

- [x] **Step 3: Implement the minimal serializable project model and helpers**

  Add a `#[serde(default)]` versioned `ProjectPreferences` with sanitized bounded widths, table memories, deduplicated bounded recents, and an explicit `category_state_initialized` flag. Normalize existing BIN paths with `fs::canonicalize` when possible, convert separators to `/`, and lowercase the identity for Windows-stable matching. Use `sha256_hex(identity.as_bytes())` for a safe file name below the settings file's parent directory in `projects/`.

  Factor the existing atomic JSON replacement logic into a reusable helper that creates the parent directory, writes a uniquely named temporary file, replaces an existing target with a backup/restore fallback, and removes temporary artifacts on success. Use it for both global and project preference files.

- [x] **Step 4: Run the focused test and verify green**

  Run `cargo test -p tuner-app project_preferences_round_trip_is_identity_scoped`.

  Expected result: PASS, including mismatch fallback and no write to the real settings path.

- [x] **Step 5: Run the existing preference tests**

  Run `cargo test -p tuner-app preferences_file_replacement_round_trips_without_leaving_swap_files` and `cargo test -p tuner-app settings_v1_migrate_to_category_first_workspace_defaults`.

  Expected result: PASS, proving the helper refactor did not regress global preference migration or replacement safety.

### Task 2: Make workspace state BIN-scoped and restore it through the load lifecycle

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` in `TunerApp`, BIN/XDF dialog handlers, table actions, and preference flushing.
- Test: `crates/tuner-app/src/lib.rs` in the existing app test module.

**Interfaces:**
- Consumes the Task 1 project persistence helpers.
- Adds runtime `project_identity`, `project_preferences`, `project_preferences_dirty`, and methods that activate/save a project, initialize categories, synchronize runtime table records, and restore compatible open tables.

- [x] **Step 1: Write failing tests for collapsed defaults and BIN isolation**

  Add one test that loads fixture documents into a headless app, activates a new project, initializes project categories, and asserts `category_state_initialized` is true and every grouped category is in `collapsed_categories`. Add a second test that gives two different BIN identities different drawer widths/category states, switches between them using the project activation helper, and asserts each state returns with the correct identity and without changing the loaded document bytes.

- [x] **Step 2: Run the new focused tests and verify red**

  Run `cargo test -p tuner-app new_project_categories_are_collapsed_by_default` and `cargo test -p tuner-app project_state_is_isolated_by_bin_identity` separately.

  Expected result: compilation/test failure because the project activation/runtime state API does not exist.

- [x] **Step 3: Implement project activation and default category initialization**

  Add `TunerApp` runtime project state. Before switching away from a different active BIN, synchronize and save the current project. After a successful `open_bin_dialog`, activate the normalized BIN identity, load its project record or a clean project default, and initialize every currently visible category as collapsed when no category state exists. When an XDF opens, ensure categories are initialized and restore only table keys whose XDF fingerprint matches the current document.

  Keep table keys XDF-fingerprint-qualified. Restore open/active tables only for existing semantic IDs; ignore stale IDs. Save current open table keys and active key whenever table tabs are opened, closed, reordered, focused, pinned, or arranged.

- [x] **Step 4: Run the focused tests and verify green**

  Re-run both Task 2 tests. Expected result: PASS, with each BIN receiving independent state and new projects starting collapsed.

- [x] **Step 5: Add and pass open-table restoration coverage**

  Add a test that opens a real fixture parameter, changes its remembered table memory and active/open state, serializes the project record, creates a fresh headless app for the same BIN/XDF identity, restores the project, and asserts the same table key, geometry, zoom, scroll, tab order, and active key are selected. Run `cargo test -p tuner-app project_state_restores_open_table_layout` and expect PASS.

- [x] **Step 6: Route autosave and reset/layout commands to the active project**

  Update table actions, drawer collapse/width changes, browser organization/category actions, layout presets, reset preferences, and idle flushing to stage/save `ProjectPreferences`. Keep global appearance/shortcut changes in `AppPreferences`; synchronize legacy fields only where needed for migration compatibility. Ensure headless mode changes memory only and never writes to disk.

- [x] **Step 7: Run the full app test suite**

  Run `cargo test -p tuner-app`.

  Expected result: PASS, including all pre-existing table, drawer, command, migration, editing, fixture, and debug-report tests.

### Task 3: Make browser, inspector, diagnostics, and table content resize-safe

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` in browser/inspector/diagnostics/settings/debug/table rendering.
- Test: `crates/tuner-app/src/lib.rs` with pure state/layout helper tests where a behavior can be verified headlessly.

**Interfaces:**
- Consumes active `ProjectPreferences` and existing `TableWindowMemory` scroll/geometry state.
- Produces bounded egui content frames with stable scroll IDs and no content-driven minimum width that blocks resizing.

- [x] **Step 1: Add a failing helper-level test for default frame policy**

  Add a small pure test for the chosen frame policy constants/helpers, asserting browser and inspector frames allow their configured minimum panel widths and table content enables both axes. The test should fail until the constants/helper are introduced, keeping the UI policy explicit rather than relying on an untestable literal.

- [x] **Step 2: Run the focused test and verify red**

  Run `cargo test -p tuner-app scrollable_frame_policy` and `cargo test -p tuner-app browser_and_inspector_frames_scroll_long_labels_horizontally`.

  Expected result: compilation/test failure because the policy helper does not exist.

- [x] **Step 3: Implement independent scrollable frames**

  Wrap browser results in a bounded two-axis `ScrollArea` configured with stable IDs and disabled auto-shrink so long labels can overflow without expanding the panel. Keep search/organization controls fixed above the result frame. Wrap inspector details in its own two-axis frame so axis metadata cannot push the right drawer beyond the viewport. Keep diagnostics, settings, command palette, and debug report content inside their own scrollable frames.

  Configure each floating table's grid frame for both-axis scrolling with shrink disabled, preserve the saved scroll offset only on the first render after restoration, and capture the resulting offset after rendering. Ensure window defaults and min sizes remain clamped to the canvas, not to table title or grid content width.

- [x] **Step 4: Run the focused test and full app tests**

  Run `cargo test -p tuner-app scrollable_frame_policy`, `cargo test -p tuner-app browser_and_inspector_frames_scroll_long_labels_horizontally`, then `cargo test -p tuner-app`.

  Expected result: PASS with no regressions in table rendering state or preference tests.

### Task 4: Update user-facing documentation and verify the delivered behavior

**Files:**
- Modify: `README.md` with project-state location/defaults and scrolling behavior.
- Modify: `docs/superpowers/specs/2026-09-13-floating-workspace-design.md` (approved design record, already updated with this slice).
- Modify: `docs/superpowers/plans/2026-09-13-project-workspace-persistence.md` as steps are completed.

**Interfaces:**
- Documents the user-visible behavior without exposing implementation-only details as required setup.

- [x] **Step 1: Update the README**

  Explain that a new BIN starts with collapsed categories, reopening the same BIN restores its workspace, project records live under `%APPDATA%\\TunerNook\\projects\\`, and browser/inspector/table frames scroll independently so long content does not block resizing.

- [x] **Step 2: Run formatting, build, and tests**

  Run `cargo fmt --all -- --check`, `cargo build --workspace`, and `cargo test --workspace`.

  Expected result: all commands pass.

- [x] **Step 3: Validate the fixture without modifying it**

  Run `cargo run --quiet -p tuner-cli -- xdf-validate "Test bin and xdf/SCGa05_cal.xdf" "Test bin and xdf/SCGa05_cal.bin"`, then recompute SHA-256 for both fixture files and compare to the pre-change hashes recorded in the project context.

  Expected result: validation reports zero issues and both fixture hashes are unchanged.

- [ ] **Step 4: Perform a native smoke check if a native UI surface is available**

  Open the fixture pair in `target/debug/tuner-app.exe`, confirm categories start closed, expand one category, open and resize a table narrower than its title/list content, scroll its grid, minimize/restore drawers, close/reopen the same BIN, and confirm state restoration. If the environment still exposes no native computer surface, record that limitation rather than claiming visual verification.

  Environment result: this host exposes no native computer surface, so the visual smoke check remains pending for a manual Windows run.

- [x] **Step 5: Review the diff and report the exact artifacts**

  Run `rg -n "ProjectPreferences|project_settings_path|category_state_initialized|ScrollArea" crates/tuner-app/src/lib.rs README.md docs/superpowers` and inspect the final diff. Report the executable path, tests run, fixture validation result, and any native smoke limitation.
