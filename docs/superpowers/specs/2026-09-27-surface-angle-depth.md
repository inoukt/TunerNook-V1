# 3D Surface Angle-Dependent Drawing

## Goal

Keep the 3D surface mesh visually ordered when users orbit away from the
default view. The default view and surface values must remain unchanged.

## Behavior

- Each complete grid cell is split into two triangle primitives along its
  top-left to bottom-right diagonal. Triangles are painted back-to-front using
  average camera-space depth, not screen-space vertical position.
- Camera depth uses the same orthographic yaw/pitch transform as point
  projection: `sin(pitch) * z + cos(pitch) * (sin(yaw) * x + cos(yaw) * y)`.
- Only cell-border edges are stroked; the internal triangulation diagonal is
  hidden so the existing map-grid appearance remains.
- Both normal and compare surfaces continue to use the shared
  `project_surface_geometry` result.
- Face holes, point locations, axis positions, selection, and editing behavior
  are unchanged.

## Out of Scope

Perspective projection, changing orbit controls, and modifying BIN/XDF data.

## Release Identity

Advance the app patch version from `0.1.0` to `0.1.1` for this change and show
`TunerNook v0.1.1` in the native title bar. Keep `APP_NAME` equal to
`TunerNook` because it also names the settings directory.

## Verification

Add rotated-view regressions for depth ordering and a warped cell that currently
projects to a self-crossing quad. Existing default-angle and missing-cell tests
must remain green; run the app and workspace checks.
