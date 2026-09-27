# TunerNook Floating Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the flat parameter/editor experience with a category-first, ADHD-friendly floating table workspace with tabs, remembered geometry, organization commands, and collapsible side drawers.

**Architecture:** Keep `WorkspaceState` as the only owner of BIN/XDF data and all transaction operations. Add presentation-only table-window records and versioned UI preferences to `TunerApp`; render each open table with a stable `egui::Window` ID and use `egui::Panel::show_switched` for browser/inspector drawers. Persist geometry and organization state as rounded UI metadata keyed by the active XDF normalized fingerprint plus semantic parameter ID.

**Tech Stack:** Rust 1.98.1, `eframe`/`egui` 0.36.2, `rfd` 0.17.2, `serde`/`serde_json`, and the existing `tuner-core`/`tuner-xdf` crates.

**Spec:** `docs/superpowers/specs/2026-09-13-floating-workspace-design.md`

## Global Constraints

- Keep `tuner-core` and `tuner-xdf` unchanged; this slice is presentation/state orchestration in `tuner-app`.
- Use floating windows inside one native TunerNook window; do not create separate OS windows or processes.
- Category organization defaults to `Categories`; search still matches title, unique ID, semantic ID, and category.
- Table window keys are `xdf.normalized_fingerprint + "|" + parameter.semantic_id`.
- Persist only UI metadata; never persist BIN bytes, transactions, or automatic source-file reopen instructions.
- All edits still call `ParameterDefinition::write_engineering_cell` inside one `BinDocument` transaction.
- Save As remains explicit and no-overwrite; fixture BIN/XDF files are read-only and their hashes must remain unchanged.
- Settings version 1 must migrate to the new version with defaults for new fields; unknown fields remain ignored.
- Headless tests must disable persistence or use explicit temporary paths and must not write the real user settings file.

---

### Task 1: Add persistent workspace-view models and migration

**Files:**

- Modify: `crates/tuner-app/src/lib.rs`
- Modify: `docs/superpowers/specs/2026-09-13-floating-workspace-design.md` only if implementation clarifies a user-visible rule
- Test: `crates/tuner-app/src/lib.rs`

**Interfaces:**

- Consumes: existing `AppPreferences`, `WorkspaceState`, and XDF normalized fingerprints.
- Produces: `BrowserOrganization`, `TableWindowMemory`, expanded drawer fields, table-memory maps, settings v1 migration, and `OpenTable`/arrangement state used by later tasks.

- [x] **Step 1: Write failing model and migration tests.**

Add tests with these exact behaviors:

```rust
#[test]
fn settings_v1_migrate_to_category_first_workspace_defaults() {
    let migrated = preferences_from_json(
        r#"{
            "version": 1,
            "layout": "Standard",
            "theme": "Dark",
            "density": "Comfortable",
            "table_display": "Engineering",
            "show_browser": true,
            "show_editor": true,
            "show_inspector": true,
            "show_diagnostics": true,
            "shortcuts": {}
        }"#,
    );
    assert_eq!(migrated.version, SETTINGS_VERSION);
    assert_eq!(migrated.browser_organization, BrowserOrganization::Categories);
    assert!(!migrated.browser_collapsed);
    assert_eq!(migrated.browser_width, 300);
}

#[test]
fn table_window_memory_round_trips_and_clamps_zoom() {
    let mut memory = TableWindowMemory {
        x: -20,
        y: 40,
        width: 720,
        height: 480,
        zoom_percent: 250,
        scroll_x: 12,
        scroll_y: 34,
    };
    memory.sanitize();
    assert_eq!(memory.zoom_percent, 200);
    let encoded = serde_json::to_string(&memory).unwrap();
    assert_eq!(serde_json::from_str::<TableWindowMemory>(&encoded).unwrap(), memory);
}
```

- [x] **Step 2: Run the model tests and confirm the new interfaces are missing.**

Run `cargo test -p tuner-app settings_v1_migrate_to_category_first_workspace_defaults table_window_memory_round_trips_and_clamps_zoom`.

Expected: compilation fails because the new preference fields and memory type do not exist. Keep the assertions unchanged.

- [x] **Step 3: Implement version-2 preferences and runtime models.**

Add:

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum BrowserOrganization {
    Categories,
    FavoritesFirst,
    RecentFirst,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TableWindowMemory {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub zoom_percent: u16,
    pub scroll_x: u32,
    pub scroll_y: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OpenTable {
    key: String,
    semantic_id: String,
    title: String,
    category: String,
    pinned: bool,
    memory: TableWindowMemory,
    restore_scroll: bool,
    geometry_request: bool,
}
```

Extend `AppPreferences` with `#[serde(default)]` and these fields:

```rust
pub browser_organization: BrowserOrganization,
pub collapsed_categories: Vec<String>,
pub favorite_tables: Vec<String>,
pub recent_tables: Vec<String>,
pub browser_collapsed: bool,
pub inspector_collapsed: bool,
pub browser_width: u32,
pub inspector_width: u32,
pub table_windows: BTreeMap<String, TableWindowMemory>,
pub tab_orders: BTreeMap<String, Vec<String>>,
```

Set `SETTINGS_VERSION` to `2`. Accept versions 1 and 2 in
`preferences_from_json`, set migrated values to version 2, and call a
sanitizer that clamps widths to `180..=640`, inspector width to `220..=640`,
zoom to `50..=200`, deduplicates favorites/categories, and bounds recent keys
to 32. `TableWindowMemory::default()` must be `(80, 80, 760, 520, 100, 0, 0)`.

Add `table_key(xdf_fingerprint: &str, semantic_id: &str) -> String` and
`OpenTable` construction helpers on `TunerApp`; opening a table must load its
remembered `TableWindowMemory` or the default without touching documents.

- [x] **Step 4: Run model and migration tests.**

Run `cargo fmt --all -- --check` and `cargo test -p tuner-app settings_v1_migrate_to_category_first_workspace_defaults table_window_memory_round_trips_and_clamps_zoom`.

Expected: both tests pass and old settings JSON still loads with dark theme,
default category organization, and no drawer collapse.

### Task 2: Implement category-first browsing and collapsible drawers

**Files:**

- Modify: `crates/tuner-app/src/lib.rs`
- Test: `crates/tuner-app/src/lib.rs`

**Interfaces:**

- Consumes: `WorkspaceState::filtered_parameters`, new `BrowserOrganization`, `collapsed_categories`, `browser_collapsed`, `inspector_collapsed`, and remembered widths.
- Produces: grouped browser rendering, category/favorite/recent controls, arrow rails, and drawer commands.

- [x] **Step 1: Add grouped-parameter and drawer-state tests.**

Add a `grouped_parameters(&self, query: &str) -> Vec<ParameterGroup>` helper
where `ParameterGroup { category: String, parameters: Vec<ParameterSummary> }`
is sorted by category/title/semantic ID. Test that the synthetic fixture gives
one `Map` group with one parameter and that a headless app can toggle
`browser_collapsed`/`inspector_collapsed` without changing `workspace.bin` or
`workspace.xdf`.

- [x] **Step 2: Implement category grouping and drawer rendering.**

Replace the flat `show_browser` body with category headers using
`CollapsingHeader::open(Some(!collapsed_categories.contains(&category)))`.
Render counts, favorite stars, and recent/favorite sections according to
`browser_organization`; clicking a parameter calls `open_table` and marks it
recent. Add `Expand all`, `Collapse all`, and organization controls while
preserving global text search.

Render browser and inspector through `Panel::show_switched`:

```rust
let mut expanded = !self.preferences.browser_collapsed;
let mut collapse_requested = false;
egui::Panel::show_switched(
    ui,
    &mut expanded,
    egui::Panel::left("browser-collapsed").exact_size(32.0),
    egui::Panel::left("browser-expanded")
        .resizable(true)
        .min_size(180.0)
        .default_size(self.preferences.browser_width as f32),
    |ui, is_expanded| {
        if is_expanded {
            if ui.button("‹").on_hover_text("Collapse browser").clicked() {
                collapse_requested = true;
            }
            self.show_browser_contents(ui);
        } else if ui.button("›").on_hover_text("Expand browser").clicked() {
            collapse_requested = true;
        }
    },
);
if collapse_requested {
    expanded = !expanded;
}
self.preferences.browser_collapsed = !expanded;
```

Use the mirrored `<`/`>` rail on the inspector. Read
`egui::PanelState::load(ui.ctx(), Id::new("browser-expanded"))` and the
inspector equivalent after rendering to remember the last expanded widths;
clamp them before writing preferences. The arrow must remain visible whenever
the drawer is mounted, and the existing View panel visibility toggles continue
to hide/show the complete drawer.

- [x] **Step 3: Add drawer command descriptors and persistence hooks.**

Add `CollapseBrowser` and `CollapseInspector` built-ins with stable IDs
`view.collapse-browser` and `view.collapse-inspector`. Route arrows, menu
entries, and command-palette entries through those IDs or the same state
methods. Mark preference changes dirty and flush after pointer interaction is
released so dragging a divider does not write settings every frame.

- [x] **Step 4: Run category and drawer tests.**

Run `cargo test -p tuner-app grouped_parameters -- --nocapture`, then run the
drawer-state test by its exact name, and run `cargo fmt --all -- --check`.
Expected: grouped ordering, category-first defaults, collapse toggles, and
document isolation all pass.

### Task 3: Add floating table windows, tabs, zoom, and organization

**Files:**

- Modify: `crates/tuner-app/src/lib.rs`
- Test: `crates/tuner-app/src/lib.rs`

**Interfaces:**

- Consumes: `OpenTable`, `TableWindowMemory`, `WorkspaceState::cell_view`, `select_cell`, and `apply_engineering_text`.
- Produces: `TunerApp::open_table`, `focus_table`, `close_table`, `toggle_table_pin`, `move_table_tab`, `cascade_tables`, `tile_tables`, `close_other_tables`, `reset_table_layout`, and the floating canvas renderer.

- [x] **Step 1: Write failing tab and organization tests.**

Using the existing synthetic fixture, add tests that:

```rust
#[test]
fn opening_tables_creates_unique_tabs_and_focuses_existing_tab() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(Some(bin), Some(xdf), None, None);
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&id));
    assert!(app.open_table(&id));
    assert_eq!(app.open_tables.len(), 1);
    assert_eq!(
        app.active_table_key.as_deref(),
        Some(app.open_tables[0].key.as_str())
    );
}

#[test]
fn table_organization_changes_tabs_without_mutating_documents() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(Some(bin), Some(xdf), None, None);
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    app.open_table(&id);
    let bytes = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    app.cascade_tables();
    app.reset_table_layout();
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), bytes.as_slice());
}
```

Add a second parameter to the synthetic fixture or use two real fixture IDs so
the test covers tab order, close-other, pin, and recent ordering.

- [x] **Step 2: Implement open/focus/close/pin/recent table state.**

Implement `open_table(&mut self, semantic_id: &str) -> bool` to construct the
key, select the parameter, reuse an existing tab, or append one new `OpenTable`.
Persist recent keys newest-first with a maximum of 32. `focus_table` updates
`active_table_key`, selects the shared workspace parameter, and schedules the
egui layer to move to top. `close_table` removes only that tab and focuses the
nearest remaining tab; `close_other_tables` preserves the active and pinned
tabs. Pin state is persisted in `favorite_tables` and is visible on tabs.

Restore saved tab order from `tab_orders` when a table is opened, but ignore
stale keys. Reorder tabs using explicit left/right controls so the behavior is
keyboard-accessible and deterministic; save the order under the active XDF
normalized fingerprint.

- [x] **Step 3: Render the tab strip and floating table windows.**

Replace the single central editor with `show_workspace_canvas`. The tab strip
must show active/favorite state, close buttons, a quick-switch search field, and
organization buttons for focus, cascade, tile, close-other, and reset. For each
open table, clone its view model before rendering and use a stable window ID:

```rust
let window_id = egui::Id::new(("table-window", table.key.as_str()));
egui::Window::new(&table.title)
    .id(window_id)
    .resizable(true)
    .movable(true)
    .collapsible(false)
    .constrain_to(canvas_rect)
    .default_pos(egui::pos2(table.memory.x as f32, table.memory.y as f32))
    .default_size(egui::vec2(
        table.memory.width as f32,
        table.memory.height as f32,
    ))
    .open(&mut window_open)
    .show(context, |ui| show_table_contents(ui, table));
```

Inside each window retain the existing safe editor controls and grid. Add
bounded zoom buttons (50–200%, increments of 10%) and apply the zoom to cell
width/height and table spacing. Give each table's `ScrollArea` a stable ID and
restore its saved offset only on first open; after rendering, capture
`ctx.memory().area_rect(window_id)` and `ScrollArea` output into rounded
`TableWindowMemory` values. A closed window removes its runtime tab but leaves
its geometry memory available for reopening.

- [x] **Step 4: Implement cascade, tile, focus, and reset commands.**

Add built-ins and stable IDs:

```text
view.focus-active-table   Focus Active Table
view.cascade-tables       Cascade Tables
view.tile-tables          Tile Tables
view.close-other-tables   Close Other Tables
view.reset-table-layout   Reset Table Layout
```

`cascade_tables` assigns successive positions `(40 + 28*n, 40 + 28*n)` with
the default size. `tile_tables` divides the current canvas into a bounded grid
using the number of open tables. `reset_table_layout` replaces every open
table's memory with defaults. Each command sets `geometry_request=true` and
requests one explicit `egui::Context::memory_mut(|memory| memory.reset_areas())`
before the next render, then the normal window defaults establish the requested
positions/sizes. The command changes only presentation state and never the
workspace documents.

- [x] **Step 5: Run tab/window state tests.**

Run `cargo test -p tuner-app opening_tables_creates_unique_tabs_and_focuses_existing_tab table_organization_changes_tabs_without_mutating_documents` and `cargo fmt --all -- --check`. Expected: open/focus deduplication, tab organization, geometry defaults, and document immutability pass.

### Task 4: Integrate settings, command palette, docs, and verification

**Files:**

- Modify: `crates/tuner-app/src/lib.rs`
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-09-13-floating-workspace-design.md`
- Modify: `docs/superpowers/plans/2026-09-13-floating-workspace.md`
- Test: `crates/tuner-app/src/lib.rs`

**Interfaces:**

- Consumes: complete category/drawer/table-window state and the existing stable command registry.
- Produces: persisted organization workflow, updated documentation, verification evidence, and an explicit native GUI smoke-test checklist.

- [x] **Step 1: Add command dispatch and persistence tests.**

Test that `view.collapse-browser`, `view.collapse-inspector`,
`view.focus-active-table`, `view.cascade-tables`, `view.tile-tables`, and
`view.reset-table-layout` dispatch without changing BIN/XDF bytes. Test that
preferences round-trip preserves organization, drawer collapse, a memory map,
and tab order, while malformed JSON still returns defaults.

- [x] **Step 2: Finish command palette/View/Workspace integration.**

Place drawer controls in View → Panels, organization commands in View →
Workspace, and table quick-switch/arrange actions in the tab strip. Menus,
shortcuts, and palette entries use the stable command IDs; direct arrows and
table controls call the same underlying state methods. Persist after layout,
drawer, favorite, recent, tab-order, zoom, scroll, and geometry changes. Keep
disabled commands visible with reasons when no active table exists.

- [x] **Step 3: Update README and spec/plan status.**

Document category-first browsing, floating in-app windows, tabs, remembered
geometry/zoom, favorite/recent organization, and arrow rails. State clearly
that these are internal windows, not OS-level windows, and that no source BIN
is automatically written or reopened. Keep the spec implementation status
open until the native GUI smoke test has actually been performed.

- [x] **Step 4: Run complete automated verification.**

Run:

```powershell
cargo fmt --all -- --check
cargo build --workspace
cargo test --workspace
cargo run --quiet -p tuner-cli -- xdf-validate "Test bin and xdf/SCGa05_cal.xdf" "Test bin and xdf/SCGa05_cal.bin"
Get-FileHash -Algorithm SHA256 "Test bin and xdf\SCGa05_cal.xdf"
Get-FileHash -Algorithm SHA256 "Test bin and xdf\SCGa05_cal.bin"
```

Confirm `valid:true`, `issue_count:0`, `parameter_count:2915`,
`bin_size_bytes:654336`, and the known fixture hashes.

- [ ] **Step 5: Perform native GUI smoke test and self-review.**

Launch `cargo run -p tuner-app`, open the fixture pair, verify category
headers and counts, open at least two tables, move/resize/zoom a table, switch
tabs, pin/favorite one table, collapse and restore browser and inspector with
the arrow buttons, run cascade/tile/reset, restart, and verify geometry memory.
Confirm no fixture hash changes and no unintended output file remains. Then
scan for unfinished-work wording, check every UI edit path, and mark this plan
and spec complete only when the native surface visibly confirms the flow.

## Plan self-review checklist

- Scope is one coherent floating-workspace slice; compare/transfer/hex/plotting remain follow-on capabilities.
- `WorkspaceState` remains the document/mutation boundary; floating windows only consume its owned view methods.
- Settings contain rounded presentation metadata only and migrate version 1 safely.
- Drawer collapse is reversible with visible rails and preserves expanded widths.
- Every organization action is deterministic, command-registry-backed, and document-immutable.

Current environment note: automated compilation and state tests cover the
native egui code, but this host currently exposes no native computer surface
for the visual smoke test. Leave Task 4 Step 5 open until that interaction can
be performed manually.
