# Project, Workspace, and Status UX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Save and restore a complete BIN-referenced project/workspace manifest without writing BIN bytes, and make the main toolbar, empty workspace, search, edit status, diagnostics, and window-reset behavior clearer.

**Architecture:** Keep existing per-BIN `ProjectPreferences` as the source of truth for active and named workspaces. Add a small versioned `.tnproj` manifest that references BIN/XDF paths and carries `ProjectPreferences`; route Open Project through the existing BIN/XDF loaders and workspace restoration. Reuse the command registry, geometry memories, and current BIN Save As path for the remaining UI changes.

**Tech Stack:** Rust 2021, egui/eframe 0.36.2, serde/serde_json, existing `rfd` dialogs and background operation coordinator; no new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-25-project-workspace-ux-design.md`

## Global Constraints

- Project files reference existing BIN/XDF paths only; they never contain, copy, or write BIN/XDF bytes.
- Saving a project does not clear BIN dirty state; modified BIN output still uses the existing BIN Save As flow.
- Per-BIN workspace state and named workspace snapshots remain independent and are restored only for the matching BIN identity.
- Reset Workspace Windows changes geometry only in the active workspace; it preserves other snapshots, window open/minimized state, view settings, and document data.
- Keep existing BIN Save As no-overwrite behavior and command IDs/shortcuts; no new dependency.
- There is no `.git`; before editing source, create `.superpowers/sdd/2026-09-25-project-workspace-ux/progress.md` and before snapshots. Use final snapshots instead of commits.
- Do not rerun any one focused test workflow more than 10 times; change test type or broaden verification instead.

## File Structure

- Create `crates/tuner-app/src/project_file.rs` for the manifest DTO, file version, validation, JSON encode/decode, and isolated round-trip tests.
- Modify `crates/tuner-app/src/lib.rs` for the project-file association, command registry/dialogs/restore handoff, reset-geometry command, responsive toolbar/canvas, search labels, and status/diagnostics presentation.
- Modify `crates/tuner-app/src/tests.rs` for app-level project lifecycle, active-workspace reset, and headless UI width/state regressions.
- Modify `AGENTS.md` and the SDD execution ledger after implementation and verification. Do not split the existing large `lib.rs` as unrelated refactoring.

## Review Focus

1. A manifest with an unsupported version, a `preferences.bin_identity` that does not match the canonical BIN path identity, missing BIN, or missing optional XDF must not silently replace the currently open documents. A missing optional XDF is a reported BIN-only restore.
2. Saving while BIN is dirty must serialize project/workspace references but leave BIN bytes, dirty state, and undo history unchanged.
3. Resetting geometry in one layout must not mutate another named workspace or accidentally reset zoom, color, search query, selected cells, or open/minimized state.
4. Narrow toolbar widths and the four document-presence states must retain an obvious next action without invoking a different file command.
5. Value-search guidance must track BIN availability and field scope without changing wildcard or value matching.

---

### Task 1: Versioned project manifest model

**Files:**
- Create: `crates/tuner-app/src/project_file.rs`
- Modify: `crates/tuner-app/src/lib.rs` (module declaration and `ProjectPreferences` manifest-path field/version)
- Test: unit tests in `crates/tuner-app/src/project_file.rs`

**Interfaces:**
- Produces `TunerProjectFile { format_version: u32, bin_path: PathBuf, xdf_path: Option<PathBuf>, preferences: ProjectPreferences }`.
- Produces `TunerProjectFile::from_json(source: &str) -> Result<TunerProjectFile, String>`, `TunerProjectFile::validate(&self) -> Result<(), String>`, and `TunerProjectFile::to_json(&self) -> Result<String, String>`.
- Adds `ProjectPreferences.project_file_path: Option<PathBuf>`; it is BIN-project scoped, not workspace scoped.

- [x] **Step 1: Write failing manifest round-trip and validation tests**

Add `pub mod project_file;` and tests that the manifest retains BIN/XDF references and all active/named workspace state, accepts a BIN-only manifest, and rejects an unsupported format version, empty BIN path, and a `preferences.bin_identity` that differs from `project_identity_for_path(bin_path)`. Assert old `ProjectPreferences` JSON defaults `project_file_path` to `None`.

- [x] **Step 2: Run the focused manifest tests and observe expected failures**

Run: `cargo test -p tuner-app project_file -- --nocapture`

Expected: compilation/test failure because `project_file` and `TunerProjectFile` do not exist yet.

- [x] **Step 3: Add the minimal manifest module and serde-defaulted association field**

Add the versioned DTO with `from_json`/`validate`/`to_json`, `project_file_path` defaulting to `None`, and increment `PROJECT_SETTINGS_VERSION` from 11 to 12. Preserve deserialization of older `ProjectPreferences` JSON through serde defaults and the existing sanitizer.

- [x] **Step 4: Run the focused manifest tests**

Run: `cargo test -p tuner-app project_file -- --nocapture`

Expected: manifest round-trip and invalid-version/path/identity tests pass.

### Task 2: Open, Save, and Save As project manifests

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` (commands, File menu, dialogs, pending restore state, save/open lifecycle)
- Modify: `crates/tuner-app/src/tests.rs` (project save/open integration and safety)

**Interfaces:**
- Add registered command IDs `file.open-project`, `file.save-project`, and `file.save-project-as` with matching `BuiltinCommand` variants and availability reasons.
- Add `TunerApp::save_project_manifest(path: &Path) -> Result<(), WorkspaceError>`, `save_project_dialog`, `save_project_as_dialog`, and `open_project_dialog`.
- Add `TunerApp::open_project_file(path: PathBuf) -> Result<(), WorkspaceError>` and `pending_project_file: Option<TunerProjectFile>` so the selected manifest survives the existing asynchronous BIN restore.
- Save captures the live active workspace via `capture_active_workspace_view()`, references `workspace.bin_path` and optional `workspace.xdf_path`, and atomically writes `.tnproj`. Keep top-level `xdf_path` and serialized `preferences.last_xdf_path` synchronized. Save Project uses `ProjectPreferences.project_file_path`; if absent it opens Save Project As. Save As serializes the intended manifest association, but updates in-memory/persisted preferences only after the atomic write succeeds.
- Open validates the manifest and required BIN before changing current state, starts the existing async BIN load, applies manifest preferences only after the restored canonical BIN path identity matches, sets the manifest association to the opened `.tnproj`, then starts the referenced XDF load if present. Keep top-level `xdf_path` and `preferences.last_xdf_path` synchronized. A missing XDF restores BIN-only with an explicit warning; a missing BIN or identity mismatch clears pending manifest state and keeps the current documents.

- [x] **Step 1: Write failing command and save-safety tests**

Test that File exposes Open/Save/Save As Project; Save Project without an associated path requests Save As; after association, Save Project updates that manifest; and Save Project As creates a `.tnproj` containing all workspace snapshots and references. Assert the manifest XDF path matches `preferences.last_xdf_path`. With BIN dirty, assert its bytes, dirty flag, and undo depth are unchanged and the status directs the user to BIN Save As.

- [x] **Step 2: Run the focused project-save test and observe expected failure**

Run: `cargo test -p tuner-app project_manifest_save -- --nocapture`

Expected: test fails because project commands and the manifest-save path are not registered.

- [x] **Step 3: Implement project save commands and atomic manifest association**

Register command descriptors, availability, and File-menu entries. Capture the active workspace before writing. On successful Save As, set `project_file_path` and persist project preferences; leave `BinDocument` untouched. When dirty, report “Project/workspace saved; BIN edits remain unsaved—use Save As to export.”

- [x] **Step 4: Re-run project-save and existing BIN no-overwrite tests**

Run: `cargo test -p tuner-app project_manifest_save -- --nocapture`

Then run: `cargo test -p tuner-app save_as_creates_new_file_and_refuses_existing_target -- --nocapture`

Expected: project-manifest tests pass and BIN Save As still refuses an existing destination.

- [x] **Step 5: Write failing Open Project lifecycle tests**

Use temporary BIN/XDF fixtures and a temporary `.tnproj` to test successful async restore of the named workspace and persistence of the opened manifest association, BIN-only open when the optional XDF is missing, normalization of the XDF reference into `preferences.last_xdf_path`, and rejection of an invalid/mismatched manifest without replacing already-open document state.

- [x] **Step 6: Run the focused Open Project tests and observe expected failure**

Run: `cargo test -p tuner-app open_project_manifest -- --nocapture`

Expected: test fails because no Open Project action or pending manifest restore path exists.

- [x] **Step 7: Implement Open Project through existing BIN/XDF operation handlers**

Add a `.tnproj` file dialog, validate before switching, carry the manifest through the existing BIN workspace-restore result, apply its `ProjectPreferences` only for the matched BIN identity, then queue the referenced XDF using `start_xdf_load`. Preserve the existing loading overlay and do not introduce a second BIN/XDF parser.

- [x] **Step 8: Run focused project open/save tests**

Run: `cargo test -p tuner-app project_manifest -- --nocapture`

Expected: save, dirty-BIN safety, valid restore, BIN-only restore, and invalid/missing-reference cases pass.

### Task 3: Reset Window Geometry in the Active Workspace

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` (new command and reset method)
- Modify: `crates/tuner-app/src/tests.rs` (active/inactive snapshot and preserved-state tests)

**Interfaces:**
- Add command ID `view.reset-workspace-windows` and `BuiltinCommand::ResetWorkspaceWindows`; expose it under View → Workspace and the existing Arrange menu.
- Add `TunerApp::reset_workspace_windows()`.

- [x] **Step 1: Write failing reset-scope and preservation tests**

Seed two workspace snapshots with different window rectangles; open/minimize a table and tool; set non-default zoom/color/search/selection and dirty BIN state; reset the active workspace; assert geometry defaults are applied to active and remembered-closed windows while the other snapshot, open/minimized state, view settings, BIN bytes, XDF identity, and data revision are unchanged.

- [x] **Step 2: Run the focused reset test and observe expected failure**

Run: `cargo test -p tuner-app reset_workspace_windows -- --nocapture`

Expected: test fails because the reset command/method is absent.

- [x] **Step 3: Reset geometry only in active project preferences**

For tables, restore default position and set `fit_to_content = Some(true)` so dimensions are recalculated at each table's current zoom. For surfaces, Search, Hex, Map Finder, Compare/Compare 3D, and utility windows, reset only x/y/width/height to existing type defaults. Keep current table zoom/color/precision, surface camera/view settings, tool configuration, dock state, selections, and documents. Request geometry for currently open windows and leave inactive `saved_workspace_snapshots` untouched.

- [x] **Step 4: Run the focused reset test and the existing table-reset regression**

Run: `cargo test -p tuner-app reset_workspace_windows -- --nocapture`

Then run: `cargo test -p tuner-app reset_table_layout_recalculates_dimension_aware_default_size -- --nocapture`

Expected: active-only reset and preserved state pass; existing reset-table behavior remains unchanged.

### Task 4: Responsive Toolbar, Context-Aware Canvas, and Search Labels

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` (`show_toolbar`, `show_workspace_canvas`, browser filter, Advanced Search title/help)
- Modify: `crates/tuner-app/src/tests.rs` (headless UI label/action/width checks)

**Interfaces:**
- Reuse command IDs `file.open-bin` and `file.open-xdf` for all canvas actions.
- Preserve existing `SearchState`, `SearchMatchMode::Wildcard`, and search execution behavior.

- [x] **Step 1: Write failing narrow-toolbar and empty-canvas UI checks**

Render the application at 360, 520, and 900 logical-pixel widths and assert Open BIN/XDF, project save, BIN Save As, Undo, Redo, and status labels remain painted/reachable. Render the canvas with no docs, BIN-only, XDF-only, and both docs/no table, and assert the expected next-action text/buttons for each state.

- [x] **Step 2: Run the focused toolbar/canvas UI tests and observe expected failure**

Run: `cargo test -p tuner-app toolbar_reflows -- --nocapture`

Expected: narrow-width layout or document-state guidance assertions fail against the single-row toolbar/generic prompt.

- [x] **Step 3: Reflow action/status groups and render context-aware open actions**

Use an `horizontal_wrapped` action row and a separate `horizontal_wrapped` status row. In the no-table canvas, render only the Open BIN/XDF command buttons appropriate for loaded documents; preserve the existing choose-parameter text when both documents are loaded.

- [x] **Step 4: Run toolbar/canvas tests**

Run: `cargo test -p tuner-app toolbar_reflows -- --nocapture`

Expected: all widths and four document states pass without changing file-loading behavior.

- [x] **Step 5: Write failing quick-filter/Advanced Search guidance tests**

Assert the browser exposes “List filter”, the separate window exposes “Advanced Search”, Wildcard mode explains `*` and `?`, and the BIN-required notice appears when value search is selected without a BIN but not when a BIN is loaded. Also assert metadata search remains available without a BIN.

- [x] **Step 6: Run the focused search-guidance UI tests and observe expected failure**

Run: `cargo test -p tuner-app search_guidance_labels -- --nocapture`

Expected: current labels/hints do not contain the requested guidance.

- [x] **Step 7: Update only search copy and its visible guidance**

Label the browser filter as a quick List filter, rename the window title to Advanced Search, add “Wildcard: `*` = any text, `?` = one character” near the match control, and emphasize the existing BIN-required value-search note under the existing query/scope condition.

- [x] **Step 8: Run the focused search-guidance tests**

Run: `cargo test -p tuner-app search_guidance_labels -- --nocapture`

Expected: label/help states pass and the existing query engine is unchanged.

### Task 5: Unsaved BIN Status and Actionable Diagnostics

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` (`show_toolbar` status group and `show_diagnostics`)
- Modify: `crates/tuner-app/src/tests.rs` (clean/dirty and healthy/error rendering assertions)

- [x] **Step 1: Write failing status/diagnostic display tests**

For a clean BIN, assert concise non-dirty wording; for an edited BIN, assert “Unsaved BIN edits” and “Save As” guidance. For healthy mappings assert compact green/no-issue status; for validation errors assert prominent warning/error count and readable issue summary.

- [x] **Step 2: Run the focused status test and observe expected failure**

Run: `cargo test -p tuner-app unsaved_bin_and_diagnostics_status -- --nocapture`

Expected: dirty state still uses the ambiguous “dirty” label and error emphasis is not asserted.

- [x] **Step 3: Implement concise healthy and emphasized warning states**

Replace only the status copy/style. Keep Diagnostics resizable, respect its saved height, and keep details scrollable. Do not alter `BinDocument::save_as`, destination handling, or validation results.

- [x] **Step 4: Run status and no-overwrite tests**

Run: `cargo test -p tuner-app unsaved_bin_and_diagnostics_status -- --nocapture`

Then run: `cargo test -p tuner-app save_as_creates_new_file_and_refuses_existing_target -- --nocapture`

Expected: status changes pass and BIN Save As safety remains unchanged.

### Task 6: Project Documentation and Full Verification

**Files:**
- Modify: `AGENTS.md`
- Create/update during execution: `.superpowers/sdd/2026-09-25-project-workspace-ux/progress.md` and before/final snapshots

- [x] **Step 1: Record implementation decisions and exact tests in the SDD ledger**
- [x] **Step 2: Run `cargo fmt --all -- --check`**
- [x] **Step 3: Run `cargo test -p tuner-app`**
- [x] **Step 4: Run `cargo test --workspace`**
- [x] **Step 5: Run `cargo build --release -p tuner-app`**
- [x] **Step 6: Run Git Bash `scripts/smoke-test.sh --app` with its isolated smoke profile**
- [x] **Step 7: Verify the smoke app closed gracefully and the input BIN stayed unchanged; save final snapshots and update `AGENTS.md` with observed results**
