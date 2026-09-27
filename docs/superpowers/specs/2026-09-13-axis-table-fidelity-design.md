# Axis and Table Fidelity Design

Date: 2026-09-13  
Status: Draft for review  
Scope: `tuner-xdf` model/parser and `tuner-app` table/workspace UI

## Problem

The table editor currently treats row and column headers as logical positions (`R0`, `C0`) even when the XDF defines address-backed axes whose values live in the BIN. This hides useful calibration data and makes axis editing impossible. Engineering values in the table also use inconsistent precision, table zoom changes only selected content, and reopening a previously used BIN does not offer its last XDF explicitly.

The next slice must improve these behaviors without weakening BIN safety, XDF interpretation, undo/redo, validation, persistence, or the headless AI control boundary.

## Goals

1. Interpret address-backed XDF axes using their declared storage layout and expose their actual BIN values.
2. Allow safe editing of writable X/Y axis values through the same engineering-to-raw validation and transaction path used by table cells.
3. Display engineering values with two decimal places by default, with a persisted per-table precision setting from 0 through 8 places.
4. Make table zoom behave like a content zoom: title, text, controls, cell dimensions, frame spacing, and scrollable content scale together while the table window remains movable and resizable.
5. Remember the last successfully used XDF separately for each BIN and offer a non-blocking reuse prompt when that BIN is opened again.
6. Preserve deterministic, inspectable state transitions so an AI agent can control the same operations through the app’s public model and NookLink/API boundary.
7. Add focused regression tests and complete workspace verification.

## Non-goals

- Replacing the existing XDF parser with a different format implementation.
- Guessing or repairing malformed axis metadata silently.
- Writing descriptive axes that have no address-backed storage.
- Changing the existing `tuner-api/v1` wire protocol in this slice unless a compatibility-preserving additive field is required for the new axis metadata.
- Automatically applying an XDF to a BIN without showing the user the reuse choice.
- Changing raw-value display semantics: raw values remain exact and are not rounded for presentation.

## Architecture

Axis interpretation belongs in `tuner-xdf`, where the normalized XDF model already owns storage, conversion, range, and transaction semantics. The app will consume this API rather than duplicating bit-address calculations in egui code.

The app remains responsible for selection state, table presentation, persisted layout/precision, and the XDF reuse prompt. Loading and prompt actions continue through the existing background operation coordinator so parsing and file I/O do not block animation or input.

## Axis model and parsing

`AxisDefinition` will retain a signed `stride_bits` value. Parsing chooses the first meaningful stride in this order:

1. Declared major stride when nonzero.
2. Declared minor stride when nonzero.
3. Element width when both declared strides are zero.

The signed value is preserved, including negative strides. Axis range calculation uses the axis address, element width, count, and stride with checked arithmetic. The parser must not turn an invalid or unaligned address into a writable range.

The normalized fingerprint and API/CLI axis serialization will include `stride_bits`, keeping the output self-describing while remaining additive for consumers that ignore unknown fields.

The existing XDF behavior for the bundled fixtures remains intact: an addressed X/Y axis is interpreted as an axis, while a Z definition that describes the table body remains the table payload for a normal two-dimensional parameter. Descriptive axis definitions without an address remain available for labels and metadata but do not acquire synthetic BIN storage.

## Axis read and write API

`ParameterDefinition` will expose reusable axis accessors equivalent to:

- resolve an axis by index/identifier;
- calculate the checked byte/bit range for one axis element;
- read the raw axis element;
- read its engineering value through the axis conversion;
- write an engineering axis value through conversion inversion and representability checks.

The accessors will reuse the same numeric-kind, signedness, endianness, and transaction primitives as cell access. They will reject out-of-range indices, missing addresses, unsupported storage, non-byte-aligned layouts, overflow, and non-representable engineering values with structured `CellAccessError` variants. A failed write must not modify the BIN or create an undo entry.

The axis write path will return the requested engineering value, stored raw value, and stored engineering value in the existing write-result shape. It will be committed atomically through the existing transaction mechanism, followed by the normal validation refresh and status update.

## Table presentation and editing

For ordinary two-dimensional tables, the app will identify the normalized X and Y axes by their axis metadata/role identifiers and dimension counts. When an axis element has address-backed storage:

- the column or row header shows its actual engineering value in Engineering display mode;
- Raw display mode shows the exact raw value;
- clicking the header selects that axis element;
- the existing engineering-value editor and Apply action edit the selected axis element.

When an axis is descriptive only, the header uses its XDF label if present, otherwise a stable positional fallback, and is read-only. Unsupported or invalid axis storage is shown with a clear unavailable/read-only indication rather than a fabricated value. The table payload axis is not duplicated as a header.

The selected axis is explicit workspace state and is mutually exclusive with a selected payload cell. Changing selection clears the previous edit text and resets the editor to the newly selected target. Debug output will identify whether the current selection is a payload cell or an axis element, including its resolved range or the explicit reason that the mapping could not be read or written.

## Number formatting and per-table settings

`TableWindowMemory` will gain a persisted `decimal_places` setting. New and migrated tables default to `2`; values are sanitized to `0..=8`. Existing preference files without this field continue to load using the default.

Engineering values use the table’s precision setting consistently in headers, payload cells, selection/editor context where a formatted value is shown, and debug/report presentation. A value such as `14` therefore displays as `14.00` at the default precision. Raw values retain exact integer/bit representations.

The table toolbar or table settings menu will provide precision controls without changing the stored BIN value. Changing precision updates that table’s saved window memory and does not affect other tables.

## Zoom and window behavior

The existing movable/resizable table window remains the top-level egui window. Its saved position, size, scroll offsets, and zoom continue to be keyed by the stable table identity.

The table zoom percentage scales:

- window title text;
- table toolbar text and controls;
- table cell and row-header dimensions;
- frame/button padding and spacing;
- table font sizes;
- scrollable grid content.

Zoom will not silently overwrite the user’s manually chosen outer window geometry. Large tables remain constrained to the available canvas and can be navigated with scrollbars. The existing zoom controls and persisted zoom value remain the source of truth; zoom is clamped to the existing safe range.

## BIN-specific last-XDF reuse

`ProjectPreferences` will gain an optional last-XDF path. It is updated only after a successful XDF load while a BIN project is active, and it is persisted with that BIN’s project preferences. A failed or cancelled XDF load never replaces the remembered path.

After a BIN’s workspace restoration completes, the app evaluates the remembered path:

- no remembered path: continue normally;
- remembered path already equals the active XDF path: continue normally;
- remembered path exists and differs from the active XDF: show the reuse prompt;
- remembered path is missing: show the prompt with an explicit file-not-found explanation and disable the Use action.

The prompt is non-blocking and has these actions:

- **Use last XDF**: starts the normal background XDF-loading operation for the remembered path;
- **Keep current**: dismisses the prompt and preserves the currently loaded XDF, if any;
- **Choose XDF…**: opens the existing XDF picker.

The pending prompt carries the BIN project identity. If another BIN load begins or completes before the prompt is answered, the stale prompt is discarded and cannot load an XDF into the newer project. Choosing the remembered XDF clears the prompt before starting the load; the successful load then naturally matches the remembered path and does not prompt again.

## AI and NookLink compatibility

Selection, axis inspection, precision changes, zoom changes, and axis edits will be represented by deterministic workspace/table state and reusable methods rather than UI-only side effects. Existing NookLink actions continue to use `tuner-api/v1`; no action will bypass range checks or commit a write without the same transaction and validation path used by the UI.

If axis metadata is exposed through API/CLI JSON, `stride_bits` is additive and stable. Error results remain structured and human-readable so an agent can distinguish an unavailable descriptive axis from an invalid writable mapping.

## Loading, errors, and safety

All BIN/XDF loads and workspace restoration continue through the existing background operation coordinator and warning overlay. The UI disables conflicting document/edit actions while an operation is active. Prompt actions are allowed only when no conflicting document transition is active.

Axis read failures appear at the affected header and in the inspector/debug report. Axis write failures appear in the normal status/diagnostic area and leave the BIN unchanged. Missing XDF reuse paths are displayed as actionable warnings without attempting an implicit file open.

## Test plan

### `tuner-xdf`

- parse declared major/minor/packed fallback strides, including signed stride;
- calculate axis ranges with checked arithmetic and reject invalid alignment/bounds;
- read actual X/Y fixture axis values using the correct conversion, width, endian, and signedness;
- write a writable axis through a transaction and verify undo/rollback behavior;
- reject descriptive/no-address axes and unsupported layouts without mutation;
- verify normalized fingerprints and JSON serializers include the new stride metadata.

### `tuner-app`

- default and persisted decimal precision, including old preference JSON without the field;
- precision-aware engineering formatting and exact raw formatting;
- axis selection is distinct from payload-cell selection;
- axis header reads use actual values and axis Apply writes/undoes atomically;
- descriptive and unavailable axes are read-only;
- zoom scaling covers title/content dimensions through pure helper tests where possible;
- BIN-specific last-XDF preference round-trips and updates only after successful loads;
- prompt creation, Use, Keep, Choose, missing-file state, and stale-project invalidation;
- loading/prompt state transitions remain non-blocking and preserve operation safety.

### Verification

Run formatting checks, focused crate tests while developing, all workspace tests, workspace build, and the existing NookLink JSONL capability/validation probe against the bundled fixture. A native visual smoke test is attempted only if a controllable desktop surface is available; otherwise report that limitation explicitly.

## Compatibility and rollout

Preference deserialization is backward-compatible through serde defaults and sanitization. Existing table-window keys continue to identify the same tables, so geometry/zoom are retained and only the new precision field is defaulted for old entries. Existing XDFs without usable axis addresses remain safe and read-only. No destructive migration or automatic BIN rewrite is performed.
