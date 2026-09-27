# Render Efficiency and Test Module Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Keep implementation inline; no subagents.

**Goal:** Remove verified repeated work from compare/table/3D/axis repaint paths, then move the existing app test module out of `lib.rs` without changing behavior.

**Architecture:** Cache immutable compare identity and share immutable compare maps, avoid constructing destination cell views when a ready comparison fully supplies displayed data, and produce one projected vertex grid for paired 3D points/faces. Compile axis conversions once per header batch. After those independent behavior-preserving slices pass, extract the existing `#[cfg(test)]` module verbatim into `tests.rs`; broader production-module extraction remains separate.

**Tech Stack:** Rust 2021, Cargo workspace, `eframe`/`egui`; existing `Arc`, SHA-256 helper, conversion parser, and test infrastructure only. No dependencies.

**Spec:** `docs/superpowers/specs/2026-09-25-render-efficiency-design.md`

## Global Constraints

- Add no dependencies and do not change public IPC/protocol behavior.
- Preserve compare cache identity: source BIN SHA-256, destination data revision, source/destination XDF fingerprints, semantic ID, and compare mode remain the validity boundary.
- Cache only immutable compare-source identity and map data; invalidate or replace it whenever a new source BIN/map is installed.
- Preserve holes, row/column order, depth sorting, selection, and axis labels in 3D and table rendering.
- Keep the test-module move behavior-neutral; do not combine it with source edits to test logic.
- Do not claim a numeric speedup without a repeatable before/after measurement.
- Do not terminate a user-owned `tuner-app.exe` to unblock release output or smoke testing; use an isolated target directory and process-scoped GUI close when needed.

## Review Focus

1. **Compare BIN replacement and stale cache keys:** loading a different source must change the digest/key and must not reuse the old map. Pin in Task 1 with a source-replacement regression.
2. **Sparse, empty, zero-destination, or non-finite compare data:** range calculation must preserve current missing-value and percent-delta behavior. Pin in Task 1 with literal expected ranges and `None` cases.
3. **Destination, ready-compare, and pending-compare table states:** destination values must remain visible in destination mode; comparison values/placeholders and color range must remain unchanged in compare mode. Pin in Task 2.
4. **3D grids with holes, degenerate dimensions, and equal-depth faces:** points/faces must preserve cell identity, missing-cell holes, stable tie order, and the current degenerate-grid behavior. Pin in Task 3.
5. **Axis headers with identity, non-identity, malformed conversion, missing BIN/storage, and out-of-range indexes:** labels, raw values, engineering values, and errors must remain the same while parsing once per axis batch. Pin in Task 4.
6. **Private test access and accidental test edits during extraction:** all existing tests must still compile and execute, and the extracted module body must match the pre-move snapshot after removing only the module's four-space indentation level. Pin in Task 5.

---

### Task 1: Reuse compare identity and map data

**Files:**
- Modify: `crates/tuner-app/src/operations.rs` — carry the compare BIN digest produced when the background load completes.
- Modify: `crates/tuner-app/src/lib.rs` — retain the digest with the source BIN, use `Arc<CompareMapData>` in the map cache, and avoid per-frame deep clones.
- Modify: `crates/tuner-app/src/compare.rs` — stream values directly into the existing range reducers rather than collecting a temporary `Vec`.
- Test: `crates/tuner-app/src/compare.rs` and the existing app tests in `crates/tuner-app/src/lib.rs`.

**Interfaces:**
- `OperationPayload::CompareBin` carries `{ document: BinDocument, sha256: String }`; `OperationResult::compare_bin(id, subject, document)` computes the digest once before moving the document into the payload. Runtime calls already occur in the compare-load worker.
- `CompareWorkspaceState.source_bin_sha256: Option<String>` is assigned with the loaded source and cleared whenever that source is cleared. Directly constructed test states may use the existing digest helper as a fallback.
- `CompareWorkspaceState.map_cache` stores `Arc<CompareMapData>`; consumers clone the `Arc`, never the map vectors.

- [x] **Step 1: Add failing compare behavior tests.** Add a range test with finite values, a missing cell, a NaN, and a zero destination. Assert literal destination/source/absolute-delta ranges and `None` percent delta for zero destination. Add an app regression that installs source A, records a comparison key, installs source B, and asserts the key changes. Add a cache-sharing assertion that repeated table/surface retrieval returns the same `Arc` allocation.
- [x] **Step 2: Verify RED on the cache-contract tests and characterize range behavior.** Run `cargo test -p tuner-app compare_source_replacement_changes_cache_identity` and `cargo test -p tuner-app compare_cache_entries_share_immutable_map_data`; the first must reject the absent retained-digest field, and the second must reject the current owned-map cache representation. Run `cargo test -p tuner-app compare_map_range_preserves_sparse_and_non_finite_values` as a characterization; it is expected to pass before the refactor because the range result itself must stay identical.
- [x] **Step 3: Implement the minimal changes.** Compute the SHA-256 once in the compare-load result path; assign/clear it with `source_bin`; change map-cache values to `Arc`; wrap installed maps in `Arc::new`; preserve compare key fields; pass `self.values(mode)` directly to `symmetric_delta_range` or `auto_surface_range`.
- [x] **Step 4: Run focused tests.** Run `cargo test -p tuner-app compare_map_range_preserves_sparse_and_non_finite_values`, `cargo test -p tuner-app compare_source_replacement_changes_cache_identity`, `cargo test -p tuner-app compare_cache_entries_share_immutable_map_data`, `cargo test -p tuner-app compare_bin_result_carries_the_loaded_source_sha256`, and `cargo test -p tuner-app compare_map`.
- [x] **Step 5: Run the app suite once for this slice.** Run `cargo test -p tuner-app`; confirm all existing compare and transfer behavior remains green before continuing.

### Task 2: Skip unused destination cell reads in ready compare tables

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` — gate `WorkspaceState::cell_views_for` by parameter kind, selected compare mode, and whether the compare map supplies a valid color range; keep existing range fallback where it is observable.
- Test: app tests in `crates/tuner-app/src/lib.rs`.

**Interfaces:**
- Add a small pure predicate `table_needs_destination_views(kind, mode, compare_range_available) -> bool`; call it at the current `table_views` construction point. It returns true for destination mode and for a compare view whose range is unavailable, and false for non-table parameters or a ready compare range.

- [x] **Step 1: Add a failing predicate/render-state regression.** Assert destination mode still requests destination views; a ready source/delta map does not; a compare map with no range still requests destination views for the existing fallback; a non-table parameter does not build cell views. Add or extend the existing compare-table test to assert destination values are not substituted for pending compare placeholders.
- [x] **Step 2: Run the focused test and verify the expected failure.** Run `cargo test -p tuner-app ready_compare_table_skips_destination_views`; it must fail on the new assertion because the current table path always builds destination views.
- [x] **Step 3: Implement the predicate at the existing construction point.** Do not alter compare selection, visible compare cells, or `table_value_range` fallback when the predicate says views are needed.
- [x] **Step 4: Run the focused table tests.** Run `cargo test -p tuner-app ready_compare_table_skips_destination_views` and `cargo test -p tuner-app table_compare_mode`.

### Task 3: Project each 3D vertex once for points and faces

**Files:**
- Modify: `crates/tuner-app/src/surface.rs` — add one paired projection function and build faces/points from shared projected grid vertices; keep current convenience functions as wrappers for existing callers/tests.
- Modify: `crates/tuner-app/src/lib.rs` — use the paired result in normal and compare surface drawing, including drag-preview geometry; stream observed finite values into the existing color-range calculation instead of building a temporary `Vec`.
- Test: `crates/tuner-app/src/surface.rs` and the app tests in `crates/tuner-app/src/lib.rs`.

**Interfaces:**
- Add `project_surface_geometry(data, view, rect, range) -> (Vec<SurfacePoint>, Vec<SurfaceFace>)`. `project_surface` and `project_surface_points` delegate to it; the main render paths call it once and reuse both vectors.

- [x] **Step 1: Add failing geometry and range tests.** For a 3×4 fixture containing one missing cell, assert point `(row,column,value)` identities and expected face cells, assert every face corner equals the paired point position for its corresponding valid grid vertex, and assert face ordering uses depth then row then column. Add a finite-range regression asserting the streaming reducer returns `(-4.0, 10.0)` for `[-4.0, NaN, 10.0]` and `None` for empty input.
- [x] **Step 2: Verify RED before each implementation.** The geometry test's missing-API RED was observed before the paired projector was added. Add the range test before its helper and run `cargo test -p tuner-app surface_color_range_streams_finite_values`; it must fail because the streaming reducer is absent.
- [x] **Step 3: Implement a single projected-vertex grid and streaming range reduction.** Preserve current point finite-value filtering, face eligibility, projection math, stable tie-break ordering, and behavior for invalid dimensions. Reuse this result for both points and faces in standard, preview, and compare rendering. Keep the public slice-based `color_range_for_values` wrapper unchanged while the surface render path feeds its existing finite-value reducer directly from `SurfaceData.values`.
- [x] **Step 4: Run surface tests.** Run `cargo test -p tuner-app surface_`, `cargo test -p tuner-app surface_color_range_streams_finite_values`, and `cargo test -p tuner-app compare_surface`.

### Task 4: Compile axis conversions once per table-header batch

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` — add a shared axis-view helper that accepts an already parsed conversion; parse once for each X/Y header batch and preserve scalar callers.
- Test: app axis-view and table-header tests in `crates/tuner-app/src/lib.rs`.

**Interfaces:**
- Keep `WorkspaceState::axis_view_for(semantic_id, axis_index, index)` unchanged for callers. Add `WorkspaceState::axis_views_for(semantic_id, axis_index) -> Vec<Result<AxisView, WorkspaceError>>`; compile the conversion once per axis batch and reuse it for each axis entry.
- Use the already-read `RawValue` for engineering conversion, avoiding a second raw-axis read. Preserve current malformed-conversion and non-finite-value messages.

- [x] **Step 1: Add a failing batch-equivalence regression.** For literal XDF/BIN fixtures, compare every batched header `AxisView` with scalar `axis_view_for` for identity and non-identity conversions; also compare malformed conversion, missing BIN, and no-storage axis results. Assert a scalar out-of-range request still returns an error.
- [x] **Step 2: Run the focused test and verify it fails because batched axis views are not implemented.** Run `cargo test -p tuner-app axis_header_batch_matches_scalar_views`.
- [x] **Step 3: Implement the batch method and use it in both table header loops.** Keep labels, axis ranges, raw values, editability, and error text behavior unchanged. Do not introduce a persistent conversion cache.
- [x] **Step 4: Run axis tests.** Run `cargo test -p tuner-app axis_view` and `cargo test -p tuner-app axis_header_batch`.

### Task 5: Extract app tests from `lib.rs`

**Files:**
- Create: `crates/tuner-app/src/tests.rs` — the current test module contents, unchanged.
- Modify: `crates/tuner-app/src/lib.rs` — replace the inline test module with `#[cfg(test)] mod tests;`.
- Snapshot: `.superpowers/sdd/2026-09-25-render-efficiency-and-lib-split/snapshots/`.

**Interfaces:**
- The external child module continues to use `super::*`; all private parent items remain accessible. No production symbols or test logic are renamed or moved elsewhere.

- [x] **Step 1: Save the pre-move source snapshot and establish the baseline.** Copy `lib.rs` to the plan snapshot directory and run `cargo test -p tuner-app`; record the exact passing count in the ledger.
- [x] **Step 2: Move the module mechanically.** Extract the existing `#[cfg(test)] mod tests { ... }` block into `tests.rs` without editing its contents; replace it in `lib.rs` with `#[cfg(test)] mod tests;`.
- [x] **Step 3: Verify source preservation.** Compare `tests.rs` byte-for-byte with the corresponding pre-move module body after removing only its four-space inline-module indentation; fail the step on any remaining test-body difference.
- [x] **Step 4: Verify compilation and behavior.** Run `cargo test -p tuner-app`; it must pass with the same test count as the baseline. Run `cargo fmt --all -- --check`.

## Final Verification

- Run `cargo fmt --all -- --check`.
- Run `cargo test -p tuner-app` and `cargo test --workspace` once each.
- Run `cargo build --release -p tuner-app`; if a live app locks `target/release/tuner-app.exe`, build with `--target-dir target/verify` instead and leave the live process untouched.
- Run `bash scripts/smoke-test.sh` for the CLI/API checks, then run the release GUI smoke with a temporary APPDATA profile and process-scoped WM_CLOSE; together all 21 checks must pass and the fixture BIN must remain unchanged. Do not use the stock `--app` mode while another `tuner-app.exe` is running because it closes by image name.
- Review the full affected code and plan ledger; record measured claims only when an actual before/after measurement was collected.

## Deferred

- Any broad extraction of production functions or types from `crates/tuner-app/src/lib.rs`; revise the stale 2026-09-23 proposal and review it as a separate plan.
- Full-grid table virtualization or persistent per-cell caching until profiling identifies the dominant cost and a memory bound is selected.
- Removing dependencies or changing public IPC/protocol behavior.
