# TunerNook Floating Workspace Design

Status: Approved for implementation
Date: 2026-09-13
Project: TunerNook

## Context

The first desktop shell is usable, but its parameter browser is a long flat
list and the active table occupies one fixed central editor. This follow-on
slice turns the shell into a calmer desktop-like workspace for users who need
strong visual organization and low-friction context switching. The screenshots
provided by the user are visual references for the current state; they are not
application instructions.

The selected interaction model is floating windows inside the existing
TunerNook window, not separate native OS windows. This keeps table tabs,
keyboard navigation, saved layouts, and safety boundaries in one predictable
workspace while still allowing tables to move, resize, and zoom independently.

## Goals

1. Make category grouping the default browsing experience, with all category
   folders initially collapsed for a newly opened BIN.
2. Open each selected table in a movable, resizable, zoomable internal window.
3. Provide a tab strip for open tables with focus, close, pin, and ordering
   controls.
4. Remember per-table position, size, zoom, and scroll context across launches,
   keyed by XDF identity and semantic parameter ID.
5. Add favorites, recent tables, quick switching, and simple arrange commands
   to reduce navigation friction.
6. Make the browser and inspector collapsible into narrow side rails using
   visible arrow buttons; restoring them preserves their last width.
7. Give long browser/inspector content and every floating table an independent
   scrollable frame so content does not impose an unusable minimum window size.
8. Keep the existing BIN/XDF transaction and no-overwrite Save As rules intact.

## Non-goals for this slice

- Separate native OS windows or multiple processes.
- Automatic reopening of BIN/XDF source files or storing calibration bytes in
  preferences.
- Full drag-and-drop dashboard authoring, arbitrary panel plug-ins, or remote
  synchronization.
- Compare, transfer, hex, plotting, and checksum capability implementations;
  their future commands can use this workspace spine.

## Architecture

Extend `tuner-app`'s presentation model without changing `tuner-core` or
`tuner-xdf`:

```text
WorkspaceState       document ownership, selection, transactions, validation
TunerApp             open table tabs, focus/z-order, browser organization,
                     drawer state, layout commands, settings persistence
AppPreferences       versioned global UI preferences and migration fallback
ProjectPreferences   per-BIN workspace state and table placement
egui::Window         one floating table view per open semantic parameter
egui::Panel          category browser/inspector drawers and collapsed rails
```

`WorkspaceState` continues to own all document operations. `TunerApp` owns
ephemeral open-table records and uses `WorkspaceState::selected_parameter`,
`cell_view`, and edit methods to render or mutate values. A table window is
identified by a stable key composed of the XDF normalized fingerprint and the
parameter semantic ID. The key prevents geometry from one definition from
being applied to an unrelated definition with a reused parameter ID.

The first browser action opens or focuses a table window. The central editor
becomes a desktop canvas; the old single-table rendering is reused inside each
window. The active table remains synchronized with the shared workspace
selection so the inspector and edit status always refer to the focused window.

## Presentation and organization

The browser defaults to `Categories` organization. It renders category headers
with counts and collapsible groups; a newly opened BIN starts with every
category folder closed. A browser organization control can instead
rank favorites or recently opened tables first while retaining the category
tree; search remains global across title, IDs, and category. Selecting a
parameter opens/focuses its table and marks it recent; favorite and pin actions
are explicit and persisted by table key.

The top tab strip shows open tables in order. Each tab has a readable short
title, a close action, and a pin indicator. The active tab is brought to the
front of the floating canvas. A compact quick-switch control searches open,
favorite, and recent tables without requiring the user to scan the browser.

Each table window has a stable ID, a title bar, a zoom control with bounded
values from 50% to 200%, horizontal/vertical scrolling, and the existing safe
cell editing controls. The browser, inspector, diagnostics, settings/debug
content, and table grid each use an inner scrollable frame with shrink behavior
that lets the containing panel/window become narrower than long labels or a
large grid. Window geometry is read from egui's area state after
rendering and written to `TableWindowMemory`; arrange commands update the
memory and issue one explicit geometry request.

The browser and inspector use `Panel::show_switched` with two IDs: an expanded
resizable panel and a narrow collapsed rail. The rail contains a high-contrast
arrow button (`>` for the left browser, `<` for the right inspector). Clicking
the arrow restores the drawer; dragging or double-clicking the panel edge also
continues to work. The expanded width is remembered separately from the
collapsed state.

Organization commands are stable registry entries: focus active table, cascade
tables, tile tables, close other tables, reset table layout, and choose
category/favorite/recent browser ordering. They are available from
View/Workspace or View/Browser Organization and the command palette, so the
user can customize shortcuts later without another UI branch.

## Persistence

Settings move to version 2 with a tolerant migration from version 1. New
fields have defaults and unknown fields remain ignored. Persisted UI state
is split between global appearance/shortcut settings and a project record
under `%APPDATA%/TunerNook/projects/`. The project record filename is a hash
of the normalized full BIN path and includes the identity it was created for,
so project layouts remain isolated without modifying the BIN or XDF folders.
It includes:

- browser organization and expanded/collapsed category IDs, with all
  categories closed when a project has no saved category state;
- favorite and recent table keys with bounded recent history;
- browser/inspector expanded state and last expanded widths;
- per-table geometry (`x`, `y`, `width`, `height`), zoom, and last scroll;
- tab order for the current XDF identity;
- the open table keys and active table key, allowing the desktop workspace to
  return to the last project arrangement.

Project changes autosave atomically. Continuous drag/resize/scroll changes are
staged and flushed after pointer interaction settles; discrete actions save
immediately.

No BIN bytes, staged transactions, or automatic source-file reopen action is
stored. Opening a BIN remains an explicit user action; once it is open, its
saved workspace can be restored when the matching XDF is available. Headless
tests use disabled persistence or explicit temporary paths and never mutate
the real user settings file.

## Error and safety behavior

Missing or stale semantic IDs are ignored when restoring view state. A table
whose mapping cannot be read remains open with an explanatory cell error; the
window manager does not panic or close unrelated tables. Geometry is clamped to
the current viewport so an old monitor arrangement cannot strand the UI.

All value edits still call `ParameterDefinition::write_engineering_cell` inside
one `BinDocument` transaction. Floating windows, tab actions, favorites, and
arrange commands never write BIN bytes. Save As remains explicit and refuses an
existing target.

## Testing and acceptance

Headless tests will cover category grouping and default organization, default
collapsed categories, project identity/file isolation, project preference
round trips, restoration of open tables and placement, stable window keys,
opening/focusing/closing tabs, favorite/recent ordering, bounded zoom,
geometry round trips and settings v1 migration, drawer collapse state, and
arrange-command state changes without document mutation. Existing full
workspace tests, fixture validation, and hash checks remain mandatory.

The native Windows smoke test will open the fixture pair, verify category
headers, open multiple tables, move/resize/zoom a table, switch tabs, collapse
and restore both drawers, use an arrange command, restart the app, and confirm
the remembered geometry plus unchanged fixture hashes.
