# Raw Hex Editor Implementation Plan

**Goal:** Add a synchronized, undoable raw BIN Hex Editor to the desktop app.

**Architecture:** Put parsing, byte-selection, display, and window-memory types in
`crates/tuner-app/src/hex.rs`. Keep persistence, window drawing, byte writes,
commands, and NookLink integration in `crates/tuner-app/src/lib.rs`, reusing
`BinDocument::transaction` and the existing project preference flow.

**Spec:** [2026-09-22-raw-hex-editor-design.md](../specs/2026-09-22-raw-hex-editor-design.md)

## Tasks

1. Add `HexSelection`, typed display/endian modes, strict hex parsing, numeric
   decoding/search, range calculation, and ASCII formatting in `hex.rs`; re-export
   the types used by the app.
2. Add a checked `WorkspaceState` byte-write method. Reject empty/invalid ranges
   before opening a transaction, commit the accepted sequence under one label,
   then update the data revision. XDF mapping validation is range-only and the
   write does not change BIN length, so keep its existing report.
3. Add a project-scoped `HexEditorState` and floating window. Render virtualized
   rows as bytes or signed/unsigned integers/float32/float64, selectable values,
   ASCII, address jump, edit input, copy, paste, undo, and redo. Synchronize
   selection with byte ranges from the active XDF parameter in both directions.
4. Add Find Previous/Next for byte patterns or interpreted numeric values, and
   reuse `TableColorSettings`/`table_cell_fill` for automatic, fixed, or disabled
   coloring.
5. Add a `view.hex-editor` command and NookLink open, jump, select, write,
   display, coloring, search, and state actions. Persist window open state,
   geometry, address, display, color, and search settings with serde defaults,
   and update the README and IPC docs.
6. Format and compile the workspace and optimized app. Automated tests are
   omitted for this pass; the user can exercise byte edits and synchronization
   in the running app.

## Review focus

- Odd digit counts, invalid separators, optional `0x`, and empty input.
- Writes ending exactly at BIN length and writes exceeding BIN length.
- Empty or very large BINs and final partial rows.
- Packed or strided cell ranges when syncing byte selection.
- A project switch while the Hex Editor is open.
- Copy/paste focus when table and Hex Editor windows overlap.
