# TunerNook — render efficiency and staged app organization

Date: 2026-09-25  
Status: Draft for user review

## Goal

Reduce repeated work in compare, table, axis, and 3D repaint paths without
changing displayed values, selection behavior, edit safety, or BIN/XDF bytes.
Then make the oversized app source easier to evolve by moving its existing test
module out of `lib.rs`. This is an incremental first phase, not a wholesale
production-module rewrite.

## Chosen sequence

1. Optimize compare rendering, where the same source BIN digest and cached map
   are currently revisited during repaint.
2. Avoid destination cell-view construction when compare data already supplies
   the displayed cells and color range. Measure normal table rendering before
   adding a potentially large per-cell cache or changing grid virtualization.
3. Share one projected 3D vertex grid between point and face rendering, and
   avoid temporary range vectors when possible.
4. Compile axis conversions once per axis-header batch instead of once per tick.
5. Move the existing `#[cfg(test)]` module verbatim into `tests.rs`. Defer the
   wider production module split to a separate reviewed plan.

The user selected inline execution in divided parts; no subagents are planned
because these edits share the same app state/render path. The workspace has no
Git metadata, so retain the existing SDD snapshot trail instead of commits.

## Design constraints

- Add no dependencies and do not change public IPC/protocol behavior.
- Preserve compare cache identity: source BIN SHA-256, destination data revision,
  source/destination XDF fingerprints, semantic ID, and compare mode remain the
  validity boundary.
- Cache only immutable compare-source identity and map data; invalidate or
  replace it whenever a new source BIN/map is installed.
- Preserve holes, row/column order, depth sorting, selection, and axis labels in
  3D and table rendering.
- Keep the test-module move behavior-neutral; do not combine it with source
  edits to test logic.
- Do not claim a numeric speedup without a repeatable before/after measurement.

## Tests and verification

- Compare digest tests cover repeated key creation and source replacement; map
  range tests preserve current results for all compare modes, missing cells,
  zero destinations, and empty data.
- Table tests cover destination mode, ready compare maps, and pending compare
  maps so skipped work never changes visible/fallback behavior.
- Surface tests assert points and faces retain positions, values, holes, and
  depth ordering when built from shared projections.
- Axis tests compare batched reads with existing scalar reads for identity and
  non-identity conversions, missing storage, and invalid indexes.
- After each part, run its targeted tests. At the end, run formatting, app and
  workspace suites, release build, and the app smoke test from `AGENTS.md`.
- After the test-module extraction, verify the complete suite count and compare
  the moved test text against the pre-move snapshot.

## Deferred

- Full extraction of production functions from `lib.rs` into view/state/IPC
  modules; the 2026-09-23 proposal is a starting point, but its line map is
  stale and must be revised before that separate refactor.
- Full-grid table virtualization or persistent per-cell caching until profiling
  establishes which cost dominates and a memory bound is chosen.
- Any feature/dependency removal: the initial manifest and feature-tree review
  found no safe direct dependency cut.
