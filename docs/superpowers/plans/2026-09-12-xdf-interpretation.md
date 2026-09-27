# XDF Interpretation and Engineering-Unit Conversion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the current tolerant XDF parser into a typed, storage-aware normalization layer and add safe engineering-unit reads and writes for integer and IEEE-754 binary32 cells.

**Architecture:** Add a dependency-free `tuner-cal` crate for safe arithmetic conversion parsing, evaluation, and restricted structural inversion. Extend `tuner-xdf` with header/default/category/axis/object normalization, checked address and stride resolution, storage-flag decoding, and transaction-backed cell access; keep `tuner-transfer`, the CLI, and the API compatible while exposing the new metadata and rejecting unsafe storage mismatches.

**Tech Stack:** Rust 2021, stable Rust toolchain, workspace crates, standard library only in `tuner-cal`, existing `tuner-core` byte and transaction primitives, inline XML fixtures plus the supplied `SCGa05_cal.xdf`/`SCGa05_cal.bin` integration fixture.

**Spec:** `docs/superpowers/specs/2026-09-12-engineering-conversion-design.md`

## Global Constraints

- Do not execute arbitrary XDF text or delegate formula evaluation to a scripting engine.
- `tuner-cal` has no workspace dependencies; `tuner-xdf` may depend on `tuner-cal`, and `tuner-cal` must never depend on `tuner-xdf`.
- Apply explicit axis data over containing-object data, and containing-object data over header defaults; retain the selected source for diagnostics.
- Translate XDF addresses through one checked `BASEOFFSET` mapper; reject underflow, overflow, and negative final BIN positions.
- Preserve positive and negative bit strides; use packed fallback only when both table strides are explicitly zero and the table shape requires it.
- Interpret `mmedtypeflags` low bits `0x01` signed, `0x02` least-significant-byte-first, `0x04` column-major, and recognize `0x10000` with a 32-bit element as IEEE-754 binary32. The later column-major correction plan supersedes the earlier interpretation of `0x04`.
- Never reinterpret unknown or ambiguous storage flags as an integer mapping; return an explicit capability diagnostic.
- Keep units, decimal places, `outputtype`, `datatype`, `min`, and `max` as metadata unless a caller explicitly formats a value.
- All BIN writes occur only through an already-open `tuner-core::Transaction`; failed conversion or representability checks must leave the BIN unchanged.
- Integer engineering writeback accepts only a finite raw result within `1e-9 * max(1.0, abs(raw))` of `raw.round()`.
- Binary32 engineering writeback accepts only a finite `f64` that narrows to a finite `f32`, and returns the stored value plus post-write engineering value so quantization is visible.
- Do not modify `Test bin and xdf/SCGa05_cal.bin` or `Test bin and xdf/SCGa05_cal.xdf`.
- This workspace is not a Git repository; use formatting, unit, integration, CLI, and API checkpoints instead of commit commands.

---

## Files and responsibilities

Create:

- `crates/tuner-cal/Cargo.toml` — dependency-free calibration crate manifest.
- `crates/tuner-cal/src/lib.rs` — safe expression tokenizer, AST, evaluator, and structural inverse.
- `crates/tuner-xdf/tests/real_fixture.rs` — regression test for the supplied BIN/XDF counts, storage flags, and mapping.

Modify:

- `Cargo.toml` — add `crates/tuner-cal` to the workspace.
- `Cargo.lock` — update through Cargo after adding the workspace member.
- `crates/tuner-xdf/Cargo.toml` — depend on the local `tuner-cal` crate.
- `crates/tuner-xdf/src/lib.rs` — add typed header and metadata models, diagnostics, signed layout arithmetic, storage decoding, object collection, float32 raw access, and engineering-aware cell methods. Existing unit tests remain in this file because the crate currently uses that pattern.
- `crates/tuner-transfer/src/lib.rs` — compare normalized numeric storage kind and raw type semantics before allowing byte-copy transfer.
- `crates/tuner-cli/src/main.rs` — include normalized header/category/diagnostic/storage metadata in `inspect-xdf` JSON while preserving existing fields.
- `crates/tuner-api/src/main.rs` — expose the same normalized metadata in the API result.
- `README.md` — document the normalized XDF inspection fields and the safe engineering-unit boundary.

The plan intentionally keeps the existing XML parser in `tuner-xdf/src/lib.rs`; it will gain focused helpers rather than being replaced by an XML dependency. No supplied calibration files are fixtures under source control, so the integration test resolves their existing workspace paths without copying or editing them.

## Task 1: Add the safe calibration crate

**Files:**

- Create: `crates/tuner-cal/Cargo.toml`
- Create: `crates/tuner-cal/src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

**Interfaces:**

- Produces `tuner_cal::Conversion` with `parse`, `evaluate`, `invert`, and `source` methods.
- Produces `tuner_cal::ConversionError` variants for parse position, unknown identifiers, non-finite input/result, division by zero, and non-invertible formulas.
- Consumes only `&str` and finite `f64`; it does not import `tuner-xdf` or `tuner-core`.

- [x] **Step 1: Scaffold the workspace member and failing public API tests.**

  Add the manifest with no `[dependencies]` section, add the member to the root workspace, and create `src/lib.rs` containing tests that refer to the required API:

  ```rust
  #[test]
  fn identity_and_precedence_are_evaluable() {
      let conversion = Conversion::parse("2 * X + 1").unwrap();
      assert_eq!(conversion.evaluate(3.0).unwrap(), 7.0);
  }

  #[test]
  fn linear_fractional_conversion_round_trips() {
      let conversion = Conversion::parse("(2 * X + 4) / (3 * X + 5)").unwrap();
      let engineering = conversion.evaluate(2.0).unwrap();
      assert!((conversion.invert(engineering).unwrap() - 2.0).abs() < 1e-12);
  }
  ```

- [x] **Step 2: Run the focused test command and record the expected compile failure.**

  Run:

  ```powershell
  cargo test -p tuner-cal --lib
  ```

  Expected result: compilation fails because `Conversion` and its methods have not been defined.

- [x] **Step 3: Implement the minimal parser, evaluator, and inverse.**

  Define the public surface:

  ```rust
  pub struct Conversion { source: String, expression: Expr }

  pub enum ConversionError {
      Parse { position: usize, message: String },
      UnknownIdentifier { position: usize, identifier: String },
      NonFiniteInput,
      DivisionByZero,
      NonFiniteResult,
      NotInvertible,
  }

  impl Conversion {
      pub fn parse(source: &str) -> Result<Self, ConversionError>;
      pub fn evaluate(&self, raw: f64) -> Result<f64, ConversionError>;
      pub fn invert(&self, engineering: f64) -> Result<f64, ConversionError>;
      pub fn source(&self) -> &str;
  }
  ```

  Implement recursive-descent precedence for numbers, `X`, parentheses, unary signs, `+`, `-`, `*`, and `/`; reject all other identifiers; check every operation for finite output; and recognize identity, affine, and linear-fractional coefficient forms without numerical iteration.

- [x] **Step 4: Add the complete focused test matrix and verify it passes.**

  Add tests for scientific notation, unary signs, parentheses, unknown identifiers, malformed input, division by zero, non-finite input/result, affine inverse, linear-fractional inverse, zero inverse denominators, constant expressions, and unsupported nonlinear inverse shapes. Run:

  ```powershell
  cargo test -p tuner-cal --lib
  cargo fmt --all -- --check
  ```

  Expected result: all calibration tests pass and formatting is clean.

## Task 2: Normalize header, address space, categories, and diagnostics

**Files:**

- Modify: `crates/tuner-xdf/src/lib.rs:24-110, 466-585, 691-790, 1311-1418`

**Interfaces:**

- Produces `XdfHeader`, `XdfDefaults`, `XdfBaseOffset`, `XdfRegion`, `XdfCategory`, `CategoryMembership`, `DiagnosticSeverity`, and `XdfDiagnostic` public types.
- Produces `XdfBaseOffset::translate(&self, xdf_address: u64) -> Result<usize, XdfError>`.
- Extends `XdfDocument` with `header`, `categories`, and `diagnostics` while keeping `parameters`, exact hashing, normalized fingerprinting, and `unknown_element_count` available.
- Extends `ParameterDefinition` with ordered `category_memberships`; the existing optional `category` string remains as the first resolved category for compatibility.
- `parse` returns fatal XML/field errors through `XdfError`; recoverable unsupported/missing-reference conditions are attached to `diagnostics`.

- [x] **Step 1: Add failing header and category tests.**

  Add inline XML tests with the following assertions:

  ```rust
  let document = XdfDocument::parse(br#"
      <XDFFORMAT version="1.70">
        <XDFHEADER>
          <DEFAULTS datasizeinbits="16" sigdigits="4" outputtype="1" signed="0" lsbfirst="1" float="0" />
          <BASEOFFSET offset="0x2000" subtract="0" />
          <CATEGORY index="0x2" name="Fuel" />
        </XDFHEADER>
        <XDFCONSTANT uniqueid="value">
          <title>Value</title>
          <CATEGORYMEM index="0" category="2" />
          <EMBEDDEDDATA mmedaddress="0x10" />
        </XDFCONSTANT>
      </XDFFORMAT>
  "#).unwrap();

  assert_eq!(document.header.defaults.data_size_bits, Some(16));
  assert_eq!(document.header.base_offset.translate(0x10).unwrap(), 0x2010);
  assert_eq!(document.categories[0].name, "Fuel");
  assert_eq!(document.parameters[0].category_memberships[0].category_index, 2);
  assert_eq!(document.parameters[0].category_memberships[0].category_name.as_deref(), Some("Fuel"));
  ```

  Add a subtracting-base-offset case, invalid boolean/numeric cases, duplicate category index, missing category reference, and an object-outside-region diagnostic test.

- [x] **Step 2: Run the focused XDF tests and verify they fail for the missing model.**

  Run:

  ```powershell
  cargo test -p tuner-xdf --lib header_and_category
  ```

  Expected result: compilation fails because the new header fields and category membership fields do not exist.

- [x] **Step 3: Implement typed header parsing and one checked address mapper.**

  Add models with these fields:

  ```rust
  pub struct XdfDefaults {
      pub data_size_bits: Option<u32>,
      pub significant_digits: Option<u32>,
      pub output_type: Option<u32>,
      pub signed: Option<bool>,
      pub lsb_first: Option<bool>,
      pub float: Option<bool>,
  }

  pub struct XdfBaseOffset {
      pub offset: u64,
      pub subtract: bool,
  }

  impl XdfBaseOffset {
      pub fn translate(&self, xdf_address: u64) -> Result<usize, XdfError>;
  }

  pub struct XdfDiagnostic {
      pub severity: DiagnosticSeverity,
      pub code: String,
      pub path: String,
      pub message: String,
  }
  ```

  Add `XdfHeader.raw_fields: BTreeMap<String, String>` for source text. Parse decimal, `0x`/`0X`, and `$` numeric forms; accept only documented boolean forms (`0`, `1`, `true`, `false`); preserve raw source strings in the header metadata map; compute the signed offset as `+offset` or `-offset`; and use checked arithmetic for the final `usize` address.

- [x] **Step 4: Attach memberships, region checks, and recoverable diagnostics without changing fatal mapping behavior.**

  Parse the header category catalog before parameters, store each `CATEGORYMEM` slot and referenced index in order, resolve names by numeric index, and add diagnostics for duplicates, missing references, invalid region placement, and unknown attributes/elements. Keep the existing unknown-element count as a compatibility field.

- [x] **Step 5: Run the task test cycle and the existing regression suite.**

  Run:

  ```powershell
  cargo test -p tuner-xdf --lib
  cargo test --workspace
  cargo fmt --all -- --check
  ```

  Expected result: new header/category tests and every existing parser, range, identity, raw-cell, and bitfield test pass.

## Task 3: Decode storage flags and support signed layout arithmetic

**Files:**

- Modify: `crates/tuner-xdf/src/lib.rs:109-456, 791-1170`
- Modify: `crates/tuner-transfer/src/lib.rs:620-700`

**Interfaces:**

- Produces `NumericKind::{Integer, Ieee754Binary32, Unsupported}` and `StorageSpec { element_size_bits, signed, byte_order, numeric_kind, raw_type_flags, unknown_type_flags }`.
- Changes `DataLayout.row_stride_bits` and `DataLayout.column_stride_bits` to signed bit counts while keeping `address`, `element_width_bits`, `signed`, and `endianness` available for current callers.
- Adds `DataLayout.storage: StorageSpec` and `AxisDefinition.storage: Option<StorageSpec>`; descriptive axes without storage use `None`.
- Produces checked layout calculation using `i128` intermediates and returns `XdfError` for negative final positions, arithmetic overflow, invalid width, or a cell outside the resolved range.
- Produces a transfer planning rejection when source and destination numeric kind or raw storage flags differ.

- [x] **Step 1: Add failing storage and stride tests.**

  Add inline XML tests for the following cases:

  ```rust
  let integer = XdfDocument::parse(br#"<XDFFORMAT><XDFCONSTANT>
      <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="16" mmedtypeflags="0x03" />
  </XDFCONSTANT></XDFFORMAT>"#).unwrap();
  assert_eq!(integer.parameters[0].layout.signed, true);

  let float = XdfDocument::parse(br#"<XDFFORMAT><XDFCONSTANT>
      <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="32" mmedtypeflags="0x10006" />
  </XDFCONSTANT></XDFFORMAT>"#).unwrap();
  assert_eq!(float.parameters[0].layout.storage.numeric_kind, NumericKind::Ieee754Binary32);
  ```

  Add tests for `0x10006` with a non-32-bit width, unknown flag bits, negative major stride, negative minor stride, explicit packed zero strides, stride overflow, and final-address underflow. Add a transfer test where otherwise identical parameters differ only by numeric kind or raw type flags and assert a blocking issue.

- [x] **Step 2: Run the focused tests and verify the pre-implementation failures.**

  Run:

  ```powershell
  cargo test -p tuner-xdf --lib storage_flags
  cargo test -p tuner-transfer --lib storage_kind
  ```

  Expected result: compilation fails for the new storage model and signed-stride assertions.

- [x] **Step 3: Implement effective storage decoding and preserve compatibility fields.**

  Decode `mmedtypeflags` with a single helper. Set signed from `0x01`, endianness from `0x02`, column-major layout from `0x04`, and `Ieee754Binary32` only for `0x10000` plus a 32-bit element. Preserve all raw bits and calculate `unknown_type_flags` from bits outside the supported mask. Use explicit axis/object/default precedence; explicit `signed`/`endianness` attributes remain supported as compatibility input when no type flags are present.

- [x] **Step 4: Replace unsigned stride arithmetic with checked signed arithmetic.**

  Implement the cell start calculation as:

  ```rust
  let start_bit = i128::from(base_bit)
      + i128::from(row as i64) * i128::from(row_stride_bits)
      + i128::from(column as i64) * i128::from(column_stride_bits);
  let end_bit = start_bit + i128::from(element_width_bits as u64);
  ```

  Reject values outside `0..=usize::MAX * 8`, convert only after validation, calculate the complete half-open `ByteRange`, and retain the packed fallback only for explicit `0/0` table strides. Update semantic fingerprint material to include numeric kind, raw flags, and signed strides.

- [x] **Step 5: Update transfer compatibility checks and run all affected tests.**

  Add a dedicated transfer issue message for numeric-kind/raw-flag mismatch, keep existing width/signedness/endianness/stride checks, and run:

  ```powershell
  cargo test -p tuner-xdf --lib
  cargo test -p tuner-transfer --lib
  cargo test --workspace
  cargo fmt --all -- --check
  ```

  Expected result: existing table/bitfield mapping behavior remains valid, negative-stride cases are checked, and unsafe cross-kind transfers are blocked.

## Task 4: Preserve axis metadata and common XDF object variants

**Files:**

- Modify: `crates/tuner-xdf/src/lib.rs:109-245, 691-1100, 1429-1464`

**Interfaces:**

- Extends `AxisDefinition` with `AxisMetadata`, ordered `AxisLabel`, `AxisLink`, and `EmbedInfo` values while retaining address/count/range/conversion compatibility fields.
- Extends `ParameterDefinition` with original bitfield mask/source metadata; ordered `category_memberships` is defined in Task 2.
- Extends `ParameterKind` with `Flag` and adds `XdfAuxiliaryObject` records for `XDFFUNCTION`, `XDFPATCH`, `XDFCHECKSUM`, and unknown/vendor object names.
- `XdfDocument` exposes `auxiliary_objects` and diagnostics for unresolved links rather than dropping descriptive axes.

- [x] **Step 1: Add failing axis, label, link, and object-variant tests.**

  Add an inline table fixture containing one descriptive x axis, one addressed y axis, z body data, labels with a gap, `DALINK`, `embedinfo`, and an `XDFFLAG`; assert that the descriptive axis has `address == None`, labels retain source indices, links retain raw IDs, and the flag is classified separately from an ordinary table. Add a fixture with `XDFFUNCTION`, `XDFPATCH`, and `XDFCHECKSUM` and assert they appear in `auxiliary_objects` with their source names and titles.

- [x] **Step 2: Run the focused test and verify the missing metadata failures.**

  Run:

  ```powershell
  cargo test -p tuner-xdf --lib axis_metadata
  cargo test -p tuner-xdf --lib object_variants
  ```

  Expected result: compilation fails because labels, links, auxiliary objects, and the flag kind are not yet modeled.

- [x] **Step 3: Implement typed axis metadata and descriptive-axis handling.**

  Add these fields and remove only the `Eq` derives that become impossible because `AxisMetadata` contains `f64`; retain `PartialEq` for all model structs used by tests:

  ```rust
  pub struct AxisLabel { pub index: usize, pub value: String }

  pub struct AxisMetadata {
      pub units: Option<String>,
      pub unit_type: Option<u32>,
      pub decimal_places: Option<u32>,
      pub min: Option<f64>,
      pub max: Option<f64>,
      pub output_type: Option<u32>,
  }

  pub struct AxisLink {
      pub index: Option<usize>,
      pub object_id_hash: Option<String>,
  }
  ```

  Parse axis fields case-insensitively, keep missing-address axes as metadata-only, parse label indices without reordering their source values, and emit a diagnostic for gaps, duplicate indices, and unresolved object references. Use only addressed axes for BIN range validation.

- [x] **Step 4: Collect flags and retain non-cell-bearing object records.**

  Collect `XDFFLAG` as `ParameterKind::Flag`, derive a bit range from explicit `bitoffset`/`bitwidth` or a supported mask, and preserve the original mask. Collect `XDFFUNCTION`, `XDFPATCH`, and `XDFCHECKSUM` into `XdfAuxiliaryObject { kind, unique_id, title, description, path, attributes }`; do not force their axes or patch/checksum records into ordinary table-cell layouts. Record unknown top-level objects and their paths in diagnostics.

- [x] **Step 5: Run parser regressions and inspect the real XDF counts.**

  Run:

  ```powershell
  cargo test -p tuner-xdf --lib
  cargo run --quiet -p tuner-cli -- inspect-xdf "Test bin and xdf/SCGa05_cal.xdf"
  ```

  Expected result: existing tests pass; the real definition still contains 2,915 editable table parameters, while its category and axis metadata is now available in memory and its descriptive axes do not create false address errors.

## Task 5: Add binary32 raw access and engineering-unit cell operations

**Files:**

- Modify: `crates/tuner-xdf/Cargo.toml`
- Modify: `crates/tuner-xdf/src/lib.rs:179-456`

**Interfaces:**

- Extends `RawValue` with `Float32Bits(u32)` so NaN and infinity can be preserved for raw diagnostics without routing floats through integer APIs.
- Adds `EngineeringWriteResult { requested_engineering: f64, raw_value: RawValue, stored_engineering: f64 }`.
- Extends `CellAccessError` with `Conversion(tuner_cal::ConversionError)` and keeps its display/error implementation structured.
- Adds `ParameterDefinition::compile_conversion(&self) -> Result<tuner_cal::Conversion, CellAccessError>`.
- Adds `ParameterDefinition::read_engineering_cell(&self, bin: &BinDocument, row: usize, column: usize) -> Result<f64, CellAccessError>`.
- Adds `ParameterDefinition::write_engineering_cell(&self, transaction: &mut Transaction<'_>, row: usize, column: usize, engineering: f64) -> Result<EngineeringWriteResult, CellAccessError>`.
- Keeps `read_raw_cell` and `write_raw_cell` available for integer and float raw inspection, and preserves bitfield read-modify-write behavior.

- [x] **Step 1: Wire the local calibration dependency and add failing integration tests.**

  Add `tuner-cal = { path = "../tuner-cal" }` to `crates/tuner-xdf/Cargo.toml`. Define a test-only `parse_constant(address, width, type_flags, equation)` helper that builds the inline `XDFCONSTANT` XML used by the following in-memory BIN tests, then add the tests:

  ```rust
  #[test]
  fn engineering_integer_write_is_integral_and_atomic() {
      let document = parse_constant("0x00", "16", "0x02", "2 * X + 1");
      let parameter = &document.parameters[0];
      let mut bin = BinDocument::from_bytes(vec![2, 0]);
      let before = bin.bytes().to_vec();
      let mut transaction = bin.transaction("engineering edit");
      let result = parameter.write_engineering_cell(&mut transaction, 0, 0, 5.0).unwrap();
      assert_eq!(result.raw_value, RawValue::Unsigned(2));
      transaction.commit().unwrap();
      assert_ne!(bin.bytes(), before.as_slice());
  }

  #[test]
  fn binary32_read_and_write_use_ieee_bits() {
      let document = parse_constant("0x00", "32", "0x10006", "X / 2");
      let parameter = &document.parameters[0];
      let mut bin = BinDocument::from_bytes(1.5f32.to_le_bytes().to_vec());
      assert_eq!(parameter.read_engineering_cell(&bin, 0, 0).unwrap(), 0.75);
      let mut transaction = bin.transaction("float engineering edit");
      let result = parameter.write_engineering_cell(&mut transaction, 0, 0, 1.25).unwrap();
      assert_eq!(result.raw_value, RawValue::Float32Bits(2.5f32.to_bits()));
      transaction.commit().unwrap();
  }
  ```

  Add tests that a fractional integer inverse leaves bytes unchanged, a non-finite/overflowing float is rejected, and a raw float NaN is exposed as bits but rejected by engineering read.

- [x] **Step 2: Run the focused tests and verify failures before implementation.**

  Run:

  ```powershell
  cargo test -p tuner-xdf --lib engineering_integer
  cargo test -p tuner-xdf --lib binary32
  ```

  Expected result: compilation fails because the calibration dependency, float raw variant, and engineering helpers do not exist.

- [x] **Step 3: Implement storage-aware raw reads and writes.**

  For integer storage, keep the existing checked core readers and writers. For binary32 storage, read exactly four bytes through `BinDocument::read_bytes`, assemble the `u32` according to `Endianness`, and return `RawValue::Float32Bits(bits)`. Write float bits through `Transaction::write_bytes` after validating the range and byte order. Reject `RawValue::Float32Bits` for integer layouts and integer variants for float layouts with `CellAccessError::InvalidValue` or `UnsupportedLayout`.

- [x] **Step 4: Implement conversion-aware reads and writes without implicit display transforms.**

  Convert `Unsigned`, `Signed`, and finite `Float32Bits` to `f64`, call `Conversion::evaluate`, and map conversion errors into `CellAccessError::Conversion`. For writeback, call `Conversion::invert`; for integer storage, enforce the exact integral tolerance and use the existing core writer; for binary32 storage, cast only after checking finite `f32` output and return `EngineeringWriteResult` with the post-write conversion result. Perform all validation before the first transaction write.

- [x] **Step 5: Run raw-cell, conversion, and full workspace tests.**

  Run:

  ```powershell
  cargo test -p tuner-xdf --lib
  cargo test --workspace
  cargo fmt --all -- --check
  ```

  Expected result: integer, bitfield, float32, conversion, undo, and atomic-failure tests pass without changing supplied files.

## Task 6: Expose normalized metadata and protect transfer planning

**Files:**

- Modify: `crates/tuner-transfer/src/lib.rs:620-700`
- Modify: `crates/tuner-cli/src/main.rs:254-450`
- Modify: `crates/tuner-api/src/main.rs:784-980`
- Modify: `README.md`

**Interfaces:**

- `inspect-xdf` and API `inspect_xdf` retain current JSON keys and add header/default/category/diagnostic counts plus parameter/axis numeric-kind and metadata fields.
- Transfer planning treats differing normalized numeric kinds or raw type flags as blocking compatibility issues before fast-path byte copying.
- README examples explain that `inspect-xdf` is metadata inspection and `xdf-validate` is BIN range validation; neither writes the input files.

- [x] **Step 1: Add failing CLI/API serializer and transfer assertions.**

  Add unit tests beside the existing `parameter_json`, `axis_json`, and request/result helpers with string assertions for `category_count`, `diagnostic_count`, `numeric_kind`, and `header`; add a transfer test for float-versus-integer mismatch; and add a README command example containing the new fields.

- [x] **Step 2: Run the focused command and transfer tests.**

  Run:

  ```powershell
  cargo test -p tuner-cli
  cargo test -p tuner-api
  cargo test -p tuner-transfer --lib
  ```

  Expected result: new assertions fail because the serializers and compatibility check do not yet include the normalized fields.

- [x] **Step 3: Serialize metadata without changing existing result schemas.**

  Update `parameter_json` and `axis_json` in both binaries to serialize numeric kind, raw type flags, axis units/bounds/decimal places, labels, category memberships, and source diagnostics. Add document-level header and counts to `inspect-xdf`; keep JSON escaping through the existing helpers and preserve the current field names and result schema identifiers.

- [x] **Step 4: Add the transfer storage-kind gate and README explanation.**

  In `plan_parameter`, compare `source.layout.storage.numeric_kind` and `source.layout.storage.raw_type_flags` with the destination before the existing byte-copy path. Emit a blocking issue that names both values. Document the distinction between raw byte transfer, normalized XDF interpretation, and explicit engineering-unit edits.

- [x] **Step 5: Run the command/API regression cycle.**

  Run:

  ```powershell
  cargo test -p tuner-cli
  cargo test -p tuner-api
  cargo test -p tuner-transfer --lib
  cargo test --workspace
  cargo fmt --all -- --check
  ```

  Expected result: all existing JSON/transfer behavior remains compatible and the new metadata is visible.

## Task 7: Lock the supplied XDF/BIN behavior with an integration test and verify delivery

**Files:**

- Create: `crates/tuner-xdf/tests/real_fixture.rs`
- Modify: `README.md` only if the verified output requires a corrected example.

**Interfaces:**

- The integration test loads `Test bin and xdf/SCGa05_cal.xdf` and `Test bin and xdf/SCGa05_cal.bin` from paths relative to `CARGO_MANIFEST_DIR`.
- The test asserts normalized counts and calls `validate_against_document` without editing either file.

- [x] **Step 1: Add the real-fixture test with explicit acceptance assertions.**

  Use this path construction and assertions:

  ```rust
  let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  let xdf = XdfDocument::load(root.join("../../Test bin and xdf/SCGa05_cal.xdf")).unwrap();
  let bin = BinDocument::load(root.join("../../Test bin and xdf/SCGa05_cal.bin")).unwrap();

  assert_eq!(xdf.parameters.len(), 2_915);
  assert_eq!(xdf.categories.len(), 57);
  assert_eq!(xdf.parameters.iter().map(|p| p.axes.len()).sum::<usize>(), 8_745);
  assert_eq!(xdf.parameters.iter()
      .flat_map(|p| p.axes.iter())
      .filter(|axis| axis.storage.as_ref().is_some_and(|storage| storage.numeric_kind == NumericKind::Ieee754Binary32))
      .count(), 19);
  let report = xdf.validate_against_document(&bin);
  assert!(report.is_valid(), "{} mapping issues", report.issue_count());
  ```

- [x] **Step 2: Run the real-fixture test and inspect the actual diagnostic output.**

  Run:

  ```powershell
  cargo test -p tuner-xdf --test real_fixture -- --nocapture
  cargo run --quiet -p tuner-cli -- xdf-validate "Test bin and xdf/SCGa05_cal.xdf" "Test bin and xdf/SCGa05_cal.bin"
  ```

  Expected result: the test passes with 2,915 parameters, 57 categories, 8,745 axes, 19 binary32 axes, and zero mapping issues; the CLI result reports `valid:true`.

- [x] **Step 3: Run the complete verification commands.**

  Run:

  ```powershell
  cargo fmt --all -- --check
  cargo build --workspace
  cargo test --workspace
  cargo run --quiet -p tuner-cli -- inspect-xdf "Test bin and xdf/SCGa05_cal.xdf"
  cargo run --quiet -p tuner-cli -- xdf-validate "Test bin and xdf/SCGa05_cal.xdf" "Test bin and xdf/SCGa05_cal.bin"
  ```

  Confirm the two supplied files' SHA-256 values and byte contents are unchanged after all tests. Capture any remaining diagnostics by code and ensure every one is either an intentional unsupported-feature warning or a fatal validation issue that has been fixed before completion.

- [x] **Step 4: Update the spec status and delivery notes only after verification.**

  Change the spec status from `Proposed for review` to `Implemented` only after every acceptance criterion has passing command output. Add a concise README note naming the new normalized fields and raw-versus-engineering write boundary.

## Plan self-review checklist

- Spec coverage: Tasks 2–4 cover header/defaults, base offsets, regions, categories, labels, links, signed strides, storage flags, object variants, diagnostics, and semantic identity; Task 1 covers the safe conversion AST/evaluator/inverse; Task 5 covers raw and engineering cell access; Tasks 6–7 cover transfer safety, CLI/API visibility, fixture counts, and acceptance verification.
- Marker scan: run the forbidden-marker scan from the writing-plans skill against this file; the expected result is no matches.
- Type consistency: `NumericKind`, `StorageSpec`, `EngineeringWriteResult`, `RawValue::Float32Bits`, `XdfHeader`, `CategoryMembership`, and the three engineering methods are introduced before their consumers; CLI/API serializers use the same public field names as `tuner-xdf`.
- Safety review: all writes remain transaction-scoped, float values are never routed through integer writers, unknown storage is never guessed, and the supplied BIN/XDF remain read-only inputs.
