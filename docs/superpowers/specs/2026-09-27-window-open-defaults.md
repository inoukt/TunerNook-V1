# Window Opening and Restore Defaults

## Goal

Make fresh windows land in the visible editor workspace, let users choose a
default zoom for first-open tables, and restore Compare's workspace rectangle
even when egui retains state under a reused window ID.

## Behavior

- The toolbar exposes **New table zoom** with minus/value/plus controls. Its
  app-wide preference defaults to 100%, steps by 10%, and clamps to 50–200%.
- The setting is used only when a table has no saved per-table memory. A saved
  table zoom and rectangle always take precedence.
- A movable window with no saved rectangle starts in the central editor canvas,
  not over the Parameters browser. Fresh windows use a small in-canvas cascade;
  saved positions are never changed by that placement.
- Legacy typed window memories remain saved when their prior geometry was
  non-default. Default/never-opened memories are treated as fresh after
  migration. Existing utility geometry maps remain the source of truth for
  utility windows; no second geometry store is introduced.
- On reopening Compare or switching to a workspace with Compare open, apply
  that workspace's saved/clamped rectangle once. Normal move/resize behavior
  resumes immediately afterward.
- The native title advances from `TunerNook v0.1.1` to `TunerNook v0.1.2`;
  `APP_NAME` stays `TunerNook` for stable settings paths.

## Out of Scope

Independent workspaces with separate BIN/XDF documents, new window manager
frameworks, and changing saved geometry for already-positioned windows.

## Safety and Compatibility

No BIN/XDF data is touched. Older settings deserialize with their existing
geometry preserved; new optional preference/geometry metadata has safe
defaults. App settings schema advances to version 6 and project settings to
version 13.

## Verification

Test default zoom and saved-zoom precedence; in-canvas cascade bounds and
saved-position precedence; legacy memory migration; Compare one-shot workspace
restore; toolbar actions; formatting, app/workspace tests, release build, and
release GUI smoke.
