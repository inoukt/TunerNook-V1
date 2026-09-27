# 3D Surface Viewport Clipping

> Inline execution; no Git metadata is available.

**Goal:** Stop zoomed/reversed surfaces from painting beyond the graph frame.

**Approach:** Keep the existing plot painter for labels and margins. Use a
chart-bounded painter for surface geometry in both normal and Compare graphs.

## Steps

- [x] Add a failing high-zoom/reversed-angle render test using the supplied BIN/XDF map.
- [x] Clip graph geometry to the inner chart rectangle in both renderers, keeping
  axis labels outside the clip.
- [x] Bump to `0.1.4`, update notes, run the full verification battery, and
  capture final snapshots.
