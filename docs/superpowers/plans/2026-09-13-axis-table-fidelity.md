# Axis and Table Fidelity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make address-backed X/Y axes readable and safely editable, standardize per-table number precision, scale complete table-window content with zoom, and offer each BIN its remembered XDF through a responsive prompt.

**Architecture:** Put axis stride interpretation, checked range calculation, raw access, conversion, and transactional writes in `tuner-xdf`. Keep selection, formatting, window preferences, zoom presentation, and the BIN-specific XDF prompt in `tuner-app`, using existing background operations for all file work. Expose additive axis metadata through the existing manual JSON serializers and keep selection/edit operations deterministic for the NookLink/AI boundary.

**Tech Stack:** Rust 2021 workspace, `tuner-core` BIN transactions, `tuner-xdf` normalized XDF model and conversion engine, `tuner-api`/`tuner-cli` manual JSON output, `eframe`/`egui` desktop UI, `serde_json` preference persistence, and existing `OperationCoordinator` background workers.

**Spec:** `docs/superpowers/specs/2026-09-13-axis-table-fidelity-design.md`

## Global Constraints

- Default engineering precision is exactly `2` decimal places; persisted table precision is clamped to `0..=8`.
- Table zoom remains clamped to `50..=200` percent and scales title, text, controls, cell dimensions, spacing, frames, and scrollable grid content without silently replacing saved outer geometry.
- Address-backed axis range calculations use checked signed bit arithmetic and preserve negative strides.
- Descriptive axes without an address are displayed with labels/position but are never writable.
- Failed or unsupported axis writes must not mutate the BIN or create an undo entry.
- Raw values remain exact; only engineering-unit presentation is rounded.
- BIN/XDF loading, validation, workspace restoration, Save As, and report export continue through the existing non-blocking `OperationCoordinator`.
- Last-XDF reuse is opt-in through a visible prompt; a missing path is reported and cannot be loaded implicitly.
- `tuner-api/v1` remains backward-compatible; any new JSON field is additive and `stride_bits` is the stable serialized axis field.
- No source BIN is overwritten and no destructive preference migration is performed.
- This workspace has no `.git` directory, so verification checkpoints replace commit steps; do not invent or run Git commits.

## File Map

- Modify `crates/tuner-xdf/src/lib.rs` — add signed axis stride metadata, axis-specific checked ranges, raw/engineering read/write methods, structured invalid-axis errors, normalized fingerprint input, and unit tests.
- Modify `crates/tuner-xdf/tests/real_fixture.rs` — assert declared X/Y strides and read actual axis values from the supplied BIN fixture.
- Modify `crates/tuner-app/src/lib.rs` — add persisted precision and last-XDF fields, explicit axis selection/view state, axis-aware editing, precision/zoom/header UI, debug output, XDF prompt state/actions, and app tests.
- Modify `crates/tuner-api/src/main.rs` — include `stride_bits` in the axis JSON object and keep inspection output self-describing.
- Modify `crates/tuner-cli/src/main.rs` — include `stride_bits` in the axis JSON object used by `inspect-xdf` and validation output.
- Do not modify `crates/tuner-app/src/operations.rs` unless compilation proves a small helper is needed; existing `LoadingXdf`, `LoadingBin`, `RestoringWorkspace`, and operation spawning already provide the required asynchronous boundary.

### Task 1: Normalize and safely access address-backed axes

**Files:**
- Modify: `crates/tuner-xdf/src/lib.rs:25-55,335-350,409-445,465-790,2171-2292,2481-2540,2570-2618,3273-3970`
- Test: `crates/tuner-xdf/src/lib.rs` unit-test module and `crates/tuner-xdf/tests/real_fixture.rs`

**Interfaces:**
- Consumes: existing `AxisDefinition`, `StorageSpec`, `RawValue`, `EngineeringWriteResult`, `Transaction<'_>`, `calculate_range`, conversion helpers, and `lookup_signed_number`.
- Produces:
  - `AxisDefinition::stride_bits: i64`.
  - `ParameterDefinition::axis_definition(axis_index: usize) -> Result<&AxisDefinition, XdfError>`.
  - `ParameterDefinition::axis_range(axis_index: usize, index: usize) -> Result<ByteRange, XdfError>`.
  - `ParameterDefinition::read_raw_axis(bin: &BinDocument, axis_index: usize, index: usize) -> Result<RawValue, CellAccessError>`.
  - `ParameterDefinition::write_raw_axis(transaction: &mut Transaction<'_>, axis_index: usize, index: usize, value: RawValue) -> Result<(), CellAccessError>`.
  - `ParameterDefinition::read_engineering_axis(bin: &BinDocument, axis_index: usize, index: usize) -> Result<f64, CellAccessError>`.
  - `ParameterDefinition::write_engineering_axis(transaction: &mut Transaction<'_>, axis_index: usize, index: usize, engineering: f64) -> Result<EngineeringWriteResult, CellAccessError>`.
  - A structured `XdfError::InvalidAxis { semantic_id: String, axis_index: usize, index: Option<usize>, count: usize }` variant for invalid axis indices/elements, with a display message that includes the parameter semantic ID, axis index, requested element when present, and valid count.

- [ ] **Step 1: Write the failing stride-selection test.** Add a parser fixture containing three axes on one table: an `x` axis with count `3`, width `8`, major stride `24`, minor stride `-8`; a `y` axis with count `2`, width `16`, major stride `0`, minor stride `-16`; and a `z` payload with both strides `0` and width `8`. Assert `x.stride_bits == 24`, `y.stride_bits == -16`, `z.stride_bits == 8`, and that the X/Y ranges cover the signed declared locations.

```rust
#[test]
fn axis_stride_prefers_major_then_minor_then_element_width() {
    let document = XdfDocument::parse(br#"
        <XDFFORMAT><XDFTABLE uniqueid="axis-strides">
          <title>Axis stride fixture</title>
          <XDFAXIS id="x"><indexcount>3</indexcount><EMBEDDEDDATA
            mmedaddress="0x20" mmedelementsizebits="8"
            mmedmajorstridebits="24" mmedminorstridebits="-8" /></XDFAXIS>
          <XDFAXIS id="y"><indexcount>2</indexcount><EMBEDDEDDATA
            mmedaddress="0x30" mmedelementsizebits="16"
            mmedmajorstridebits="0" mmedminorstridebits="-16" /></XDFAXIS>
          <XDFAXIS id="z"><EMBEDDEDDATA mmedaddress="0x40"
            mmedelementsizebits="8" mmedrowcount="1" mmedcolcount="1"
            mmedmajorstridebits="0" mmedminorstridebits="0" /></XDFAXIS>
        </XDFTABLE></XDFFORMAT>
    "#).unwrap();
    let parameter = &document.parameters[0];
    let x = parameter.axes.iter().find(|axis| axis.id == "x").unwrap();
    let y = parameter.axes.iter().find(|axis| axis.id == "y").unwrap();
    let z = parameter.axes.iter().find(|axis| axis.id == "z").unwrap();
    assert_eq!(x.stride_bits, 24);
    assert_eq!(y.stride_bits, -16);
    assert_eq!(z.stride_bits, 8);
    assert_eq!(parameter.axis_range(0, 2).unwrap(), ByteRange { start: 0x26, end: 0x27 });
    assert_eq!(parameter.axis_range(1, 1).unwrap(), ByteRange { start: 0x2e, end: 0x30 });
}
```

- [ ] **Step 2: Run the stride test and verify it fails for the missing field/old fallback.**

Run: `cargo test -p tuner-xdf axis_stride_prefers_major_then_minor_then_element_width -- --exact`

Expected: FAIL because `AxisDefinition` has no `stride_bits` field and the current parser calculates an axis range using packed element width instead of the declared major/minor stride.

- [ ] **Step 3: Write failing axis read/write tests.** Add a compact table fixture with address-backed X and Y axes plus a Z payload. Use a BIN large enough to cover every address, write known little-endian integer values at the axis locations, and assert the following behavior:

```rust
#[test]
fn axis_read_conversion_and_transactional_write_use_declared_storage() {
    let document = XdfDocument::parse(br#"
        <XDFFORMAT><XDFTABLE uniqueid="axis-access">
          <title>Axis access fixture</title>
          <XDFAXIS id="x"><indexcount>2</indexcount><EMBEDDEDDATA
            mmedaddress="0x10" mmedelementsizebits="8"
            mmedmajorstridebits="8" mmedtypeflags="0x06" />
            <MATH equation="X * 0.5" /></XDFAXIS>
          <XDFAXIS id="y"><indexcount>2</indexcount><EMBEDDEDDATA
            mmedaddress="0x20" mmedelementsizebits="16"
            mmedmajorstridebits="16" mmedtypeflags="0x06" /></XDFAXIS>
          <XDFAXIS id="z"><EMBEDDEDDATA mmedaddress="0x30"
            mmedelementsizebits="8" mmedrowcount="2" mmedcolcount="2"
            mmedmajorstridebits="0" mmedminorstridebits="0"
            mmedtypeflags="0x06" /></XDFAXIS>
        </XDFTABLE></XDFFORMAT>
    "#).unwrap();
    let parameter = &document.parameters[0];
    let mut bin = BinDocument::from_bytes(vec![0; 0x40]);
    {
        let mut transaction = bin.transaction("seed axes");
        transaction.write_uint(0x10, 1, Endianness::Little, 10).unwrap();
        transaction.write_uint(0x11, 1, Endianness::Little, 20).unwrap();
        transaction.write_uint(0x20, 2, Endianness::Little, 100).unwrap();
        transaction.write_uint(0x22, 2, Endianness::Little, 200).unwrap();
        transaction.commit().unwrap();
    }
    assert_eq!(parameter.read_raw_axis(&bin, 0, 1).unwrap(), RawValue::Unsigned(20));
    assert_eq!(parameter.read_engineering_axis(&bin, 0, 1).unwrap(), 10.0);
    assert_eq!(parameter.axis_range(1, 1).unwrap(), ByteRange { start: 0x22, end: 0x24 });

    let result = {
        let mut transaction = bin.transaction("edit x axis");
        let result = parameter.write_engineering_axis(&mut transaction, 0, 1, 12.5).unwrap();
        transaction.commit().unwrap();
        result
    };
    assert_eq!(result.raw_value, RawValue::Unsigned(25));
    assert_eq!(parameter.read_raw_axis(&bin, 0, 1).unwrap(), RawValue::Unsigned(25));
    assert!(parameter.read_raw_axis(&bin, 3, 0).is_err());
}

#[test]
fn descriptive_axis_is_not_writable() {
    let document = XdfDocument::parse(br#"
        <XDFFORMAT><XDFTABLE><title>Descriptive axis</title>
          <XDFAXIS id="x"><indexcount>2</indexcount><LABEL index="0">low</LABEL>
            <LABEL index="1">high</LABEL></XDFAXIS>
          <XDFAXIS id="z"><EMBEDDEDDATA mmedaddress="0x00"
            mmedelementsizebits="8" mmedrowcount="1" mmedcolcount="2"
            mmedmajorstridebits="0" mmedminorstridebits="0"
            mmedtypeflags="0x06" /></XDFAXIS>
        </XDFTABLE></XDFFORMAT>
    "#).unwrap();
    let parameter = &document.parameters[0];
    let bin = BinDocument::from_bytes(vec![0; 2]);
    assert!(parameter.read_raw_axis(&bin, 0, 0).is_err());
}
```

- [ ] **Step 4: Run the focused access tests and verify they fail before implementation.**

Run: `cargo test -p tuner-xdf axis_read_conversion_and_transactional_write_use_declared_storage -- --exact`  
Run: `cargo test -p tuner-xdf descriptive_axis_is_not_writable -- --exact`

Expected: FAIL because the axis accessors do not exist.

- [ ] **Step 5: Implement the normalized stride and checked axis model.** Add `stride_bits: i64` to `AxisDefinition`; parse `MMEDMAJORSTRIDEBITS`/`MAJORSTRIDEBITS`/`ROWSTRIDEBITS` and `MMEDMINORSTRIDEBITS`/`MINORSTRIDEBITS`/`COLUMNSTRIDEBITS` using `lookup_signed_number`. Select major when nonzero, otherwise minor when nonzero, otherwise `checked_stride_from_usize(width_bits)`. Pass that stride to `calculate_range` for the stored axis range. Add `XdfError::InvalidAxis` and `axis_definition`/`axis_range` with checked signed bit arithmetic, `checked_bit_bounds`, byte conversion, and count validation.

- [ ] **Step 6: Implement axis raw and engineering access by reusing cell storage rules.** Add a private axis-bit-bound helper and shared byte-aligned integer/binary32 read/write helpers where that reduces duplication. `read_raw_axis` must reject missing address, missing storage, unsupported numeric kind, non-byte-aligned start/width, out-of-range element, and BIN bounds. `write_raw_axis` must enforce integer/binary32 type compatibility, signedness, endian, and width. `read_engineering_axis` parses the axis conversion or identity and evaluates the raw value. `write_engineering_axis` inverts conversion, performs the same finite/representability checks as `write_engineering_cell`, stages the write in the supplied transaction, and returns `EngineeringWriteResult`.

- [ ] **Step 7: Run the focused XDF tests and record the green checkpoint.**

Run: `cargo test -p tuner-xdf axis_stride_prefers_major_then_minor_then_element_width -- --exact`  
Run: `cargo test -p tuner-xdf axis_read_conversion_and_transactional_write_use_declared_storage -- --exact`  
Run: `cargo test -p tuner-xdf descriptive_axis_is_not_writable -- --exact`

Expected: PASS, with no BIN mutation from the rejected descriptive-axis access.

- [ ] **Step 8: Add supplied-fixture assertions for actual X/Y axes.** In `crates/tuner-xdf/tests/real_fixture.rs`, find the parameter titled `Setpoint map for port flap`, assert its X axis has address `0x1336`, count `10`, width `8`, stride `8`, and its Y axis has address `0x12D58`, count `10`, width `16`, stride `16`. For every index, compare `read_raw_axis` with direct `BinDocument::read_uint` at `base + index * stride_bytes`, and compare engineering reads with the axis conversion formula. This catches the previous packed-width interpretation without hard-coding an unrelated table body.

- [ ] **Step 9: Run the real-fixture tests.**

Run: `cargo test -p tuner-xdf --test real_fixture`

Expected: PASS, including the existing 2,915-parameter validation and the new actual axis-value assertions.

### Task 2: Add explicit app target state and persisted precision

**Files:**
- Modify: `crates/tuner-app/src/lib.rs:85-160,251-344,1032-1320,1357-1562,1564-1610,2060-2128,3017-3037,5222-5245,5462-end`
- Test: `crates/tuner-app/src/lib.rs` unit-test module

**Interfaces:**
- Consumes: Task 1 axis accessors, existing `WorkspaceState`, `TableWindowMemory`, `ProjectPreferences`, and `EngineeringWriteResult`.
- Produces:
  - `TableWindowMemory::decimal_places: u8`, default `2`, sanitized to `0..=8`.
  - `AxisSelection { axis_index: usize, index: usize }`.
  - `AxisView { axis_index, index, label: Option<String>, range: Option<ByteRange>, raw: Option<RawValue>, engineering: Option<f64>, editable: bool, error: Option<String> }`.
  - `WorkspaceState::selected_axis: Option<AxisSelection>`.
  - `WorkspaceState::select_axis(axis_index: usize, index: usize) -> Result<(), WorkspaceError>`.
  - `WorkspaceState::selected_axis_view() -> Result<AxisView, WorkspaceError>`.
  - `WorkspaceState::axis_view_for(semantic_id: &str, axis_index: usize, index: usize) -> Result<AxisView, WorkspaceError>`.
  - `format_f64_with_precision(value: f64, decimal_places: u8) -> String`, with `format_f64(value)` delegating to precision `2`.

- [ ] **Step 1: Write failing precision and migration tests.** Extend the existing table-memory test with `decimal_places: 9`, assert sanitization to `8`, assert default memory is `2`, assert `14.0` renders as `14.00`, and deserialize an older table-window JSON object that contains no `decimal_places` field.

```rust
#[test]
fn table_precision_defaults_to_two_and_clamps_old_or_invalid_values() {
    assert_eq!(TableWindowMemory::default().decimal_places, 2);
    let mut memory = TableWindowMemory { decimal_places: 9, ..Default::default() };
    memory.sanitize();
    assert_eq!(memory.decimal_places, 8);
    let old: TableWindowMemory = serde_json::from_str(
        r#"{"x":1,"y":2,"width":760,"height":520,"zoom_percent":100,"scroll_x":0,"scroll_y":0}"#,
    ).unwrap();
    assert_eq!(old.decimal_places, 2);
    assert_eq!(format_f64_with_precision(14.0, 2), "14.00");
    assert_eq!(format_f64(14.0), "14.00");
    assert_eq!(raw_value_text(RawValue::Unsigned(14)), "u14");
}
```

- [ ] **Step 2: Run the precision test and verify it fails.**

Run: `cargo test -p tuner-app table_precision_defaults_to_two_and_clamps_old_or_invalid_values -- --exact`

Expected: FAIL because `decimal_places` and `format_f64_with_precision` do not exist and integer engineering values still use the old zero-decimal formatter.

- [ ] **Step 3: Write failing workspace axis-selection tests.** Add the `AXIS_TABLE_XDF` app fixture with X/Y address-backed axes and a Z payload, seed the BIN byte at X index `1` with raw value `20`, then assert `select_axis(0, 1)` returns the actual raw/engineering view, `select_cell` clears `selected_axis`, selecting an axis clears edit text, and applying engineering text edits the axis with one undo entry.

```rust
#[test]
fn workspace_axis_selection_reads_and_edits_the_axis_target() {
    let xdf = XdfDocument::parse(AXIS_TABLE_XDF).unwrap();
    let mut workspace = WorkspaceState::default();
    let mut bytes = vec![0; 0x40];
    bytes[0x11] = 20;
    workspace.set_documents(
        Some(BinDocument::from_bytes(bytes)),
        Some(xdf),
        None,
        None,
    );
    let id = workspace.filtered_parameters("")[0].semantic_id.clone();
    workspace.select_parameter(&id);
    workspace.select_axis(0, 1).unwrap();
    let view = workspace.selected_axis_view().unwrap();
    assert_eq!(view.axis_index, 0);
    assert_eq!(view.index, 1);
    assert!(view.editable);
    assert_eq!(view.raw, Some(RawValue::Unsigned(20)));
    assert_eq!(view.engineering, Some(10.0));
    workspace.apply_engineering_text("12.50").unwrap();
    assert_eq!(workspace.selected_axis_view().unwrap().raw, Some(RawValue::Unsigned(25)));
    workspace.undo().unwrap();
    assert_eq!(workspace.selected_axis_view().unwrap().raw, Some(RawValue::Unsigned(20)));
    workspace.select_cell(0, 0).unwrap();
    assert!(workspace.selected_axis.is_none());
}
```

- [ ] **Step 4: Run the workspace axis test and verify it fails.**

Run: `cargo test -p tuner-app workspace_axis_selection_reads_and_edits_the_axis_target -- --exact`

Expected: FAIL because the workspace has no axis target state or axis access methods.

- [ ] **Step 5: Implement target state using the test fixture.** Keep `AXIS_TABLE_XDF` beside the existing compact test XDF fixtures with X at `0x10`/8-bit/stride 8/conversion `X * 0.5`, Y at `0x20`/16-bit/stride 16/identity conversion, and Z payload at `0x30`. Add `AxisSelection` and `AxisView`, add `selected_axis` to `WorkspaceState`, clear it from `select_parameter` and `select_cell`, and validate axis/index in `select_axis`.

- [ ] **Step 6: Implement `axis_view_for` and selected-axis access.** For an address-backed axis, resolve the range and raw value through Task 1 and retain an engineering error in `AxisView.error` when conversion/read fails. For a descriptive axis, return its matching label, no range/raw/engineering value, `editable: false`, and no fabricated BIN value. If BIN is missing, return an unavailable view with a clear error. `selected_axis_view` must resolve the current selected parameter and selection.

- [ ] **Step 7: Branch `apply_engineering_text` through the axis transaction path.** Keep the existing finite-number parsing. If `selected_axis` is set, call `write_engineering_axis`, commit only on success, recompute validation, report the axis ID/index in the success message, and leave the transaction aborted on any error. If no axis is selected, preserve the current cell behavior. Keep `edit_text` and undo/redo semantics identical for both targets.

- [ ] **Step 8: Implement precision persistence and formatting.** Add `decimal_places` to `TableWindowMemory::default`, `sanitize`, every explicit test literal, and `default_table_window_memory` through struct update. Add `format_f64_with_precision` using fixed-point formatting at the clamped requested precision, make `format_f64` call it with `2`, and keep `raw_value_text` unchanged. `ProjectPreferences::sanitize` will sanitize all table memories so old project files migrate safely; Task 4 owns the separate `last_xdf_path` field.

- [ ] **Step 9: Run focused app tests and record the green checkpoint.**

Run: `cargo test -p tuner-app table_precision_defaults_to_two_and_clamps_old_or_invalid_values -- --exact`  
Run: `cargo test -p tuner-app workspace_axis_selection_reads_and_edits_the_axis_target -- --exact`

Expected: PASS, with one undo operation restoring the previous axis raw value and raw display remaining exact.

### Task 3: Render actual axis headers and complete table zoom

**Files:**
- Modify: `crates/tuner-app/src/lib.rs:3890-4115,5222-5245,5462-end`
- Test: `crates/tuner-app/src/lib.rs` unit-test module

**Interfaces:**
- Consumes: Task 2 `AxisView`, `AxisSelection`, precision formatter, and `TableWindowMemory::decimal_places`; existing `show_table_window`, `OpenTable`, `TableDisplay`, and scroll persistence.
- Produces:
  - `TableAxisRole::{X,Y}` and `table_axis_index(parameter, role, expected_count) -> Option<usize>`.
  - `table_axis_header_text(view: &AxisView, role: TableAxisRole, index: usize, display: TableDisplay, decimal_places: u8) -> String`.
  - `TableUiScale` and `table_ui_scale(zoom_percent: u16) -> TableUiScale` for deterministic content metrics.
  - A toolbar precision control that updates the active table’s `decimal_places` without changing BIN bytes.

- [ ] **Step 1: Write failing pure presentation tests.** Cover address-backed engineering headers, Raw headers, descriptive labels, unavailable mappings, role fallback, and zoom metrics.

```rust
#[test]
fn axis_header_text_uses_actual_values_and_table_precision() {
    let view = AxisView {
        axis_index: 0, index: 1, label: None,
        range: Some(ByteRange { start: 0x10, end: 0x11 }),
        raw: Some(RawValue::Unsigned(25)), engineering: Some(12.5),
        editable: true, error: None,
    };
    assert_eq!(table_axis_header_text(&view, TableAxisRole::X, 1, TableDisplay::Engineering, 2), "12.50");
    assert_eq!(table_axis_header_text(&view, TableAxisRole::X, 1, TableDisplay::Raw, 2), "u25");
    let label_view = AxisView { label: Some("Low load".to_string()), raw: None, engineering: None, range: None, editable: false, error: None, ..view.clone() };
    assert_eq!(table_axis_header_text(&label_view, TableAxisRole::X, 0, TableDisplay::Engineering, 2), "Low load");
}

#[test]
fn table_zoom_scales_content_metrics_without_changing_zoom_bounds() {
    let normal = table_ui_scale(100);
    let large = table_ui_scale(150);
    assert_eq!(normal.factor, 1.0);
    assert_eq!(large.factor, 1.5);
    assert!(large.cell_width > normal.cell_width);
    assert!(large.cell_height > normal.cell_height);
    assert!(large.body_font_size > normal.body_font_size);
    assert_eq!(table_ui_scale(10).factor, 0.5);
    assert_eq!(table_ui_scale(500).factor, 2.0);
}
```

- [ ] **Step 2: Run the presentation tests and verify they fail.**

Run: `cargo test -p tuner-app axis_header_text_uses_actual_values_and_table_precision -- --exact`  
Run: `cargo test -p tuner-app table_zoom_scales_content_metrics_without_changing_zoom_bounds -- --exact`

Expected: FAIL because the role/header/scale helpers do not exist and the grid still renders `C0`/`R0` labels.

- [ ] **Step 3: Implement deterministic axis-role and header helpers.** Implement `table_axis_index` to prefer IDs `x`, `column`, `columns` for X and `y`, `row`, `rows` for Y, requiring the expected dimension count. If explicit IDs are absent, use only the corresponding positional axis (`0` for X, `1` for Y) when its count matches; return `None` for a Z-only payload so table body data is never shown as a header. Implement `table_axis_header_text` so it chooses engineering or exact raw value, then label, then `C{index}`/`R{index}` fallback, and uses `unavailable` plus the error tooltip for failed mappings.

- [ ] **Step 4: Implement table zoom metrics and scoped egui style.** Add `TableUiScale` with factor, cell width/height, row-label width, body/title font sizes, toolbar spacing, and frame/button padding. Clone the current egui style inside the table window scope, scale text styles and spacing fields, use the scaled explicit dimensions for labels/buttons/text edits, and pass a scaled `RichText` title to the stable window ID. Keep `.resizable(true)`, `.movable(true)`, `.constrain_to(canvas_rect)`, `.max_size(canvas_rect.size())`, saved position/size, scroll restoration, and zoom persistence.

- [ ] **Step 5: Replace logical header labels with clickable axis values.** In `show_table_window`, resolve X/Y axis indices once per table, render each header as a selectable fixed-size button using `axis_view_for`, and record `(axis_index, index)` when clicked. On click, focus the table and call `workspace.select_axis`; retain the existing payload-cell click path. Highlight the selected axis separately from the selected payload cell and show read-only/unavailable tooltips for descriptive or failed axes.

- [ ] **Step 6: Make the editor target-aware and add per-table precision controls.** If the active selection is an axis, show its raw value and axis identity in the editor row; otherwise show the selected payload-cell raw value. Add `precision −`, current `N dp`, and `precision +` controls clamped to `0..=8`, store changes in the table’s `memory.decimal_places`, and use that value for payload cells, axis headers, and selected engineering context. Task 5 owns the corresponding debug/report fields. Do not mutate the global `preferences.table_display` when changing precision.

- [ ] **Step 7: Run focused UI/model tests and inspect the diff.**

Run: `cargo test -p tuner-app axis_header_text_uses_actual_values_and_table_precision -- --exact`  
Run: `cargo test -p tuner-app table_zoom_scales_content_metrics_without_changing_zoom_bounds -- --exact`  

Expected: PASS. Confirm the diff still uses stable table window IDs and no table click can edit a descriptive/no-address axis.

### Task 4: Remember and prompt for the last XDF per BIN

**Files:**
- Modify: `crates/tuner-app/src/lib.rs:251-344,1888-1978,2248-2288,2872-2908,4397-4482,5290-5460`
- Test: `crates/tuner-app/src/lib.rs` unit-test module

**Interfaces:**
- Consumes: existing `ProjectPreferences` persistence, `OperationCoordinator`, `OperationResult`, `OperationPayload`, and `OperationKind::LoadingXdf/LoadingBin/RestoringWorkspace`.
- Produces:
  - `ProjectPreferences::last_xdf_path: Option<PathBuf>`.
  - `PendingXdfRestore { bin_identity: String, path: PathBuf, available: bool }`.
  - `TunerApp::use_last_xdf() -> Result<(), WorkspaceError>`.
  - `TunerApp::keep_current_xdf()`.
  - A shared `start_xdf_load(path: PathBuf) -> Result<(), WorkspaceError>` used by the normal picker and reuse action.
  - `show_xdf_reuse_prompt(&mut self, context: &egui::Context)` with Use/Keep/Choose actions and a missing-file explanation.

- [ ] **Step 1: Write failing preference and prompt-decision tests.** Verify `ProjectPreferences` round-trips `last_xdf_path`, old JSON without it yields `None`, and a helper/state transition offers a prompt only when the active BIN identity has a remembered path different from the current XDF path.

```rust
#[test]
fn project_preferences_remember_last_xdf_without_breaking_old_files() {
    let mut preferences = ProjectPreferences::for_identity("bin-a");
    preferences.last_xdf_path = Some(PathBuf::from("C:/calibration/a.xdf"));
    let encoded = serde_json::to_string(&preferences).unwrap();
    let decoded: ProjectPreferences = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.last_xdf_path, preferences.last_xdf_path);
    let mut old_value = serde_json::to_value(ProjectPreferences::for_identity("bin-a")).unwrap();
    old_value.as_object_mut().unwrap().remove("last_xdf_path");
    let old: ProjectPreferences = serde_json::from_value(old_value).unwrap();
    assert_eq!(old.last_xdf_path, None);
}
```

- [ ] **Step 2: Run the persistence test and verify it fails.**

Run: `cargo test -p tuner-app project_preferences_remember_last_xdf_without_breaking_old_files -- --exact`

Expected: FAIL because `last_xdf_path` is not part of `ProjectPreferences` and old table-memory JSON does not yet migrate the new field.

- [ ] **Step 3: Write failing operation-state tests.** Use the existing `OperationResult::restored` constructor and an active `RestoringWorkspace` operation to test prompt creation, then assert `use_last_xdf()` starts an active `LoadingXdf` operation for the remembered path, `keep_current_xdf()` clears only the prompt, and a new BIN load clears a prompt whose identity no longer matches.

```rust
#[test]
fn last_xdf_prompt_is_project_scoped_and_use_starts_background_load() {
    let mut app = TunerApp::headless();
    app.project_identity = Some("bin-a".to_string());
    app.project_preferences = ProjectPreferences::for_identity("bin-a");
    app.project_preferences.last_xdf_path = Some(PathBuf::from("missing-a.xdf"));
    let restore_id = app.operations.begin(OperationKind::RestoringWorkspace, "bin-a");
    app.install_operation_result(OperationResult::restored(
        restore_id,
        "bin-a",
        app.project_preferences.clone(),
    ));
    assert_eq!(app.pending_xdf_restore.as_ref().unwrap().bin_identity, "bin-a");
    assert!(app.use_last_xdf().is_err());
    app.keep_current_xdf();
    assert!(app.pending_xdf_restore.is_none());
}
```

The test must use a temporary real XDF file for the successful Use case so `available == true` and the background worker has a valid path; the missing-path branch must assert the explicit error and no active load operation.

- [ ] **Step 4: Run the prompt tests and verify they fail.**

Run: `cargo test -p tuner-app last_xdf_prompt_is_project_scoped_and_use_starts_background_load -- --exact`

Expected: FAIL because no pending prompt state or reuse action exists.

- [ ] **Step 5: Implement persistence and successful-load recording.** Add `last_xdf_path: Option<PathBuf>` to `ProjectPreferences::default` and `sanitize`; update legacy/project preference copying only where the field belongs to project scope. Add `pending_xdf_restore: Option<PendingXdfRestore>` to both `TunerApp::new` and `TunerApp::headless`. In `install_operation_result`, update the project’s remembered path only in the successful `OperationPayload::Xdf` branch when a BIN project identity and nonempty XDF subject exist; mark project preferences dirty and persist them. Do not update it for errors or cancellations.

- [ ] **Step 6: Implement project-scoped prompt evaluation.** After a `Restored(project)` payload is installed and project tables are restored, compare `project.last_xdf_path` with `workspace.xdf_path` using canonicalized paths when files exist and normalized case/path text otherwise. Create `PendingXdfRestore` only for a different remembered path. Set `available` from `Path::is_file()`. Clear pending state whenever a new BIN load starts or an XDF load is explicitly selected, and check identity again immediately before acting.

- [ ] **Step 7: Implement the shared load action and prompt UI.** Move the existing `open_xdf_dialog` operation-start code into `start_xdf_load`, keep the busy-operation guard, and call it from both the picker and `use_last_xdf`. `use_last_xdf` must reject a stale/missing prompt with a clear status and must not start an operation. `keep_current_xdf` clears the prompt and reports that the current XDF was kept. Call `show_xdf_reuse_prompt` from the main `update` path after ordinary panels and before the operation overlay; render it as a non-blocking egui window with a visible path/missing warning, disabled Use button when unavailable, Keep current, and Choose XDF…. It is shown only after restoration is complete and cannot start conflicting document work while another operation is active.

- [ ] **Step 8: Run the prompt/persistence tests and record the green checkpoint.**

Run: `cargo test -p tuner-app project_preferences_remember_last_xdf_without_breaking_old_files -- --exact`  
Run: `cargo test -p tuner-app last_xdf_prompt_is_project_scoped_and_use_starts_background_load -- --exact`

Expected: PASS, with successful Use entering `OperationKind::LoadingXdf`, Keep clearing only pending prompt state, and stale/missing paths never being opened.

### Task 5: Preserve agent-visible axis metadata and diagnostics

**Files:**
- Modify: `crates/tuner-api/src/main.rs:1023-1075`
- Modify: `crates/tuner-cli/src/main.rs:454-505`
- Modify: `crates/tuner-app/src/lib.rs:1357-1562,2949-3040`
- Test: existing crate tests plus targeted serializer/debug assertions

**Interfaces:**
- Consumes: Task 1 `AxisDefinition::stride_bits`, Task 2 `AxisView`/selection, Task 3 precision memory, Task 4 `ProjectPreferences::last_xdf_path`, and the existing `tuner-api/v1`/CLI JSON schemas.
- Produces: additive `"stride_bits": <signed integer>` in every serialized axis object; reports that identify axis selection, precision, and mapping errors without UI-only state.

- [ ] **Step 1: Add failing serializer/debug assertions.** Extend the existing API/CLI inspection tests or their fixture assertions to require `stride_bits` in serialized axis JSON. Extend app debug tests to require axis selection/range/readability details and each open table’s `decimal_places`.

```rust
assert!(serialized_axis.contains("\"stride_bits\":8"));
assert!(report.contains("selected_axis:"));
assert!(report.contains("decimal_places: 2"));
```

- [ ] **Step 2: Run the targeted tests and verify they fail.**

Run: `cargo test -p tuner-api`  
Run: `cargo test -p tuner-cli`  
Run: `cargo test -p tuner-app debug_report_contains_document_identity_and_selection_details -- --exact`

Expected: FAIL because the manual serializers and reports do not include the new field/details.

- [ ] **Step 3: Add `stride_bits` to both manual axis JSON serializers.** Preserve all existing keys and append the signed field near `element_width_bits`/`address`. Do not change the protocol name, result schema, request actions, or meaning of existing fields. Update any string-format argument order and compile-time tests together.

- [ ] **Step 4: Update debug/report output.** Include axis stride and address/range in the selected-parameter axis lines, include the selected axis target when active, include precision in each table-memory line, and include `last_xdf_path` plus pending reuse identity/path in the app section. Use exact raw text and two-decimal engineering formatting unless the table-specific precision is available.

- [ ] **Step 5: Run serializer/debug tests and the API probe.**

Run: `cargo test -p tuner-api -p tuner-cli -p tuner-app`  
Run: `cargo run -p tuner-api -- --help`

Expected: PASS; help still advertises `tuner-api/v1` and the axis JSON is additive.

### Task 6: Integrate, format, build, and verify the complete slice

**Files:**
- Modify only files listed in Tasks 1–5 after reviewing the combined diff.
- Test: workspace test suites and bundled fixture probe.

**Interfaces:**
- Consumes: all green task checkpoints.
- Produces: a formatted, buildable app with verified axis fidelity, precision migration, full zoom metrics, project-scoped XDF reuse, and agent-readable diagnostics.

- [ ] **Step 1: Review all explicit struct literals and serialization defaults.**

Run: `rg -n "TableWindowMemory \{|ProjectPreferences \{|AxisDefinition \{" crates`

Update every constructor/test literal for the new fields, verify old JSON defaults to `decimal_places == 2` and `last_xdf_path == None`, and confirm no `unwrap` was added to user-controlled axis reads or prompt actions.

- [ ] **Step 2: Run formatting and focused regression suites.**

Run: `cargo fmt --all -- --check`  
Run: `cargo test -p tuner-xdf --lib`  
Run: `cargo test -p tuner-xdf --test real_fixture`  
Run: `cargo test -p tuner-app`

Expected: all commands PASS with no formatting changes required.

- [ ] **Step 3: Run the full workspace test suite and build.**

Run: `cargo test --workspace`  
Run: `cargo build --workspace`

Expected: all workspace tests/doc-tests pass and all binaries build successfully.

- [ ] **Step 4: Run the existing NookLink JSONL smoke probe against the bundled fixture.** Start `tuner-api` as a child process and send one JSON object per line:

```json
{"action":"capabilities","request_id":"axis-fidelity-capabilities"}
{"action":"inspect_xdf","request_id":"axis-fidelity-inspect","xdf":"Test bin and xdf/SCGa05_cal.xdf"}
{"action":"xdf_validate","request_id":"axis-fidelity-validate","xdf":"Test bin and xdf/SCGa05_cal.xdf","bin":"Test bin and xdf/SCGa05_cal.bin"}
```

Expected: capability status `ok`, protocol `tuner-api/v1`; XDF inspection includes signed `stride_bits`; validation status `ok`, parameter count `2915`, `category_reference_mode` `one-based-position`, `valid: true`, and zero blocking issues.

- [ ] **Step 5: Attempt a native visual smoke test only if a controllable desktop surface is available.** Open the built app, load the bundled BIN/XDF, confirm the first table’s X/Y headers show actual engineering values, click an axis header, change precision, zoom to 150%, move/resize the window, close/reopen it, and reopen the BIN to verify the XDF prompt. If the native surface is unavailable, report that limitation and rely on the passing model/API tests; do not claim visual verification.

- [ ] **Step 6: Final diff and artifact review.**

Run `git status --short` only if a repository is present; otherwise run `Get-ChildItem -LiteralPath 'docs/superpowers','crates' -Recurse -File | Where-Object { $_.Extension -in '.rs','.md' } | Select-Object -ExpandProperty FullName` and inspect the changed files. Confirm no source BIN/XDF fixture was modified and that the implementation matches the approved spec.
