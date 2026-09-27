# Window Opening and Restore Defaults Implementation Plan

> **For agentic workers:** Execute inline; this project has no Git metadata. Steps use checkbox syntax.

**Goal:** Apply a user-selected zoom to first-open tables, cascade fresh windows inside the editor canvas, and restore Compare geometry once per workspace switch.

**Architecture:** Reuse existing typed memories and utility geometry records. Add a default-zoom app preference, a small pure cascade-rectangle helper, and a saved-position marker on typed window memories so only fresh geometry is placed. Compare gets a one-shot geometry request when reopened/restored; all windows return to normal egui dragging after that frame.

**Tech Stack:** Rust, egui/eframe, serde, Cargo tests.

**Spec:** `docs/superpowers/specs/2026-09-27-window-open-defaults.md`

## Global Constraints

- New table zoom defaults to 100%, steps by 10%, and clamps to 50–200%.
- A saved per-table zoom/rectangle takes precedence over the app default.
- Fresh windows start within the central editor canvas; saved positions remain unchanged.
- App name remains `TunerNook`; app patch version advances to `0.1.2`.
- Do not modify BIN/XDF bytes or add dependencies.

## Review Focus

- Older table/typed-window memories still preserve their saved non-default positions.
- A genuinely new table uses the current default zoom, while a reopened table keeps its own zoom.
- Several fresh windows opened in one frame receive distinct, bounded cascade positions.
- Switching to a workspace with Compare open overrides egui's retained rectangle only once.
- Narrow/hidden browser panels and small canvases do not place a fresh window outside the editor canvas.

---

### Task 1: Default zoom for new tables

**Files:**
- Modify: `crates/tuner-app/src/lib.rs`, `crates/tuner-app/src/tests.rs`
- Test: `crates/tuner-app/src/tests.rs`

**Interfaces:**
- Consumes: existing `AppPreferences`, `default_table_window_memory`, and `open_table_internal`.
- Produces: `AppPreferences.default_table_zoom_percent`; first-open table memory uses it only when no saved/recent table memory exists.

- [x] Add failing tests for the 100% default, preference round-trip/clamping, toolbar minus/plus, new-table zoom, and saved-table zoom precedence.
- [x] Run the focused zoom tests and confirm the current hard-coded 100% behavior fails.
- [x] Add the preference, expose toolbar controls, and route only the no-memory table fallback through it; bump app settings schema to 6.
- [x] Run the focused tests and verify saved per-table memory remains unchanged.

### Task 2: Fresh-window cascade and Compare restore

**Files:**
- Modify: `crates/tuner-app/src/window_geometry.rs`, `crates/tuner-app/src/lib.rs`, `crates/tuner-app/src/hex.rs`, `crates/tuner-app/src/map_search.rs`, `crates/tuner-app/src/search.rs`, `crates/tuner-app/src/compare.rs`, `crates/tuner-app/src/surface.rs`, `crates/tuner-app/src/tests.rs`
- Test: `crates/tuner-app/src/window_geometry.rs`, `crates/tuner-app/src/tests.rs`

**Interfaces:**
- Consumes: current central-canvas rect, typed window memories, utility geometry map, and `WindowId`/dock entries.
- Produces: `window_geometry::cascade_rect(bounds, size, index)` and one-shot position restoration for fresh/saved Compare geometry.

- [x] Add failing tests for bounded cascade rectangles, distinct first-open window offsets, saved-position precedence, legacy geometry migration, and Compare restore after workspace switching.
- [x] Run the focused geometry/window tests and confirm the current fresh defaults can start over the browser and Compare retains egui's stale rect.
- [x] Add the cascade helper and backward-compatible saved-position metadata; apply it only when a typed memory lacks a captured position. Reuse utility geometry maps for utility windows.
- [x] Add Compare's one-shot fixed-geometry request for reopen/workspace restore, then clear it after rendering so future drags/resizes are unrestricted.
- [x] Run focused tests and verify old non-default positions and existing manual table resize behavior.

### Task 3: Release identity, documentation, and full verification

**Files:**
- Modify: `crates/tuner-app/Cargo.toml`, `crates/tuner-app/src/main.rs`, `crates/tuner-app/src/tests.rs`, `Cargo.lock`, `AGENTS.md`

- [x] Confirm the versioned native title test fails for the new expected version.
- [x] Bump `tuner-app` to `0.1.2`, preserve `APP_NAME`, and record settings/schema versions and behavior in `AGENTS.md`.
- [x] Run `cargo fmt --all -- --check`, `cargo test -p tuner-app`, `cargo test --workspace`, `cargo build --release -p tuner-app`, and `scripts/smoke-test.sh --app`.
- [x] Save final snapshots and complete a self-review; record any limitations caused by the no-Git workspace.
