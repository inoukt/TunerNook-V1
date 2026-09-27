# Map Finder score calibration plan

**Goal:** Stop directionally incoherent grids from receiving a free 50% share
of the directional score, without slowing scans or suppressing coherent maps.

**Spec:** `docs/superpowers/specs/2026-09-26-map-finder-score-calibration.md`

**Scope:** `crates/tuner-app/src/map_search.rs` only, plus project notes and SDD
records. Do not change axes search, candidate clustering, decoding, or BIN/XDF
data.

## Task 1: Correct the directional score

- [x] Add a test with an extreme mixed prefix and a coherent control grid.
- [x] Observe the existing scorer fail the regression (87/100 on the fixture).
- [x] Replace the directional majority fraction with `abs(up - down) / edge_count`.
- [x] Confirm the fixture falls below 82 and the coherent control remains above it.
- [x] Run formatting, app tests, workspace tests, release build, and GUI smoke.
- [x] Record baseline/final snapshots and update `AGENTS.md`.

## Review focus

- Balanced increases/decreases get no directional bonus; all-one-direction data
  still gets full credit.
- Zero-delta edges remain in the denominator, so plateaus do not gain artificial
  consistency.
- The prior 10×10 float32 map at `0x1EF8F0` remains above the scan threshold.
- No range, raw value, or axis assignment is rewritten by scoring.
