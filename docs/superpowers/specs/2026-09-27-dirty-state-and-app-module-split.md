# Dirty-state tracking and staged app module split

Date: 2026-09-27  
Status: Approved implementation scope

## Goal

Make BIN dirty-state queries constant time while preserving exact edit/undo
semantics, and begin a low-risk production split of the oversized app root.

## Design

`BinDocument` keeps an exact count of bytes that differ from its immutable
original image. New documents start at zero. A committed transaction adjusts
the count for each net byte transition against the original byte; undo and redo
apply the inverse/forward transition. Dropped or aborted transactions do not
change the count. Since the byte vector has no resize operation, this count is
the complete dirty predicate and the existing changed-byte-count result.

Move table color range, palette, cell-fill, and selection-sweep helpers into a
focused `table_colors` module. Keep `color_range_for_values` and
`table_cell_fill` available at their current crate-root paths, keep existing
render/test helper access, and preserve the implementation byte-for-byte
apart from imports and visibility needed for the move. This is the first
production extraction only; later modules need their own scoped slices.

Update the old module-split proposal and `AGENTS.md` so they reflect that tests
already live in `tests.rs`, current source boundaries differ from the 2026-09-23
line map, and production extraction is staged. Increment the app patch version
from 0.1.4 to 0.1.5 as required by project policy.

## Constraints

- Add no dependencies and do not change BIN/XDF bytes or public UI/IPC behavior.
- Dirty count remains exact across multiple writes per transaction, undo, redo,
  transaction abort/drop, and edits that return bytes to their original values.
- Preserve the root API paths for public table-color helpers.
- Keep the performance and source-movement changes in separate implementation
  tasks and snapshots.
- The workspace has no Git metadata; record exact test results and snapshots in
  `.superpowers/sdd/` instead of commits.

## Review focus

1. A transaction changes a byte then returns it to its starting value: no dirty
   count or history entry should be introduced.
2. An edit changes a byte and a later committed edit restores its original
   value: the document should become clean even though undo history remains.
3. Undo/redo across a mix of original and already-changed bytes must keep the
   exact count and dirty flag synchronized.
4. Aborted or dropped transactions must leave the count and data unchanged.
5. Moving coloring helpers must preserve root-level public paths and existing
   palette, range, sweep, and rendering behavior.
