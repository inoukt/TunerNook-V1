# 3D Surface Visualization Implementation Plan

> This workspace has no `.git` directory; use source and test checkpoints instead of commits. Implement inline after the approved design.

**Goal:** Add a dependency-free interactive 3D surface window for open XDF tables, with auto-range, saved camera/layout state, command-palette access, and AI-agent IPC control.

**Approach:** Put pure range/projection/data-shaping logic in `crates/tuner-app/src/surface.rs`. Reuse `WorkspaceState::cell_view_for`, `axis_view_for`, `table_axis_index`, `table_cell_fill`, existing `OpenTable` keys, egui windows, project persistence, commands, and IPC. Keep this slice UI-only; no new BIN/XDF editing path.

## Task 1: Add tested surface model and auto-range [x]

Files:

- Add `crates/tuner-app/src/surface.rs`.
- Modify `crates/tuner-app/src/lib.rs` to register/re-export the module.

Tests first:

- `surface_auto_range_adds_padding_and_handles_flat_values` proves finite min/max, 5% padding, and non-zero flat fallback.
- `surface_projection_is_deterministic_and_depth_sorted` proves the same camera/data produce stable projected points and back-to-front face order.
- `surface_data_preserves_missing_cells_as_holes` proves unavailable/non-finite cells do not become fake zeroes.

Implement only the smallest pure types required by the UI:

```rust
pub struct SurfaceData { pub rows: usize, pub columns: usize, pub x: Vec<f64>, pub y: Vec<f64>, pub values: Vec<Option<f64>> }
pub struct SurfaceRange { pub minimum: f64, pub maximum: f64 }
pub struct SurfaceViewMemory { /* serde-persisted camera and display settings */ }
pub struct SurfaceFace { pub row: usize, pub column: usize, pub points: [egui::Pos2; 4], pub value: f64, pub depth: f32 }
pub fn auto_surface_range(values: impl IntoIterator<Item = f64>) -> Option<SurfaceRange>;
pub fn project_surface(data: &SurfaceData, view: &SurfaceViewMemory, rect: egui::Rect, range: SurfaceRange) -> Vec<SurfaceFace>;
```

Range normalization must ignore non-finite values, pad non-flat ranges by 5%, and use `max(abs(value) * 0.05, 1.0)` for flat ranges. Projection uses normalized actual axis coordinates, normalized Z, yaw/pitch, pan, and zoom with finite clamping.

Run the three focused tests and confirm they fail before implementing the model, then pass after implementation.

## Task 2: Persist per-table surface state [x]

Files:

- Modify `crates/tuner-app/src/lib.rs` `ProjectPreferences`, sanitization, defaults, project restore/staging, and `TunerApp` fields.

Tests first:

- `surface_preferences_round_trip_and_sanitize_extreme_values` proves saved camera/layout survives JSON and invalid values are clamped.
- `surface_open_state_is_project_scoped_by_table_key` proves a different BIN/XDF fingerprint cannot reopen the old surface.

Add `surface_windows: BTreeMap<String, SurfaceViewMemory>` and `open_surface_keys: Vec<String>` to `ProjectPreferences` with serde defaults. Keep surface state project-scoped; do not add a global duplicate unless an existing compatibility path requires it. Add helpers to open, close, focus, reset, stage, and restore surfaces while filtering keys to currently available open tables.

Run the focused persistence tests and the existing project preference tests.

## Task 3: Build surface snapshots from real table/axis values [x]

Files:

- Modify `crates/tuner-app/src/lib.rs` around workspace/table helpers.

Tests first:

- `surface_snapshot_uses_actual_axes_and_engineering_values` uses the supplied fixture and asserts axis values and a known cell value appear in the snapshot.
- `surface_snapshot_falls_back_to_indexes_and_raw_values` covers missing axis storage/conversion failures.

Implement `TunerApp::surface_data_for(&self, semantic_id)`. Read every cell through `cell_view_for`; select finite engineering values first, then raw values. Resolve X/Y axis indices with `table_axis_index`; read finite engineering axis values, then raw values, then index positions. Preserve `None` for cells with neither finite representation. Compute and expose a finite-value diagnostic count for the UI.

Run focused snapshot tests and the full XDF fixture tests.

## Task 4: Add the movable interactive 3D window [x]

Files:

- Modify `crates/tuner-app/src/lib.rs` table controls, frame update, persistence hooks, and debug state.
- Use `crates/tuner-app/src/surface.rs` for projection/face drawing.

Tests first:

- `open_surface_focuses_existing_surface_without_duplicates` proves repeated open/focus requests do not duplicate windows.
- `surface_command_is_available_for_a_table_and_renders_headless` proves command availability and one egui frame with a selected table do not start an operation or panic.

Add a `3D` button to the active table window, render one `egui::Window` per open surface, and raise it when clicked. Use `allocate_painter` for the plot. Controls: `Auto range`, `Reset view`, `−/+` height, `Wireframe`, `Axes`, and a compact numeric range readout. Pointer drag orbits, secondary/middle drag pans, and scroll zooms; every changed state is clamped and persisted. Draw faces with existing table colors and a neutral fallback for missing values; draw grid/axes labels only when enabled.

Use a small `SurfaceInteraction` helper to keep pointer math out of the main window function. Make the default view auto-fit from the data range and table dimensions. Do not mutate BIN/XDF data.

Run focused UI tests, then the complete app library suite.

## Task 5: Add command registry and AI IPC control [x]

Files:

- Modify `crates/tuner-app/src/lib.rs` command descriptors, dispatch, capabilities, state, and debug report.
- Modify `crates/tuner-app/src/ui_ipc.rs` only if request fields are needed.
- Modify `docs/tuner-ui-ipc.md` with examples.

Tests first:

- `surface_commands_dispatch_through_shared_helpers` covers `view.3d-surface` and reset behavior.
- `ui_ipc_surface_actions_open_focus_reset_and_report_state` covers exact semantic-ID requests and returned state.

Add stable `view.3d-surface` command. Add `open_surface`, `focus_surface`, and `reset_surface_view` direct actions using exact `semantic_id` and the same TunerApp helpers as visible controls. Include all three actions in capabilities and `surface` state in `ui_ipc_state`/debug report. Reject unknown IDs and malformed requests with contextual errors.

Run IPC tests and the app suite.

## Task 6: Verification and handoff [x]

Run:

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo check --workspace
cargo build --workspace
```

Start the built native executable and use the authenticated local IPC endpoint to open a real fixture surface, inspect state, reset the view, and close the exact process. Confirm no BIN bytes or XDF source file changes. Document remaining roadmap items: scalar/function/flag/raw-hex editors, richer 2D charts, and optional GPU acceleration.

Verification completed 2026-09-14: `cargo fmt --all -- --check`, `cargo test --workspace`,
`cargo check --workspace`, and `cargo build --workspace` passed. The isolated native smoke test
also passed for the supplied fixture and left no TunerNook process or UI-IPC manifest running.
