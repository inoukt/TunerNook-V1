# 3D Surface Angle-Dependent Drawing Implementation Plan

> **For agentic workers:** Execute this bounded plan inline; the project has no Git metadata.

**Goal:** Correct surface face painter ordering at rotated viewing angles.

**Architecture:** Keep the current orthographic point projection. Split each grid cell into two triangles, calculate each triangle's mean depth in the same camera coordinate system, then paint in ascending back-to-front order. Normal and compare surfaces share this geometry path.

**Tech Stack:** Rust, egui, Cargo unit tests.

**Spec:** `docs/superpowers/specs/2026-09-27-surface-angle-depth.md`

## Global Constraints

- Rendering-only change; do not modify BIN/XDF contents or editing semantics.
- Add no dependency and preserve existing default-angle projection.
- Follow TDD and keep before/final source snapshots because the workspace has no `.git` directory.
- Bump the app patch version once and keep the stable settings-directory name unchanged.

## Review Focus

- At yaw 0 with a rising surface, painter order follows camera depth rather than screen Y.
- At yaw 0/pitch -25, a warped cell is emitted as valid triangles rather than a self-crossing convex polygon.
- Default yaw/pitch and surfaces with missing cells retain their current geometry behavior.
- Compare 3D uses the shared corrected geometry without a separate rendering implementation.
- The native title displays the package version while settings continue to use `APP_NAME`.

---

### Task 1: Sort faces by camera-space depth

**Files:**
- Modify: `crates/tuner-app/src/surface.rs`, `crates/tuner-app/src/lib.rs`
- Test: `crates/tuner-app/src/surface.rs`

**Interfaces:**
- Consumes: existing `SurfaceData`, `SurfaceViewMemory`, `SurfaceRange`, and `project_surface_geometry`.
- Produces: `SurfaceTriangle.depth` is each triangle's average camera-space depth; triangles remain sorted ascending, back-to-front and carry cell-border edge flags.

- [x] Write `surface_faces_sort_by_camera_depth_after_rotation` using a 2×3 map with values `[0.0, 0.5, 1.0]` on both rows, yaw `0°`, pitch `34°`, and range `0..1`; assert triangle order `(0,0,0), (0,1,0), (0,0,1), (0,1,1)`.
- [x] Run the focused depth test and confirm it fails because current order follows screen Y.
- [x] Write `surface_projection_splits_warped_cells_into_triangles` for values `[0,1,0,0]`, yaw `0°`, pitch `-25°`; assert one cell emits two triangle primitives.
- [x] Run the focused triangle test and confirm it fails because current projection emits one quad.
- [x] Split each complete cell into two triangles, compute each triangle's camera depth from the shared transform, and preserve only outer cell-edge strokes in both renderers.
- [x] Re-run the focused tests, then formatting, app tests, workspace tests, release build, and GUI smoke.
- [x] Save final snapshot and record verification/review in the SDD ledger.

### Task 2: Show the incremented app version in the native title

**Files:**
- Modify: `crates/tuner-app/Cargo.toml`, `crates/tuner-app/src/lib.rs`, `crates/tuner-app/src/main.rs`
- Test: `crates/tuner-app/src/tests.rs`

**Interfaces:**
- Consumes: package version from `CARGO_PKG_VERSION`.
- Produces: `APP_TITLE` as `TunerNook v<package-version>`; `APP_NAME` remains `TunerNook`.

- [x] Add `native_window_title_displays_app_version` and confirm it fails before `APP_TITLE` exists.
- [x] Bump `tuner-app` from `0.1.0` to `0.1.1`, set the native title to `APP_TITLE`, and preserve `APP_NAME`.
- [x] Run the title test and include it in the full app/workspace verification.
