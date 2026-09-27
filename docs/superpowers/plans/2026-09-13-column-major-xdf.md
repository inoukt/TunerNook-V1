# Column-Major XDF Storage Semantics Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

Goal: Correct XDF mmedtypeflags bit semantics, support column-major packed table addressing, and expose the corrected model through every existing application boundary.

Architecture: Keep the correction inside tuner-xdf's normalized storage and layout boundary. StorageSpec records the orientation and raw flags, DataLayout stores effective logical strides, and raw and engineering cell access continues to use the same checked transaction paths. The CLI, API, transfer planner, fixture test, and documentation consume the public model without duplicating flag interpretation.

Tech Stack: Stable Rust 1.98.1, dependency-free workspace crates, the existing hand-written XDF XML parser, tuner-core transactions, CLI/API JSON string serializers, and Cargo tests.

Spec: docs/superpowers/specs/2026-09-13-column-major-xdf-design.md

## Global Constraints

- 0x01 means signed integer storage, 0x02 means least-significant-byte-first, 0x04 means column-major layout, and 0x10000 identifies IEEE-754 binary32 only with a 32-bit element.
- Explicit type flags without the binary32 marker are integer storage; unknown bits remain diagnostics and are never guessed.
- Packed zero strides use row-major (columns times width, width) or column-major (width, rows times width) logical strides.
- Negative explicit strides remain checked signed arithmetic; ranges must be nonnegative, finite, and within the BIN address space.
- All writes remain transaction-scoped and no command overwrites an input BIN or XDF.
- Existing result schema identifiers and JSON fields remain compatible; new fields are additive.
- The workspace has no desktop tuner-app crate, so this plan updates the reusable backend and existing CLI/API surfaces only.
- The supplied Test bin and xdf files are read-only fixtures; their bytes and hashes must remain unchanged.

---

### Task 1: Update the normalized storage contract

Files:

- Create: docs/superpowers/specs/2026-09-13-column-major-xdf-design.md
- Modify: crates/tuner-xdf/src/lib.rs near StorageSpec and resolve_storage
- Test: crates/tuner-xdf/src/lib.rs unit-test module near the storage tests

Interfaces:

- Consumes: existing StorageSpec, DataLayout, NumericKind, and resolve_storage.
- Produces: StorageSpec { column_major: bool, ... }; NumericKind::Integer for explicit 0x04 and 0x06; NumericKind::Ieee754Binary32 only for valid 0x10000 width-32 storage.

- [x] Step 1: Write the failing storage assertions.

  Extend the existing storage test with:

~~~rust
let column_major = XdfDocument::parse(
    br#"<XDFFORMAT><XDFCONSTANT>
        <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8" mmedtypeflags="0x06" />
    </XDFCONSTANT></XDFFORMAT>"#,
)
.unwrap();
assert_eq!(
    column_major.parameters[0].layout.storage.numeric_kind,
    NumericKind::Integer
);
assert!(column_major.parameters[0].layout.storage.column_major);
assert!(float.parameters[0].layout.storage.column_major);
assert!(float
    .diagnostics
    .iter()
    .all(|diagnostic| diagnostic.code != "unsupported-storage"));
~~~

- [x] Step 2: Run the focused test to verify the old interpretation fails.

  Run:

~~~powershell
cargo test -p tuner-xdf --lib decodes_storage_flags_and_preserves_signed_strides -- --nocapture
~~~

  Expected result: failure because StorageSpec has no column_major field and
  the current 0x06 path is Unsupported.

- [x] Step 3: Change the public storage model and flag decoder.

  Add the field and replace the floating flag constant and branch with:

~~~rust
pub struct StorageSpec {
    pub element_size_bits: u32,
    pub signed: bool,
    pub byte_order: Endianness,
    pub numeric_kind: NumericKind,
    pub column_major: bool,
    pub raw_type_flags: u32,
    pub unknown_type_flags: u32,
}

const COLUMN_MAJOR_STORAGE_FLAG: u32 = 0x04;
const BINARY32_STORAGE_FLAG: u32 = 0x10000;
const KNOWN_STORAGE_FLAGS: u32 =
    SIGNED_STORAGE_FLAG | LSB_FIRST_STORAGE_FLAG
    | COLUMN_MAJOR_STORAGE_FLAG | BINARY32_STORAGE_FLAG;
~~~

  In the explicit-flag branch, set column_major from bit 0x04. Select
  binary32 only for the binary32 marker and a 32-bit width. Select Integer for
  every other explicit flag combination, including 0x04 and 0x06. In the
  fallback branch, set column_major to false.

- [x] Step 4: Run the focused storage tests.

~~~powershell
cargo test -p tuner-xdf --lib decodes_storage_flags_and_preserves_signed_strides -- --nocapture
cargo test -p tuner-xdf --lib flags_that_do_not_describe_supported_float_storage_are_diagnostic -- --nocapture
~~~

  Expected result: the storage test passes; wrong-width 0x10006 and unknown
  flag cases remain diagnostic.

### Task 2: Make packed cell ranges and cell access honor orientation

Files:

- Modify: crates/tuner-xdf/src/lib.rs near parse_parameter and range helpers
- Test: crates/tuner-xdf/src/lib.rs near packed-storage and cell-access tests

Interfaces:

- Consumes: StorageSpec.column_major, Dimensions, checked signed-stride arithmetic, cell_range, read_raw_cell, and write_raw_cell.
- Produces: effective logical row and column strides that map column-major packed tables as (row + column times rows) times width; all existing raw and engineering methods use those strides automatically.

- [x] Step 1: Write a failing non-square column-major read/write test.

~~~rust
#[test]
fn column_major_packed_cells_follow_logical_rows_and_columns() {
    let document = XdfDocument::parse(
        br#"
            <XDFFORMAT>
              <XDFTABLE uniqueid="column-major">
                <XDFAXIS id="z">
                  <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8"
                      mmedrowcount="2" mmedcolcount="3"
                      mmedmajorstridebits="0" mmedminorstridebits="0"
                      mmedtypeflags="0x06" />
                </XDFAXIS>
              </XDFTABLE>
            </XDFFORMAT>
        "#,
    )
    .unwrap();
    let parameter = &document.parameters[0];
    let bin = BinDocument::from_bytes(vec![10, 20, 30, 40, 50, 60]);

    assert_eq!(parameter.layout.row_stride_bits, 8);
    assert_eq!(parameter.layout.column_stride_bits, 16);
    assert_eq!(parameter.cell_range(1, 0).unwrap(), ByteRange { start: 1, end: 2 });
    assert_eq!(parameter.cell_range(0, 1).unwrap(), ByteRange { start: 2, end: 3 });
    assert_eq!(parameter.read_raw_cell(&bin, 1, 0).unwrap(), RawValue::Unsigned(20));
    assert_eq!(parameter.read_raw_cell(&bin, 0, 1).unwrap(), RawValue::Unsigned(30));

    let mut bin = bin;
    {
        let mut transaction = bin.transaction("column-major cell edit");
        parameter
            .write_raw_cell(&mut transaction, 1, 2, RawValue::Unsigned(99))
            .unwrap();
        transaction.commit().unwrap();
    }
    assert_eq!(bin.bytes(), &[10, 20, 30, 40, 50, 99]);
}
~~~

- [x] Step 2: Run the new test to verify the old path fails.

~~~powershell
cargo test -p tuner-xdf --lib column_major_packed_cells_follow_logical_rows_and_columns -- --nocapture
~~~

  Expected result: failure because 0x06 is currently unsupported and the
  zero-stride fallback currently derives row-major strides.

- [x] Step 3: Derive orientation-aware packed strides.

  In parse_parameter, calculate packed logical strides as:

~~~rust
let packed_row_stride_bits = if storage.column_major {
    checked_stride_from_usize(&object, width_bits)?
} else {
    checked_stride_from_usize(
        &object,
        width_bits
            .checked_mul(dimensions.columns)
            .ok_or_else(|| XdfError::Overflow {
                object: object.clone(),
                message: "packed row stride overflows usize".to_string(),
            })?,
    )?
};
let packed_column_stride_bits = if storage.column_major {
    checked_stride_from_usize(
        &object,
        width_bits
            .checked_mul(dimensions.rows)
            .ok_or_else(|| XdfError::Overflow {
                object: object.clone(),
                message: "packed column stride overflows usize".to_string(),
            })?,
    )?
} else {
    checked_stride_from_usize(&object, width_bits)?
};
let (row_stride_bits, column_stride_bits) =
    if explicit_row_stride_bits == Some(0) && explicit_column_stride_bits == Some(0) {
        (packed_row_stride_bits, packed_column_stride_bits)
    } else {
        (
            explicit_row_stride_bits.unwrap_or(packed_row_stride_bits),
            explicit_column_stride_bits.unwrap_or(packed_column_stride_bits),
        )
    };
~~~

  Keep cell_bit_bounds and calculate_range expressed in effective logical
  strides. Do not swap coordinates a second time in raw access. Include
  column_major in canonical_storage.

- [x] Step 4: Run focused layout and safety tests.

~~~powershell
cargo test -p tuner-xdf --lib column_major_packed_cells_follow_logical_rows_and_columns -- --nocapture
cargo test -p tuner-xdf --lib zero_table_strides_describe_packed_storage -- --nocapture
cargo test -p tuner-xdf --lib cell_ranges_and_typed_reads_follow_strides -- --nocapture
cargo test -p tuner-xdf --lib -- --nocapture
~~~

  Expected result: the new column-major mapping passes, the existing
  row-major default test retains its current range, and all XDF unit tests
  pass.

### Task 3: Propagate corrected semantics through application boundaries

Files:

- Modify: crates/tuner-cli/src/main.rs serializer helpers and tests
- Modify: crates/tuner-api/src/main.rs serializer helpers and tests
- Modify: crates/tuner-transfer/src/lib.rs storage compatibility tests if needed
- Modify: crates/tuner-xdf/tests/real_fixture.rs
- Modify: README.md
- Modify: docs/superpowers/specs/2026-09-12-engineering-conversion-design.md
- Modify: docs/superpowers/plans/2026-09-12-xdf-interpretation.md

Interfaces:

- Consumes: public StorageSpec.column_major, corrected NumericKind, existing JSON serializers, transfer canonical storage, and the real fixture.
- Produces: additive column_major JSON fields; a fixture assertion that all ordinary 0x06 data is integer; documentation with no contradictory 0x04 storage description.

- [x] Step 1: Add failing JSON and fixture assertions.

  In the existing CLI and API serializer tests, assert:

~~~rust
assert!(serialized.contains(r#""column_major":true"#));
assert!(serialized.contains(r#""numeric_kind":"integer""#));
~~~

  In real_fixture.rs, collect all axes and assert:

~~~rust
assert_eq!(
    axes.iter()
        .filter(|axis| axis.storage.as_ref().is_some_and(|storage| {
            storage.raw_type_flags == 0x06
                && storage.numeric_kind == NumericKind::Integer
                && storage.column_major
        }))
        .count(),
    2_896
);
assert_eq!(
    axes.iter()
        .filter(|axis| axis.storage.as_ref().is_some_and(|storage| {
            storage.raw_type_flags == 0x10006
                && storage.numeric_kind == NumericKind::Ieee754Binary32
                && storage.column_major
        }))
        .count(),
    19
);
assert!(xdf
    .diagnostics
    .iter()
    .all(|diagnostic| diagnostic.code != "unsupported-storage"));
~~~

- [x] Step 2: Run focused boundary tests.

~~~powershell
cargo test -p tuner-cli
cargo test -p tuner-api --bin tuner-api
cargo test -p tuner-xdf --test real_fixture -- --nocapture
~~~

  Expected result: a compilation or assertion failure until the new field and
  corrected fixture classification are serialized.

- [x] Step 3: Serialize orientation in CLI and API storage JSON.

  Extend both storage_json helpers with the additive key:

~~~rust
format!(
    r#"{{"element_size_bits":{},"signed":{},"byte_order":"{}","numeric_kind":"{}","column_major":{},"raw_type_flags":{},"unknown_type_flags":{}}}"#,
    storage.element_size_bits,
    storage.signed,
    endianness_name(storage.byte_order),
    storage.numeric_kind.as_str(),
    storage.column_major,
    storage.raw_type_flags,
    storage.unknown_type_flags
)
~~~

  Preserve all existing parameter and axis keys and schema names. Update
  canonical_storage to include column_major.

- [x] Step 4: Update fixture, transfer, README, spec, and plan expectations.

  Keep the transfer mismatch test for different numeric kinds and raw flags;
  add an identical-0x06 check if the existing helper can express it, proving
  corrected integer storage does not self-block. Change README, the earlier
  engineering-conversion spec, and the earlier plan so every reference states
  that 0x04 is column-major and 0x10000 identifies binary32.

- [x] Step 5: Run boundary tests and inspect corrected output.

~~~powershell
cargo test -p tuner-cli
cargo test -p tuner-api --bin tuner-api
cargo test -p tuner-transfer --lib
cargo test -p tuner-xdf --test real_fixture -- --nocapture
cargo run --quiet -p tuner-cli -- inspect-xdf "Test bin and xdf/SCGa05_cal.xdf"
~~~

  Expected result: CLI JSON reports integer 0x06 storage with column_major
  true, 19 binary32 axes remain, and no ordinary fixture axis is unsupported.

### Task 4: Complete application-level verification and delivery notes

Files:

- Modify: docs/superpowers/specs/2026-09-13-column-major-xdf-design.md
- Modify: docs/superpowers/plans/2026-09-13-column-major-xdf.md
- Modify: README.md only if command output requires a corrected example

Interfaces:

- Consumes: all corrected parser, cell, serializer, transfer, and fixture behavior.
- Produces: passing workspace verification, unchanged fixture hashes, and written artifacts marked complete only after evidence is captured.

- [x] Step 1: Run formatting, build, and complete tests.

~~~powershell
cargo fmt --all -- --check
cargo build --workspace
cargo test --workspace
~~~

  Expected result: all commands exit 0 with no new unused-field warnings.

- [x] Step 2: Validate the supplied BIN/XDF and capture diagnostics.

~~~powershell
cargo run --quiet -p tuner-cli -- xdf-validate "Test bin and xdf/SCGa05_cal.xdf" "Test bin and xdf/SCGa05_cal.bin"
~~~

  Confirm valid:true, issue_count:0, parameter_count:2915, and
  bin_size_bytes:654336. Inspect XDF output and confirm parameter_count:2915,
  category_count:57, diagnostic_count:763, and auxiliary_object_count:0.

- [x] Step 3: Confirm supplied fixture hashes are unchanged.

~~~powershell
Get-FileHash -Algorithm SHA256 "Test bin and xdf\SCGa05_cal.xdf"
Get-FileHash -Algorithm SHA256 "Test bin and xdf\SCGa05_cal.bin"
~~~

  Expected hashes:

The local SCGa05 fixture XDF/BIN hashes were verified against their recorded
baseline; values are omitted from this public plan.

- [x] Step 4: Self-review and mark the written artifacts complete.

  Verify every checkbox is checked, all field names match the Rust model and
  serializers, and no documentation presents 0x04 as a storage-type marker. After
  verification passes, change this spec status to Implemented and mark every
  plan step [x].

## Plan self-review checklist

- Spec coverage: Task 1 covers the flag contract and public model; Task 2 covers packed orientation and checked cell access; Task 3 covers CLI, API, transfer, fixture, and documentation propagation; Task 4 covers full verification and unchanged inputs.
- Type consistency: StorageSpec.column_major is introduced before parser, canonical identity, serializers, and fixture consumers use it; DataLayout continues to expose effective logical row and column strides.
- Safety review: explicit unsupported binary32 widths remain diagnostics, unknown bits remain visible, signed arithmetic remains checked, all writes use existing transactions, and supplied fixture files are never edited.
