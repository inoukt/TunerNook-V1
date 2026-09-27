# Per-workspace canvas background image Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let each numeric workspace ID use a persistent canvas background image from global app settings, independent of the loaded BIN.

**Architecture:** Add a global `AppPreferences` map keyed only by workspace ID and keep original, content-addressed image files beside the main settings file. A single active-workspace texture cache paints behind floating windows; canvas-only context options update global settings without changing BIN/XDF data.

**Tech Stack:** Rust 2021, egui/eframe 0.36.2, serde/serde_json, the existing `image` crate with PNG/JPEG/BMP decoding, existing `rfd` picker, and standard filesystem APIs.

**Spec:** `docs/superpowers/specs/2026-09-25-workspace-background-image-design.md`

## Global Constraints

- Background settings are global `AppPreferences`, keyed only by numeric workspace ID; they are not stored in `ProjectPreferences`, project manifests, or `WorkspaceViewState`.
- Equal workspace IDs in different BIN projects intentionally share one global background entry.
- Copy original PNG/JPEG/BMP bytes without resizing or re-encoding into `workspace-backgrounds/` beside the main settings file; persist only a relative reference.
- When a source image exceeds `RawInput.max_texture_side`, downscale only the transient decoded GPU texture with aspect ratio preserved; never alter the managed original.
- Do not delete copied assets when a workspace reference is removed; an asset may be shared.
- The background paints only the central editor canvas, behind floating windows; existing table and cell context menus remain unchanged.
- The no-image path performs no image file access, decoding, or texture upload; cache at most the active workspace texture.
- Missing, unreadable, unsupported, or unsafe image references must not block workspace use; report the problem and permit replacement/removal.
- BIN/XDF content and data revision remain unchanged by all background operations.
- Before the first source edit, create the SDD ledger and exact before-snapshots for every existing source file that will change.
- No `.git` directory exists. Track work in `.superpowers/sdd/2026-09-25-workspace-background-image/progress.md` and before/final snapshots; do not invent commits.
- Do not test one workflow more than 10 times; use distinct focused tests and then broader verification.
- At each implementation-task boundary run `cargo fmt --all -- --check`, `cargo test -p tuner-app`, `cargo test --workspace`, and `cargo build --release -p tuner-app`. Run `bash scripts/smoke-test.sh --app` for final release smoke.

## Review Focus

1. A legacy settings file missing the new map defaults to no background and continues loading (Task 1 regression).
2. Same numeric workspace IDs across BINs share a setting, while different IDs remain independent and project manifests do not capture it (Task 3 regression).
3. Corrupt, missing, unsupported, absolute, or parent-traversing image references cannot escape the managed folder or prevent workspace use (Tasks 1–2 regressions).
4. A right-click on blank canvas opens background options without stealing table/cell input or replacing their context menus (Task 3 interaction regression).
5. No-image, 1080p, and 4K rendering stays bounded to one active texture, with no repeated decode/upload on unchanged frames, selection, or table movement (Tasks 2 and 4 cache/performance checks).

---

### Task 1: Global settings model and managed image assets

**Files:**
- Create: `crates/tuner-app/src/workspace_background.rs`
- Modify: `crates/tuner-app/src/lib.rs` (module declaration, `AppPreferences`, defaults, settings sanitization)
- Modify: `crates/tuner-app/Cargo.toml` (enable only PNG/JPEG/BMP image decoding)
- Test: `crates/tuner-app/src/workspace_background.rs` and app preference tests in `crates/tuner-app/src/tests.rs`
- Create: `.superpowers/sdd/2026-09-25-workspace-background-image/progress.md` and before-snapshots for changed source files

**Interfaces:**
- Produce `WorkspaceBackgroundPlacement::{Fit, Fill}` and serde-defaulted `WorkspaceBackgroundSettings { image: Option<String>, placement, opacity_percent: u8 }`.
- Produce `AppPreferences.workspace_backgrounds: BTreeMap<u64, WorkspaceBackgroundSettings>`; absent entries use the settings default.
- Produce `asset_directory(settings_path: &Path) -> PathBuf`, `validate_asset_reference(reference: &str) -> Result<(), String>`, `resolve_asset_path(settings_path: &Path, reference: &str) -> Result<PathBuf, String>`, and `copy_background_asset(source: &Path, settings_path: &Path) -> Result<String, String>`.
- `copy_background_asset` validates PNG/JPEG/BMP, copies source bytes unchanged to a content-addressed file, and returns only a safe relative filename. Reuse `tuner_core::sha256_hex`; do not create a second hash implementation or use BIN identity/path as input.

- [x] **Step 1: Write failing settings and asset tests**

Create the execution ledger and before-snapshots first. Then test serde round-trip for two workspace IDs, legacy settings defaulting to an empty map, opacity clamping, rejection of absolute/parent-traversing references, byte-identical asset copy, content deduplication, and rejection of unsupported/corrupt files.

- [x] **Step 2: Run the focused tests and observe the expected failure**

Run: `cargo test -p tuner-app workspace_background -- --nocapture`

Expected: the new background model, preference field, and asset helpers are missing.

- [x] **Step 3: Implement the model and app-managed asset helper**

Add the serde model and global preference map with backward-compatible defaults; increment `SETTINGS_VERSION` to 3. Clamp opacity to 0–100 and clear unsafe references during settings load. Derive the asset root from the main settings file’s parent; validate format by image signature, preserve original bytes, and use a content hash plus normalized extension for a non-clobbering asset name. If an occupied name has different/unreadable bytes, retain it and choose a collision-safe suffix so Replace can recover a damaged managed asset.

- [x] **Step 4: Run focused tests and the task verification battery**

Run the focused model/asset tests, then formatting, app tests, workspace tests, and optimized app build. Expected: legacy settings load and the original file bytes round-trip unchanged.

### Task 2: Single-texture canvas rendering and cache

**Files:**
- Modify: `crates/tuner-app/src/workspace_background.rs` (cache, decode, placement geometry, paint)
- Modify: `crates/tuner-app/src/lib.rs` (`TunerApp` cache field and canvas paint call)
- Test: `crates/tuner-app/src/workspace_background.rs`

**Interfaces:**
- Produce `WorkspaceBackgroundTextureCache::paint(&mut self, ui: &egui::Ui, settings_path: &Path, workspace_id: u64, settings: Option<&WorkspaceBackgroundSettings>, canvas: egui::Rect)`.
- `paint` returns `Option<String>` containing the cached load error, if any, so the canvas/context menu can explain an unavailable image.
- Cache key is `(workspace_id, relative image reference)`; hold at most one result for that key, including a cached failure. Opacity/placement changes reuse the texture.
- Paint one clipped image shape before floating table/surface windows. Fit contains and centers; Fill covers and center-crops via UVs.

- [x] **Step 1: Write failing render/cache tests**

Test Fit/Fill geometry, no-image zero decode/upload work, one decode/upload for repeated same-key frames, cache replacement on workspace/image change, and cached missing-file errors. Verify a 3840×2160 original uses a 2048×1152 texture when the renderer limit is 2048, remains 3840×2160 at a 4096 limit, and is preserved byte-for-byte. Generate 1080p and 4K test PNGs and assert subsequent unchanged frames emit no additional texture-set delta.

- [x] **Step 2: Run the focused cache tests and observe the expected failure**

Run: `cargo test -p tuner-app workspace_background -- --nocapture`

Expected: no texture cache or canvas image rendering exists.

- [x] **Step 3: Implement the active-only texture cache and painter**

Decode only after a cache-key change. Read `max_texture_side` from egui input; when required, downscale only the transient decoded image with aspect ratio preserved and a fast triangle filter. Convert pixels once to `ColorImage`, upload one texture, and discard the previous active result when the key changes or the setting has no image. Retain a failed result for the same key so errors do not trigger frame-by-frame I/O.

- [x] **Step 4: Integrate the painter before floating windows**

Call the cache from `show_workspace_canvas` before `show_table_window` and `show_surface_windows`; surface cached errors as a clear canvas message and keep drawing clipped to the central canvas.

- [x] **Step 5: Run focused render/cache tests and the task verification battery**

Run focused render/cache tests, then formatting, app tests, workspace tests, and optimized app build. Expected: the canvas image appears behind floating windows, and the no-image path performs no asset work.

### Task 3: Blank-canvas background menu and settings actions

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` (canvas context menu, file picker, global setting updates, errors)
- Modify: `crates/tuner-app/src/tests.rs` (headless pointer/menu and safety tests)

**Interfaces:**
- Add a canvas-only secondary-click response for empty editor canvas. Keep it below floating window layers and keep table/cell context-menu code unchanged.
- Add actions **Set background image…**, **Fit**, **Fill**, **Opacity**, and conditional **Remove background image**. These remain available with no BIN because they update global settings by workspace ID.
- On image selection, copy through `copy_background_asset`; update only `AppPreferences.workspace_backgrounds[active_workspace_id]` after a successful copy, then persist main settings. Asset errors keep the prior setting.
- Produce `TunerApp::set_workspace_background_image(&mut self, source: &Path) -> Result<(), WorkspaceError>` for file-picker and test reuse.

- [x] **Step 1: Write failing pointer-routing and settings-safety tests**

Test that secondary-clicking blank canvas opens the menu, the menu exposes the required actions, and a table/cell right-click still reaches its existing menu without opening workspace options. With no BIN, verify Set background image remains available and is saved for workspace ID 0. Test file selection through an injected path helper, replacement/removal after an image-load error, global settings persistence, identical workspace-ID behavior across BIN projects, `.tnproj` independence, and unchanged BIN/XDF bytes and data revision. Removing a project workspace must not clear the corresponding global ID entry. Selection and table movement must not increase the cache decode count.

- [x] **Step 2: Run focused UI tests and observe the expected failure**

Run: `cargo test -p tuner-app workspace_background_context_menu -- --nocapture`

Expected: no blank-canvas menu or global setting action exists.

- [x] **Step 3: Implement menu actions and image replacement/removal**

Use the existing `rfd::FileDialog` pattern with PNG/JPEG/BMP filters. Apply placement and opacity to the entry for the active numeric workspace ID and persist main settings. Save only through `save_preferences`; do not call `persist_preferences`, which also stages and saves BIN-scoped workspace state. When a load error is cached, display a clear unavailable message while keeping Replace and Remove available. Removing a reference never deletes the copied file.

- [x] **Step 4: Run focused UI/safety tests and the task verification battery**

Run the pointer-routing and file-operation tests, then formatting, app tests, workspace tests, and optimized app build. Expected: floating table/cell interactions and BIN/XDF state are unchanged.

### Task 4: Cross-project persistence and performance validation

**Files:**
- Modify: `AGENTS.md` (completed feature status and verification results)
- Update: `.superpowers/sdd/2026-09-25-workspace-background-image/progress.md` and final source snapshots

**Interfaces:**
- Use existing workspace switching and main settings serialization; do not duplicate settings in project snapshots or `.tnproj` manifests.
- The final verification record includes actual test counts and whether a modest/integrated GPU was available for the renderer check.

- [x] **Step 1: Verify cross-workspace persistence regressions**

Run the Task 1 settings round-trip and Task 3 switching/integration regressions. Verify distinct global backgrounds for IDs 1 and 2, ID 1 sharing across BIN projects, `.tnproj` independence, and that removing a per-project workspace does not erase a global ID entry.

- [x] **Step 2: Measure the no-image, 1080p, and 4K cache cases**

Run: `cargo test -p tuner-app workspace_background -- --nocapture`. Record first-load decode/texture-upload preparation timings for no image, 1080p, and 4K on the test renderer's 2048-side limit; verify unchanged frames, selection changes, and table movement do not cause repeated work.

- [x] **Step 3: Check available graphics hardware and attempt renderer timing**

Inspect available display adapters and attempt a release-renderer timing check on a modest/integrated GPU. Distinguish CPU/headless texture-delta measurements from actual GPU measurements; report if the requested GPU class is unavailable.

- [x] **Step 4: Run final verification and record the result**

Run: `cargo fmt --all -- --check`; `cargo test -p tuner-app`; `cargo test --workspace`; `cargo build --release -p tuner-app`; `bash scripts/smoke-test.sh --app`.

Expected: all tests and the release smoke pass; the smoke confirms graceful GUI close and no changes to the supplied fixture BIN. Record exact counts, GPU availability, timings, and any limitation in the SDD ledger and `AGENTS.md`.

- [x] **Step 5: Complete snapshots and self-review**

Capture final versions of every changed source/document file in the SDD final snapshot, compare against the before snapshot, and self-review menu routing, global-vs-project persistence, asset path safety, cache invalidation, and compatibility. No Git commit is possible in this workspace.
