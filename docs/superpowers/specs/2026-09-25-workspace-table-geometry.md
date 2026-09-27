# Workspace Table Geometry Preservation

## Goal

Switching named workspaces restores the target workspace's exact saved manual table geometry, even when egui already has a window with the same ID from the workspace being left.

## Behavior

- Manual table position, width, height, zoom, and `fit_to_content=false` are independent per workspace.
- Switching may change a table's rectangle to the target workspace's saved rectangle; it must not auto-fit or borrow the outgoing workspace's geometry.
- Switching back restores the first workspace's geometry exactly, subject only to the existing usable-canvas clamp.
- Switching remains presentation-only; BIN bytes, XDF identity, and data revision are unchanged.

## Constraints

- Reuse `WorkspaceViewState`, `TableWindowMemory`, `switch_workspace_snapshot`, and the existing egui table window ID.
- Do not add a second geometry store or change manual-size semantics.
- The workspace has no `.git`; use before/final snapshots and the SDD ledger.
- Do not repeat an identical focused test workflow more than 10 times.
