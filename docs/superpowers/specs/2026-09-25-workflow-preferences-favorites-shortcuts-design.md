# Workflow Preferences, Favorites, and Shortcuts

Status: Approved by user  
Date: 2026-09-25  
Project: TunerNook

## Summary

Improve everyday navigation without adding another preferences or command
system. Persist the selected-cell Sweep effect as an app preference, show
favorited parameters in a dedicated linked section of the Parameters browser,
make the existing Settings tool easier to reach, and turn its existing command
shortcut fields into a discoverable keybinding editor. Shift+1 through Shift+4
switch among the first four workspaces in stable numeric workspace-ID order,
independent of which workspace is currently active.

## Existing architecture

- `AppPreferences` is the app-wide settings store and already owns command
  shortcut bindings.
- The Settings UI is already an in-app utility window (`WindowId::Settings`)
  and the command registry already supplies the list of bindable commands.
- Favorite table keys already identify an XDF fingerprint and semantic
  parameter ID. The browser's star action and the table dock's pinned state
  synchronize through existing favorite helpers.
- Named workspace snapshots belong to the active BIN project and have stable
  numeric IDs. The selector displays the active workspace first, so its visual
  order changes when the user switches layouts.
- `animate_selected_cell_colors` currently defaults to off at runtime and is
  not saved. The rendering path already animates selected cells and graph
  points when enabled.

## Goals

1. Sweep defaults to enabled for new and migrated app settings, and the user's
   toggle survives app restart.
2. The Parameters browser has a collapsible Favorites section containing
   linked entries for favorite parameters present in the currently loaded XDF.
3. File → Settings and a visible gear button open/focus the existing Settings
   utility; they do not create duplicate Settings windows.
4. Tools → Keyboard Shortcuts opens/focuses the shortcut editor in that same
   Settings utility.
5. Users can record, clear, and reset shortcuts for every registered command,
   including commands available from the command palette.
6. Shift+1…Shift+4 are the defaults for workspace slots 1…4. Users can rebind
   them through the same editor.
7. Preference editing changes UI settings only; BIN/XDF bytes, identities, and
   document revisions are unchanged. Changing a global preference does not
   rewrite saved workspace-view snapshots. The existing Settings utility's
   own open/focus/geometry state continues to follow the current dock policy.

## Design

### Persisted Sweep preference

Add a boolean to `AppPreferences`, defaulting to `true`. Use the normal
serde-default migration path so old settings files without the field also
start with Sweep enabled. The toolbar toggle updates the preference and saves
it through the existing preferences persistence path. Rendering reads the
preference directly; remove the separate transient runtime boolean to prevent
the control and saved value from drifting apart.

### Linked Favorites browser section

Render a collapsible `Favorites` header above the existing category tree when
an XDF is loaded. Resolve entries from the current XDF catalog and the existing
favorite keys; do not copy parameter definitions, table buffers, or open
windows into a second store. Use the same title/filter and selection styling
as category entries. Clicking a favorite uses the existing `open_table` path,
which already reuses and focuses an open table with the same stable key.

Unfavoriting through either the browser star or the table's pin control removes
the linked entry on the next UI frame. It does not close the table. Favorites
that do not resolve in the current XDF are not rendered and are not silently
deleted from persisted settings.

### Settings entry points

Add `Settings…` to the File menu and a gear button to the main toolbar. Both
route through the existing Settings command/window ID, focus it if already
open, and preserve its existing geometry and dock behavior. Add `Keyboard
Shortcuts…` under Tools; it opens the same Settings window and focuses its
shortcut section. Do not add a second settings store or shortcut window.

### Shortcut editor and workspace slots

Replace manual chord entry as the primary interaction with a small recorder:
choose a command, press a chord, or clear/reset its binding. Keep a searchable
list of registered command descriptors and show the current binding and
command-palette category. The editor reports duplicate/conflicting bindings
and does not accept a new chord already assigned to another action until the
conflict is resolved. Empty remains the explicit unbound state.

Add four stable workspace-slot commands to the existing registry and default
shortcut map. Slots are the first four existing workspaces sorted by numeric
ID, independent of active-first selector display order. Switching by slot
uses the existing guarded workspace-switch path, preserving its current
capture/restore and document-safety behavior. If a slot is unavailable in the
current BIN project, its command is disabled and the shortcut is a no-op.
Extend the chord parser/recorder to accept number keys so Shift+1…Shift+4 are
valid. Conflicts between workspace-slot keys and ordinary command shortcuts
are handled by the same editor.

Shortcut values remain app-wide in `AppPreferences`; workspace slots resolve
against the currently loaded project's active workspace plus saved snapshots
at invocation time. Renaming a workspace does not rewrite shortcut settings.
When loading older settings with a partial shortcut override map, add defaults
only for newly introduced workspace-slot command IDs; preserve existing custom
values and explicit empty/unbound entries.

## Persistence and compatibility

- Sweep and command bindings use the existing global `settings.json` and
  preference save path.
- Favorites remain in the existing favorite-key store and retain their
  current project/XDF identity behavior.
- Workspace switching remains in project preferences. The four shortcuts
  target ID-sorted slots in the active project, not globally stored window
  states or the active-first visual menu order.
- Legacy settings default Sweep to enabled and retain the current default
  command bindings plus the four workspace-slot bindings.

## Out of scope

- The generic minimize/maximize/reset/always-on-top window controls and
  cross-workspace map-open prompt (Phase 2).
- Maps Search redesign and the notepad/drawing tool (later phases).
- A second Settings window, cloned favorite tables, changes to BIN/XDF data,
  global hotkeys that work while TunerNook is unfocused, or OS-level window
  management.
- NookLink protocol changes or agent-side preference writes. New UI commands
  continue to use the existing command and UI-control paths; app-side
  preference validation remains authoritative.

## Test and acceptance plan

- Old settings JSON missing Sweep deserializes with Sweep enabled; toggling
  off/on round-trips through the current preference serializer.
- Toolbar interaction reflects and persists the same preference consumed by
  table and graph rendering.
- Favorites section shows only current-XDF links, obeys the list filter,
  opens/focuses the existing table instead of duplicating it, and disappears
  immediately when unfavorited from either supported control.
- File → Settings, the gear button, and Tools → Keyboard Shortcuts all focus
  the same Settings window; the latter selects/scrolls to the shortcut editor.
- Shortcut capture accepts supported chords including Shift+1…Shift+4;
  invalid and conflicting chords are surfaced and not silently assigned.
- Every command descriptor is available to bind; empty, reset-to-default, and
  legacy shortcut settings behave consistently.
- Workspace-slot shortcuts follow stable ID order, respect the existing
  workspace-switch guard, and do nothing safely when the requested slot is
  absent.
- Preference edits leave BIN bytes, XDF identity, document revision, and saved
  workspace-view snapshots unchanged; the Settings window's own open/focus
  state may still persist through its existing dock lifecycle.

## Risks and decisions

- Numeric ID ordering keeps workspace-slot bindings stable as layouts are
  renamed or switched. Make the slot-to-name mapping visible in the shortcut
  editor so a user can see which layout each number currently targets.
- Sweep can be visually distracting. It remains a single, easy-to-toggle
  preference and is enabled by default as explicitly requested.
- Shortcut capture must avoid stealing keystrokes while a text field is being
  edited. The recorder captures only after the user explicitly starts
  recording, and ordinary command dispatch remains suppressed while a text
  input owns keyboard focus.
