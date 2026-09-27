# Table Interaction and Visual Range Design

**Date:** 2026-09-14  
**Status:** Approved in conversation

## Goal

Make table windows behave like predictable desktop work surfaces: clicking any visible part raises the table, cells support rectangular mouse and keyboard selection, clipboard actions operate on the selected rectangle, values can be visually color-ranged like established tuning tools, and the debug report always remains above the workspace.

## Behavior

1. A pointer press inside a table window raises it immediately and makes it the active table. This includes the title bar, blank content, controls, and grid; the existing list-click behavior remains unchanged.
2. A cell click selects one cell. A primary-button drag selects the inclusive rectangle between the press and release cells. `Ctrl+A` selects every cell in the active table. Axis header selection remains separate and clears the cell range.
3. `Ctrl+C` copies the selected rectangle as tab-delimited columns and newline-delimited rows using the current table display and precision. `Ctrl+V` pastes the clipboard rectangle from the selected range's top-left cell. Pasting validates the complete shape and every conversion before committing one BIN transaction, so failure never partially edits the BIN and success creates one undo entry.
4. The selection state is public and deterministic: UI events and AI-facing methods use the same normalized `CellSelection`, copy, and paste operations.
5. Each table has persisted coloring settings: `Off`, `AutoRange`, or `FixedRange`; low/middle/high colors; and optional fixed minimum/maximum. Auto range uses the table's displayed numeric values, falling back to raw numeric values where engineering conversion is unavailable. Fixed range clamps outliers. A legend and color controls live in the table toolbar, and selection outlines remain visible over the fill.
6. New table windows default to `AutoRange`; existing saved table settings migrate through serde defaults. Coloring changes persist with the table window/project settings.
7. The debug report uses `egui::Order::Foreground`, gets an explicit stable ID, and is moved to the top when opened or interacted with. It stays above table windows until closed.

## Completed follow-up

Cell actions are exposed through the toolbar, table-title `Cells` menu, and a
pointer-anchored right-click menu. All three paths dispatch the same stable
`edit.*` command IDs. The local `tuner-ui/v1` JSONL bridge also exposes state,
debug snapshots, direct file/table actions, and the same command dispatcher to
development tools and AI agents.

## Error handling and compatibility

- Empty, malformed, non-rectangular, out-of-bounds, or non-convertible clipboard input reports a workspace error without changing the BIN.
- Existing single-cell editor, axis editing, raw/engineering display, geometry persistence, and undo/redo remain compatible.
- Newly added serialized fields have defaults and sanitization; persisted range endpoints must be finite and ordered.

## Testing

- Unit tests cover normalized ranges, select-all dimensions, displayed/raw clipboard formatting, atomic paste success/failure, and color interpolation including fixed-range clamping and missing-value fallback.
- Headless egui tests cover table-window focus/raise behavior, drag-selection requests, Ctrl+A, clipboard event handling, and foreground debug report ordering.
- Run the focused tests, the complete workspace test suite, formatting check, and workspace build.
