# Workspace Table Size Lifecycle

## Problem

Table windows can appear to lose a manually selected size after closing/reopening or switching saved workspaces. Zoom and window geometry are separate persisted values; preserving 100% zoom alone does not prove the saved rectangle is correct.

## Required behavior

- A table's live manually resized rectangle is captured into the active workspace before it is closed or switched away from.
- Table windows use the shared shell-safe work area, not the narrower central editor pane, so browser/inspector layout changes do not resize them. They may float over those side panels while remaining below the toolbar and above the dock/diagnostics.
- Restoring or reopening the table reapplies that workspace's exact saved rectangle, subject only to the shell-safe screen clamp.
- Geometry remains independently saved per workspace; equal saved sizes remain equal after switching, and one workspace's change does not overwrite another's.
- If a saved rectangle is clamped and the user explicitly resizes it, persist the resulting rectangle; passive clamping alone must not rewrite the saved target.
- BIN bytes, XDF identity, and data revision remain unchanged.

## Constraints

- Reuse the existing `TableWindowMemory`, workspace snapshot, and geometry-request paths.
- No second geometry store or global-size synchronization.
- Preserve manual-size behavior (`fit_to_content=false`).
- This workspace has no Git metadata; retain before/final snapshots and update its SDD ledger.
