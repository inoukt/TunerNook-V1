# Table Interaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add desktop-style table focus, rectangular cell selection and clipboard editing, customizable value-range coloring, and a topmost debug report to TunerNook.

**Architecture:** Keep interaction state in `WorkspaceState` and expose public selection/copy/paste methods so UI and AI control share one contract. Persist coloring alongside existing `TableWindowMemory`, render the table's color ramp from pure helpers, and route egui pointer/keyboard/clipboard events through the same workspace methods. Raise table layers on pointer press and render the debug report in the foreground order.

**Tech Stack:** Rust 2021, egui/eframe 0.36.2, serde JSON, existing `tuner-core` BIN transactions and `tuner-xdf` conversion APIs.

**Spec:** `docs/superpowers/specs/2026-09-14-table-interaction-design.md`

## Global Constraints

- Preserve the existing `WorkspaceState` single-cell and axis editing contracts.
- Multi-cell paste must be one atomic BIN transaction and must not mutate on validation failure.
- New serialized settings must use serde defaults and sanitize malformed range endpoints.
- UI and agent-facing methods must use the same public selection model.
- No new runtime dependency is needed; use egui's existing clipboard events/output commands.
- Verify with `cargo fmt --all -- --check`, focused tests, `cargo test --workspace`, and `cargo build --workspace`.

---

### Task 1: Cell selection and clipboard model

**Files:**
- Modify: `crates/tuner-app/src/lib.rs:1295-1740` for `CellSelection`, workspace state, and atomic clipboard operations.
- Test: `crates/tuner-app/src/lib.rs` test module near the existing workspace editing tests.

**Interfaces:**
- Produces `pub struct CellSelection { pub anchor: (usize, usize), pub focus: (usize, usize) }` with `bounds()` and `contains(row, column)` helpers.
- Produces `WorkspaceState::selected_cells: Option<CellSelection>`.
- Produces `WorkspaceState::select_cell_range(anchor_row, anchor_column, focus_row, focus_column)`, `select_all_cells()`, `selected_cell_range()`, `copy_selected_cells(display, decimal_places)`, and `paste_cells(text)`.
- `paste_cells` returns the number of written cells and uses `RawValue` tokens for raw clipboard values or engineering numbers for ordinary numeric tokens.

- [ ] **Step 1: Write failing range-selection tests.**

```rust
#[test]
fn cell_range_selection_normalizes_drag_and_select_all() {
    let (xdf, bin) = column_major_fixture();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);

    workspace
        .select_cell_range(1, 2, 0, 1)
        .expect("drag range should be valid");
    let selection = workspace.selected_cell_range().unwrap();
    assert_eq!(selection.bounds(), (0, 1, 1, 2));
    assert!(selection.contains(0, 1));
    assert!(selection.contains(1, 2));
    assert!(!selection.contains(0, 0));

    workspace.select_all_cells().unwrap();
    assert_eq!(workspace.selected_cell_range().unwrap().bounds(), (0, 0, 1, 2));
}
```

- [ ] **Step 2: Run the focused test and verify the expected missing-API failure.**

Run: `cargo test -p tuner-app cell_range_selection_normalizes_drag_and_select_all -- --exact`

Expected: compile failure because `CellSelection`/range methods do not exist yet.

- [ ] **Step 3: Implement the smallest normalized range model and selection methods.**

Validate both corners against the selected parameter dimensions, retain the requested anchor/focus for drag direction, expose normalized inclusive bounds, and make `select_cell` delegate to a one-cell range. Clear cell selection when an axis is selected and restore a one-cell selection when a parameter is selected.

- [ ] **Step 4: Run the focused test and verify it passes.**

Run: `cargo test -p tuner-app cell_range_selection_normalizes_drag_and_select_all -- --exact`

Expected: PASS.

- [ ] **Step 5: Write failing clipboard and atomic-paste tests.**

```rust
#[test]
fn selected_cells_copy_as_displayed_grid_and_paste_as_one_edit() {
    let (xdf, bin) = column_major_fixture();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);
    workspace.select_cell_range(0, 1, 1, 2).unwrap();

    assert_eq!(
        workspace.copy_selected_cells(TableDisplay::Engineering, 2).unwrap(),
        "30.00\t50.00\n40.00\t60.00"
    );
    assert_eq!(workspace.paste_cells("101\t102\n103\t104").unwrap(), 4);
    assert_eq!(workspace.bin.as_ref().unwrap().bytes(), &[10, 20, 101, 103, 102, 104]);
    assert_eq!(workspace.bin.as_ref().unwrap().undo_depth(), 1);
}

#[test]
fn invalid_multicell_paste_leaves_bin_and_undo_history_unchanged() {
    let (xdf, bin) = column_major_fixture();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);
    workspace.select_cell_range(0, 1, 1, 2).unwrap();
    let before = workspace.bin.as_ref().unwrap().bytes().to_vec();

    assert!(workspace.paste_cells("101\t102\n103").is_err());
    assert_eq!(workspace.bin.as_ref().unwrap().bytes(), before.as_slice());
    assert_eq!(workspace.bin.as_ref().unwrap().undo_depth(), 0);
}
```

- [ ] **Step 6: Run the clipboard tests and verify they fail for the missing implementation.**

Run: `cargo test -p tuner-app selected_cells_copy_as_displayed_grid_and_paste_as_one_edit -- --exact` and `cargo test -p tuner-app invalid_multicell_paste_leaves_bin_and_undo_history_unchanged -- --exact`

Expected: compile failure because copy/paste methods do not exist yet.

- [ ] **Step 7: Implement displayed copy, raw-token parsing, and atomic paste.**

Format every selected row with tabs and no trailing newline. Use the requested display/precision, falling back to raw text when engineering conversion is unavailable. Parse rectangular input before opening a transaction; write each cell through `write_raw_cell` or `write_engineering_cell`, abort on the first failure, commit once, recompute validation, update the selection to the pasted rectangle, and report a precise error through the existing status path.

- [ ] **Step 8: Run focused and existing workspace tests.**

Run: `cargo test -p tuner-app selected_cells_copy_as_displayed_grid_and_paste_as_one_edit -- --exact`, `cargo test -p tuner-app invalid_multicell_paste_leaves_bin_and_undo_history_unchanged -- --exact`, then `cargo test -p tuner-app`.

Expected: PASS with no regression failures.

### Task 2: Table window focus, drag selection, and keyboard clipboard routing

**Files:**
- Modify: `crates/tuner-app/src/lib.rs:2417-2515` for transient drag state and initialization.
- Modify: `crates/tuner-app/src/lib.rs:4521-4935` for table interaction and layer raising.
- Test: `crates/tuner-app/src/lib.rs` egui/headless interaction tests.

**Interfaces:**
- Consumes the public `WorkspaceState` selection/copy/paste API from Task 1.
- Produces pointer-press focus behavior, drag-to-range behavior, Ctrl+A, Ctrl+C, and `Event::Paste` handling.

- [ ] **Step 1: Write failing pure interaction tests.**

```rust
#[test]
fn table_pointer_press_requires_focus_raise_even_without_child_click() {
    assert!(table_pointer_should_raise(true, true));
    assert!(!table_pointer_should_raise(false, true));
    assert!(!table_pointer_should_raise(true, false));
}

#[test]
fn table_cell_button_selection_uses_every_cell_in_drag_bounds() {
    let selection = CellSelection::new((1, 2), (3, 4));
    assert!(selection.contains(2, 3));
    assert!(!selection.contains(0, 3));
}
```

- [ ] **Step 2: Run the tests and verify the expected missing-helper failure.**

Run: `cargo test -p tuner-app table_pointer_press_requires_focus_raise_even_without_child_click -- --exact` and `cargo test -p tuner-app table_cell_button_selection_uses_every_cell_in_drag_bounds -- --exact`

Expected: compile failure because the interaction helper and range highlight path are not implemented.

- [ ] **Step 3: Implement interaction routing.**

Track the drag anchor and current cell by table key. On `primary_pressed` over a cell, start the anchor; while the primary button is down, update the current hovered cell and defer one `select_cell_range` request until after the window body. Route Ctrl+A only when the table area is hovered and no text editor owns keyboard input. Route Ctrl+C through `context.copy_text` and Ctrl+V through egui `Event::Paste` only when the table is hovered and no text editor owns input. Raise the window whenever its area response is hovered during a pointer press and call `context.move_to_top` immediately.

- [ ] **Step 4: Render the range highlight and status feedback.**

Use `selected_cell_range().contains(row, column)` for cell selection styling, retain the primary selected cell for the existing editor, and report copied/pasted cell counts in `WorkspaceState.status` without mutating the BIN on copy.

- [ ] **Step 5: Run focused and full app tests.**

Run: `cargo test -p tuner-app`

Expected: PASS.

### Task 3: Persisted table coloring and value-range rendering

**Files:**
- Modify: `crates/tuner-app/src/lib.rs:112-390` for color enums/settings, defaults, sanitization, and pure ramp helpers.
- Modify: `crates/tuner-app/src/lib.rs:4521-4935` for toolbar controls, legend, and cell fills.
- Modify: `crates/tuner-app/src/lib.rs:3600-3655` for diagnostics/report visibility of coloring state.
- Test: `crates/tuner-app/src/lib.rs` color and serialization tests.

**Interfaces:**
- Produces `TableColorMode`, `TableColorSettings`, `color_range_for_values`, and `table_cell_fill`.
- Extends `TableWindowMemory` with serde-default coloring settings.

- [ ] **Step 1: Write failing color and migration tests.**

```rust
#[test]
fn auto_color_ramp_is_monotonic_and_fixed_range_clamps() {
    let settings = TableColorSettings::default();
    let colors = color_ramp_samples(&settings, (0.0, 100.0), &[0.0, 50.0, 100.0]);
    assert_ne!(colors[0], colors[1]);
    assert_ne!(colors[1], colors[2]);

    let mut fixed = settings;
    fixed.mode = TableColorMode::FixedRange;
    fixed.fixed_min = Some(10.0);
    fixed.fixed_max = Some(90.0);
    assert_eq!(table_cell_fill(&fixed, 0.0, Some((0.0, 100.0))), table_cell_fill(&fixed, 10.0, Some((0.0, 100.0))));
    assert_eq!(table_cell_fill(&fixed, 100.0, Some((0.0, 100.0))), table_cell_fill(&fixed, 90.0, Some((0.0, 100.0))));
}

#[test]
fn old_table_memory_gets_color_defaults_and_sanitizes_invalid_range() {
    let mut memory: TableWindowMemory = serde_json::from_str(r#"{"width":800}"#).unwrap();
    assert_eq!(memory.coloring.mode, TableColorMode::AutoRange);
    memory.coloring.fixed_min = Some(f64::NAN);
    memory.coloring.fixed_max = Some(1.0);
    memory.sanitize();
    assert!(memory.coloring.fixed_min.is_none());
}
```

- [ ] **Step 2: Run the tests and verify the expected missing-type/helper failure.**

Run: `cargo test -p tuner-app auto_color_ramp_is_monotonic_and_fixed_range_clamps -- --exact` and `cargo test -p tuner-app old_table_memory_gets_color_defaults_and_sanitizes_invalid_range -- --exact`

Expected: compile failure because the coloring types/helpers do not exist yet.

- [ ] **Step 3: Implement serde-safe settings and pure interpolation.**

Store RGBA colors as `[u8; 4]`, use a dark-friendly thermal default, interpolate low→middle→high in two segments, return no fill for `Off` or unavailable values, fall back to an observed range for incomplete fixed settings, and make invalid/non-finite endpoints safe during sanitization. Change only the affected preference derives from `Eq` to `PartialEq` if f64 endpoints require it.

- [ ] **Step 4: Run color and serialization tests.**

Run: `cargo test -p tuner-app auto_color_ramp_is_monotonic_and_fixed_range_clamps -- --exact`, `cargo test -p tuner-app old_table_memory_gets_color_defaults_and_sanitizes_invalid_range -- --exact`, then `cargo test -p tuner-app`

Expected: PASS.

- [ ] **Step 5: Add the table UI controls and cell rendering.**

Add a `Colors` menu with Off/Auto range/Fixed range, preset colors, three color pickers, optional fixed min/max drag values, and a reset action. Compute the displayed numeric range once per table, show low/mid/high legend swatches, apply fills to cell buttons, and retain the selected-cell stroke/highlight. Persist changed `TableWindowMemory` through the existing table-memory path.

- [ ] **Step 6: Verify the table UI path and existing preferences tests.**

Run: `cargo test -p tuner-app`

Expected: PASS.

### Task 4: Foreground debug window and diagnostics coverage

**Files:**
- Modify: `crates/tuner-app/src/lib.rs:5914-5968` for stable ID, foreground order, and explicit raise-on-open/interact.
- Modify: `crates/tuner-app/src/lib.rs:1892-2090` and `3600-3655` to include selection/color state where useful in debug output.
- Test: `crates/tuner-app/src/lib.rs` debug-window and report tests.

**Interfaces:**
- Consumes existing debug-report command and table state.
- Produces a debug report window that is always above table layers while open.

- [ ] **Step 1: Write the failing foreground-order regression test.**

```rust
#[test]
fn debug_report_is_configured_as_foreground_window() {
    let context = egui::Context::default();
    let mut app = TunerApp::headless();
    app.debug_report_open = true;
    context.run(egui::RawInput::default(), |ctx| app.show_debug_report(ctx));
    let id = egui::Id::new(("debug-report", app.project_scope()));
    let rect = context.memory(|memory| memory.area_rect(id)).unwrap();
    assert_eq!(context.layer_id_at(rect.center()).map(|layer| layer.order), Some(egui::Order::Foreground));
}
```

- [ ] **Step 2: Run the regression test and verify it fails before the window is moved to foreground.**

Run: `cargo test -p tuner-app debug_report_is_configured_as_foreground_window -- --exact`

Expected: FAIL because the existing debug report uses the default middle layer.

- [ ] **Step 3: Configure and raise the debug report window.**

Give the window a project-scoped stable `Id`, set `.order(egui::Order::Foreground)`, and call `context.move_to_top` for that layer after showing it and whenever the report opens or is clicked.

- [ ] **Step 4: Run the focused report test and existing debug-report tests.**

Run: `cargo test -p tuner-app debug_report -- --nocapture`

Expected: PASS.

### Task 5: Full verification and handoff

**Files:**
- Modify: `.superpowers/sdd/2026-09-14-table-interaction/progress.md` with verified task evidence.
- Create: `.superpowers/sdd/2026-09-14-table-interaction/final-review-package.md` for independent review context.

- [ ] **Step 1: Run formatting and focused tests.**

Run: `cargo fmt --all -- --check` and `cargo test -p tuner-app`.

Expected: exit code 0 and all app tests passing.

- [ ] **Step 2: Run the complete workspace suite and build.**

Run: `cargo test --workspace` and `cargo build --workspace`.

Expected: exit code 0 for both.

- [ ] **Step 3: Inspect the changed files and run the existing NookLink/API smoke probe if available.**

Confirm no unrelated files were changed, and verify the public app state remains serializable/agent-readable. If native UI surfaces are available, perform a short smoke test of focus, drag, Ctrl+A, copy/paste, coloring menu, and debug foreground behavior; if not, record that limitation.

- [ ] **Step 4: Request independent code review and resolve all Critical/Important findings.**

Provide the reviewer the spec, plan, changed-file list, and fresh test output. Apply any necessary fixes through another failing-test cycle.

- [ ] **Step 5: Run fresh verification after review and report the actual status.**

Run the complete verification commands again and record their exit codes and test counts in the SDD progress file. Do not claim completion without this fresh evidence.
