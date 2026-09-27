# Column-Major XDF Storage Semantics Design

Status: Implemented
Date: 2026-09-13
Project: TunerNook

## Context

The completed engineering-conversion slice normalized mmedtypeflags bit 0x04
as a storage-type marker rather than an orientation flag. That interpretation is unsafe for the
supplied SCGa05_cal.xdf: 2,896 table-body axes use 0x06 with 8- or 16-bit
elements and ordinary integer conversions, while 19 axes use 0x10006 with
32-bit elements. The XDF table editor describes the relevant bit as the
populate-by-column mode, and the fixture generator uses 0x06 for normal z
data and adds 0x10000 only for IEEE-754 binary32 data.

References used for this correction:

- TunerPro XDF table editor help:
  https://tunerpro.net/WebHelp/source/xdftableeditor.htm
- A2L2XDF serialization:
  https://github.com/bri3d/a2l2xdf/blob/master/a2l2xdf.py
- MxT XDF support notes:
  https://github.com/TracqiTechnology/MxT/blob/master/README.md

## Goals

1. Normalize 0x04 as an explicit column-major layout flag.
2. Treat explicit type flags without the binary32 marker as integer storage,
   including the fixture 0x06 table bodies.
3. Preserve 0x10000 plus 32-bit width as the explicit IEEE-754 binary32
   representation and keep wrong-width combinations unsupported.
4. Resolve packed table cells in the declared row or column order, including
   non-square tables, while preserving signed and explicit stride behavior.
5. Expose the corrected layout and storage semantics through CLI and API
   inspection JSON without removing existing result fields.
6. Keep raw access, engineering conversion, transfer gating, and transaction
   safety consistent with the corrected normalized model.

## Non-goals

- A new desktop renderer; this workspace currently has no tuner-app crate.
- Guessing a floating-point representation for an XDF that lacks the binary32
  marker.
- Rewriting supplied BIN or XDF fixtures.
- Changing the exact-XDF hash gate for bulk transfer.

## Normalized model

StorageSpec gains a column_major boolean.

| Bit | Meaning |
| --- | --- |
| 0x01 | signed integer storage |
| 0x02 | least-significant byte first |
| 0x04 | column-major or populate-by-column table layout |
| 0x10000 | IEEE-754 binary32 marker when element width is 32 bits |

NumericKind::Ieee754Binary32 is selected only for a 32-bit element carrying
0x10000. An explicit flag value such as 0x06 is integer storage with
column_major=true. Header DEFAULTS float="1" remains a fallback only when
there is no explicit type-flag value; it resolves to binary32 only for a
32-bit element. When there is no explicit flag, column_major is false.

DataLayout.row_stride_bits and column_stride_bits remain the effective logical
strides for (row, column) access. StorageSpec.column_major is retained
separately so callers can explain the source layout and semantic identity does
not discard the orientation bit.

## Cell addressing

When both XDF stride attributes are explicitly zero, the parser derives packed
logical strides from the orientation:

~~~text
row-major:    row_stride = columns * element_width
              column_stride = element_width

column-major: row_stride = element_width
              column_stride = rows * element_width
~~~

The existing signed-stride path remains authoritative when one or both stride
attributes are nonzero. Explicit negative strides continue to be supported;
the range calculator examines the minimum and maximum reachable cell offsets
before converting to a byte range. One-dimensional axes keep their existing
contiguous addressing because their logical shape is 1 x count.

For a 2x3 column-major table with 8-bit cells, logical coordinates map as:

~~~text
(0,0) -> base + 0
(1,0) -> base + 1
(0,1) -> base + 2
(1,1) -> base + 3
(0,2) -> base + 4
(1,2) -> base + 5
~~~

All raw and engineering cell methods use this one effective layout mapping.

## Diagnostics and compatibility

The earlier parser's unsupported-storage diagnostics for ordinary table-body
flags are removed. They remain for a binary32 marker on a non-32-bit element
and other genuinely unsupported combinations. Unknown bits still produce
unknown-storage-flags while preserving the known semantics.

The normalized fingerprint includes column_major through canonical storage
material. The transfer planner continues to require matching raw type flags and
numeric kind before a raw byte-copy path is allowed.

## Testing and acceptance

- Unit tests prove 0x06 is integer and column-major, 0x10006 is binary32 and
  column-major, and a wrong-width binary32 marker remains diagnostic.
- A non-square 2x3 fixture proves both read and write cell coordinates follow
  column-major packed order.
- CLI and API inspection tests expose column_major and corrected numeric kinds.
- The supplied fixture remains at 2,915 parameters, 57 categories, and 8,745
  axes; exactly 19 axes are binary32 and no ordinary 0x06 axis is marked
  unsupported.
- cargo fmt --all -- --check, cargo build --workspace, and cargo test
  --workspace pass.
- The supplied BIN and XDF SHA-256 values remain unchanged.
