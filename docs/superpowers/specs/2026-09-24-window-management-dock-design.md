# Screen-safe window dock and focus model

Status: Approved for implementation  
Date: 2026-09-24  
Project: TunerNook

## Context

TunerNook already uses movable in-app egui windows for tables and tools. Table
tabs currently sit at the top of the editor canvas, while each tool owns its
own open/focus state. Some windows constrain themselves to the current
workspace; others do not. The native app also requests a fixed 1400×900 inner
size with a 960×640 minimum, which can exceed the usable display area after
Windows scaling or on a smaller monitor.

The user approved one compact bottom dock for open tables and tools, with
focus/minimize behavior and screen-safe placement. “Multiple workspaces” means
named desktop-layout snapshots for the same loaded BIN/XDF environment—not
independent documents or editing sessions. Profiles must restore the user's
window arrangement and view settings after closing and reopening the app.

## Goals

1. Keep the native app and every persistent in-app window reachable within the
   current display/workspace bounds, including after restoring old geometry or
   moving to a smaller display.
2. Replace the top table-tab strip with a slim bottom dock listing open table
   and tool windows. Keep the dock compact and horizontally scrollable.
3. Make each dock item restore and focus its window; clicking the already
   focused item minimizes it without closing or losing its state.
4. Keep closing a window distinct from minimizing it. Existing title-bar close
   behavior closes/removes it; closed tools continue to launch through current
   menus and commands.
5. Preserve current BIN/XDF data safety and existing table geometry memory.
6. Let the user save, switch, rename, and remove named window layouts within
   the active BIN project, and restore the selected snapshot after restart.
7. Keep the added window state inspectable and controllable through NookLink,
   without allowing an agent to dismiss user-consent or approval prompts.

## Proposed design

### Dock and window lifecycle

Add one bottom shell panel, placed above Diagnostics, and move the existing
table tabs into it rather than rendering duplicate table navigation. A
horizontal scroll area handles large numbers of entries. Each open table,
open surface, and persistent tool window gets one stable target ID, a concise
label, a tooltip with its full title, and a visual focused/minimized state.

The dock includes open tables and surfaces, Search, Map Finder, Hex Editor,
Compare BIN/graph, XDF Maker / Editor, Action History, Debug Report, Settings,
and NookLink when those windows are open. Closed tools remain available from
their existing launch paths. Modal prompts, file dialogs, operation overlays,
and the transient command palette are not dock entries.

- Clicking a non-focused or minimized item restores and raises it.
- Clicking the focused item toggles it to minimized; its dock entry remains.
- Closing through the window's close control removes it from the dock and uses
  the existing close semantics for that window type.
- If the focused window is minimized or closed, focus moves to the most
  recently focused remaining window, or back to the workspace canvas.
- Open/minimized and active-window state is saved in the active layout profile.

The manager should derive open/closed status from existing app state where
possible and add only the minimum shared focus/minimize state needed. It must
not duplicate document ownership or editing state. Table selection and
editing continue to route through the current `WorkspaceState` and existing
transactions.

### Workspace snapshots

Add a workspace selector to the bottom dock, reusing the active per-BIN
`ProjectPreferences` as the canonical working layout. It already persists
table geometry/zoom/scroll/coloring, tab order and open/active table keys;
surface geometry/camera and open/active keys; Search state; Compare settings
and geometry; Hex state; Map Finder settings; and browser/panel arrangement.
Do not add a second live window-state store or reimplement these persistence
paths.

Add active workspace identity/name and saved *inactive* snapshots. In the
running app, existing `ProjectPreferences` fields remain the single canonical
state for the active layout; do not also store a duplicate active snapshot.
Each inactive snapshot uses existing serializable memory types for the layout
subset: open/minimized window IDs, active/focused ID, tab order, table/tool
view settings, and per-window geometry. Map existing fields into and out of
these snapshots instead of replacing their persistence or renderer paths.
Add only current selection, dock focus/minimized details, and tool-open state
that have no existing persisted equivalent. For utility windows without an
existing per-project geometry memory (Action History, Debug Report, Settings,
NookLink, and the XDF editor), use one compact stable-ID-keyed rectangle map;
do not duplicate table, Search, Hex, Map Finder, Compare, or 3D-surface
geometry. For windows already represented
by existing fields, derive open state from those fields. Switching captures
the current canonical fields into the outgoing snapshot, applies the target
snapshot back through the existing project-preference apply/restore path, and
persists the same project file.

The first migration labels the existing live settings `Default`; it does not
copy them into a second record. Creating a snapshot stores the current layout
under its old name and activates a duplicate of that layout under the new
name. Rename works for active or inactive snapshots. Delete is refused for
the last layout; deleting the active one switches to a remaining snapshot.
The active layout continues to auto-save through current behavior, including
after geometry interaction settles; profile changes save immediately using
the existing atomic project-preference writer.
Disable snapshot create/switch/rename/delete while BIN/XDF load, restore, or
save operations are active; window focus and minimize can still be used.

Persist the stable view settings already supported: table zoom, scroll,
precision/coloring; surface camera, zoom, range and display options; Hex
format/endian/search/color settings; Search query/mode/sort; Map Finder scan
settings; Compare mode/filter/source paths; utility-window size and position;
and panel/browser arrangement.
Preserve selected table/cell identity when the referenced definition still
exists. Computed search results, Map Finder candidate caches, and in-flight
operations are not snapshots; regenerate them only through their existing
actions.

Switching layouts never reloads BIN/XDF or changes the live document or its
undo history. At app restart, the existing BIN+XDF restore prompt remains the
consent point; choosing it reloads the same project and then restores its
active layout. BIN-only restore waits for a matching XDF before restoring
XDF-dependent windows. Stale table/surface IDs are skipped using existing
project/XDF identity checks.

Windows whose underlying content is currently session-only (for example an
unsaved XDF-authoring draft or Action History backed by volatile undo data)
are not reopened after restart as if their contents had survived. Report such
entries during restore; do not create an empty substitute window that could
mislead the user.

Snapshots are UI/view-state backups, not ROM or definition backups. Never
serialize BIN bytes, unsaved BIN edits, BIN undo/redo history, or an unsaved
XDF-authoring draft as part of this feature. Existing explicit Save As and
no-overwrite behavior remains authoritative. If the app exits with unsaved
document edits, the existing document safety behavior is unchanged.

### Screen-safe geometry

Choose a windowed native startup size and position that fit the available
display work area at the active DPI; do not force the app to maximize. Use a
conservative fallback if monitor work-area data is unavailable. Reduce the
minimum size on constrained displays so the OS does not place an oversized
window partly outside the screen.

Clamp all persistent in-app window rectangles to the current usable app area
when restoring and while rendering. The usable area excludes the top toolbar,
bottom dock, and visible diagnostics panel. If a saved window is larger than
the current usable area, shrink it to fit; when that area is smaller than the
usual minimum, adapt the minimum so its title bar and controls remain
reachable. Persist corrected geometry through the existing settings paths.

Reuse existing bounds helpers where their coordinate spaces match. Add a
shared geometry helper only if it actually removes divergent clamping logic;
do not introduce a generic window framework for one dock.

### NookLink control

Expose a read-only window list in `tuner-ui/v1` state with stable ID, kind,
label, open/minimized/focused status. Add allowlisted focus, minimize, restore,
and close actions targeting those IDs. Expose available snapshots and the
active snapshot, with allowlisted create-from-current, switch, rename, and
delete actions. Invalid or closed targets return a clear error; close uses
the same dirty-draft/user-confirmation behavior as the visible UI. Agent
actions may change presentation state only; BIN/XDF edits, approval
challenges, and modal user decisions remain on their existing protected
paths.

## Deliberately deferred

Snapshots do not switch BIN/XDF documents, projects, or edit histories. They
are saved views of the same project. Persisting unsaved ROM mutations, undo
history, XDF editor drafts, or in-flight analysis results would be a separate
recovery feature with its own safety design.

## Non-goals

- Multiple native OS windows or a second process.
- Persisting unsaved BIN/XDF edits, undo history, or XDF-authoring drafts.
- Multiple independent document sessions per project.
- A plugin/add-on window registry.
- Changing BIN/XDF document state, edit history, or approval rules.

## Error handling and compatibility

Missing or stale window IDs do not panic and do not alter documents. Old
project geometry is clamped on restore and written back only through existing
project-preference persistence. If the current monitor is too small for a
stored window, the window shrinks and remains usable rather than preserving an
off-screen rectangle. Closed tools remain reopenable through existing menu or
command actions. Existing project settings without workspace snapshots migrate
to the active `Default` workspace, keeping the existing project-preference
fields as its one canonical state and creating no duplicate snapshot. Existing
open windows, view settings, and geometry are preserved. If a snapshot refers to a missing XDF definition or
removed tool, skip only that entry and report it in workspace restore status;
keep restoring the remaining layout.

## Tests and acceptance

- Geometry unit tests cover negative/off-screen positions, oversized saved
  dimensions, smaller viewports, and repeated clamping without drift.
- Headless UI tests cover dock entry enumeration and focus → minimize → restore
  → close transitions for tables and representative tool windows.
- Project preference tests cover legacy-to-Default migration, profile
  create/duplicate/rename/delete, full view-state round-trip, auto-save after
  geometry settles, active-state canonicalization (no duplicate active
  snapshot), and switching snapshots without changing BIN/XDF identity or
  bytes. Utility window rectangles round-trip with each layout; existing
  typed memories remain authoritative for windows that already have them.
- Tests verify minimizing does not discard table/tool state, closing follows
  the existing close behavior, and dock actions do not mutate BIN bytes or
  XDF definitions.
- Startup restore tests verify that choosing BIN+XDF reopens the active
  snapshot and BIN-only waits for its matching XDF before reopening those tools.
- NookLink tests verify window/snapshot state visibility, valid
  focus/minimize/restore/profile switching, and rejection of stale/unknown IDs.
- A native Windows smoke check verifies the app launches within the display
  work area, the dock remains reachable with Diagnostics visible, and
  restored oversized windows are clamped.
- Run the formatter, app tests, workspace tests, optimized app build, and the
  release GUI smoke test.
