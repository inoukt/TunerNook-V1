# Map Finder score calibration

## Problem

The Map Finder can give a high score to a grid containing extreme, directionally
mixed values. Its current directional term rewards `max(increases, decreases)`
as a fraction of all edges, so unrelated directions receive about half credit.
When extreme values expand the global range, ordinary neighboring changes also
look artificially smooth.

## Intended behavior

- Directional consistency is the absolute difference between increasing and
  decreasing edges, divided by all edges in that direction. Balanced directions
  receive no consistency credit; a fully one-way surface receives full credit.
- Keep the existing smoothness, diversity, weighting, threshold, raw values,
  and scan configuration unchanged.
- The score remains a heuristic, not a probability or a map-validity claim.
- A regression grid with the screenshot's extreme mixed prefix scores below the
  configured 82 threshold, while a coherent increasing grid remains above it.

This is a one-formula calibration. It adds no scan work, dependencies, or
automatic map/axis assignment.
