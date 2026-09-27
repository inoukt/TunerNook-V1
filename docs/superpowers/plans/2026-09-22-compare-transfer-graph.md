# Compare, Transfer, and Graph Comparison Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a desktop Compare workspace that compares a read-only source BIN with the active destination BIN, visualizes map differences in tables and 3D surfaces, and applies only explicitly reviewed transfer plans.

**Architecture:** Keep pure comparison math and serializable view types in a new `crates/tuner-app/src/compare.rs` module. Keep document loading, project persistence, windows, commands, IPC, and operation orchestration in the existing `crates/tuner-app/src/lib.rs`/`operations.rs` boundary. Reuse `tuner-core::compare_bytes`, `tuner-transfer::TransferPlan`, the existing `SurfaceData` projection, `WorkspaceState` selection, and the existing background-operation coordinator.

**Tech Stack:** Rust 2021, `eframe`/`egui` 0.36.2, serde/serde_json, `tuner-core`, `tuner-xdf`, `tuner-transfer`, existing `SurfaceData`/`SurfaceViewMemory`, and standard-library filesystem/threading only.

**Spec:** [2026-09-22-compare-transfer-graph-design.md](../specs/2026-09-22-compare-transfer-graph-design.md)

## Global Constraints

- The active workspace BIN is the only editable destination; the compare/source BIN is read-only.
- Transfer planning requires equal exact XDF SHA-256 hashes; normalized fingerprints are diagnostic only.
- Applying a transfer must use one existing `BinDocument` transaction and create one undo entry.
- Save As must remain explicit and must not overwrite an existing input or output file.
- Missing, non-finite, or incompatible mapped values remain unavailable; never silently substitute zeroes or position-match unrelated maps.
- Compare, graph, and transfer work must remain responsive through the existing background coordinator.
- Add no external runtime dependency; add only the existing workspace crate `tuner-transfer` to `tuner-app`.
- All UI and AI-agent mutations must use the same state methods and semantic IDs.
- Persist new settings with serde defaults and sanitize paths, filters, geometry, modes, and map keys.
- Do not repeat one test workflow more than ten times; use pure, integration, UI, IPC, and end-to-end verification as separate workflows.
- This checkout has no `.git` directory; use source/test checkpoints instead of commit steps.

## Review Focus

- Different BIN lengths: byte comparison includes trailing-only changes, while mapped comparison reports range validity without indexing past either document. Test in Task 3.
- Zero destination values in percentage delta mode: show unavailable percentage rather than infinity or a fake number. Test in Task 1.
- Different XDF hashes: comparison remains diagnostic where possible, but transfer is blocked with an explicit machine-readable issue. Test in Task 6.
- Stale source or destination bytes after dry-run planning: Apply rejects the plan and leaves destination bytes and undo history unchanged. Test in Task 6.
- Missing source map cells or incompatible dimensions: graphs preserve holes and do not fabricate aligned values. Test in Tasks 1 and 5.

---

### Task 1: Add the pure comparison model and delta math

**Files:**

- Create: `crates/tuner-app/src/compare.rs`
- Modify: `crates/tuner-app/src/lib.rs` module declarations and re-exports
- Modify: `crates/tuner-app/Cargo.toml` to add `tuner-transfer = { path = "../tuner-transfer" }`
- Test: `crates/tuner-app/src/compare.rs` unit tests

**Interfaces:**

- Produces `CompareValueMode::{Destination, Source, AbsoluteDelta, PercentDelta}` with `Serialize`, `Deserialize`, `Copy`, `Eq`, and `Default`; the default is `Destination`.
- Produces `CompareCell::from_values(destination: Option<f64>, source: Option<f64>) -> Self`.
- Produces `CompareCell::value(&self, mode: CompareValueMode) -> Option<f64>` using `source - destination` for absolute delta and `100.0 * delta / destination.abs()` for percentage delta.
- Produces `CompareMapData::new(semantic_id, rows, columns, x, y, cells)`, `value(row, column, mode)`, and `values(mode)`.
- Produces `symmetric_delta_range(values: impl IntoIterator<Item = f64>) -> Option<(f64, f64)>`, returning a finite zero-centered range for delta modes and `None` when no finite values exist.

- [x] **Step 1: Write failing comparison tests**

```rust
#[test]
fn compare_cell_delta_is_source_minus_destination() {
    let cell = CompareCell::from_values(Some(10.0), Some(15.0));

    assert_eq!(cell.value(CompareValueMode::Destination), Some(10.0));
    assert_eq!(cell.value(CompareValueMode::Source), Some(15.0));
    assert_eq!(cell.value(CompareValueMode::AbsoluteDelta), Some(5.0));
    assert_eq!(cell.value(CompareValueMode::PercentDelta), Some(50.0));
}

#[test]
fn compare_cell_percentage_delta_is_unavailable_for_zero_destination() {
    let cell = CompareCell::from_values(Some(0.0), Some(12.0));

    assert_eq!(cell.value(CompareValueMode::AbsoluteDelta), Some(12.0));
    assert_eq!(cell.value(CompareValueMode::PercentDelta), None);
}

#[test]
fn compare_map_preserves_missing_cells_in_every_mode() {
    let map = CompareMapData::new(
        "table:map".to_string(),
        1,
        2,
        vec![0.0, 1.0],
        vec![0.0],
        vec![
            CompareCell::from_values(Some(1.0), None),
            CompareCell::from_values(None, Some(2.0)),
        ],
    );

    assert_eq!(map.value(0, 0, CompareValueMode::Source), None);
    assert_eq!(map.value(0, 1, CompareValueMode::Destination), None);
}

#[test]
fn symmetric_delta_range_handles_flat_and_signed_values() {
    assert_eq!(symmetric_delta_range([0.0, 0.0]), Some((-1.0, 1.0)));
    let (minimum, maximum) = symmetric_delta_range([-10.0, 4.0]).unwrap();
    assert!(minimum < -10.0);
    assert!(maximum > 10.0);
    assert!((minimum + maximum).abs() < f64::EPSILON);
}
```

- [x] **Step 2: Run the focused tests and verify the expected RED failure**

Run:

```powershell
cargo test -p tuner-app --lib compare_cell_delta_is_source_minus_destination -- --exact
```

Expected: compile failure because `compare.rs`, `CompareCell`, and `CompareValueMode` do not yet exist.

- [x] **Step 3: Implement the smallest pure comparison module**

Add the module declaration and public re-exports. Implement finite-value checks, the exact delta sign, zero-denominator handling, bounds-safe map indexing, and a symmetric range with a minimum flat-map span of `[-1.0, 1.0]`. Do not read BIN/XDF documents in this module and do not add UI dependencies beyond the existing crate graph.

- [x] **Step 4: Run the varied pure-model tests**

Run:

```powershell
cargo test -p tuner-app --lib compare_cell -- --nocapture
cargo test -p tuner-app --lib compare_map_preserves_missing_cells -- --exact
cargo test -p tuner-app --lib symmetric_delta_range_handles_flat_and_signed_values -- --exact
```

Expected: all focused pure comparison tests pass without starting a UI operation.

- [x] **Step 5: Format and checkpoint**

Run `cargo fmt --all -- --check`. Record the source and test checkpoint; do not create a commit because this workspace has no Git metadata.

### Task 2: Add project-scoped compare state and persistence

**Files:**

- Modify: `crates/tuner-app/src/lib.rs` `ProjectPreferences`, sanitization, `TunerApp` state, reset logic, debug report, and project restore/staging hooks
- Modify: `crates/tuner-app/src/compare.rs` serializable `CompareWindowMemory` and compare-state helpers
- Test: `crates/tuner-app/src/lib.rs` existing preference/project test module

**Interfaces:**

- Produces `CompareWindowMemory` with `x`, `y`, `width`, `height`, and `sanitize()` using the same bounded window conventions as table/search windows.
- Adds serde-defaulted `ProjectPreferences` fields: `compare_bin_path: Option<PathBuf>`, `compare_xdf_path: Option<PathBuf>`, `compare_mode: CompareValueMode`, `compare_filter: String`, `compare_changed_only: bool`, `compare_window: CompareWindowMemory`, `compare_surface_windows: BTreeMap<String, SurfaceViewMemory>`, and `compare_selected_semantic_id: Option<String>`.
- Produces a runtime `CompareWorkspaceState` containing `window_open: bool`, `source_bin: Option<BinDocument>`, `source_bin_path: Option<PathBuf>`, `source_xdf: Option<XdfDocument>`, `source_xdf_path: Option<PathBuf>`, `mode: CompareValueMode`, `filter: String`, `changed_only: bool`, `selected_semantic_id: Option<String>`, `selected_cells: Option<CellSelection>`, `map_cache: BTreeMap<CompareCacheKey, CompareMapData>`, `transfer_plan: Option<TransferPlan>`, and `status: StatusMessage`.
- Does not serialize `BinDocument`, `XdfDocument`, comparison snapshots, or transfer plans.

Test-only fixture helpers created in the existing `#[cfg(test)]` module are `open_compare_with_test_documents()`, `app_with_matching_compare_documents()`, `app_with_compare_documents_and_different_xdfs()`, and `compare_map_with_values(values)`. They construct a headless app or pure map from the existing small XDF fixtures and never become production APIs.

- [x] **Step 1: Write failing persistence/isolation tests**

```rust
#[test]
fn compare_preferences_round_trip_and_sanitize_missing_values() {
    let mut preferences = ProjectPreferences::for_identity("bin-a");
    preferences.compare_bin_path = Some(PathBuf::from("C:/tuning/source.bin"));
    preferences.compare_xdf_path = Some(PathBuf::from("C:/tuning/source.xdf"));
    preferences.compare_mode = CompareValueMode::PercentDelta;
    preferences.compare_filter = "torque".to_string();
    preferences.compare_changed_only = true;
    preferences.compare_selected_semantic_id = Some("table:map".to_string());
    preferences.compare_window.width = 1;
    preferences.compare_window.height = 10_000;

    let encoded = serde_json::to_string(&preferences).unwrap();
    let decoded: ProjectPreferences = serde_json::from_str(&encoded).unwrap();
    let mut sanitized = decoded.clone();
    sanitized.sanitize();

    assert_eq!(sanitized.compare_mode, CompareValueMode::PercentDelta);
    assert_eq!(sanitized.compare_filter, "torque");
    assert!(sanitized.compare_window.width >= 420);
    assert!(sanitized.compare_window.height <= 1_800);
}

#[test]
fn compare_state_is_isolated_by_bin_identity() {
    let settings_path = temporary_test_path("settings.json");
    let first = project_identity_for_path(Path::new("C:/tuning/first.bin"));
    let second = project_identity_for_path(Path::new("C:/tuning/second.bin"));
    let first_path = project_settings_path(&settings_path, &first);
    let second_path = project_settings_path(&settings_path, &second);

    let mut first_preferences = ProjectPreferences::for_identity(&first);
    first_preferences.compare_bin_path = Some(PathBuf::from("source.bin"));
    save_project_preferences(&first_path, &first_preferences).unwrap();

    let loaded_second = load_project_preferences(&second_path, &second);
    assert_eq!(loaded_second.compare_bin_path, None);

    std::fs::remove_file(first_path).unwrap();
}
```

- [x] **Step 2: Run the persistence tests and verify RED**

Run:

```powershell
cargo test -p tuner-app --lib compare_preferences_round_trip_and_sanitize_missing_values -- --exact
```

Expected: compile failure because compare preference fields and `CompareWindowMemory` do not exist.

- [x] **Step 3: Add serde-defaulted preference fields and runtime state**

Implement defaults, sanitization, old-settings compatibility, project application/staging, reset clearing, and debug-report lines. When the active BIN identity changes, clear the loaded compare document, map cache, selection, and plan before applying the new project preferences. Retain missing compare paths for display but never auto-load them without user action.

- [x] **Step 4: Run persistence and existing project tests**

Run:

```powershell
cargo test -p tuner-app --lib compare_preferences -- --nocapture
cargo test -p tuner-app --lib compare_state_isolated_by_bin_identity -- --exact
cargo test -p tuner-app --lib project_preferences_round_trip_is_identity_scoped -- --exact
```

Expected: compare persistence and prior project-isolation tests pass.

### Task 3: Add background compare document loading and map snapshots

**Files:**

- Modify: `crates/tuner-app/src/operations.rs` operation kinds, messages, payloads, constructors, and worker methods
- Modify: `crates/tuner-app/src/lib.rs` compare load methods, result installation, cache invalidation, and diagnostics
- Modify: `crates/tuner-app/src/compare.rs` document-to-summary and document-to-map snapshot functions
- Test: `crates/tuner-app/src/operations.rs` and `crates/tuner-app/src/lib.rs`

**Interfaces:**

- Add `OperationKind::{LoadingCompareBin, LoadingCompareXdf, BuildingCompareMap, BuildingTransferPlan}` with user-facing phase messages.
- Add `OperationPayload::{CompareBin(BinDocument), CompareXdf(XdfDocument), CompareMap(CompareMapData), TransferPlan(TransferPlan)}`.
- Add `OperationCoordinator::spawn_load_compare_bin`, `spawn_load_compare_xdf`, `spawn_compare_map`, and `spawn_transfer_plan`.
- Add `TunerApp::start_compare_bin_load(path)`, `TunerApp::start_compare_xdf_load(path)`, `TunerApp::request_compare_map(semantic_id)`, and `TunerApp::install_compare_result(result)`.
- Add `build_compare_map_data(source_xdf, destination_xdf, source_bin, destination_bin, semantic_id) -> Result<CompareMapData, String>` and `OperationResult::compare_bin(id, subject, document)`/`OperationResult::compare_xdf(id, subject, document)` constructors.
- Add `CompareCacheKey { source_bin_sha256: String, destination_revision: u64, source_xdf_fingerprint: String, destination_xdf_fingerprint: String, semantic_id: String, mode: CompareValueMode }` with `Ord`/`Eq` so it can key the map cache.

- [x] **Step 1: Write failing operation and snapshot tests**

```rust
#[test]
fn compare_map_snapshot_reads_source_and_destination_values() {
    let (xdf, destination) = column_major_fixture();
    let source = BinDocument::from_bytes(vec![10, 40, 30, 50, 50, 80]);
    let data = build_compare_map_data(
        &xdf,
        &xdf,
        &source,
        &destination,
        "table:uid:map",
    )
    .unwrap();

    assert_eq!(data.value(0, 0, CompareValueMode::Destination), Some(10.0));
    assert_eq!(data.value(0, 0, CompareValueMode::Source), Some(10.0));
    assert_eq!(data.value(0, 1, CompareValueMode::AbsoluteDelta), Some(10.0));
}

#[test]
fn stale_compare_bin_result_cannot_replace_a_newer_source() {
    let mut app = TunerApp::headless();
    let first = app
        .operations
        .begin(OperationKind::LoadingCompareBin, "first.bin");
    let second = app
        .operations
        .begin(OperationKind::LoadingCompareBin, "second.bin");

    app.install_compare_result(OperationResult::compare_bin(
        first,
        "first.bin",
        BinDocument::from_bytes(vec![1]),
    ));
    assert!(app.compare.source_bin.is_none());

    app.install_compare_result(OperationResult::compare_bin(
        second,
        "second.bin",
        BinDocument::from_bytes(vec![2]),
    ));
    assert_eq!(app.compare.source_bin.as_ref().unwrap().bytes(), &[2]);
}
```

- [x] **Step 2: Run one pure snapshot test and one stale-result test to confirm RED**

Run:

```powershell
cargo test -p tuner-app --lib compare_map_snapshot_reads_source_and_destination_values -- --exact
cargo test -p tuner-app --lib stale_compare_bin_result_cannot_replace_a_newer_source -- --exact
```

Expected: compile failure because compare snapshot functions, state, payloads, and operation workers do not exist.

- [x] **Step 3: Implement compare workers and stale-result handling**

Clone source/destination documents into worker inputs. Build map data by semantic ID using the same `cell_view_for`/axis-resolution path as the active surface snapshot. Prefer finite engineering values, then finite raw values; preserve `None` for failures. Install results only when the operation ID and source request match the current compare generation. Replacing or closing the source document clears snapshots, plans, and selections.

- [x] **Step 4: Run operation and fixture tests**

Run:

```powershell
cargo test -p tuner-app --lib compare_map_snapshot_reads_source_and_destination_values -- --exact
cargo test -p tuner-app --lib stale_compare_bin_result_cannot_replace_a_newer_source -- --exact
cargo test -p tuner-app --lib path_based_document_load_uses_async_workspace_restore_before_validation -- --exact
```

Expected: compare workers pass while existing document workers remain green.

### Task 4: Add the desktop Compare window and table difference view

**Files:**

- Modify: `crates/tuner-app/src/lib.rs` command registry, file menu, toolbar, compare window rendering, selection/focus helpers, and frame update
- Modify: `crates/tuner-app/src/compare.rs` summary filtering/sorting helpers
- Test: `crates/tuner-app/src/lib.rs` headless UI tests

**Interfaces:**

- Add built-in commands `file.open-compare-bin`, `file.open-compare-xdf`, `view.compare`, `view.close-compare`, and `transfer.build-plan`.
- Add `TunerApp::show_compare_window(context)`, `TunerApp::open_compare_bin_dialog()`, `TunerApp::open_compare_xdf_dialog()`, `TunerApp::close_compare()`, and `TunerApp::focus_compare_semantic(semantic_id)`.
- Add `CompareMapSummary` fields for semantic ID, title, category, kind, dimensions, changed-cell count, source/destination availability, and transfer status.
- Use the existing `CellSelection` and `WorkspaceState::select_cell_range` contracts for compare-table selection.

- [x] **Step 1: Write failing headless window and selection tests**

```rust
#[test]
fn compare_window_renders_without_starting_document_work() {
    let context = egui::Context::default();
    let mut app = TunerApp::headless();
    app.compare.window_open = true;

    context
        .run_ui(egui::RawInput::default(), |ctx| {
            app.show_compare_window(ctx);
        })
        .drop_without_applying_deltas();

    assert!(app.operations.active().is_none());
    let id = egui::Id::new(("compare-window", app.project_scope()));
    assert!(context.memory(|memory| memory.area_rect(id).is_some()));
}

#[test]
fn compare_selection_focuses_the_matching_parameter_without_duplicate_tables() {
    let mut app = TunerApp::headless();
    app.open_compare_with_test_documents();

    assert!(app.focus_compare_semantic("table:uid:map"));
    assert!(app.focus_compare_semantic("table:uid:map"));
    assert_eq!(app.compare.selected_semantic_id.as_deref(), Some("table:uid:map"));
    assert_eq!(app.open_tables.len(), 1);
}
```

- [x] **Step 2: Run the headless UI tests and confirm RED**

Run:

```powershell
cargo test -p tuner-app --lib compare_window_renders_without_starting_document_work -- --exact
```

Expected: compile failure because compare window state and rendering do not exist.

- [x] **Step 3: Implement the minimum compare window**

Add the source/destination header, close/reload controls, missing-file status, filter, changed-only toggle, mode selector, map list, and selected-map table. Keep source values read-only. Render destination/source/delta/percent values using the existing precision and color settings; do not introduce a second selection model. Place the window in the existing frame update after table/surface windows and persist its rectangle when it changes.

- [x] **Step 4: Implement difference filtering and selection synchronization**

Build summaries from semantic IDs, apply category/title/type/dimension sort through existing catalog helpers, and make map-list selection focus the matching table without resetting an existing cell selection. Table clicks update the compare semantic ID and range; compare-only selection must not mutate destination BIN bytes.

- [x] **Step 5: Run varied UI checks**

Run:

```powershell
cargo test -p tuner-app --lib compare_window_renders_without_starting_document_work -- --exact
cargo test -p tuner-app --lib compare_selection_focuses_the_matching_parameter_without_duplicate_tables -- --exact
cargo test -p tuner-app --lib table_pointer_press_requires_focus_raise_even_without_child_click -- --exact
```

Expected: compare rendering/selection and existing floating-window focus behavior pass.

### Task 5: Add 2D and 3D graph comparison

**Files:**

- Modify: `crates/tuner-app/src/surface.rs` range helpers only where reusable for zero-centered delta ranges are needed
- Modify: `crates/tuner-app/src/compare.rs` mode-specific surface conversion and range helpers
- Modify: `crates/tuner-app/src/lib.rs` compare graph controls, point/marquee handling, surface memory persistence, and focus ordering
- Test: `crates/tuner-app/src/compare.rs`, `crates/tuner-app/src/surface.rs`, and `crates/tuner-app/src/lib.rs`

**Interfaces:**

- Produce `CompareMapData::surface_data(mode) -> SurfaceData` with real axis values and `None` holes preserved.
- Produce `CompareMapData::range(mode, auto_range) -> Option<SurfaceRange>`; delta modes use a symmetric zero-centered range, source/destination modes use existing padded auto-range.
- Produce `TunerApp::show_compare_surface(context, semantic_id)` using `SurfaceViewMemory`, point selection, rectangular selection, axis labels, auto-range, zoom, and existing focus rules.

- [x] **Step 1: Write failing graph-data tests**

```rust
#[test]
fn compare_surface_data_uses_delta_values_and_preserves_holes() {
    let map = CompareMapData::new(
        "table:map".to_string(),
        2,
        2,
        vec![0.0, 1.0],
        vec![0.0, 1.0],
        vec![
            CompareCell::from_values(Some(1.0), Some(3.0)),
            CompareCell::from_values(Some(2.0), None),
            CompareCell::from_values(Some(4.0), Some(1.0)),
            CompareCell::from_values(Some(5.0), Some(5.0)),
        ],
    );
    let surface = map.surface_data(CompareValueMode::AbsoluteDelta);

    assert_eq!(surface.value(0, 0), Some(2.0));
    assert_eq!(surface.value(0, 1), None);
    assert_eq!(surface.value(1, 0), Some(-3.0));
}

#[test]
fn compare_delta_graph_range_is_symmetric_around_zero() {
    let map = compare_map_with_values(&[-2.0, 4.0, 1.0]);
    let range = map.range(CompareValueMode::AbsoluteDelta, true).unwrap();

    assert!((range.minimum + range.maximum).abs() < f64::EPSILON);
    assert!(range.minimum < -4.0);
    assert!(range.maximum > 4.0);
}
```

- [x] **Step 2: Run the graph-data tests and confirm RED**

Run:

```powershell
cargo test -p tuner-app --lib compare_surface_data_uses_delta_values_and_preserves_holes -- --exact
```

Expected: compile failure because compare-to-surface conversion and mode-specific ranges do not exist.

- [x] **Step 3: Implement the 2D compare coloring path**

Feed `CompareCell::value(mode)` into the existing table color-range helpers. Use a zero-centered range for absolute/percentage delta and preserve the configured palette/customization. Ensure cells with `None` values use the existing unavailable-cell appearance and do not receive a misleading zero color.

- [x] **Step 4: Implement the 3D compare surface**

Convert compare map data to `SurfaceData`, reuse projection/axis/nearest-point/marquee helpers, and persist one `SurfaceViewMemory` per semantic ID under the project. Keep the compare graph in front when clicked; clicking a point updates compare selection but does not forcibly focus the ordinary table window. Make source, destination, delta, and percentage mode changes invalidate only the selected map’s comparison cache/range.

- [x] **Step 5: Test graph selection and rendering**

Run:

```powershell
cargo test -p tuner-app --lib compare_surface_data_uses_delta_values_and_preserves_holes -- --exact
cargo test -p tuner-app --lib compare_delta_graph_range_is_symmetric_around_zero -- --exact
cargo test -p tuner-app --lib surface_marker_click_selects_the_referenced_cell -- --exact
cargo test -p tuner-app --lib surface_primary_drag_selects_a_logical_cell_rectangle -- --exact
```

Expected: graph mode data, range behavior, point selection, and marquee selection pass without destination edits.

### Task 6: Add transfer dry-run, review, and atomic apply

**Files:**

- Modify: `crates/tuner-app/src/operations.rs` transfer-plan worker and payload
- Modify: `crates/tuner-app/src/lib.rs` transfer selection state, plan building, review UI, apply, Save As handoff, cache invalidation, and diagnostics
- Modify: `crates/tuner-app/src/compare.rs` transfer status mapping helpers
- Test: `crates/tuner-app/src/lib.rs` transfer integration tests

**Interfaces:**

- Produce `TunerApp::build_transfer_plan(selected: BTreeSet<String>) -> Result<(), WorkspaceError>`.
- Produce `TunerApp::approve_transfer_address(semantic_id: &str) -> Result<(), WorkspaceError>`.
- Produce `TunerApp::apply_transfer_plan() -> Result<(), WorkspaceError>`.
- Store the current `tuner_transfer::TransferPlan` only in memory; store approvals separately in `BTreeSet<String>`.
- Map `TransferEntryStatus` and `TransferIssueCode` to stable display labels and IPC strings without parsing human messages.

- [x] **Step 1: Write failing transfer integration tests**

```rust
#[test]
fn mismatched_compare_xdf_hash_blocks_transfer_plan() {
    let mut app = app_with_compare_documents_and_different_xdfs();

    app.build_transfer_plan(BTreeSet::from(["table:uid:map".to_string()]))
        .unwrap();

    let plan = app.compare.transfer_plan.as_ref().unwrap();
    assert!(plan.is_blocked());
    assert!(plan.issues.iter().any(|issue| {
        issue.code == tuner_transfer::TransferIssueCode::XdfHashMismatch
    }));
}

#[test]
fn approved_transfer_applies_as_one_destination_undo_entry() {
    let mut app = app_with_matching_compare_documents();
    let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();

    app.build_transfer_plan(BTreeSet::from(["table:uid:map".to_string()]))
        .unwrap();
    assert!(app.compare.transfer_plan.as_ref().unwrap().is_ready());
    app.apply_transfer_plan().unwrap();

    assert_ne!(app.workspace.bin.as_ref().unwrap().bytes(), before.as_slice());
    assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), 1);
    assert!(app.workspace.undo().is_ok());
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), before.as_slice());
}

#[test]
fn stale_transfer_plan_does_not_mutate_destination() {
    let mut app = app_with_matching_compare_documents();
    app.build_transfer_plan(BTreeSet::from(["table:uid:map".to_string()]))
        .unwrap();
    let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let destination_byte = before[0].wrapping_add(1);
    let mut destination_transaction = app.workspace.bin.as_mut().unwrap().transaction("test");
    destination_transaction.write_u8(0, destination_byte).unwrap();
    destination_transaction.commit().unwrap();

    let error = app.apply_transfer_plan().unwrap_err();
    assert!(error.message.contains("changed"));
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), before.as_slice());
}
```

- [x] **Step 2: Run the transfer tests and confirm RED**

Run:

```powershell
cargo test -p tuner-app --lib mismatched_compare_xdf_hash_blocks_transfer_plan -- --exact
```

Expected: compile failure because compare transfer state and methods do not exist.

- [x] **Step 3: Implement background plan construction**

Clone source/destination BINs and XDFs plus `TransferOptions` into `spawn_transfer_plan`. Build the plan without mutating either document. Install only the current plan request. Reset the plan whenever source/destination hashes, XDF paths, selection, or address approvals change.

- [x] **Step 4: Implement the transfer review panel**

Show plan status, selected count, changed bytes/ranges, each entry status, blocking issue code, and an explicit approval control for `AddressReviewRequired`. Keep Apply disabled while the plan is blocked or requires review. Make a no-change plan visibly valid but non-mutating.

- [x] **Step 5: Implement atomic apply and Save As handoff**

Call `TransferPlan::apply` with the read-only source document and the actual active destination `BinDocument`. Preserve the returned transaction as one undo entry. Invalidate search results, comparison map caches, validation, and the old transfer plan after a successful apply. Leave the existing Save As command responsible for publishing a new file and refusing existing targets.

- [x] **Step 6: Run varied transfer verification**

Run:

```powershell
cargo test -p tuner-app --lib mismatched_compare_xdf_hash_blocks_transfer_plan -- --exact
cargo test -p tuner-app --lib approved_transfer_applies_as_one_destination_undo_entry -- --exact
cargo test -p tuner-app --lib stale_transfer_plan_does_not_mutate_destination -- --exact
cargo test -p tuner-transfer --lib -- --nocapture
```

Expected: hash gates, address-review behavior, one-undo application, stale-plan safety, and existing transfer-engine tests pass.

### Task 7: Add command registry, NookLink IPC, and debug reporting

**Files:**

- Modify: `crates/tuner-app/src/ui_ipc.rs` request fields `changed_only: Option<bool>` and `selected_semantic_ids: Option<Vec<String>>`
- Modify: `crates/tuner-app/src/lib.rs` command descriptors, dispatch, capabilities, state snapshot, debug report, and direct-action handlers
- Modify: `docs/tuner-ui-ipc.md` supported actions and examples
- Modify: `README.md` desktop compare/graph/transfer workflow
- Test: `crates/tuner-app/src/lib.rs` UI IPC tests

**Interfaces:**

- Add direct IPC actions: `open_compare_bin`, `open_compare_xdf`, `close_compare`, `set_compare_mode`, `set_compare_filter`, `build_transfer_plan`, `approve_transfer_address`, and `apply_transfer_plan`.
- `set_compare_mode` accepts only `destination`, `source`, `delta`, and `percent_delta`.
- `build_transfer_plan` accepts `selected_semantic_ids` through the request structure and returns the current plan snapshot.
- State includes `compare` with paths, hashes, byte summary, mode, selected semantic/cell range, mapped summaries, graph state, plan status, and issues.
- Debug report includes source/destination identity, exact/normalized XDF hashes, current mode, map/cache state, plan status, and last compare operation.

- [x] **Step 1: Write failing IPC/capability tests**

```rust
#[test]
fn ui_ipc_compare_actions_are_discoverable_and_change_mode() {
    let mut app = app_with_matching_compare_documents();
    let capabilities = app.ui_ipc_capabilities();
    let actions = capabilities["actions"].as_array().unwrap();
    assert!(actions.iter().any(|value| value == "open_compare_bin"));
    assert!(actions.iter().any(|value| value == "build_transfer_plan"));

    let request = ui_ipc::UiIpcRequest::for_test("set_compare_mode", "", None, None);
    let response = app.handle_ui_ipc_request(&request);
    assert!(!response.ok, "missing mode must be rejected");
}

#[test]
fn ui_ipc_state_reports_compare_hashes_and_plan_status() {
    let app = app_with_matching_compare_documents();
    let state = app.ui_ipc_state();
    assert!(state["compare"]["source_bin_sha256"].is_string());
    assert!(state["compare"]["destination_bin_sha256"].is_string());
    assert_eq!(state["compare"]["plan_status"], "none");
}
```

- [x] **Step 2: Run the focused IPC test and confirm RED**

Run:

```powershell
cargo test -p tuner-app --lib ui_ipc_compare_actions_are_discoverable_and_change_mode -- --exact
```

Expected: compile failure or assertion failure because compare actions and state are not exposed.

- [x] **Step 3: Implement command and direct-action routing**

Route menu, toolbar, command palette, and IPC actions to the same `TunerApp` methods. Validate paths, operation-busy state, semantic IDs, modes, selected IDs, and plan status with structured errors. Return a state snapshot after every successful mutation.

- [x] **Step 4: Document the agent contract and debug fields**

Add JSONL examples to `docs/tuner-ui-ipc.md`, document the four mode strings and plan lifecycle, and update the README’s current-foundation/desktop-workflow sections. Do not document unsupported live ECU or checksum behavior as implemented.

- [x] **Step 5: Run IPC and debug verification**

Run:

```powershell
cargo test -p tuner-app --lib ui_ipc_compare_actions_are_discoverable_and_change_mode -- --exact
cargo test -p tuner-app --lib ui_ipc_state_reports_compare_hashes_and_plan_status -- --exact
cargo test -p tuner-app --lib app_debug_report_includes_search_and_operation_state -- --exact
```

Expected: compare actions are discoverable, state is machine-readable, and existing debug reporting remains green.

### Task 8: Full verification and release checkpoint

**Files:**

- Modify only files needed to correct verification failures; do not add unrelated refactors.
- Test: workspace test/build commands and native launch smoke check

- [x] **Step 1: Run formatting and focused feature tests**

```powershell
cargo fmt --all -- --check
cargo test -p tuner-app --lib compare -- --nocapture
cargo test -p tuner-app --lib transfer -- --nocapture
cargo test -p tuner-app --lib ui_ipc -- --nocapture
```

- [x] **Step 2: Run the complete workspace suite once**

```powershell
cargo test --workspace
```

Expected: all existing and new tests pass, including real-fixture tests. If a failure is found, add or correct a focused test before rerunning the affected scope; do not repeatedly rerun the same unchanged workflow.

- [x] **Step 3: Build the optimized application**

```powershell
cargo build --release -p tuner-app
```

Expected: `target\release\tuner-app.exe` exists and includes the compare/graph/transfer UI.

- [x] **Step 4: Perform one native launch smoke check**

Start the release executable from the repository root, wait briefly for the window to become responsive, confirm it can render with no documents loaded, and close only the process started by the check. Do not treat the smoke check as a substitute for the automated tests.

- [x] **Step 5: Update the handoff**

Report the implemented compare modes, graph semantics, transfer safety behavior, IPC actions, test counts, release executable path, and any known non-goals. Link the source, spec, and plan files with absolute workspace paths.
