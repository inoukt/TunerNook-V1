# XDF Interpretation and Engineering-Unit Conversion Design

**Status:** Implemented  
**Date:** 2026-09-12  
**Project:** TunerNook

## Context

The current foundation can locate the editable table payload and preserves XDF
conversion text, but it still loses much of the format's meaning while
normalizing it. The supplied `SCGa05_cal.xdf` is a useful stress fixture: it has
2,915 tables, 8,745 axes, 57 header categories, ordered `CATEGORYMEM`
memberships, axis `LABEL` entries, header `DEFAULTS`, a `BASEOFFSET`, and three
storage-flag families (`0x02`, `0x06`, and `0x10006`). It also has 92 unique
conversion equations across 8,745 math nodes. Every equation uses the single
variable `X`, numeric constants, arithmetic operators, and parentheses; the
dominant forms are `X` and rational expressions such as
`((a * X) - b) / (c - (d * X))`.

The next slice is therefore a format-aware normalization boundary plus a safe
engineering-unit conversion layer. It must preserve useful source metadata,
distinguish storage semantics from display metadata, and make formulas useful
to editors without executing untrusted XDF text or silently changing a
requested value. Parsing should be tolerant of descriptive or unsupported XDF
objects, but it must never turn an ambiguous storage definition into a guessed
integer mapping.

## Goals

1. Normalize the XDF header, defaults, base-address translation, regions,
   category catalog, object kinds, storage mappings, axes, labels, and
   diagnostics into explicit typed data instead of dropping them.
2. Resolve effective per-object and per-axis storage settings using a documented
   precedence order, including signedness, bit/byte order, element width,
   floating-point kind, dimensions, and signed strides.
3. Preserve enough source metadata and provenance that a caller can explain why
   a value maps to a particular BIN range or why a feature is unsupported.
4. Parse the supported XDF expression subset into a safe, dependency-free AST.
5. Evaluate raw integer and IEEE-754 binary32 values to engineering values using
   checked `f64` arithmetic.
6. Invert identity, affine, and linear-fractional expressions for
   engineering-to-raw editing when the inverse is mathematically defined.
7. Reject unknown syntax, non-finite values, division by zero, non-invertible
   formulas, ambiguous storage, invalid ranges, and non-representable values
   with structured errors or diagnostics.
8. Make the normalized document and conversion boundary reusable by the future
   desktop editor, CLI, and local API without changing the existing raw
   transaction safety model.

## Non-goals for this slice

- Executing arbitrary code or delegating evaluation to a scripting engine.
- The complete TunerPro expression language, arbitrary function calls,
  conditionals, lookup tables, and external variables. Unsupported expressions
  remain visible as source text and produce an explicit conversion diagnostic.
- Applying units, decimal places, or `outputtype` as hidden numeric transforms;
  these remain display metadata unless a future explicit formatter requests
  them.
- Full semantic editing of checksum algorithms, arbitrary patches, structures,
  and every vendor-specific XDF extension. These objects are retained or
  diagnosed rather than misclassified as ordinary cells.
- Automatic user-visible rounding, clamping, or silent saturation on writeback.
- A full desktop UI, charting, or conversion-aware CLI/API commands.

## Approaches considered

### Recommended: typed normalization plus safe AST and restricted inverse

Add a dependency-free `tuner-cal` crate and make `tuner-xdf` a typed XDF
normalizer. The XDF crate resolves source precedence and storage semantics; the
calibration crate parses the arithmetic grammar once, evaluates the AST, and
separately recognizes affine and linear-fractional forms for deterministic
inversion. `tuner-xdf` depends on `tuner-cal` only for conversion-aware cell
access. Unsupported inverse shapes remain readable when evaluation is valid but
cannot be written through the engineering-unit API.

This covers the semantics observed in the supplied definition while keeping
unsafe expression execution and ambiguous write policy explicit and auditable.

### Read-only generic evaluator

Add the same parser and evaluator but postpone inversion and engineering-unit
writeback. This is simpler initially, but it leaves the editor with two value
models and requires another interface migration before engineering edits work.

### Full XDF expression and object engine

Implement the broader TunerPro expression language, every object type, and
round-trip support for all vendor extensions. This would maximize format
coverage but would make unsafe or ambiguous behavior harder to audit and would
delay the desktop editing milestone. The recommended boundary gives us broad
interpretation now while keeping unsupported execution and writeback explicit.

## Architecture

The dependency direction will be:

```text
tuner-xdf  ->  tuner-core
tuner-xdf  ->  tuner-cal
future UI/API  ->  tuner-xdf
future UI/API  ->  tuner-cal
```

The arrows mean "depends on." More precisely, `tuner-cal` has no workspace
dependencies. `tuner-xdf` may depend on `tuner-cal` for conversion-aware cell
methods, while `tuner-cal` never depends on `tuner-xdf`; this prevents a cycle
and keeps the evaluator usable by future UI/API crates.

## Format-aware XDF normalization

The parser will separate three layers:

1. Source values: original strings, element names, attributes, and object paths
   useful for diagnostics.
2. Effective values: checked numeric, boolean, enum, and address values after
   applying XDF precedence rules.
3. Capabilities: whether the normalized value can be safely read or written by
   the current BIN model.

An absent optional field is different from an explicit zero, false, or empty
string. Unknown attributes and elements do not change the effective mapping;
they are retained in object diagnostics/counts so the parser does not claim
more support than it has.

### Header, defaults, and address space

`XdfDocument` will expose an `XdfHeader` containing the format version, title,
description, header flags, `DEFAULTS`, `BASEOFFSET`, and one or more `REGION`
records. `DEFAULTS` will be typed as data-size bits, significant digits, display
output type, signed, least-significant-byte-first, and floating-point defaults.
The raw text is retained alongside each successfully parsed field.

`BASEOFFSET` will be represented as the encoded offset plus its subtract flag,
with one checked mapper used everywhere. Its effective signed offset is
`+offset` when `subtract` is false and `-offset` when `subtract` is true; an XDF
address is translated to a BIN offset by adding that signed offset. Address
underflow, overflow, and negative final BIN positions are errors. Regions are
kept in XDF address space and used as bounds metadata; an object outside a
declared region produces a diagnostic, while the actual BIN range check remains
authoritative.

For a storage field, explicit axis data wins over the containing object, which
wins over header defaults. The chosen source is recorded so a caller can
explain an effective width, signedness, byte order, or float kind. The existing
`XDFDATA`/`EMBEDDEDDATA` selection remains explicit: use the addressed
`EMBEDDEDDATA` selected by the object shape, prefer the table `z` payload for a
table body, and report conflicting candidates instead of silently merging them.

### Storage flags and layout

The normalized layout will use signed bit strides and an explicit storage
descriptor rather than treating all values as unsigned byte-aligned integers:

```text
StorageSpec {
    element_size_bits: u32,
    signed: bool,
    byte_order: Little | Big,
    numeric_kind: Integer | Ieee754Binary32 | Unsupported,
    column_major: bool,
    raw_type_flags: u32,
    unknown_type_flags: u32,
}
```

For `mmedtypeflags`, the known low flags are signed (`0x01`), least-significant
byte first (`0x02`), and column-major/populate-by-column (`0x04`). The `0x10000`
IEEE-754 binary32 marker is recognized when present with a 32-bit element. The
complete raw flag value is always preserved. Unknown bits, inconsistent
width/kind combinations, and unsupported bit ordering produce an explicit
capability diagnostic rather than a guessed mapping. An explicit flag value
such as the fixture's `0x06` z-axis storage is normalized as little-endian
integer data with column-major layout; the fixture's `0x10006` z-axis values
are normalized as little-endian IEEE-754 binary32 storage with the same layout.

`outputtype`, `datatype`, `unittype`, `decimalpl`, `min`, and `max` are retained
as display/range metadata and are not allowed to override raw storage flags.
Integer reads use the existing checked core readers. Binary32 reads decode the
four stored bytes with the resolved byte order and expose the finite value to
the conversion layer; non-finite raw floats are readable as raw bits but cannot
be converted to an engineering value or written back.

Layout calculation will support positive and negative major/minor strides in
bits. For cell `(row, column)`, the start bit is the checked signed sum of the
base bit address, `row * major_stride_bits`, and `column * minor_stride_bits`.
The result must be nonnegative and the complete cell must fit in the BIN. When
both table strides are explicitly zero, the packed row/column fallback is used
only for a shape that requires it (`row_stride = columns * width`,
`column_stride = width`); it is not used to conceal an incomplete or
contradictory layout. Bit-level layouts are preserved during parsing, while
the current read/write capability still gates operations that require a
byte-aligned core primitive.

### Categories and axis metadata

The document will retain a category catalog keyed by the numeric `CATEGORY`
index. Each parameter will preserve ordered `CATEGORYMEM` entries, including
the membership slot and referenced category index, and will expose resolved
category names when the catalog contains them. Missing or duplicate category
references are diagnostics, not silent loss of membership.

`AxisDefinition` will include its identity, count, optional storage mapping,
units, unit type, decimal places, engineering bounds, display output type,
conversion source, ordered labels, `DALINK` data, and `embedinfo` data. Labels
preserve their source index and value; gaps and duplicate indices are reported.
An axis with no address is valid as descriptive metadata and may be linked to a
separate object. An unresolved `DALINK` or `embedinfo` reference remains
visible and is marked unavailable rather than being assigned a guessed address.
Axis conversions are independent from the table-cell conversion: x/y axis
values and z/body cells each retain their own source formula.

### Common object kinds

The normalized object model will distinguish at least:

- `XDFCONSTANT`, `XDFTABLE`, and `XDFBITFIELD` editable parameters;
- `XDFFLAG` as a flag/bitfield variant, retaining its original mask and any
  explicit bit offset/width;
- `XDFFUNCTION` as a function object with separately named input/output axes;
- `XDFPATCH` and `XDFCHECKSUM` metadata records with explicit read/write
  capability status; and
- unknown or vendor-specific objects recorded in diagnostics with their source
  name and location.

The first writeback slice covers cell-bearing constants, tables, bitfields, and
flags when their effective storage is supported. Functions, patches, checksum
definitions, and unsupported extensions are parsed enough to remain visible and
matchable but are not treated as ordinary table cells.

### Diagnostics and identity

Add structured diagnostics with severity, stable code, source path, and message.
Fatal XML/field errors still return `XdfError`; recoverable issues are attached
to `XdfDocument` and surfaced by validation. Diagnostics must distinguish:

- unsupported feature versus malformed definition;
- a descriptive axis with no storage versus a missing address on an editable
  object;
- an unknown flag bit versus a known but unsupported storage kind; and
- a semantic duplicate versus two objects that only share an address.

Semantic identity remains address-independent. The identity material now
includes normalized object kind, title/category memberships, effective storage
semantics, dimensions/strides, conversion source, bitfield mask, and axis
identity/metadata that affects meaning. Repeated or address-like source IDs are
normalized to a stable semantic hash with deterministic aliases, as in the
existing parser.

### `tuner-cal` public model

- `Conversion::parse(source: &str) -> Result<Conversion, ConversionError>`
- `Conversion::evaluate(raw: f64) -> Result<f64, ConversionError>`
- `Conversion::invert(engineering: f64) -> Result<f64, ConversionError>`
- `Conversion::source() -> &str`

The AST remains an implementation detail so the grammar can evolve without
forcing consumers to depend on parser nodes. `Conversion` is cloneable and can
be cached by a caller for repeated table-cell access.

The parser accepts:

```text
expression  := additive
additive    := multiplicative (('+' | '-') multiplicative)*
multiplicative := unary (('*' | '/') unary)*
unary       := ('+' | '-') unary | primary
primary     := number | X | '(' expression ')'
```

Numbers include decimal and scientific notation. Identifiers other than `X`
are rejected. Parsing never evaluates text directly.

Evaluation checks every operation for division by zero and non-finite results.
Input and output must be finite. Errors include source position where syntax
allows it, so the caller can show a useful definition warning.

### Inversion policy

Inversion is structural and deterministic; it will not use numerical iteration.
The recognizer accepts:

- identity: `X`;
- affine: `a * X + b`;
- linear-fractional: `(a * X + b) / (c * X + d)`.

Parentheses, unary signs, zero coefficients, and constant folding are allowed.
For `y = (aX + b) / (cX + d)`, the inverse is
`X = (b - y*d) / (y*c - a)`. A zero inverse denominator, a non-finite result,
or a structurally unsupported expression returns `ConversionError::NotInvertible`
or the corresponding domain error.

The conversion layer returns an `f64` raw result. Integer quantization is not
implicit; `tuner-xdf` engineering-unit writeback accepts only a finite result
whose distance from `raw.round()` is at most
`1e-9 * max(1.0, abs(raw))`, then writes `raw.round()` and lets the existing
core integer writer enforce width and signedness. Larger fractions are
rejected rather than silently rounded. Binary32 narrowing is a separate,
explicit storage operation described below and returns the stored value and
post-write engineering value to make its quantization visible.

## `tuner-xdf` and `tuner-cal` integration

`ParameterDefinition` keeps its original conversion string for identity and
diagnostics and gains the normalized storage and metadata described above. It
also gains conversion-aware helpers:

- `compile_conversion`, which treats a missing string as identity;
- `read_engineering_cell`, which reads an integer or binary32 cell, converts it
  to `f64`, and applies the cell conversion;
- `write_engineering_cell`, which inverts an engineering value and writes the
  resulting integer or binary32 representation through the caller's existing
  `Transaction`; and
- metadata accessors for effective axis labels, categories, storage flags,
  source formulas, and diagnostics.

Cell access uses a storage-aware raw value model. Integer writeback accepts only
the finite integral result described in the inversion section. Binary32
writeback accepts a finite result that narrows to a finite `f32`; the stored
IEEE representation and post-write engineering value are returned so the caller
can show the unavoidable float32 quantization. Neither path clamps, saturates,
or hides a failed representability check.

All range, bitfield, endianness, signedness, width, float-kind, and transaction
checks remain in force. A conversion or storage failure occurs before any write.
No method edits a BIN outside an already-open core transaction. The conversion
layer never uses `outputtype`, units, decimal places, or min/max as an implicit
rounding or clamping rule.

The existing raw helpers remain available for diagnostics and low-level tools;
engineering helpers are an additional, explicitly typed layer. A float cell is
never routed through an integer `RawValue`, and a bitfield write performs the
same read-modify-write preservation of unselected bits as today.

## Testing strategy

The implementation will use test-first development with these groups:

1. Header/default parsing, including hexadecimal values, strict boolean
   variants, base-offset add/subtract mapping, region metadata, and default
   precedence.
2. Wrapper/payload selection for direct `EMBEDDEDDATA`, `XDFDATA`, addressed
   table z payloads, conflicting candidates, and missing descriptive-axis
   addresses.
3. Signed stride arithmetic, packed zero-stride fallback, negative strides,
   bit-alignment bounds, and overflow/underflow rejection.
4. Storage flag decoding for signed, LSB-first, column-major, IEEE binary32,
   unknown bits, unsupported widths, little/big endian reads, and binary32
   finite/non-finite writeback.
5. Category catalog resolution, ordered memberships, labels with gaps or
   duplicates, axis display metadata, and unresolved `DALINK`/`embedinfo`
   references.
6. Common object collection for constants, tables, bitfields, flags, functions,
   patches, checksums, and explicit handling of unknown objects.
7. Lexer/parser precedence, unary signs, scientific numbers, parentheses, and
   rejection of unknown identifiers or malformed input.
8. Evaluation of identity, affine, division, negative constants, and the actual
   formula shapes extracted from `SCGa05_cal.xdf`.
9. Inversion round trips for identity, affine, and linear-fractional formulas;
   explicit tests for zero denominators and non-invertible shapes.
10. `tuner-xdf` integration tests for engineering reads, exact integral writes,
    binary32 writes, signed/unsigned width enforcement, bitfield preservation,
    and atomic undo.
11. A real-fixture smoke test that confirms 57 categories, 2,915 table
    parameters, 8,745 axes, 19 `0x10006` binary32 z axes, resolved category
    membership counts, and every unique conversion formula. The existing BIN/XDF
    validation must remain at 2,915 parameters with zero mapping errors.

## Acceptance criteria

- `cargo test --workspace` passes with parser, evaluator, integration, and
  existing safety tests.
- The supplied XDF's header, category memberships, labels, axis metadata,
  signed strides, and storage flags are inspectable from the normalized model;
  its 19 `0x10006` axes are identified as binary32 rather than integer data.
- Every conversion formula in the supplied XDF parses without executing source
  text.
- A supported formula can be read in engineering units and written back only
  when its inverse produces a representable integer or finite binary32 value.
- Unsupported, ambiguous, malformed, or unsafe cases produce explicit errors or
  diagnostics and leave the transaction and BIN unchanged.
- `cargo fmt --all -- --check` and `cargo build --workspace` pass.

## Format references used for the decoder

- [OpenEEC XDF model](https://github.com/OpenEEC-Project/SAD806x/blob/master/SADXdf.cs)
  for header fields, category membership shape, axis metadata, and the known
  `mmedtypeflags` low bits.
- [a2l2xdf generator](https://github.com/bri3d/a2l2xdf/blob/master/a2l2xdf.py)
  for the IEEE binary32 flag combination used by generated XDFs.
- [TunerPro general conversions help](https://www.tunerpro.net/WebHelp/source/genconv.htm)
  for the conversion-expression context.
