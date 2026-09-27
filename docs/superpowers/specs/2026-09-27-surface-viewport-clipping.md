# 3D Surface Viewport Clipping

## Goal

Keep zoomed or edge-on 3D surfaces inside the graph's inner chart frame. The
plot margins remain available for axis labels and window controls.

## Behavior

- Clip surface fills, wireframe edges, grid/axis lines, points, and selection
  overlays to the inner chart rectangle (inset enough to preserve its border).
- Keep axis labels and status text in the existing plot-margin painter.
- Apply the same clipping to normal and Compare 3D views.
- Preserve yaw, pitch, zoom, pan, auto-range, and table/BIN behavior.
- Advance the app package/title from `0.1.3` to `0.1.4`; do not change
  `APP_NAME`.

## Reproduction

Use the supplied SCGa05 fixture's 16 × 18 Airflow-to-Torque map and the saved
view state from the reported screenshot: yaw 61.47992°, pitch 11.319976°,
zoom 199%, wireframe enabled. At this view, projected surface vertices extend
outside the chart frame. They should be clipped at that frame rather than paint
into its margins.

## Verification

Add a headless render regression proving triangles extend beyond the chart at
the reproduction view but their emitted paint is clipped to the chart. Verify
both graph call sites share that painter boundary; then run formatting, app and
workspace tests, release build, and GUI smoke test.
