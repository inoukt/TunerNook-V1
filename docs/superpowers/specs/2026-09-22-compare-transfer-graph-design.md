# Compare, Transfer, and Graph Comparison Design

**Date:** 2026-09-22  
**Status:** Written spec approved in conversation; implementation plan in review

## Goal

Add a desktop Compare workspace that can inspect a second BIN beside the active editable BIN, visualize table differences in 2D and 3D, and transfer explicitly reviewed maps through the existing atomic transfer engine without overwriting input files.

## Current foundation

The workspace already provides:

- a destination BIN/XDF editor with typed scalar, flag, bitmask, axis, and table editing;
- movable, resizable table windows and interactive 3D surfaces;
- persisted project state, selection synchronization, color ranges, and background operations;
- `tuner-core::compare_bytes` for byte-level comparison;
- `tuner-transfer::TransferPlan` for exact-XDF-gated semantic planning, address-review gates, stale-hash protection, and one-transaction application;
- CLI/API compare and transfer commands;
- authenticated `tuner-ui/v1` control for the running desktop UI.

The missing slice is the desktop workflow that brings these capabilities together.

## Scope

### Included

1. Load one read-only compare/source BIN beside the active editable destination BIN.
2. Optionally load a source XDF; when omitted, the active XDF is used for both sides.
3. Show byte-level and mapped-parameter differences.
4. Show source, destination, absolute delta, and percentage delta table values.
5. Reuse the existing 3D surface projection for source, destination, and delta graphs.
6. Keep table, graph, and compare-list selection synchronized by semantic ID and cell range.
7. Build a dry-run transfer plan from selected semantic parameters.
8. Require exact source/destination XDF hash equality before transfer can become ready.
9. Require explicit approval for address-review entries and block unsafe plans.
10. Apply an approved plan as one undoable edit to the active in-memory destination BIN, followed by explicit Save As for a new output file.
11. Persist compare paths and view preferences in the active BIN project.
12. Expose compare state and actions through the existing NookLink UI IPC bridge.

### Excluded from this slice

- live ECU communication, flashing, logging, or acquisition;
- automatic checksum correction or a claim that a saved BIN is flash-ready;
- editing the read-only source BIN;
- merging two different XDF schemas through implicit conversion;
- a second native taskbar process for compare windows;
- dual overlaid 3D surfaces as the default visualization.

## User model and safety rules

The active workspace BIN remains the destination and the only editable document. The compare BIN is read-only and is never modified. All compare operations are descriptive until the user explicitly selects maps, reviews the dry run, and applies the plan.

The app must never overwrite either input BIN. Applying a transfer changes only the active in-memory destination document through the existing `BinDocument` transaction path. The user must use Save As to publish a new BIN; existing output paths remain protected by the current no-overwrite save behavior.

Parameter transfer is allowed only when the source and destination XDF exact SHA-256 hashes match. A normalized fingerprint is displayed as diagnostic context, not used as an authorization substitute. A missing, invalid, or mismatched source XDF keeps the compare graph available when values can be read with the active XDF, but disables transfer planning until the pair is valid.

## Comparison data model

Add a focused comparison module, `crates/tuner-app/src/compare.rs`, rather than growing the existing UI orchestration file with all comparison math. The module owns pure data shaping and mode calculations; `lib.rs` owns loading, persistence, windows, and command/IPC integration. Add the existing workspace crate `tuner-transfer` to `crates/tuner-app/Cargo.toml`; no new third-party dependency is needed.

The comparison module provides types equivalent to:

```rust
pub enum CompareValueMode {
    Destination,
    Source,
    AbsoluteDelta,
    PercentDelta,
}

pub struct CompareCell {
    pub destination: Option<f64>,
    pub source: Option<f64>,
    pub delta: Option<f64>,
    pub percent_delta: Option<f64>,
}

pub struct CompareMapData {
    pub semantic_id: String,
    pub rows: usize,
    pub columns: usize,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub cells: Vec<CompareCell>,
}
```

`delta` is defined as `source - destination`, so a positive delta is the value the destination would gain by matching the source. `percent_delta` is `100 * delta / abs(destination)`; when the destination value is zero or either side is unavailable/non-finite, the percentage is `None` and the UI displays an em dash rather than an invented number.

The data builder reads both sides through the existing XDF storage/conversion APIs. Engineering values are preferred; finite raw values are retained as a fallback for display and diagnostics. Missing or unreadable cells remain `None`. Dimensions, axis values, and semantic IDs come from the selected XDF definitions and must not be inferred from visible titles. When separate source and destination XDFs describe incompatible dimensions or axes, the affected map is shown as unavailable for mapped comparison rather than being padded or position-matched silently.

Byte comparison uses `tuner_core::compare_bytes` and reports lengths, changed-byte count, and contiguous changed ranges. Mapped comparison is keyed by semantic ID and reports unavailable source/destination parameters separately from equal or changed maps.

## Graph comparison

The compare workspace reuses `SurfaceData`, `SurfaceViewMemory`, `auto_surface_range`, `project_surface`, `project_surface_points`, and existing point-selection behavior.

The graph mode selects which value stream becomes the surface height and color input:

| Mode | Surface values | Range behavior |
|---|---|---|
| Destination | destination values | existing padded auto-range |
| Source | source values | existing padded auto-range |
| Absolute delta | `source - destination` | symmetric zero-centered range |
| Percentage delta | `100 * delta / abs(destination)` | symmetric zero-centered range |

Delta modes use a finite fallback range around zero for flat maps. Missing cells stay holes. The existing auto-range and fixed-range controls remain available, but the mode determines the default range when Auto range is enabled.

The same graph supports point clicks and rectangular marquee selection. A point click selects the corresponding semantic ID and cell in the compare table without stealing focus from the graph. The active table selection remains highlighted while the compare graph is focused. A graph background click clears only the compare selection when it is intentionally empty, matching the existing surface interaction contract.

The first version renders one selected value stream at a time. It does not overlay two opaque surfaces, because overlapping geometry makes sign and cell correspondence difficult to read. Source/destination side-by-side comparison remains available through the mode selector and linked values; an overlay can be added later without changing the data model.

## Desktop workspace

Add a persistent Compare window using the existing movable/resizable window conventions. Its layout is:

- **Source/Destination bar:** compare BIN path, optional source XDF path, exact-hash status, and close/reload controls;
- **Difference browser:** changed-only toggle, category filter, sort, semantic title, dimensions, changed-cell count, and transfer status;
- **Center editor:** selected map table with the active comparison mode and source/destination values available in the inspector;
- **Graph control:** 2D cell-difference coloring plus the existing 3D surface window for the selected map;
- **Transfer review:** selected map count, dry-run status, blocking issues, address-review approvals, changed bytes/ranges, and Apply/Save As controls;
- **Diagnostics:** source/destination hashes, conversion/read failures, stale-plan messages, and operation progress.

The compare table uses the existing cell selection model. Selecting cells from the difference browser, table, or graph updates the same semantic ID and range. The destination editor remains usable for ordinary edits, but transfer planning is invalidated whenever the source, destination, XDF pair, or selected map set changes.

Compare BIN/XDF loading, comparison snapshot construction, and transfer planning run through the existing background coordinator. A stale result cannot replace a newer compare document or plan. While an operation is active, conflicting compare/edit actions are disabled and use the existing amber progress overlay.

## Transfer workflow

1. The user opens a compare BIN and optionally its XDF.
2. The app validates both documents and displays byte/mapped differences even when transfer is unavailable.
3. The user selects semantic parameters or categories from the difference browser.
4. The app builds `tuner_transfer::TransferPlan` with `TransferOptions`.
5. The review panel shows each `ParameterTransfer` as Ready, Unchanged, Review Required, or Blocked, including exact issue codes and changed ranges.
6. The user explicitly approves any address-review entries.
7. The app rebuilds the plan with those approvals and rechecks the current source/destination hashes.
8. Apply commits all planned writes to the active destination BIN through one transaction, preserving one undo step and invalidating stale search/compare caches.
9. Save As writes a new BIN only; the original source and destination files remain untouched.

If either BIN changes after planning, the plan is rejected and must be rebuilt. If the XDF hashes differ, the UI explains the mismatch and leaves Apply disabled. A plan with no changed bytes is valid but reports Unchanged and does not create an edit.

## Persistence

Extend `ProjectPreferences` with serde-defaulted comparison state:

- compare BIN path;
- compare XDF path, when explicitly selected;
- compare value mode;
- difference-browser filter and changed-only setting;
- compare window geometry and graph view memory;
- last selected compare semantic ID.

Missing paths are retained as recoverable history but are reported as unavailable and never loaded automatically without user action. A different active BIN identity must not reuse the previous project’s compare document or transfer plan.

Transfer plans themselves are not persisted. They contain file hashes and must always be rebuilt from current documents.

## NookLink UI IPC

Add stable direct actions and capability entries:

- `open_compare_bin` with `path`;
- `open_compare_xdf` with `path`;
- `close_compare`;
- `set_compare_mode` with `command` equal to `destination`, `source`, `delta`, or `percent_delta`;
- `set_compare_filter` with `filter` and optional `changed_only` control;
- `build_transfer_plan` with selected semantic IDs;
- `approve_transfer_address` with `semantic_id`;
- `apply_transfer_plan`;
- `compare_state` through the existing state snapshot.

The state snapshot reports source/destination paths and hashes, byte comparison, mapped counts, active mode, selected map/cell range, graph state, plan status, blocking issues, and operation progress. IPC actions use semantic IDs and plan hashes; they never target visible titles alone.

## Testing strategy

Tests use different workflows rather than repeating one end-to-end path:

1. Pure comparison math: sign-correct delta, percentage-zero handling, missing cells, and symmetric delta ranges.
2. Byte and mapped comparison: changed ranges, equal maps, missing semantic IDs, and raw fallback.
3. XDF safety: identical hashes enable planning; mismatched hashes block it with a structured issue.
4. Transfer integration: selected maps, explicit address approval, stale source/destination rejection, one undo entry, and no input overwrite.
5. Persistence: compare state round-trip, missing paths, and BIN-identity isolation.
6. Graph behavior: point/marquee selection maps to the correct cell; delta mode retains holes and auto-ranges correctly.
7. UI behavior: compare window renders headless, mode changes invalidate/rebuild cached data, and focus/selection synchronization remains stable.
8. IPC behavior: capability discovery, state snapshot, valid actions, and rejected stale/blocked actions.

The completion gate remains formatting, the focused comparison/transfer tests, `cargo test --workspace`, and an optimized build plus a short native launch smoke test. No test workflow is repeated more than ten times; each verification pass changes scope or test type.

## Non-functional constraints

- Add no external runtime dependency; reuse existing egui, core, XDF, transfer, and operation infrastructure.
- Keep the source BIN and source XDF read-only.
- Keep all mutations on the UI thread and route writes through existing transaction APIs.
- Keep compare calculations cached by document hashes, XDF fingerprint, semantic ID, and destination data revision.
- Keep the UI responsive for large BIN/XDF files and surface errors in the existing diagnostics/report path.
