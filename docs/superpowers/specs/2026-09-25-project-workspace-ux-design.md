# Project, Workspace, and Status UX

## Goal

Make project/workspace state explicitly saveable and restorable without writing BIN bytes, while making the main workflow, search modes, and edit/diagnostic state understandable at narrow widths.

## Approved project-file design

- Add a versioned `.tnproj` JSON manifest. It stores references to the existing BIN path and optional XDF path, plus the complete BIN-scoped `ProjectPreferences`, including the active layout and all named workspace snapshots. It does not copy BIN/XDF content or contain dirty in-memory BIN bytes.
- Add **Open Project…**, **Save Project**, and **Save Project As…**. Opening validates the manifest/version and BIN identity, then uses the existing asynchronous document load, validation, and workspace-restore flow. Missing/invalid references must be reported before replacing the current documents; an unavailable optional XDF may be opened as BIN-only with an explicit warning.
- **Save Project** updates the associated manifest; if none exists it invokes **Save Project As…**. **Save Project As…** selects a new manifest path and associates it with the current BIN project. Both capture the live active workspace before serializing.
- The manifest's top-level optional `xdf_path` and `preferences.last_xdf_path` represent the same selected XDF; save/open normalize them to one value. The manifest path association is updated in project preferences only after a successful save, and opening a manifest associates it after the BIN identity is confirmed.
- Project save never writes or copies BIN bytes. If the loaded BIN has unsaved edits, report that the project/workspace was saved but those edits remain unsaved and require the existing BIN **Save As** command. The existing BIN no-overwrite behavior does not change.
- Persist the associated manifest path with the BIN-scoped project preferences so the File → Save Project command remains useful after restart. Theme, shortcuts, recent-file lists, and other installation-wide preferences are not part of the project manifest.

## Workspace window reset

- Add **Reset Workspace Windows** to View → Workspace (and the existing window Arrange menu where appropriate).
- It resets geometry in the active workspace only, including open and remembered-closed tables and the saved geometry of floating tools/surfaces. Other named workspace snapshots are untouched.
- Reset positions/sizes to the existing type defaults; use content-fit table sizing at each table's current zoom. Preserve which windows are open/minimized/focused, table zoom/precision/color settings, search/map-finder configuration, selections, filters, documents, and BIN edit history.
- Reapply geometry to currently open windows through existing geometry-request paths and persist the updated active workspace.

## Responsive main workflow

- Split the main toolbar into a wrapping action group and a separate wrapping status group. Keep File/Edit/View/Tools, Open BIN, Open XDF, project save, BIN Save As, Undo, and Redo reachable at narrow widths; retain existing command IDs and shortcut behavior.
- Replace the generic empty-canvas prompt with existing command-backed actions: no documents offers Open BIN and Open XDF; BIN-only explains that XDF is needed to browse definitions and offers Open XDF; XDF-only explains that BIN is needed for values/mapping validation and offers Open BIN; with both loaded and no table open, guide the user to choose a parameter.
- Label the browser field **List filter** (quick filtering of the displayed list). Label the floating search window **Advanced Search**. Explain Wildcard mode using the supported `*` (zero or more characters) and `?` (one character), without changing query semantics.
- Make the existing “BIN required for value search” notice easy to see whenever a query includes raw/engineering values but no BIN is loaded. Metadata-only search remains available and unchanged.

## Edit and diagnostics status

- Replace the ambiguous BIN “dirty” label with clear copy such as **Unsaved BIN edits · use Save As to export**. Clean BIN status remains concise. This is a status-only change; project save does not clear BIN dirty state.
- Keep healthy diagnostics content compact. When validation/mapping/XDF warnings exist, elevate the issue count and summary with warning/error styling and keep detailed messages reachable in the scrollable panel. Preserve the user's resizable diagnostics height and current no-overwrite/save semantics.

## Safety and compatibility

- Never write the source BIN or XDF as part of project save/reset. Project file paths refer to existing documents only.
- Opening a project must not silently apply settings to a different BIN identity.
- Existing `.tnook`-unrelated preferences and old `ProjectPreferences` JSON continue to deserialize through serde defaults/migrations; bump the project settings version only if a new persisted association field requires it.
- No new dependency is required.

## Verification

- Project manifest round-trip preserves BIN/XDF references, active workspace, saved workspace snapshots, window geometry, and project-scoped view settings. Invalid version/identity/path inputs do not replace the active project.
- Save Project with dirty BIN leaves BIN bytes, dirty state, and undo history unchanged; BIN Save As still creates a new file and refuses to overwrite an existing destination.
- Reset Workspace Windows resets active workspace geometry, including remembered closed windows, while preserving other snapshots, open/minimized state, view settings, selections, BIN bytes, XDF identity, and data revision.
- Headless UI tests cover narrow toolbar widths and empty-canvas states with no documents, BIN only, XDF only, and both documents.
- UI tests assert List filter/Advanced Search labels, wildcard hint, and value-search guidance with and without a BIN; matching/search behavior remains unchanged.
- Clean/edited BIN status and healthy/error diagnostics states are covered; the existing BIN Save As/no-overwrite test remains green.
