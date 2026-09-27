# TunerPro UI Function Reference (clean-room)

Source material: tunerpro.net official features page, TunerPro V5 Online Help
(preferences), the v5 change history (2009–2010 builds), and the HP Academy
overview of TunerPro / TunerPro RT. This is a functional inventory only — no
code or assets were copied. Use it as the feature map for TunerNook's UI and
for cross-agent handoff (Codex or otherwise).

TunerPro ships in two editions: **TunerPro** (free, offline editing) and
**TunerPro RT** (adds real-time data acquisition and emulation). The same UI
core is shared; RT adds dockable acquisition/emulation tooling.

---

## 1. Application shell and workspace

- MDI-style workspace: multiple child editors (tables, scalars, functions,
  flags, hex editor) open at once, tabbed/cascade/tilable.
- Dockable tab containers: XDF item tree, parameter summary list, data
  acquisition tab container (floatable), dashboard, utility panes.
- Full-screen mode for editors and utility panes (double-click tab title bar).
- View menu: show/hide item tree, summary list, toolbars, status bar.
- Status bar: table dimensions when a table is selected; address-tracing
  status ("Waiting" / "Tracing" / "ERROR!"); DA record state (green when
  recording); compare-bin title.
- Title bar shows loaded BIN/XDF names and active compare bin.
- Multi-instance support (`-multiinstance` command-line switch).
- `-file <path>` command-line switch to pre-load XDF/ADX/BIN files
  (multiple shortcuts = multiple car configs).
- File associations for `.bin`, `.ecu/.xdf`, `.adx`, `.adl`, `.bsl`.
- Keyboard shortcuts: user-customizable via a Preferences tab; most menu
  commands are assignable (Shift/Alt/Ctrl + ASCII/F-key/nav key).
- Graph window "always on top" preference.
- Next/Previous Bin Editor navigation (',' and '.') — cycle open editors.
- Window menu: Close All, tile, cascade, list of open editors.

## 2. File / BIN management

- Open BIN; Save BIN; Save As; recent-file MRU lists.
- Load Last BIN at startup; Load Last XDF at startup (default XDF).
- Default (auto-loading) bin definitions: map ID byte at an ID offset to an
  XDF; auto-loads matching definition; optional prompt when ID unrecognized.
- Bin Change Logging: optional plain-text `.log` beside the BIN recording
  every edit (original value, new value, item, timestamp).
- Bin stacking tool: compile multiple BINs into one image (`.bsl` layouts).
- Export BIN data/contents to plain text (includes tables, functions, axis
  labels; units optionally included; delimiters sanitized).
- Checksum recalculation as defined in the XDF when uploading whole BIN.
- Emulation verification decoupled from XDF bin-size (works without XDF).

## 3. XDF (definition) management

- Open/save XDF; XML-based v5 format; readable parser-failure messages
  (e.g., "created with a newer version").
- Import definitions from other formats (v4 XDF, ECU-specific importers).
- Definition encryption/password protection (use discouraged).
- XDF item tree: parameters grouped by type (scalars/tables/functions/flags)
  or by category; category mode; uncategorized grouping; empty nodes hidden;
  sort by type or category; selection tracking with editors/summary list.
- Parameter Summary List (formerly Item Summary List): values, units column,
  slider init, editing item details from the list, red highlight for
  uncommitted edits (cleared on save).
- Parameter editors via F2: address, dimensions, strides, populate-by-row/
  column, conversion equations, linked variables, aliases (two items editing
  the same address space update each other, with commit prompt).
- Axis definitions: internal/pure, scaled, normalized, linked 2D tables;
  LSB-first/signed flags; linked-object update notifications.
- XDF item defaults (stored in the XDF): default word size, signedness,
  LSB-first for new items.
- Units library; more unit types over time.
- Open Next/Previous Bin Editor from XDF menu.
- XDF header: bin size, title, author, etc.; XML-escape safety on save.

## 4. Parameter editors (BIN editing)

- **Table editor** (2D/3D tables):
  - Grid with calculated values or raw hex; per-cell editing; uncommitted
    change tracking; commit on OK/save.
  - 2D graphing with multi-point dragging; 3D surface plot with colored
    height map, rotation/translation/zoom (state persists per editor).
  - Auto-range 2D/3D; wireframe mode; color-by-value gradients (real-time).
  - Row/column reversal; reverse rows/columns preference; color table cells
    by min/max; column auto-width.
  - Table manipulation: scale rows, columns, whole tables; 3D smoothing with
    user-defined alpha; range selection editing (pull fuel/spark out of a
    region); stepping (+/-) with global-offset awareness.
  - Copy/paste tables to/from Excel; paste marks table changed and updates
    graph.
  - Compare mode: overlay up to 4 compare bins; edit-vs-compare difference
    display; compare line in 2D graph; compare in function editor too.
  - Axis display: row/col axis values from linked or internal axes.
  - Address tracing overlay (RT): live hit marker bubble in the table.
- **Scalar (constant) editor**:
  - Slider + numeric entry; real-time emulator updates on slider release;
    raw-hex view toggle; out-of-range address safety; change-log entries.
- **Function editor**:
  - Curve graph with draggable points, stepping, compare mode overlay,
    0-point crash safety.
- **Flag (bitmask) editor**:
  - Bit size 8/16/32; bit-number selector; named bit states; compare
    highlighting in difference tool.
- **Raw editor**: full-featured hex/octal/decimal/binary editor.

## 5. Search, navigation, and difference tools

- Find items by title keyword, address, or size; item finder window;
  double-click result opens editor (even when tree is hidden).
- List differences between two BINs rapidly (difference tool); flags
  highlighted in item tree; double-click to open editor.
- Compare current BIN against up to 4 compare BINs simultaneously
  (Load/Setup Compare Bins; compare title in title bar).
- View BIN graphically (pattern/table discovery views).
- Jump forward/backward 1 sample (RT log playback; Shift+[/]).

## 6. Math engine

- Full-featured equation engine: dozens of functions (sin, cos, min, max, …),
  referential calculation (output of one calc feeds another), linked
  variables in conversions; conversion editor with variable typing
  (native vs. linked).

## 7. Data acquisition (RT)

- ADX definitions: packets, headers/footers, "align to header" option,
  DTCs, bitmasks, lookup tables (hex/int/float/string with auto-conversion),
  ADXValue equations (XML-escape safe), monitors, dashboards, histograms,
  gauges (customizable needle arc, colors), lists.
- Real-time monitoring/dashboard; fully customizable; dockable tab windows;
  multi-series side-scrolling history charts; trend histograms.
- Record to file (ADL); playback with pause/seek (1/10 samples); custom
  playback speed; background scanning after stop.
- Log exporter (XDL→CSV): item selection (alphabetized), move up/down
  ordering, available/exported item counts, delimiter sanitization.
- A/D extra channels (WBO2 etc. via AutoProm); passthrough modes
  (8192/160-baud GM; Quarterhorse shared port); 160-baud sync fixes.
- Data tracing: link XDF constants/tables to ADX items; cursor querying in
  monitors (click/drag); tracing state in status bar.
- Interface config: COM port enumeration (valid ports only), interface type
  selection (Standard Serial / AutoProm / Shared Port / Moates Passthrough),
  cable test button; detect hardware at startup; release hardware.
- History table (histogram) axes: auto or manually defined to match XDF.

## 8. Emulation (RT)

- Emulator support via plug-in architecture (Moates Ostrich 1/2.0,
  Quarterhorse, Roadrunner, BURN1/2, AutoProm, Jaybird, ALDU1, F3/F8,
  Xtronics Romulator I/II, MAFT Pro, CobraRTP, MOTIV ReFlex, Split Second).
- Upload whole BIN or edited cells ("Upload Whole Tables" preference);
  checksum recalc on full upload; "Disable Checksum By Uploading" patch.
- Real-time edits: graph/slider changes push live; keep-editors-open option.
- Dual emulator support (16-bit, mirrored I/O).
- Simultaneous emulate + log (passthrough/shared port).
- Address hit tracing from any 64KB bank (Ostrich 2.0).
- Emulation audible notifications; Initialize Attached Devices tool.

## 9. Preferences (behavior surfaces)

- General: startup loading (last XDF/BIN), edit logging, item-list tracking,
  warn-on-close, display data raw-hex vs. calculated, compare mode
  (edit−compare vs. compare−edit), color table cells, reverse rows/columns.
- Graph: always-on-top, color 3D, wireframe, auto-range, point selection
  radius.
- Colors: workspace background, highlight, editable/non-editable cells,
  selected-cell outline, grid lines, editable text.
- Keyboard shortcuts editor (see §1).
- Default bin definitions editor (see §2).
- DA/emulation: interface type/port, high-speed USB, keep editors open,
  upload whole tables, real-time graph/slider push, dual emulator,
  checksum-disable upload, hardware detect at startup.

## 10. Plug-ins and extensibility

- Plug-in architecture for emulation hardware (vendors ship their own).
- Data acquisition breaking out to plug-ins (ScannerPro extensible).
- Custom Tools: user-defined menu entries launching external apps with
  parameterized command lines (e.g., pass current BIN path).
- Update check directly from menus.

## 11. Misc behaviors worth cloning

- Uncommitted-change red highlighting; prompts on editor close.
- Alias propagation between editors of the same address space.
- Linked-object update notifications (axis → dependent tables).
- Splash screen; file-type icons; drag & drop file opening in all windows.
- Units shown in summary list; sorted combos; tooltips in definition editors.
- Per-table 3D graph state persistence while editor stays open;
  View→Reset Perspective.

---

## TunerNook mapping notes (current state)

Already present in this workspace (crates/tuner-app, egui GUI + tuner-core/
tuner-xdf/tuner-transfer/tuner-cal):

- BIN/XDF open, inspect, hashes; categorized/sortable browser and Ctrl+F search;
  floating table windows with zoom and project memory; scalar, flag, bitmask,
  raw, axis, and formula editing; async loading and diagnostics; rectangular
  cell selection, native clipboard, right-click actions, one-undo cell tools,
  range coloring, and recorded undo history; compare BIN modes and 3D surfaces;
  guarded transfer planning/apply; raw hex/ASCII plus typed integer/float
  interpretation, search, and auto-coloring synchronized with the active table;
  and authenticated local NookLink control of supported commands and state.

Not yet present (candidates, ordered by value):

1. Broader NookLink coverage for every visible control, including complete
   graph focus/selection/editing and customizable workspace settings.
2. Advanced table operations: row/column or whole-map scaling, smoothing,
   interpolation, stepping, gradients, and range-fill workflows.
3. Full XDF authoring for parameters, functions, categories, metadata, and
   reusable definitions; current XDF save exports effective conversion edits.
4. Checksum providers and verified checksum repair during Save As.
5. Automatic XDF matching by BIN identification bytes or known fingerprints.
6. Multiple compare BIN overlays, plus richer standalone 2D plotting.
7. A persistent per-BIN edit journal beyond the current undo/redo history.
8. BIN text export, bin stacking, and user-defined external tools.

The `tuner-ui/v1` protocol controls the running desktop app through a local
authenticated JSONL bridge, while `tuner-api/v1` remains the file-oriented
inspection, comparison, transfer, and byte-edit API. NookLink coverage is
useful but does not yet expose every UI gesture or setting.

The larger TunerPro RT roadmap remains separate: ADX/ADL acquisition,
dashboards, hardware interfaces, live tracing, emulator plug-ins, and live
upload. XDF import/encryption and separate native OS editor windows are also
outside the current implementation.
