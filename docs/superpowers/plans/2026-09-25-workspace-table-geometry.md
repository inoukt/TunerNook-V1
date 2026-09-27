# Workspace Table Geometry Implementation Plan

> **For agentic workers:** Implement inline, test-first, and record the task in `.superpowers/sdd/2026-09-25-workspace-table-geometry/progress.md`.

**Goal:** Make workspace switches restore each table's own saved manual rectangle.

**Architecture:** Exercise the existing workspace snapshot methods and draw the same table ID through a real headless egui context before and after switching. Change the shared restore path only if the regression shows that target geometry is not reapplied.

**Tech Stack:** Rust 2021, egui/eframe 0.36.2; no new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-25-workspace-table-geometry.md`

## Global Constraints

- Preserve per-workspace `TableWindowMemory` as the sole source of saved table geometry.
- Honor `fit_to_content=false`; do not auto-fit manual rectangles.
- Canvas clamping remains the only allowed geometry adjustment on restore.
- Workspace switching must not change BIN bytes, XDF identity, or data revision.
- No Git metadata exists; record snapshots and verification in the SDD folder.
- Do not repeat the focused test more than 10 times.

## Review Focus

1. A reused egui window ID must take the target workspace rectangle, not retain the outgoing rect.
2. Switching back and forth between different manual sizes must reproduce each saved rectangle.
3. A rectangle larger than the canvas must still be clamped by the existing geometry path.
4. Zoom and `fit_to_content=false` must remain associated with the target workspace.
5. Switching remains presentation-only and preserves document identity/revision.

---

### Task 1: Verify per-workspace manual table geometry

**Files:**
- Modify: `crates/tuner-app/src/tests.rs`
- Modify `crates/tuner-app/src/lib.rs` only if the regression fails.

**Interfaces:** Use `create_workspace_snapshot`, `switch_workspace_snapshot`, `show_table_window`, and `TableWindowMemory`.

- [x] Add a regression that opens one table, saves workspace A at 900×600 with manual sizing, creates workspace B at 640×400 with manual sizing, switches A→B→A using the same egui context/window ID, and asserts each rendered rectangle returns to its saved size and position.
- [x] Verify RED: the existing egui area kept position (100,80) when workspace B requested (180,120); its 640×400 size had already been applied.
- [x] Apply the target workspace's saved position with `fixed_pos` during the one-frame geometry request. The request clears immediately, so normal movability resumes on subsequent frames. No arrange/reset behavior was changed.
- [x] Run the focused regression and existing workspace/layout/table-geometry tests; assert BIN/XDF identity and data revision are unchanged.
