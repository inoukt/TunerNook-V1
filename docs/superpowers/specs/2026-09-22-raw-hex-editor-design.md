# Raw Hex Editor Design

## Goal

Add an in-app raw BIN editor that lets a user inspect and edit bytes while keeping
the table editor, undo history, project workspace, and NookLink agent controls
in sync.

## Scope

- A movable/resizable Hex Editor window with saved project geometry and byte
  position.
- Virtualized 16-byte rows showing address, hexadecimal bytes, and printable
  ASCII.
- Interpreted views for unsigned/signed 8/16/32/64-bit integers and float32/
  float64, with selectable endianness and float precision.
- Click and drag byte selection, address jump, copy selected bytes, and paste or
  type validated hexadecimal bytes at the selection start.
- Find previous/next exact byte patterns in Hex mode or exact typed values in
  an interpreted numeric mode. Numeric scans use aligned values from BIN offset
  zero; byte-pattern scans may start at any byte.
- Automatic value coloring across the BIN with the existing Off/Auto/Fixed
  palette controls and editable colors/range.
- One atomic `BinDocument` transaction per accepted edit. Undo and redo use the
  existing shared history. Input BIN files remain untouched; Save As publishes
  a new file.
- Selecting cells in the active parameter highlights their mapped bytes in the
  hex view. Selecting a byte that overlaps a cell in the active parameter
  selects that cell without taking focus away from the hex window.
- NookLink actions expose window open/focus, address jump, byte-range selection,
  edits, and a structured hex state snapshot.

## Design

Add a small `hex.rs` module for hex parsing, typed decoding/search, byte selection
bounds, ASCII display, and window memory. The app window reads only the visible
rows, so memory use for rendering does not grow with BIN size. It passes every accepted edit through one
`WorkspaceState` method that validates bounds before creating a transaction;
successful edits invalidate cached comparisons and searches through the same
app revision path used by existing editors.

The active XDF parameter is the reverse-sync scope for a byte click. This gives
work proportional to that parameter's dimensions and avoids scanning every
table in a large XDF on every pointer event. Bytes outside the active parameter
remain selectable and editable, but have no inferred table mapping.

Hex entry accepts pairs of hexadecimal digits separated by whitespace or
commas, with optional `0x` prefixes. It rejects empty, malformed, overflowing,
or out-of-BIN ranges before mutation. Applying a pasted sequence writes the
sequence at the selected range start and creates exactly one undo entry.

Project preferences store whether the window is open, its geometry, current
address, display mode, endianness, precision, coloring, and search query. They
use serde defaults for existing project files. The UI command and NookLink
actions call the same app methods as mouse interaction.

## Safety and limits

- No operation writes to the source BIN path.
- A write is all-or-nothing and undoable; malformed paste leaves bytes and
  history unchanged.
- BIN offsets are zero-based. Display addresses are hexadecimal byte offsets.
- ASCII shows printable bytes and dots for non-printable bytes.
- Reverse table synchronization searches only the active parameter.
- No checksum recalculation, ECU communication, disassembly, or standalone OS
  process is included.

## Acceptance

- Large BIN display draws only visible rows.
- Byte selection and editing work with and without an XDF.
- Table selection highlights its mapped byte ranges; clicking an active-table
  byte selects its containing cell while the hex window remains foreground.
- A multi-byte write produces one undo entry and one redo entry.
- Invalid hex text and out-of-range writes do not mutate the BIN.
- Window memory is project-scoped and older settings files still load.
- NookLink can inspect and manipulate the same state as the UI.
