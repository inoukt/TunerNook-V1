# TunerNook Catalog, Search, and Responsive Loading Design

Status: Implemented
Date: 2026-09-13
Project: TunerNook

## Context

TunerNook already has a usable egui desktop shell with persistent internal
table windows, tabs, project layout memory, a debug report, and safe BIN/XDF
editing boundaries. The next slice must make a large XDF pleasant to explore
without sacrificing parser correctness or UI responsiveness.

The supplied SCGa05 XDF demonstrates two related problems. Its
`CATEGORYMEM` values are one-based positions into the header category list,
although the current parser resolves them as declared numeric indices. That
produces misleading flat labels and missing-category warnings. The XDF also
contains nested category memberships that should be shown as a TunerPro-style
tree. In addition, parsing, validation, and report generation currently run on
the egui event thread, so a loading indicator cannot animate during the costly
parts of those operations.

The user also wants the catalog to show table size, to sort tables by useful
metadata, and to keep a powerful search workspace open while browsing. A true
second Windows taskbar window is intentionally deferred: the current
`eframe`/`egui_glow` renderer reports that multiple native viewports are not
supported reliably. The first implementation therefore provides a persistent,
movable, resizable, refocusable internal search window with a taskbar-like
workspace tab and saved geometry. The state boundary will leave room for a
future native companion window without coupling search logic to egui widgets.

## Goals

1. Resolve the supplied XDF's one-based category references while preserving
   declared-index behavior for ordinary XDFs and retaining diagnostics for
   genuinely missing references.
2. Present nested memberships as a stable category tree. Newly discovered
   category folders start collapsed; saved project expansion state wins on
   subsequent openings.
3. Add table metadata to parameter summaries, browser rows, tabs, table title
   bars, inspectors, and debug reports: kind, rows × columns, element count,
   mapped byte footprint, and address.
4. Allow catalog ordering by category, title, type, dimensions/elements, byte
   size, address, favorites, or recent use, with ascending/descending order.
   Category hierarchy remains the primary structure when category mode is
   selected; ordering is applied within siblings.
5. Add a Ctrl+F Search workspace that can remain open, be moved/resized, and
   restore its geometry and filters. It searches metadata and mapped values,
   supports exact and wildcard matching, and can favorite/open a result
   directly.
6. Move BIN/XDF loading, validation, project restoration, Save As, and debug
   export into a background operation coordinator wherever the underlying
   operation is safe to detach from the UI thread.
7. Show a gentle amber busy overlay with a pulsing caution character and
   rotating contextual messages, disable conflicting actions while an
   operation is active, and surface failures in the existing status and debug
   areas.
8. Add deterministic headless tests for category dialect detection/tree paths,
   catalog metadata and sorting, search parsing/matching/value search,
   operation state transitions, persisted search state, and stale background
   results.

## Non-goals

- A true separate native Windows taskbar process/window in this slice.
- Editing multiple cells at once, plotting, interpolation, compare mode,
  transfer, hex editing, or plugin-provided search providers.
- Storing BIN bytes, search result snapshots, or source-file content in
  preferences.
- Treating a malformed or unsupported value as a match merely because its
  display text contains an error message.
- Sound, flashing warnings, automatic source-file overwrite, or implicit
  saving of BIN bytes.

## Architecture

```text
tuner-xdf
  category reference dialect + resolved membership names
        |
tuner-app catalog/search/operations pure models
        |
WorkspaceState        document ownership, selection, value reads, edits
TunerApp              window/session state, sorting, search UI, orchestration
egui                  browser, persistent internal windows, busy overlay
```

### Category interpretation

The parser keeps the raw `CategoryMembership.category_index` value for
diagnostics and compatibility, and adds a resolved category index/mode to the
normalized document. Category reference mode is detected once per document:

- Use declared-index mode by default.
- When header indices are a contiguous zero-based list and a membership value
  is one greater than the largest declared index while `value - 1` is a valid
  header position, use one-based-position mode for the document. This is the
  unambiguous signal in the supplied fixture (`57` refers to position 57 while
  the highest declared index is `56`).
- If no mode has decisive evidence, retain declared-index mode. A missing
  reference remains a `missing-category` diagnostic rather than being guessed
  from a nearby value.

The normalized membership exposes the resolved category name and resolved
header index, while the raw value remains available. Parameter category paths
are constructed by sorting memberships by their `slot`/level and retaining
resolved names. The first path component is the legacy `ParameterDefinition::category`
value for compatibility. The app uses the full path for tree rendering and
uses a stable escaped path key for expansion persistence.

The supplied fixture is expected to yield the source-truth root groups:

```text
Airflow (100), Fuel (261), MPI (88), Immo Power Class (5),
Impulse Combustion (28), Limiter (91), Turbocharger (67), Misc (311),
MPI Inhibitors (4), Spark (232), System (6), Torque Management (54),
Torque Model (52), Torque Request (82), Diagnostic Init (30),
Engine Diagnostics (1504)
```

The screenshot's MPI count of 68 is not substituted for the XDF's actual 88.

### Catalog metadata and ordering

`ParameterSummary` gains:

- `category_path: Vec<String>` and a stable `category_path_key`;
- `kind`;
- `rows`, `columns`, `element_count`;
- `byte_size` from the normalized mapped range;
- `address`;
- `semantic_id`, `unique_id`, and title.

The browser has independent selectors for organization and ordering. The
default organization is `Categories`; ordering defaults to title ascending.
The available sort keys are `Category`, `Title`, `Type`, `Dimensions`,
`Elements`, `Bytes`, `Address`, `Favorite`, and `Recent`. A direction toggle
is visible beside the selector. Sorts are stable and use semantic ID as the
final tie-breaker. Favorite/recent organization changes ranking without
destroying the category tree.

Visible table labels use a compact, consistent format such as:

```text
Limiter · Maximum allowed torque · table · 12×12 · 144 el · 288 B
```

The title bar and tab may shorten the human title when space is tight, but the
full metadata is available in a tooltip and the inspector.

### Search workspace

Search is modeled separately from the browser filter so Ctrl+F can stay open
without stealing the browser's current context. `SearchState` contains:

- open/closed state and `SearchWindowMemory` geometry/zoom;
- query text;
- match mode: `Contains`, `Exact`, or `Wildcard` (`*` matches any sequence and
  `?` matches one character, case-insensitive);
- field scope: `All`, `Metadata`, `Title`, `Category`, `IDs`, `Type`,
  `Address/Size`, `Raw Values`, or `Engineering Values`;
- result sort key/direction;
- selected result and a bounded result count.

Metadata matching uses normalized lower-case text. Exact mode compares the
whole normalized field value. Wildcards are compiled into a small linear
matcher with no regular-expression dependency. Numeric value searches accept
decimal and hexadecimal integer forms for raw values and finite decimal forms
for engineering values. Engineering matches use a documented relative
tolerance of `1e-9 * max(abs(query), 1.0)` so formatting differences do not
hide an equal value. Value errors are retained as result diagnostics but never
match.

Search results display title, category path, type, shape, element count, byte
size, address, and the first matching value location/value when applicable.
Unreadable raw cells and engineering conversion failures appear as per-cell
diagnostics without becoming matches. Each row has a favorite toggle and an
Open/Focus action. Double-clicking a result opens or focuses its table. Ctrl+F focuses the search query if the window exists,
otherwise it opens the window and focuses the query. Closing the window does
not discard the last query or filters. A compact Search tab/button in the
workspace header acts like a taskbar entry for reopening/focusing it.

Search evaluates in a background worker after the query/filter debounce. The
worker receives an immutable snapshot of the XDF and BIN bytes and returns a
generation-tagged result. A newer query invalidates older results; stale
results are ignored.

### Background operations and busy feedback

`OperationCoordinator` owns a monotonically increasing operation ID, active
operation kind, phase text, start time, cancellation/stale state, and a
channel for worker results. It accepts `OperationKind` values:

```text
LoadingBin, LoadingXdf, Validating, RestoringWorkspace,
SavingBin, ExportingDebugReport, Searching
```

The coordinator is UI-testable without threads through explicit
`begin/update/complete/fail` transitions. Real workers use `std::thread` and
`std::sync::mpsc`; document objects are built off-thread and installed on the
UI thread only after the operation ID and source identity are checked.

Loading a BIN or XDF from a native picker starts the worker after the picker
returns; the OS file picker itself is allowed to block because it is outside
the parser operation. Validation and project restoration are scheduled after
document installation. Save As and debug export use immutable snapshots so
the UI document is not borrowed across the worker boundary. A failed or stale
operation never replaces a newer document or clears unrelated open tables.

While active, conflicting document, edit, and second-search actions are
disabled. The UI requests repaint at a short interval. The overlay uses an
amber caution glyph whose scale/opacity follows a smooth sine pulse and
rotates among operation-specific messages every 1.4 seconds. It does not
flash, play sound, or prevent the user from inspecting already loaded data.
The bottom diagnostics panel keeps the final success/error message and the
debug report includes the active/last operation, duration, and stale-result
counts.

### Persistence and future native companion support

Project preferences are bumped to a new version with tolerant defaults. The
following are project-scoped because they depend on the active XDF:

- category tree expansion state;
- browser organization, sort key, sort direction, and browser filter;
- favorite/recent keys;
- search query, match mode, field scope, result sort, and last selected result;
- open table keys, tab order, table geometry, and active table.

Search window geometry and zoom are stored with the project workspace so each
calibration project restores its own arrangement. Missing or invalid fields
fall back to safe defaults. Window positions are clamped to the current
viewport.

The search engine and coordinator do not depend on egui. A later native
companion window can reuse them by exchanging serialized `SearchState` and
generation-tagged results over IPC; the first slice does not add that process
or protocol.

## UI behavior

- Browser starts in category mode, with category folders collapsed on first
  open and restored thereafter.
- Category headers show counts of all table results in that subtree; nested
  children are shown below the parent with indentation and their own subtree
  counts.
- Browser rows show favorite control, title, type, and compact dimensions/bytes.
- Table window title bars and tabs include dimensions and byte size.
- Sort controls remain available in category mode and never make a long label
  force the drawer wider; the existing horizontal/vertical scroll frames stay
  in place.
- Search opens as a resizable/movable internal desktop window. It can sit over
  the editor, remain open while table windows are moved, and be reopened from
  the Search workspace tab.
- Empty/no-document states explain which document is required. Searches that
  need values explain that a BIN is required and still return metadata matches.
- The busy overlay is centered over the usable workspace, with the toolbar and
  diagnostics status still visible.

## Error and safety behavior

- Parser dialect detection is documented in the debug report and emits a
  warning/info diagnostic when one-based mode is selected.
- Invalid category paths fall back to `Uncategorized` without panicking.
- Search parse errors (for example an invalid numeric value) are shown inline
  and yield zero value matches without blocking metadata search when the scope
  includes metadata.
- Worker errors are reported with operation and path context. Stale results
  are silently discarded except for a debug counter.
- BIN edits continue to go through `ParameterDefinition` transactions. Search,
  sorting, favorites, window placement, and loading indicators never mutate
  BIN bytes.
- Save As and report export retain no-overwrite behavior. The worker checks the
  target before publishing and reports a race-safe backend error if it becomes
  occupied.

## Testing and acceptance

Headless tests must cover:

1. one-based fixture category detection and exact-index synthetic fallback;
2. nested category paths, stable tree keys, counts, and first-open collapse;
3. summary size metadata and each catalog sort key/direction;
4. contains/exact/wildcard metadata matching and numeric raw/engineering value
   matching, including invalid-value behavior;
5. direct favorite/open actions from search state;
6. search geometry/filter persistence and tolerant migration;
7. operation state transitions, rotating-message selection, stale result
   rejection, and disabled conflicting actions;
8. existing workspace edits, project layout persistence, debug report, fixture
   validation, and source BIN/XDF hashes.

Native smoke verification should open the fixture pair, observe the category
tree and loading overlay, expand a nested Limiter or Torque Model branch,
change sort options, open Search with Ctrl+F, search for a title and a value,
favorite a result, open it, move/resize Search and a table, close/reopen Search,
restart, and confirm the saved state. No fixture file may be written.
