# TunerNook Desktop App Shell Design

Status: Implementation complete; native GUI smoke test pending in a native-enabled session
Date: 2026-09-13
Project: TunerNook

## Context

The reusable TunerNook backend now provides bounds-checked BIN documents,
transactional edits, normalized XDF parameters, engineering conversions,
column-major table addressing, validation, and transfer planning. The
workspace does not yet contain the planned desktop application. This slice
adds the first runnable Windows-first application without moving safety logic
into UI code.

The supplied `Test bin and xdf` pair is the smoke-test fixture. Opening it in
the app must be read-only with respect to the fixture files; editing requires
an explicit Save As output path. Troubleshooting is a first-class workflow: a
detailed report page makes the current document, parser, mapping, selection,
history, and UI state easy to inspect and share.

## Goals

1. Add a `tuner-app` desktop executable using `egui`/`eframe`.
2. Let a user open a BIN and an XDF through native file dialogs, independently
   or in either order.
3. Show normalized XDF parameters grouped by category with text filtering and
   selection.
4. Show the selected parameter's metadata, storage, conversion, validation
   state, and attached axis metadata in an inspector.
5. Render scalar values and supported table cells in engineering units, with a
   raw-value fallback when engineering conversion is unavailable.
6. Edit one selected cell at a time through `ParameterDefinition` and a
   `tuner-core` transaction; provide undo and redo controls.
7. Save a modified BIN only through an explicit Save As dialog, preserving the
   existing no-overwrite safety rule.
8. Surface recoverable load, mapping, conversion, and save errors in the app
   instead of panicking or silently ignoring them.
9. Make workflow, layout, and display choices user-configurable from the first
   shell so future features can register capabilities without rewriting the
   main window.
10. Provide a detailed debug report window with clipboard copy and safe export,
    built from the same live state used by the workspace.

## Non-goals for this slice

- Compare-mode UI, batch-transfer UI, or checksum calculation.
- Hex-editor UI, copy/paste, multi-cell transforms, interpolation, smoothing,
  plotting, or 3D visualization.
- Autosave/session recovery or an unsafe third-party plugin ABI.
- Direct editing of axis-definition objects; attached axis metadata is shown
  read-only.
- A network service or browser front end.
- Changing the public core/XDF/API/CLI safety contracts.
- Remote telemetry or automatic report upload; report sharing is explicit and
  user-triggered.

These features remain later vertical slices over the same `WorkspaceState`
boundary. The customization foundation is included now so later slices add
commands and panels through stable extension points instead of accumulating
hard-coded toolbar branches.

## Architecture

Add a workspace member `crates/tuner-app` with a small library plus binary:

```text
tuner-app/src/lib.rs  -> WorkspaceState, view-model helpers, egui application
tuner-app/src/main.rs -> native eframe startup only
        |
        +-> tuner-core  (BIN bytes, transactions, undo/redo, Save As)
        +-> tuner-xdf   (XDF model, cell access, validation, conversions)
        +-> eframe/egui (rendering and input)
        +-> rfd         (native open/save dialogs)
```

`WorkspaceState` owns the optional active `BinDocument` and `XdfDocument`,
their source paths, pending dialog values, current filter, selected semantic
ID, selected cell, and the latest user-visible status. It is the only app
boundary allowed to call load, validation, cell read/write, undo/redo, or
Save As. The egui view reads state and dispatches intent methods; it never
accesses or mutates a byte slice directly.

The app shell also owns an `AppPreferences` value and a `CommandRegistry`.
Preferences are UI/workflow state only: panel visibility, layout preset, theme,
UI density, table value display mode, and shortcut bindings. The registry gives
each action a stable string ID, label, category, description, default shortcut,
and availability reason. The toolbar, menu, keyboard handler, and command
palette all consume the same registry entries. A future compare, transfer,
checksum, or visualization module can add a descriptor and handler through the
registry without changing parameter storage code or duplicating enablement
logic.

Loading a BIN does not require an XDF. Loading an XDF does not require a BIN.
Whenever both are present, the state recomputes `validate_against_document`
and retains all warnings for the diagnostics panel. A parameter can be
selected even when it has a warning, but an edit is rejected with the exact
backend error and leaves the BIN unchanged.

`WorkspaceState::debug_report` produces a text snapshot of the active BIN
identity, dirty/edit-history state, XDF identity and normalized counts, parser
diagnostics, mapping-validation issues, selected parameter storage/layout, and
selected-cell values. `TunerApp` appends settings, visible panels, and command
availability. The report window can copy this text to the clipboard or export
it to a user-selected new text file; it never uploads or implicitly writes a
source document.

## Dependencies and startup

Use:

```toml
eframe = { version = "0.36.2", default-features = false, features = ["default_fonts", "glow"] }
rfd = { version = "0.17.2", default-features = false }
```

The crate is included in the workspace and built by `cargo build --workspace`.
The native entry point creates an `eframe::NativeOptions` window titled
`TunerNook`, with a desktop-sized initial viewport, and runs `TunerApp`.
Application startup must not load the supplied fixture or any implicit path.

## UI layout and interactions

The first window uses four durable regions:

- **Toolbar:** Open BIN, Open XDF, Save As, Undo, Redo, and a compact dirty /
  validation status summary.
- **Left parameter browser:** a filter field, an All category entry, category
  headings, and matching parameter rows. Filtering matches title, unique ID,
  semantic ID, and category name case-insensitively.
- **Center editor:** a scalar/flag value row or a scrollable table grid. Each
  cell shows an engineering value when available and a raw value tooltip or
  fallback. A click selects a cell; a single edit field and Apply button stage
  one cell edit. Table coordinates are shown as zero-based row/column labels.
- **Right inspector:** selected parameter identity, kind, source title,
  address/range, dimensions, logical strides, signedness, endianness,
  numeric kind, raw flags, orientation, conversion source, axis labels and
  units, plus applicable diagnostics.

The View menu exposes panel visibility, layout presets (`Standard`, `Data
Entry`, and `Diagnostics`), light/dark/system theme, compact/comfortable
density, and engineering/raw table display. `Ctrl+K` opens a command palette
that filters every registered command by ID, label, category, and description;
disabled commands remain visible with their availability reason. A keyboard
settings view lets the user replace a command's shortcut or restore defaults.
Shortcuts are persisted as stable command-ID mappings, so adding a future
command does not invalidate existing preferences.

The View and Tools menus also expose `Open Debug Report` (default `F12`). The
report window is intentionally independent of the diagnostics panel: the panel
is a compact live summary, while the report is a complete troubleshooting
snapshot suitable for copying into an issue or saving beside a test run.

When no BIN or XDF is loaded, the center area explains the missing prerequisite
and provides the corresponding open action. When an XDF is loaded without a
BIN, metadata remains browseable but values and editing are unavailable.

`rfd::FileDialog` supplies open dialogs filtered to BIN/XDF extensions but
does not restrict the user to those extensions. The Save As dialog defaults to
the active BIN stem plus `-edited.bin`; the backend still rejects an existing
target and the app reports that error inline.

## Value and edit policy

The center editor uses the normalized model as follows:

- A selected scalar is rendered as a one-cell grid.
- A table uses `dimensions.rows` and `dimensions.columns`; cell access goes
  through `read_engineering_cell` and falls back to `read_raw_cell` when the
  conversion or storage is unsupported.
- Raw integer values display as signed or unsigned according to `StorageSpec`.
  Binary32 values display as finite engineering values when conversion allows
  it and retain their raw IEEE bits in the inspector.
- The edit field accepts one finite decimal engineering value. The state calls
  `write_engineering_cell` inside a transaction labeled `edit <title>`, commits
  only after backend validation succeeds, and refreshes the selected value.
- Unsupported, non-integral integer, non-finite, out-of-range, invalid-cell,
  and conversion failures are displayed as an error and do not create an undo
  entry.
- Undo and redo call the existing `BinDocument` history methods and clear or
  preserve selection only as needed to refresh values. Dirty state is derived
  from `BinDocument::is_dirty()`.

## Error and diagnostics behavior

Every user operation has a short status message with severity and a persistent
recent-message area. Errors include the operation and path where available.
The app does not terminate for malformed XML, missing files, invalid mappings,
or rejected writes. XDF diagnostics remain visible independently of fatal
load errors. Core mutation errors are reported only after the transaction has
rolled back or failed before mutation.

The compact diagnostics panel shows the latest status, mapping issues, and XDF
parser diagnostics. The debug report expands this into document hashes and
paths, byte counts and dirty/history state, normalized object counts, every
parser/validation diagnostic, selected mapping geometry and storage flags,
selected-cell values, settings, panel state, and command enablement reasons.

The app reuses `tuner-core`'s diagnostic vocabulary for operation context in
future logging work; this first UI slice keeps the user-visible message model
local and does not add a second machine-readable protocol.

## Customization and evolution boundary

The shell separates capabilities from placement:

- `CommandDescriptor` describes what an action does and when it is available;
  `CommandRegistry` owns stable IDs, default shortcuts, and dispatch.
- `PanelId` and `LayoutPreset` describe where built-in views appear;
  `AppPreferences` controls visibility and presentation without changing the
  document model.
- `AppExtension` is a safe in-process boundary for future built-in modules to
  contribute command and panel descriptors. The first slice registers only the
  core workspace commands and does not load arbitrary dynamic libraries.
- Settings are versioned JSON under the platform app-data directory, with an
  explicit reset-to-defaults action and graceful fallback when a file is
  missing, malformed, or contains an unknown field.

This gives the app an evolving spine: new functions can be introduced as
capabilities, exposed through the same palette/shortcut system, and placed by
preferences without coupling them to a particular fixed layout. It does not
promise that every future module is implemented in this slice.

## Testing

The `tuner-app` library tests cover:

1. Loading BIN and XDF independently and recomputing validation when both are
   available.
2. Case-insensitive filtering and deterministic parameter ordering.
3. Selecting a fixture table and reading a non-square column-major cell.
4. Successful engineering edit through one committed transaction, dirty state,
   undo, and redo.
5. Rejected invalid edit leaves bytes, dirty state, and undo depth unchanged.
6. Save As creates a new file and refuses an existing target.
7. Command IDs are stable, default shortcuts are available, custom shortcuts
   round-trip through settings, and disabled commands retain explanations.
8. Layout presets and panel toggles change presentation state without changing
   BIN/XDF state; malformed settings restore defaults.
9. The debug report contains document identity, parser/mapping diagnostics,
   selected storage details, history state, and app customization state.

The normal workspace tests remain mandatory. A manual Windows smoke test runs
the app, opens the supplied BIN/XDF, selects a parameter, edits a supported
cell, observes the dirty state, undoes it, and verifies Save As output. The
original fixture hashes are checked after the smoke test.

## Acceptance criteria

- `cargo fmt --all -- --check`, `cargo build --workspace`, and
  `cargo test --workspace` pass with `tuner-app` included.
- `cargo run -p tuner-app` opens a native TunerNook window without requiring
  command-line arguments or changing any fixture.
- The supplied XDF can be opened, its 2,915 parameters can be filtered and
  selected, and the app shows its normalized `column_major`/storage metadata.
- The supplied BIN can be opened independently, then paired with the XDF;
  validation warnings are visible and no fixture bytes are changed.
- A supported scalar or table cell can be edited, undone, and redone through
  the existing transaction/journal path.
- Save As writes a new BIN only when the target does not already exist.
- No UI code directly mutates BIN byte buffers, and all rejected edits leave
  the active BIN byte-identical to its pre-edit state.
- The command palette and keyboard actions use the same registry as the
  toolbar/menu, and user preferences survive an app restart or reset cleanly.
- The debug report is available from the View/Tools workflow, can be copied or
  exported without overwriting an existing report, and never transmits data
  automatically.
- Built-in capabilities can be extended through descriptors and handlers
  without changing the core/XDF crates or the persisted settings schema.

## Follow-on slices

After this shell is verified, the next app slices can add the synchronized hex
editor and compare view, then batch-transfer dry-run/commit UI, and finally
visualization, checksum status, recoverable session state, and an extensible
diagnostic event log that can feed the current report page. Each slice must
continue using `WorkspaceState` and the existing core transaction boundaries.
