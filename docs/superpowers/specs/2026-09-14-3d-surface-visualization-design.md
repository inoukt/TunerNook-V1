# 3D Surface Visualization Design

## Goal

Add a useful first 3D view for table parameters without creating a second data or editing model. The view is a movable/resizable egui window backed by the selected table, with interactive camera controls and a reliable automatic range so ordinary tuning maps look readable immediately.

## Scope

Included:

- A `3D Surface` window opened from an open table, the command palette, or `tuner-ui/v1`.
- Table cells rendered as a projected colored surface, using finite engineering values and raw-value fallback when engineering conversion is unavailable.
- Actual X/Y axis values when address-backed axis data can be read; index positions otherwise.
- Mouse orbit, wheel zoom, pan, reset view, height exaggeration, wireframe toggle, and auto-range toggle.
- Per-table window/camera persistence in the BIN project preferences.
- Stable command and IPC state so an AI agent can open, focus, inspect, and reset a surface view.
- Pure data/range/projection tests and a headless UI render test.

Excluded from this slice:

- New BIN editing behavior.
- Scalar/function/bitmask/raw-hex editors.
- GPU-specific rendering or a third-party plotting dependency.
- Multi-series charts, live logging, and 3D point clouds.

## Rendering and auto-range

The surface uses the existing normalized table model and `egui::Painter`; each table cell becomes one projected quadrilateral. Faces are painter-sorted by depth so the result remains legible without a rendering backend change. Existing table palettes are reused for fill colors.

The data range is calculated from finite display values. By default, values are engineering values when finite, otherwise their raw numeric representation. The Z range receives 5% padding of its span; a flat or single-value table receives a minimum padding of `max(abs(value) * 0.05, 1.0)`. This prevents a flat map from collapsing into a zero-height surface and prevents the highest/lowest faces from touching the plot frame. Auto-fit derives height and camera distance from the X/Y extent and padded Z span. Fixed/manual range remains available for comparison work and is persisted.

## State and interaction

`SurfaceWindowMemory` is keyed by the existing fingerprint/semantic table key and stores window geometry, yaw, pitch, zoom, pan, height exaggeration, range mode, fixed bounds, wireframe, and axes visibility. Invalid or extreme persisted values are clamped during load.

Opening an already-open surface focuses and raises it. Closing a surface only changes project UI state; it does not affect the BIN, XDF, table selection, or undo history. Surface controls are UI-only and are safe while no BIN is loaded as long as the XDF can provide values.

## Agent control

Add `view.3d-surface` to the command registry and `open_surface`, `focus_surface`, and `reset_surface_view` to `tuner-ui/v1`. Requests identify a table by exact `semantic_id`; state reports open surfaces and camera/range settings. These actions call the same helpers as the visible UI.

## Errors and testing

Unavailable cells become holes with a diagnostic count rather than aborting the whole plot. If no finite values exist, the window displays a clear message and keeps camera controls usable. Tests cover auto-range padding/flat data, actual axis extraction, deterministic projection/depth ordering, persistence sanitization, command/IPC behavior, and headless rendering.
