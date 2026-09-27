# Screen-safe dock and workspace snapshots — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Keep implementation single-agent unless the user asks otherwise.

**Goal:** Keep TunerNook windows reachable and add a bottom dock plus auto-saved, reopenable window-layout snapshots for the same BIN/XDF project.

**Architecture:** Reuse `ProjectPreferences` as the canonical state for the active layout. Store only inactive named layout snapshots, capturing and applying the existing persisted view-state fields and memory types. Add a small window-target/focus model for dock behavior, route table actions through current methods, and keep document data/undo ownership unchanged.

**Tech Stack:** Rust 2021, `eframe`/`egui` 0.36.2, `serde`/`serde_json`; no new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-24-window-management-dock-design.md`

## Global Constraints

- TunerNook remains one native egui window; no second process or native child-window system.
- BIN/XDF documents, BIN bytes, unsaved edits, undo/redo history, and unsaved XDF drafts are never stored in layout snapshots.
- Existing `ProjectPreferences` fields remain canonical for the active layout; do not add a duplicate live copy of current table/tool geometry.
- A layout switch changes presentation only and never reloads BIN/XDF or mutates document data.
- Preserve the startup BIN+XDF consent prompt; restore the selected layout only after the matching project/XDF is loaded.
- Clamp restored geometry to the current usable area, including the bottom dock and visible diagnostics panel.
- Workspace has no `.git`; record before/final snapshots and progress in `.superpowers/sdd/2026-09-24-window-management-dock/` instead of adding commit steps.
- Do not repeat an identical focused test workflow more than 10 times; use the final app/workspace suites once each.

## Review Focus

1. **Legacy project preferences:** old JSON must become the live `Default` layout without losing open-table keys, tab order, geometry, or tool settings. Pin this in Task 3.
2. **Active-state duplication:** after edits and layout switches, the active layout must exist only in current `ProjectPreferences`; inactive snapshots are the only copies. Pin this in Task 3–4.
3. **Stale XDF/table identities:** after reopening with a different or reduced XDF, skip only stale windows/selections and keep valid layout entries. Pin this in Task 4.
4. **Small/high-DPI displays:** negative or oversized saved rectangles must clamp without zero-size loops or unreachable title bars. Pin this in Task 1 and the Windows smoke test.
5. **Focus/minimize versus close:** minimizing preserves the window and its state; closing uses the current close behavior and does not route edits to a hidden table. Pin this in Task 2 and Task 5.
6. **Data safety:** switching/creating/deleting a layout cannot change BIN bytes, XDF identity, or approval state. Assert in Task 4 and Task 6.

---

### Preflight: SDD trail before code

**Files:**
- Create: `.superpowers/sdd/2026-09-24-window-management-dock/progress.md`
- Create: `.superpowers/sdd/2026-09-24-window-management-dock/snapshots/before/`

- [x] Record the approved spec/plan paths, baseline test counts, and no-Git status in `progress.md`.
- [x] Copy the current `lib.rs`, `main.rs`, `map_search.rs`, `compare.rs`, `ui_ipc.rs`, `docs/tuner-ui-ipc.md`, and `AGENTS.md` into matching `snapshots/before/` paths before editing product code.

---

### Task 1: Shared geometry clamping and safe native startup

**Files:**
- Create: `crates/tuner-app/src/window_geometry.rs`
- Modify: `crates/tuner-app/src/lib.rs` (`table_effective_geometry` and module declaration)
- Modify: `crates/tuner-app/src/map_search.rs` (`clamp_map_finder_geometry`)
- Modify: `crates/tuner-app/src/compare.rs` (`CompareWindowMemory::constrain_to_bounds`)
- Modify: `crates/tuner-app/src/main.rs` (native startup options)
- Test: `crates/tuner-app/src/window_geometry.rs` unit tests and existing app geometry tests

**Interfaces:**
- Produces `window_geometry::clamp_rect(rect: egui::Rect, bounds: egui::Rect, minimum: egui::Vec2) -> egui::Rect`.
- Produces `window_geometry::fit_root_window(work_area: egui::Rect, desired: egui::Vec2, minimum: egui::Vec2) -> RootWindowGeometry`, where `RootWindowGeometry` contains `outer_position`, `inner_size`, and `minimum_inner_size` in egui points.
- Produces `window_geometry::shell_safe_rect(viewport: egui::Rect, toolbar: egui::Rect, dock: egui::Rect, diagnostics: Option<egui::Rect>) -> egui::Rect`, the root window content area excluding the toolbar and bottom strips.
- Existing typed memories keep their public fields; their adapters use `clamp_rect` rather than introducing a second geometry model.

- [x] **Step 1: Write failing clamp tests.**

```rust
#[test]
fn clamp_rect_keeps_oversized_offscreen_window_inside_small_view() {
    let bounds = egui::Rect::from_min_size(egui::pos2(40.0, 30.0), egui::vec2(300.0, 180.0));
    let saved = egui::Rect::from_min_size(egui::pos2(-500.0, 600.0), egui::vec2(640.0, 480.0));
    assert_eq!(
        clamp_rect(saved, bounds, egui::vec2(160.0, 100.0)),
        bounds
    );
}

#[test]
fn clamp_rect_preserves_in_bounds_geometry_and_is_idempotent() {
    let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let saved = egui::Rect::from_min_size(egui::pos2(120.0, 80.0), egui::vec2(400.0, 300.0));
    let fitted = clamp_rect(saved, bounds, egui::vec2(160.0, 100.0));
    assert_eq!(fitted, saved);
    assert_eq!(clamp_rect(fitted, bounds, egui::vec2(160.0, 100.0)), fitted);
}

#[test]
fn fit_root_window_centers_inside_dpi_scaled_work_area() {
    let work_area = egui::Rect::from_min_size(egui::pos2(120.0, 40.0), egui::vec2(1280.0, 680.0));
    let fitted = fit_root_window(work_area, egui::vec2(1400.0, 900.0), egui::vec2(960.0, 640.0));
    assert!(fitted.inner_size.x <= 1248.0);
    assert!(fitted.inner_size.y <= 624.0);
    assert!(fitted.minimum_inner_size.x <= fitted.inner_size.x);
    assert!(fitted.minimum_inner_size.y <= fitted.inner_size.y);
    assert!(fitted.minimum_inner_size.y <= 468.0);
    assert!(fitted.outer_position.x >= work_area.left());
    assert!(fitted.outer_position.y >= work_area.top());
    assert!(fitted.outer_position.x + fitted.inner_size.x + 32.0 <= work_area.right());
    assert!(fitted.outer_position.y + fitted.inner_size.y + 56.0 <= work_area.bottom());
    let small = fit_root_window(
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 520.0)),
        egui::vec2(1400.0, 900.0),
        egui::vec2(960.0, 640.0),
    );
    assert!(small.inner_size.x <= 768.0 && small.inner_size.y <= 464.0);
    assert!(small.minimum_inner_size.y < small.inner_size.y);
}

#[test]
fn clamp_rect_handles_empty_work_area() {
    let bounds = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(0.0, 100.0));
    assert_eq!(
        clamp_rect(
            egui::Rect::from_min_size(egui::pos2(-100.0, 500.0), egui::vec2(300.0, 200.0)),
            bounds,
            egui::vec2(160.0, 100.0),
        ),
        egui::Rect::from_min_size(bounds.min, egui::Vec2::ZERO)
    );
}

#[test]
fn shell_safe_rect_excludes_toolbar_dock_and_diagnostics() {
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let toolbar = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 44.0));
    let dock = egui::Rect::from_min_size(egui::pos2(0.0, 720.0), egui::vec2(1200.0, 32.0));
    let diagnostics = egui::Rect::from_min_size(egui::pos2(0.0, 752.0), egui::vec2(1200.0, 48.0));
    assert_eq!(
        shell_safe_rect(viewport, toolbar, dock, Some(diagnostics)),
        egui::Rect::from_min_max(egui::pos2(0.0, 44.0), egui::pos2(1200.0, 720.0))
    );
}
```

- [x] **Step 2: Run the geometry tests and verify red.**

Run: `cargo test -p tuner-app window_geometry -- --nocapture`  
Expected: compile failure because `window_geometry::clamp_rect` does not exist.

- [x] **Step 3: Add the minimal shared rect helper and typed adapters.**

```rust
pub fn clamp_rect(rect: egui::Rect, bounds: egui::Rect, minimum: egui::Vec2) -> egui::Rect {
    let max_size = bounds.size().max(egui::Vec2::ZERO);
    if max_size.x <= f32::EPSILON || max_size.y <= f32::EPSILON {
        return egui::Rect::from_min_size(bounds.min, egui::Vec2::ZERO);
    }
    let min_size = minimum.min(max_size).min(max_size * 0.75);
    let size = egui::vec2(
        rect.width().max(min_size.x).min(max_size.x),
        rect.height().max(min_size.y).min(max_size.y),
    );
    let max_position = bounds.max - size;
    let min = egui::pos2(
        rect.left().clamp(bounds.left(), max_position.x),
        rect.top().clamp(bounds.top(), max_position.y),
    );
    egui::Rect::from_min_size(min, size)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RootWindowGeometry {
    pub outer_position: egui::Pos2,
    pub inner_size: egui::Vec2,
    pub minimum_inner_size: egui::Vec2,
}

pub fn fit_root_window(
    work_area: egui::Rect,
    desired: egui::Vec2,
    minimum: egui::Vec2,
) -> RootWindowGeometry {
    let chrome = egui::vec2(32.0, 56.0);
    let safe_inner = (work_area.size() - chrome).max(egui::Vec2::ZERO);
    let inner_size = desired.min(safe_inner).max(egui::Vec2::ZERO);
    let minimum_inner_size = minimum.min(inner_size).min(inner_size * 0.75);
    let outer_position = work_area.min + (work_area.size() - inner_size - chrome) * 0.5;
    RootWindowGeometry { outer_position, inner_size, minimum_inner_size }
}

pub fn shell_safe_rect(
    viewport: egui::Rect,
    toolbar: egui::Rect,
    dock: egui::Rect,
    diagnostics: Option<egui::Rect>,
) -> egui::Rect {
    let top = toolbar.bottom().clamp(viewport.top(), viewport.bottom());
    let bottom_panel_top = diagnostics.map_or(dock.top(), |rect| rect.top().min(dock.top()));
    let bottom = bottom_panel_top.clamp(top, viewport.bottom());
    egui::Rect::from_min_max(
        egui::pos2(viewport.left(), top),
        egui::pos2(viewport.right(), bottom),
    )
}
```

Implement `clamp_rect` with component-wise size caps and position clamping. Implement `fit_root_window` by reserving 32×56 logical points for native chrome, capping the desired inner size to the remaining work area, capping the minimum to the desired minimum and 75% of the fitted inner size, and centering the resulting outer rectangle. This keeps the app resizable on constrained displays. Use the helper in `table_effective_geometry`, `clamp_map_finder_geometry`, and compare memory bounds conversion. Keep egui `Window::constrain_to` where it already handles the correct app canvas; add explicit constraints later for windows that currently lack them.

Implement `shell_safe_rect` from the current root content rectangle and the live toolbar, dock, and diagnostics panel response rectangles. If Diagnostics is hidden, use the dock's top edge as the bottom bound. Side panels remain overlayable; tables continue using their narrower central editor canvas.

- [x] **Step 4: Fit the root window to the Windows work area.**

In `main.rs`, use a small `#[cfg(target_os = "windows")]` FFI wrapper for `SystemParametersInfoW(SPI_GETWORKAREA)` and `GetDpiForSystem`, convert the primary work-area `RECT` from physical pixels to egui points, and feed it to `fit_root_window`. Set the viewport position/inner/minimum sizes from that result and keep `with_clamp_size_to_monitor_size(true)`. If the API fails or on non-Windows, use a centered `1200×620` window with an `800×480` minimum and monitor-size clamping. Do not maximize or add a dependency.

```rust
#[cfg(target_os = "windows")]
#[repr(C)]
struct WinRect { left: i32, top: i32, right: i32, bottom: i32 }

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn SystemParametersInfoW(action: u32, parameter: u32, output: *mut std::ffi::c_void, flags: u32) -> i32;
    fn GetDpiForSystem() -> u32;
}

#[cfg(target_os = "windows")]
fn primary_work_area_points() -> Option<egui::Rect> {
    const SPI_GETWORKAREA: u32 = 0x0030;

    let mut rect = WinRect { left: 0, top: 0, right: 0, bottom: 0 };
    if unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut rect as *mut _ as *mut _, 0) } == 0 {
        return None;
    }
    let scale = 96.0 / unsafe { GetDpiForSystem() }.max(1) as f32;
    Some(egui::Rect::from_min_max(
        egui::pos2(rect.left as f32 * scale, rect.top as f32 * scale),
        egui::pos2(rect.right as f32 * scale, rect.bottom as f32 * scale),
    ))
}

#[cfg(not(target_os = "windows"))]
fn primary_work_area_points() -> Option<egui::Rect> {
    None
}
```

- [x] **Step 5: Run focused geometry regressions.**

Run: `cargo test -p tuner-app window_geometry -- --nocapture`  
Expected: clamp and work-area geometry tests pass; existing table/map/compare geometry tests remain green.

---

### Task 2: Stable dock target and focus/minimize state

**Files:**
- Create: `crates/tuner-app/src/window_manager.rs`
- Modify: `crates/tuner-app/src/lib.rs` (module declaration and `ProjectPreferences` dock state)
- Test: `crates/tuner-app/src/window_manager.rs`

**Interfaces:**
- `WindowId` is a serde-stable enum for Table(key), Surface(key), Search, MapFinder, HexEditor, Compare, CompareSurface, XdfEditor, ActionHistory(table_key), DebugReport, Settings, and NookLink.
- `WindowId::to_stable_id() -> String` and `WindowId::from_stable_id(&str) -> Option<WindowId>` use fixed singleton IDs `search`, `map_finder`, `hex_editor`, `compare`, `compare_surface`, `xdf_editor`, `debug_report`, `settings`, and `nooklink`, plus `table:<opaque-key>`, `surface:<opaque-key>`, and `history:<opaque-key>`.
- `WindowDockState` stores `focused: Option<WindowId>`, `minimized: BTreeSet<WindowId>`, `focus_order: Vec<WindowId>`, and `open_tool_windows: BTreeSet<WindowId>`. `open_tool_windows` is only for tool windows without an existing persisted open flag; table/surface/Search/Hex/Map Finder open state is derived from current project fields.
- Methods: `focus(&mut self, id: WindowId)`, `focused(&self) -> Option<&WindowId>`, `open_tool(&mut self, id: WindowId)`, `minimize(&mut self, id: &WindowId)`, `restore(&mut self, id: &WindowId)`, `close(&mut self, id: &WindowId)`, `is_minimized(&self, id: &WindowId) -> bool`, `is_open_tool(&self, id: &WindowId) -> bool`.

- [x] **Step 1: Write lifecycle tests against the desired dock state.**

```rust
#[test]
fn focusing_minimizing_restoring_and_closing_tracks_recent_target() {
    let table = WindowId::Table("xdf|table:a".into());
    let nooklink = WindowId::NookLink;
    assert_eq!(WindowId::from_stable_id(&table.to_stable_id()), Some(table.clone()));
    let mut state = WindowDockState::default();
    state.focus(table.clone());
    state.open_tool(nooklink.clone());
    state.focus(nooklink.clone());
    state.minimize(&nooklink);
    assert!(state.is_minimized(&nooklink));
    assert_eq!(state.focused(), Some(&table));
    state.restore(&nooklink);
    assert_eq!(state.focused(), Some(&nooklink));
    state.close(&nooklink);
    assert_eq!(state.focused(), Some(&table));
    assert!(!state.is_open_tool(&nooklink));
}
```

- [x] **Step 2: Run the focused test and verify red.**

Run: `cargo test -p tuner-app focusing_minimizing_restoring_and_closing_tracks_recent_target -- --nocapture`  
Expected: compile failure because the target/state types are absent.

- [x] **Step 3: Implement the pure target/state model.**

Derive `Clone`, `Debug`, `Deserialize`, `Eq`, `Ord`, `PartialEq`, `PartialOrd`, and `Serialize` for stable IDs/state. Implement the stable string forms as `table:<opaque-key>`, `surface:<opaque-key>`, `history:<opaque-key>`, and fixed lowercase singleton names; parse keys with `split_once(':')` so IDs inside keys are preserved. `minimize` removes the target from focus order and selects the first remaining valid target; `restore` clears minimized status then calls `focus`; `open_tool` records only tool IDs without an existing open flag; `close` removes open-override/focus/minimized/order state. Existing table/surface/tool open booleans remain owned by `TunerApp`/`ProjectPreferences`.

```rust
pub fn focus(&mut self, id: WindowId) {
    self.minimized.remove(&id);
    self.focus_order.retain(|known| known != &id);
    self.focus_order.insert(0, id.clone());
    self.focused = Some(id);
}

pub fn minimize(&mut self, id: &WindowId) {
    self.minimized.insert(id.clone());
    self.focus_order.retain(|known| known != id);
    if self.focused.as_ref() == Some(id) {
        self.focused = self.focus_order.first().cloned();
    }
}
```

- [x] **Step 4: Verify lifecycle and serde behavior.**

Run: `cargo test -p tuner-app window_manager -- --nocapture`  
Expected: focus fallback, minimize/restore, close cleanup, and a JSON round-trip pass.

---

### Task 3: Reusable workspace snapshot schema and legacy migration

**Files:**
- Create: `crates/tuner-app/src/workspace_layouts.rs`
- Modify: `crates/tuner-app/src/lib.rs` (`ProjectPreferences`, version migration, active selection persistence, serde derives for `AxisSelection`/`CellSelection`)
- Test: `crates/tuner-app/src/workspace_layouts.rs` and embedded app project-preference tests

**Interfaces:**
- Add `WorkspaceSnapshot { id: u64, name: String, state: WorkspaceViewState }` for inactive layouts only.
- Add `WorkspaceViewState::capture(&ProjectPreferences) -> Self` and `apply_to(&self, &mut ProjectPreferences)`; dock/minimize/focus fields live in `ProjectPreferences` and are captured with the existing view fields.
- Add `ProjectPreferences.active_workspace_id: u64`, `active_workspace_name: String`, `saved_workspace_snapshots: Vec<WorkspaceSnapshot>`, `dock_state: WindowDockState`, `selected_semantic_id: Option<String>`, `selected_cell: (usize, usize)`, `selected_cells: Option<CellSelection>`, `selected_axis: Option<AxisSelection>`, and `utility_window_geometry: BTreeMap<String, WindowGeometryMemory>`. Keep existing preference fields as the canonical active layout.
- Add the utility rectangle map to `WorkspaceViewState`; it covers only utility windows with no existing typed per-project geometry memory.
- Bump `PROJECT_SETTINGS_VERSION` from `9` to `11`; versions ≤10 become active workspace `Default` without storing a duplicate snapshot.

- [x] **Step 1: Add failing migration and capture/apply tests.**

In the new module's test block, import the crate types with `use super::*;` and `use crate::hex::HexDisplayFormat;`.

```rust
#[test]
fn old_project_layout_becomes_default_without_duplicate_snapshot() {
    let mut legacy = ProjectPreferences::for_identity("bin-a");
    legacy.open_table_keys = vec!["xdf|table:a".into()];
    legacy.active_table_key = Some("xdf|table:a".into());
    legacy.table_windows.insert(
        "xdf|table:a".into(),
        TableWindowMemory {
            x: 40,
            y: 30,
            width: 700,
            height: 420,
            zoom_percent: 125,
            scroll_x: 40,
            scroll_y: 90,
            decimal_places: 2,
            coloring: TableColorSettings::default(),
            fit_to_content: Some(false),
        },
    );
    let mut old = serde_json::to_value(legacy).unwrap();
    old["version"] = serde_json::json!(9);
    old.as_object_mut().unwrap().remove("active_workspace_id");
    old.as_object_mut().unwrap().remove("active_workspace_name");
    old.as_object_mut().unwrap().remove("saved_workspace_snapshots");
    old.as_object_mut().unwrap().remove("dock_state");
    old.as_object_mut().unwrap().remove("utility_window_geometry");
    old.as_object_mut().unwrap().remove("selected_semantic_id");
    old.as_object_mut().unwrap().remove("selected_cell");
    old.as_object_mut().unwrap().remove("selected_cells");
    old.as_object_mut().unwrap().remove("selected_axis");

    let restored = project_preferences_from_json(&old.to_string(), "bin-a");
    assert_eq!(restored.active_workspace_name, "Default");
    assert_eq!(restored.active_workspace_id, 0);
    assert_eq!(restored.open_table_keys, ["xdf|table:a"]);
    assert_eq!(restored.table_windows["xdf|table:a"].zoom_percent, 125);
    assert_eq!(restored.table_windows["xdf|table:a"].scroll_y, 90);
    assert!(restored.saved_workspace_snapshots.is_empty());
}

#[test]
fn workspace_view_state_round_trips_existing_table_and_tool_memories() {
    let mut project = ProjectPreferences::for_identity("bin-a");
    project.open_table_keys = vec!["xdf|table:a".into()];
    project.table_windows.insert(
        "xdf|table:a".into(),
        TableWindowMemory {
            x: 40,
            y: 30,
            width: 700,
            height: 420,
            zoom_percent: 125,
            scroll_x: 40,
            scroll_y: 90,
            decimal_places: 2,
            coloring: TableColorSettings::default(),
            fit_to_content: Some(false),
        },
    );
    project.hex_window.display_format = HexDisplayFormat::Float32;
    project.selected_semantic_id = Some("table:main".into());
    project.selected_cell = (3, 4);
    project.selected_cells = Some(CellSelection::new((1, 2), (3, 4)));
    project.selected_axis = Some(AxisSelection { axis_index: 0, index: 2 });
    let snapshot = WorkspaceSnapshot {
        id: 1,
        name: "Wide map".into(),
        state: WorkspaceViewState::capture(&project),
    };
    project.saved_workspace_snapshots.push(snapshot.clone());
    let encoded = serde_json::to_string(&project).unwrap();
    let loaded: ProjectPreferences = serde_json::from_str(&encoded).unwrap();
    assert_eq!(loaded.saved_workspace_snapshots[0], snapshot);
    let mut restored = ProjectPreferences::for_identity("bin-a");
    snapshot.state.apply_to(&mut restored);
    assert_eq!(restored.table_windows["xdf|table:a"].zoom_percent, 125);
    assert_eq!(restored.table_windows["xdf|table:a"].scroll_y, 90);
    assert_eq!(restored.hex_window.display_format, HexDisplayFormat::Float32);
    assert_eq!(restored.selected_cell, (3, 4));
    assert_eq!(restored.selected_cells, Some(CellSelection::new((1, 2), (3, 4))));
}
```

- [x] **Step 2: Run the focused migration test and verify red.**

Run: `cargo test -p tuner-app old_project_layout_becomes_default_without_duplicate_snapshot -- --nocapture`  
Expected: compile failure because workspace metadata/snapshot types do not exist.

- [x] **Step 3: Define `WorkspaceViewState` from existing persisted fields.**

Use one explicit serializable layout-state record built from the current preference types:

```rust
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct WorkspaceViewState {
    pub layout: LayoutPreset,
    pub show_browser: bool,
    pub show_editor: bool,
    pub show_inspector: bool,
    pub show_diagnostics: bool,
    pub browser_organization: BrowserOrganization,
    pub catalog_sort: CatalogSortKey,
    pub catalog_sort_direction: SortDirection,
    pub browser_filter: String,
    pub category_state_initialized: bool,
    pub collapsed_categories: Vec<String>,
    pub browser_collapsed: bool,
    pub inspector_collapsed: bool,
    pub browser_width: u32,
    pub inspector_width: u32,
    pub diagnostics_height: u32,
    pub table_windows: BTreeMap<String, TableWindowMemory>,
    pub tab_orders: BTreeMap<String, Vec<String>>,
    pub open_table_keys: Vec<String>,
    pub active_table_key: Option<String>,
    pub search_state: SearchState,
    pub surface_windows: BTreeMap<String, SurfaceViewMemory>,
    pub open_surface_keys: Vec<String>,
    pub active_surface_key: Option<String>,
    pub compare_bin_path: Option<PathBuf>,
    pub compare_xdf_path: Option<PathBuf>,
    pub compare_mode: CompareValueMode,
    pub compare_filter: String,
    pub compare_changed_only: bool,
    pub compare_window: CompareWindowMemory,
    pub compare_surface_windows: BTreeMap<String, SurfaceViewMemory>,
    pub compare_selected_semantic_id: Option<String>,
    pub hex_window_open: bool,
    pub hex_window: HexWindowMemory,
    pub hex_selection: Option<HexSelection>,
    pub hex_address: usize,
    pub map_finder: MapFinderMemory,
    pub utility_window_geometry: BTreeMap<String, WindowGeometryMemory>,
    pub dock_state: WindowDockState,
    pub selected_semantic_id: Option<String>,
    pub selected_cell: (usize, usize),
    pub selected_cells: Option<CellSelection>,
    pub selected_axis: Option<AxisSelection>,
}
```

Add serde derives to `AxisSelection` and `CellSelection`. Implement `capture` as `Self { field: preferences.field.clone(), ... }` for every listed field and `apply_to` as the exact inverse. Keep `bin_identity`, last-XDF path, conversion overrides, known categories, favorites/recents, and agent decision history project-global. Reuse the existing `TableWindowMemory`, `SurfaceViewMemory`, `SearchState`, `CompareWindowMemory`, `HexWindowMemory`, and `MapFinderMemory` types.

- [x] **Step 4: Implement `capture` and `apply_to` as explicit field copies.**

`capture` reads the active canonical `ProjectPreferences`; `apply_to` writes back into those same fields. Do not introduce a second active snapshot. Implement the mappings explicitly so project-global fields cannot accidentally be overwritten:

```rust
impl WorkspaceViewState {
    pub fn capture(p: &ProjectPreferences) -> Self {
        Self {
            layout: p.layout,
            show_browser: p.show_browser,
            show_editor: p.show_editor,
            show_inspector: p.show_inspector,
            show_diagnostics: p.show_diagnostics,
            browser_organization: p.browser_organization,
            catalog_sort: p.catalog_sort,
            catalog_sort_direction: p.catalog_sort_direction,
            browser_filter: p.browser_filter.clone(),
            category_state_initialized: p.category_state_initialized,
            collapsed_categories: p.collapsed_categories.clone(),
            browser_collapsed: p.browser_collapsed,
            inspector_collapsed: p.inspector_collapsed,
            browser_width: p.browser_width,
            inspector_width: p.inspector_width,
            diagnostics_height: p.diagnostics_height,
            table_windows: p.table_windows.clone(),
            tab_orders: p.tab_orders.clone(),
            open_table_keys: p.open_table_keys.clone(),
            active_table_key: p.active_table_key.clone(),
            search_state: p.search_state.clone(),
            surface_windows: p.surface_windows.clone(),
            open_surface_keys: p.open_surface_keys.clone(),
            active_surface_key: p.active_surface_key.clone(),
            compare_bin_path: p.compare_bin_path.clone(),
            compare_xdf_path: p.compare_xdf_path.clone(),
            compare_mode: p.compare_mode,
            compare_filter: p.compare_filter.clone(),
            compare_changed_only: p.compare_changed_only,
            compare_window: p.compare_window.clone(),
            compare_surface_windows: p.compare_surface_windows.clone(),
            compare_selected_semantic_id: p.compare_selected_semantic_id.clone(),
            hex_window_open: p.hex_window_open,
            hex_window: p.hex_window.clone(),
            hex_selection: p.hex_selection,
            hex_address: p.hex_address,
            map_finder: p.map_finder.clone(),
            utility_window_geometry: p.utility_window_geometry.clone(),
            dock_state: p.dock_state.clone(),
            selected_semantic_id: p.selected_semantic_id.clone(),
            selected_cell: p.selected_cell,
            selected_cells: p.selected_cells,
            selected_axis: p.selected_axis,
        }
    }

    pub fn apply_to(&self, p: &mut ProjectPreferences) {
        p.layout = self.layout;
        p.show_browser = self.show_browser;
        p.show_editor = self.show_editor;
        p.show_inspector = self.show_inspector;
        p.show_diagnostics = self.show_diagnostics;
        p.browser_organization = self.browser_organization;
        p.catalog_sort = self.catalog_sort;
        p.catalog_sort_direction = self.catalog_sort_direction;
        p.browser_filter = self.browser_filter.clone();
        p.category_state_initialized = self.category_state_initialized;
        p.collapsed_categories = self.collapsed_categories.clone();
        p.browser_collapsed = self.browser_collapsed;
        p.inspector_collapsed = self.inspector_collapsed;
        p.browser_width = self.browser_width;
        p.inspector_width = self.inspector_width;
        p.diagnostics_height = self.diagnostics_height;
        p.table_windows = self.table_windows.clone();
        p.tab_orders = self.tab_orders.clone();
        p.open_table_keys = self.open_table_keys.clone();
        p.active_table_key = self.active_table_key.clone();
        p.search_state = self.search_state.clone();
        p.surface_windows = self.surface_windows.clone();
        p.open_surface_keys = self.open_surface_keys.clone();
        p.active_surface_key = self.active_surface_key.clone();
        p.compare_bin_path = self.compare_bin_path.clone();
        p.compare_xdf_path = self.compare_xdf_path.clone();
        p.compare_mode = self.compare_mode;
        p.compare_filter = self.compare_filter.clone();
        p.compare_changed_only = self.compare_changed_only;
        p.compare_window = self.compare_window.clone();
        p.compare_surface_windows = self.compare_surface_windows.clone();
        p.compare_selected_semantic_id = self.compare_selected_semantic_id.clone();
        p.hex_window_open = self.hex_window_open;
        p.hex_window = self.hex_window.clone();
        p.hex_selection = self.hex_selection;
        p.hex_address = self.hex_address;
        p.map_finder = self.map_finder.clone();
        p.utility_window_geometry = self.utility_window_geometry.clone();
        p.dock_state = self.dock_state.clone();
        p.selected_semantic_id = self.selected_semantic_id.clone();
        p.selected_cell = self.selected_cell;
        p.selected_cells = self.selected_cells;
        p.selected_axis = self.selected_axis;
    }
}
```

Sanitize every snapshot on project load and skip invalid/duplicate IDs or names with a readable status.

- [x] **Step 5: Add defaults and version-11 migration.**

Set the legacy live fields as active ID `0`, name `Default`, with no saved inactive snapshot. Preserve every existing view field and project identity; do not move or delete prior JSON fields during migration.

- [x] **Step 6: Run the snapshot schema tests.**

Run: `cargo test -p tuner-app workspace_view_state -- --nocapture` and `cargo test -p tuner-app old_project_layout_becomes_default_without_duplicate_snapshot -- --nocapture`  
Expected: existing fields round-trip, old JSON remains usable, and active view state appears only once.

---

### Task 4: Workspace create/switch/rename/delete and reopen flow

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` (`TunerApp` workspace operations, sync/restore, startup BIN/XDF path)
- Test: embedded `crates/tuner-app/src/lib.rs` workspace and startup tests

**Interfaces:**
- `create_workspace_snapshot(name: &str) -> Result<u64, WorkspaceError>` duplicates the current layout into a new active ID and saves the outgoing active layout as inactive.
- Workspace ID `0` is `Default`; allocate the next ID as one greater than the maximum active/saved ID, and return a clear error on overflow.
- `switch_workspace_snapshot(id: u64) -> Result<(), WorkspaceError>` captures current fields, applies the target into existing project preferences, restores in-app windows, and never reloads documents.
- `rename_workspace_snapshot(id: u64, name: &str) -> Result<(), WorkspaceError>` and `delete_workspace_snapshot(id: u64) -> Result<(), WorkspaceError>` validate stable IDs and preserve at least one layout.
- Profile names are trimmed, non-empty, ≤64 Unicode scalar values, and unique case-insensitively within a BIN project.

- [x] **Step 1: Write failing CRUD/switch safety tests.**

```rust
fn app_with_project_fixture() -> TunerApp {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(Some(bin), Some(xdf), None, None);
    app.project_identity = Some("bin-a".into());
    app.project_preferences = ProjectPreferences::for_identity("bin-a");
    app
}

#[test]
fn switching_workspace_restores_layout_without_changing_bin_or_xdf() {
    let mut app = app_with_project_fixture();
    let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    let original = app.project_preferences.active_workspace_id;
    let second = app.create_workspace_snapshot("Diagnostics").unwrap();
    assert_eq!(app.project_preferences.saved_workspace_snapshots.len(), 1);
    assert!(!app.project_preferences.saved_workspace_snapshots.iter().any(|snapshot| snapshot.id == second));
    app.close_table(&key);
    app.switch_workspace_snapshot(original).unwrap();
    assert_eq!(app.open_tables.len(), 1);
    assert_eq!(app.project_preferences.active_workspace_id, original);
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), before.as_slice());
    assert_ne!(second, original);
}

#[test]
fn workspace_snapshot_names_are_unique_and_last_layout_cannot_be_deleted() {
    let mut app = app_with_project_fixture();
    let default_id = app.project_preferences.active_workspace_id;
    let diagnostics = app.create_workspace_snapshot("Diagnostics").unwrap();
    assert!(app.create_workspace_snapshot(" diagnostics ").is_err());
    app.rename_workspace_snapshot(diagnostics, "Street").unwrap();
    let track = app.create_workspace_snapshot("Track").unwrap();
    app.rename_workspace_snapshot(diagnostics, "Road").unwrap();
    assert!(app.rename_workspace_snapshot(track, " rOaD ").is_err());

    app.delete_workspace_snapshot(default_id).unwrap();
    app.delete_workspace_snapshot(track).unwrap();
    assert_eq!(app.project_preferences.active_workspace_id, diagnostics);
    assert!(app.project_preferences.saved_workspace_snapshots.is_empty());
    assert!(app.delete_workspace_snapshot(diagnostics).is_err());
    assert_ne!(diagnostics, default_id);
}
```

- [x] **Step 2: Run the focused test and verify red.**

Run: `cargo test -p tuner-app switching_workspace_restores_layout_without_changing_bin_or_xdf -- --nocapture`  
Expected: compile failure because workspace operations are not implemented.

- [x] **Step 3: Capture active view state through the existing save path.**

Before creating or switching, call `stage_open_table_memory`, `stage_open_surface_memory`, `sync_project_runtime_state`, and `sync_legacy_preferences_to_project`; extend `sync_project_runtime_state` to copy selected semantic/cell/range/axis state into the active project fields, then call `WorkspaceViewState::capture`. Keep `ProjectPreferences` as the active copy.

- [x] **Step 4: Implement create/switch without document reload.**

On switch, move the current layout into `saved_workspace_snapshots`, remove the selected snapshot from that inactive list, apply it to canonical project preferences, update active ID/name, call `apply_project_preferences_to_legacy`, replace open table/surface runtime lists without invoking document-load functions, call `restore_project_tables`/`restore_project_surfaces`, then restore saved selection only if the semantic ID and row/column/axis still validate against the current XDF. Otherwise select the restored active table's first valid cell and set the restore notice. Persist once. Do not call `start_bin_load` or `start_xdf_load`.

- [x] **Step 5: Implement rename/delete validation.**

Rename either active metadata or an inactive snapshot. Reject empty/duplicate/overlong names and unknown IDs. Deleting an inactive snapshot removes only that snapshot; deleting the active snapshot first applies a remaining snapshot. Refuse deleting the final layout.

- [x] **Step 6: Verify startup restore behavior.**

Extend `startup_restore_loads_xdf_after_bin_workspace_restoration` to seed two layouts and an active layout ID. Assert BIN+XDF restore opens the active layout after XDF load; BIN-only does not open XDF-dependent tables until the remembered XDF is loaded. Assert same BIN bytes, same XDF fingerprint, and a clear skip status for stale XDF keys.

Run: `cargo test -p tuner-app switching_workspace_restores_layout_without_changing_bin_or_xdf -- --nocapture` and `cargo test -p tuner-app startup_restore_loads_xdf_after_bin_workspace_restoration -- --nocapture`.

---

### Task 5: Bottom dock and consistent window focus/minimize behavior

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` (`eframe::App::ui`, `show_workspace_canvas`, table/surface show methods, toolbar commands)
- Modify: `crates/tuner-app/src/window_manager.rs` (target-to-entry and transition helpers)
- Test: embedded app headless UI tests

**Interfaces:**
- `DockEntry { id: WindowId, label: String, tooltip: String, kind: String }` identifies one visible table/surface in this task; Task 6 adds tool adapters.
- `TunerApp::dock_entries() -> Vec<DockEntry>` derives live entries from open table/surface collections.
- `TunerApp::focus_window(id: &WindowId) -> Result<(), WorkspaceError>`, `minimize_window`, `restore_window`, and `close_window` route table/surface IDs to the existing `focus_table`, `focus_surface`, `close_table`, and `close_surface` APIs.
- Hidden windows remain in their existing data structure; only closed tables call `close_table`, and only explicit close actions remove the corresponding target.

- [x] **Step 1: Write a headless test for dock entry/focus behavior.**

Add test-only `render_dock_frame` and `click_dock_label` helpers using `egui::Context::run_ui`, `RawInput`, `PointerMoved`/`PointerButton`, and the existing `rendered_text_rect` helper. Render `show_window_dock` inside a `CentralPanel`. Open two fixture table parameters, focus the first, and click the exact `DockEntry.label` of the second. Then click the second label again and once more. Assert the focused ID changes, the second table remains in `open_tables` while minimized, then restores on the next click, and no BIN bytes change.

```rust
#[test]
fn dock_focus_minimize_restore_preserves_open_table() {
    let (xdf, bin) = column_major_fixture();
    let before = bin.bytes().to_vec();
    let first_semantic = xdf.parameters[0].semantic_id.clone();
    let second_semantic = xdf.parameters[1].semantic_id.clone();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(Some(bin), Some(xdf), None, None);
    assert!(app.open_table(&first_semantic));
    let first_key = app.open_tables[0].key.clone();
    assert!(app.open_table(&second_semantic));
    let second_key = app.open_tables[1].key.clone();
    let second_id = WindowId::Table(second_key.clone());
    let second_label = app.dock_entries().into_iter().find(|entry| entry.id == second_id).unwrap().label;
    assert!(app.focus_table(&first_key));

    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut time = 0.0;
    click_dock_label(&mut app, &second_label, &context, screen, &mut time);
    assert_eq!(
        app.project_preferences.dock_state.focused(),
        Some(&WindowId::Table(second_key.clone()))
    );
    click_dock_label(&mut app, &second_label, &context, screen, &mut time);
    assert!(app.project_preferences.dock_state.is_minimized(&WindowId::Table(second_key.clone())));
    assert!(app.open_tables.iter().any(|table| table.key == second_key));
    click_dock_label(&mut app, &second_label, &context, screen, &mut time);
    assert!(!app.project_preferences.dock_state.is_minimized(&WindowId::Table(second_key)));
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), before.as_slice());
}

fn render_dock_frame(
    app: &mut TunerApp,
    context: &egui::Context,
    screen: egui::Rect,
    time: f64,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(screen);
    input.time = Some(time);
    input.events = events;
    context.run_ui(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.show_window_dock(ui));
    })
}

fn click_dock_label(
    app: &mut TunerApp,
    label: &str,
    context: &egui::Context,
    screen: egui::Rect,
    time: &mut f64,
) {
    let output = render_dock_frame(app, context, screen, *time, Vec::new());
    let point = rendered_text_rect(&output, label).expect("dock entry should render").center();
    output.drop_without_applying_deltas();
    for pressed in [true, false] {
        *time += 0.1;
        let event = egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let output = render_dock_frame(app, context, screen, *time, vec![egui::Event::PointerMoved(point), event]);
        output.drop_without_applying_deltas();
    }
}
```

- [x] **Step 2: Run the focused dock test and verify red.**

Run: `cargo test -p tuner-app dock_focus_minimize_restore_preserves_open_table -- --nocapture`  
Expected: test fails because the bottom dock and minimize state do not exist.

- [x] **Step 3: Replace the top table-tab strip with a bottom horizontal panel.**

Render the panel above Diagnostics and before the central canvas. Include horizontally scrolling window entries, accessible tooltips, visible focused/minimized states, and a small menu/context action per table for pin, tab order, close, and table arrangement. Task 8 adds the workspace selector. Keep Search discoverable through Ctrl+F and its existing browser action. Preserve Cascade/Tile/Close Other/Reset in View → Workspace.

- [x] **Step 4: Adapt table and surface windows.**

Skip painting minimized targets without removing `OpenTable`/`OpenSurface`; route focus to existing `focus_table`/`focus_surface`; restore selection/cell synchronization; keep title-bar close behavior. On minimize/close of active target, use `WindowDockState` focus order to select the next still-open target or the canvas.

- [x] **Step 5: Preserve table and surface tab actions.**

Move pin, left/right tab order, close, Cascade, Tile, Close Other, and Reset actions to dock context menus and the existing View → Workspace menu. Do not remove the registered command IDs.

- [x] **Step 6: Verify table/surface dock interactions.**

Run: `cargo test -p tuner-app dock_focus_minimize_restore_preserves_open_table -- --nocapture`, `cargo test -p tuner-app table_window_production_path_clamps_geometry_to_small_canvas_without_capture -- --nocapture`, and `cargo test -p tuner-app clicking_visible_surface_title_bar_raises_it_above_previous_active -- --nocapture`. Assert dock actions do not change BIN bytes or XDF definitions.

---

### Task 6: Calibration-tool dock integration

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` (tool window show methods, `focus_window`/`minimize_window`/`restore_window`/`close_window`, safe rectangle propagation)
- Modify: `crates/tuner-app/src/window_manager.rs` (tool target/open-state adapters)
- Test: embedded app headless UI tests

**Interfaces:**
- `TunerApp::dock_entries()` merges table/surface entries from their existing collections with tool entries from existing persisted open flags and `WindowDockState.open_tool_windows` overrides.
- Existing tool content and open settings remain in their current state types; the manager owns only visual focus order/minimization plus open overrides where no existing field exists.

- [x] **Step 1: Write failing Search and tool-layer tests.**

Use the `render_dock_frame`/`click_dock_label` helpers from Task 5. Test `dock_focus_minimize_restore_preserves_search_state`: set `app.search_state.open = true` and `query = "torque"`, click Search in the dock twice to minimize and once to restore; assert query and open state remain unchanged. Test `dock_tool_window_request_moves_target_layer_to_top` with Hex, Map Finder, and Compare stable IDs.

- [x] **Step 2: Run the focused tool tests and verify red.**

Run: `cargo test -p tuner-app dock_focus_minimize_restore_preserves_search_state -- --nocapture` and `cargo test -p tuner-app dock_tool_window_request_moves_target_layer_to_top -- --nocapture`.  
Expected: Search/Hex/Map/Compare targets do not yet expose uniform dock focus/minimize behavior.

- [x] **Step 3: Connect existing focus-aware tools.**

Wire Search, Map Finder, Hex Editor, Compare BIN/graph, and surfaces. Use each window's existing `focus_requested` flag or stable egui layer ID. Minimized rendering must not set the tool's existing open flag to false and must preserve its query/settings/camera.

- [x] **Step 4: Constrain calibration tools to the safe shell area.**

Capture the toolbar, dock, and optional Diagnostics `Panel::show` response rectangles in `TunerApp::ui`; compute `shell_safe_rect` from those live rectangles. Keep tables constrained to their editor canvas. Continue to clamp Compare/Hex/Map/Search/surface windows through their existing constraints and the Task 1 helper. Persist corrected tool geometry through active `ProjectPreferences` or existing tool memory.

- [x] **Step 5: Verify calibration-tool lifecycle and layering.**

Run `dock_focus_minimize_restore_preserves_search_state`, `dock_tool_window_request_moves_target_layer_to_top`, `ui_ipc_open_compare_bin_reveals_the_compare_window`, `compare_window_renders_without_starting_document_work`, and `clicking_visible_surface_title_bar_raises_it_above_previous_active`. Assert these transitions leave BIN bytes and XDF definitions unchanged.

---

### Task 7: Session and utility-window dock integration

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` (XDF Editor, Action History, Debug Report, Settings, NookLink focus/minimize/close paths and utility rectangle persistence)
- Modify: `crates/tuner-app/src/workspace_layouts.rs` and `crates/tuner-app/src/window_geometry.rs` (utility rectangle snapshot mapping and memory type)
- Modify: `crates/tuner-app/src/window_manager.rs` (session-only tool-open overrides)
- Test: embedded app headless UI tests

**Interfaces:**
- Session tool IDs are dockable/minimizable while the current process is running.
- The active profile excludes XDF draft data and undo-history contents; restart restoration clears their open overrides and reports them as skipped.
- One stable-ID-keyed rectangle map covers only utility windows without an existing typed per-project geometry memory: XDF Editor, Action History, Debug Report, Settings, and NookLink.

- [x] **Step 1: Write a failing utility-window dock test.**

Set `settings_open`, `debug_report_open`, and `nooklink_setup_open`; render the dock and verify all three utility entries exist. Focus Settings, then Debug, and assert dock focus moves. Add `debug_report_yields_to_explicit_dock_focus`: after opening Debug and verifying it starts in the foreground, explicitly focus Settings and assert Debug no longer forces itself above Settings. Do not inspect or persist challenge/draft content.

Also write `session_tool_windows_remain_dockable_when_minimized`: open XDF Editor on the fixture XDF, open a table, set `action_history_table` to its key, render the dock, minimize XDF Editor and Action History, and assert both remain open with the same draft/table key.

```rust
#[test]
fn session_tool_windows_remain_dockable_when_minimized() {
    let mut app = app_with_project_fixture();
    app.open_xdf_editor().unwrap();
    let draft = app.xdf_editor.draft.clone();
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let table_key = app.open_tables[0].key.clone();
    app.action_history_table = Some(table_key.clone());
    let history_id = WindowId::ActionHistory(table_key.clone());
    assert!(app.dock_entries().iter().any(|entry| entry.id == WindowId::XdfEditor));
    assert!(app.dock_entries().iter().any(|entry| entry.id == history_id));

    app.minimize_window(&WindowId::XdfEditor).unwrap();
    app.minimize_window(&history_id).unwrap();
    assert!(app.xdf_editor.open);
    assert_eq!(app.xdf_editor.draft, draft);
    assert_eq!(app.action_history_table.as_deref(), Some(table_key.as_str()));
}
```

- [x] **Step 2: Run the focused test and verify red.**

Run: `cargo test -p tuner-app utility_windows_are_dockable_and_focusable -- --nocapture`, `cargo test -p tuner-app session_tool_windows_remain_dockable_when_minimized -- --nocapture`, and `cargo test -p tuner-app debug_report_yields_to_explicit_dock_focus -- --nocapture`  
Expected: these existing state flags are not yet projected into dock entries.

- [x] **Step 3: Wire XDF Editor, Action History, Debug, Settings, and NookLink.**

Use `WindowDockState.open_tool_windows` for open state not already represented in project preferences. Minimize hides but retains dirty XDF drafts and in-session Action History. On process restart clear those ephemeral open IDs and report their omission. Keep Debug foreground when newly opened or explicitly selected; after another dock target is focused, stop force-raising Debug each frame.

- [x] **Step 4: Constrain utility-window geometry.**

Pass the `shell_safe_rect` computed in `TunerApp::ui` to Action History, Settings, Debug Report, and NookLink, and add `.constrain_to(safe_rect)` to those windows. XDF Editor keeps its existing canvas constraint. Clamp persisted geometry and keep title bars reachable.

- [x] **Step 5: Verify utility window behavior.**

Run `utility_windows_are_dockable_and_focusable`, `session_tool_windows_remain_dockable_when_minimized`, `debug_report_is_configured_as_foreground_window`, `debug_report_stays_above_foreground_operation_overlay`, and `debug_report_yields_to_explicit_dock_focus`.

---

### Task 8: Workspace selector and snapshot controls

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` (dock workspace selector and snapshot-management UI)
- Test: embedded app headless UI tests

**Interfaces:**
- Selector calls Task 4's `create_workspace_snapshot`, `switch_workspace_snapshot`, `rename_workspace_snapshot`, and `delete_workspace_snapshot` methods.
- Profile mutation buttons are disabled while a document load/restore/save is active; dock focus/minimize remains available.

- [x] **Step 1: Write a failing selector interaction test.**

Create a second layout through the already-tested model method, then exercise the actual dock selector using Task 5's rendered-text pointer harness:

```rust
#[test]
fn workspace_selector_renders_and_switches_saved_layout() {
    let mut app = app_with_project_fixture();
    let default_id = app.project_preferences.active_workspace_id;
    let track_id = app.create_workspace_snapshot("Track").unwrap();
    app.switch_workspace_snapshot(default_id).unwrap();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut time = 10.0;
    click_dock_label(&mut app, "Workspace: Default", &context, screen, &mut time);
    time += 0.1;
    let menu = render_dock_frame(&mut app, &context, screen, time, Vec::new());
    time += 0.1;
    for label in ["New layout…", "Rename…", "Delete…"] {
        assert!(rendered_text_rect(&menu, label).is_some(), "missing {label}");
    }
    menu.drop_without_applying_deltas();
    click_dock_label(&mut app, "Track", &context, screen, &mut time);
    assert_eq!(app.project_preferences.active_workspace_id, track_id);
}
```

- [x] **Step 2: Run the focused selector test and verify red.**

Run: `cargo test -p tuner-app workspace_selector_renders_and_switches_saved_layout -- --nocapture`  
Expected: there is no workspace selector in the dock.

- [x] **Step 3: Render compact snapshot controls.**

Place a `Workspace: <name>` menu at the left edge of the dock with selectable profiles and `New layout…`, `Rename…`, and `Delete…` actions. New duplicates the active layout; selecting another calls the existing switch method. New/Rename opens one small egui dialog with a single-line name field plus explicit Create/Rename and Cancel buttons. Delete opens a confirmation dialog showing the profile name and Delete/Cancel. Add one `WorkspaceDialog` enum (`New { name }`, `Rename { id, name }`, `Delete { id, name }`) in `TunerApp` state. Keep the table/tool list horizontally scrollable.

- [x] **Step 4: Restore startup and report skipped view entries.**

After the existing BIN+XDF prompt is accepted and the XDF finishes loading, restore the active layout from its canonical `ProjectPreferences` fields and then render its dock selection; do not look for a duplicate active snapshot. BIN-only waits until a matching XDF loads. Show a brief status/diagnostic notice for stale IDs or session-only windows that cannot reopen.

- [x] **Step 5: Verify selector persistence and safe restore.**

Run: `cargo test -p tuner-app workspace_selector_renders_and_switches_saved_layout -- --nocapture`, `cargo test -p tuner-app workspace_snapshot_names_are_unique_and_last_layout_cannot_be_deleted -- --nocapture`, and `cargo test -p tuner-app startup_restore_loads_xdf_after_bin_workspace_restoration -- --nocapture`. Assert no document bytes change.

---

### Task 9: NookLink window and workspace control

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` (`handle_ui_ipc_request`/`ui_ipc_state`)
- Modify: `crates/tuner-app/src/ui_ipc.rs` (`UiIpcRequest` optional target/profile fields)
- Modify: `docs/tuner-ui-ipc.md`
- Test: embedded app UI IPC tests and `crates/tuner-app/src/ui_ipc.rs`

**Interfaces:**
- `UiIpcRequest` gains optional `window_id: Option<String>`, `workspace_id: Option<u64>`, and `workspace_name: Option<String>`.
- UI IPC actions: `focus_window`, `minimize_window`, `restore_window`, `close_window`, `workspace_create`, `workspace_switch`, `workspace_rename`, and `workspace_delete`.
- `ui_ipc_state()` exposes `windows: [{id, kind, title, open, minimized, focused}]`, `workspaces: [{id, name, active}]`, and the active workspace ID/name.

- [x] **Step 1: Write failing NookLink tests.**

```rust
#[test]
fn ui_ipc_workspace_window_actions_are_discoverable_and_document_safe() {
    let mut app = app_with_project_fixture();
    let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let default_id = app.project_preferences.active_workspace_id;
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let create = agent_ipc_request(json!({
        "action": "workspace_create",
        "workspace_name": "Agent layout"
    }));
    let created = app.handle_ui_ipc_request(&create);
    assert!(created.ok, "{}", created.message);
    let new_id = created.data.unwrap()["workspace_id"].as_u64().unwrap();
    let state = app.ui_ipc_state();
    assert_eq!(state["active_workspace_id"], json!(new_id));
    assert!(state["workspaces"].as_array().unwrap().iter().any(|entry| entry["id"] == json!(new_id)));
    assert!(!state["windows"].as_array().unwrap().is_empty());

    let rename = agent_ipc_request(json!({
        "action": "workspace_rename",
        "workspace_id": new_id,
        "workspace_name": "Agent renamed"
    }));
    assert!(app.handle_ui_ipc_request(&rename).ok);
    let switch = agent_ipc_request(json!({"action":"workspace_switch","workspace_id":default_id}));
    assert!(app.handle_ui_ipc_request(&switch).ok);
    assert_eq!(app.project_preferences.active_workspace_id, default_id);
    let close = agent_ipc_request(json!({"action":"close_window","window_id":"table:unknown"}));
    assert!(!app.handle_ui_ipc_request(&close).ok);
    let delete = agent_ipc_request(json!({"action":"workspace_delete","workspace_id":new_id}));
    assert!(app.handle_ui_ipc_request(&delete).ok);
    let invalid = agent_ipc_request(json!({"action":"focus_window","window_id":"unknown"}));
    assert!(!app.handle_ui_ipc_request(&invalid).ok);
    let invalid_workspace = agent_ipc_request(json!({"action":"workspace_switch","workspace_id":999999}));
    assert!(!app.handle_ui_ipc_request(&invalid_workspace).ok);
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), before.as_slice());
}
```

- [x] **Step 2: Run the new IPC tests and verify red.**

Run: `cargo test -p tuner-app ui_ipc_workspace_window -- --nocapture`  
Expected: compile/test failure because workspace/window fields and actions are not registered.

- [x] **Step 3: Add optional request fields and allowlisted action dispatch.**

Add `window_id`, `workspace_id`, and `workspace_name` to `UiIpcRequest` with serde defaults, and set them to `None` in `UiIpcRequest::for_test`. Require only fields relevant to each action; reject missing/invalid IDs with explicit response errors. `close_window` must use the same dirty-draft/close-confirmation behavior as the visible UI. `workspace_create` responds with `data: {"workspace_id": <u64>, "name": <trimmed-name>}`; state uses top-level `active_workspace_id`, `active_workspace_name`, `workspaces`, and `windows`. Reuse the same app methods as visible UI. No raw-byte, BIN-write, Save As, or approval behavior is added to these actions.

- [x] **Step 4: Update UI IPC documentation and verify.**

Document the stable window IDs, profile actions, restart restore semantics, and UI-only snapshot safety boundary in `docs/tuner-ui-ipc.md`.

Run: `cargo test -p tuner-app ui_ipc_workspace_window -- --nocapture` and the workspace-selector headless UI test.

---

### Task 10: Full regression, SDD record, and release verification

**Files:**
- Modify: `AGENTS.md`
- Create/update: `.superpowers/sdd/2026-09-24-window-management-dock/progress.md`
- Create: `.superpowers/sdd/2026-09-24-window-management-dock/snapshots/before/` and `snapshots/final/`
- Verify: `crates/tuner-app`, workspace, native release app

- [x] Update the SDD ledger as each accepted task is verified and save the matching final snapshots.
- [x] Run `cargo fmt --all -- --check`.
- [x] Run `cargo test -p tuner-app` once and record the full pass count.
- [x] Run `cargo test --workspace` once and record all crate/fixture counts.
- [x] Run `cargo build --release -p tuner-app`.
- [x] Run `bash scripts/smoke-test.sh --app`; require all 21 checks and graceful close.
- [x] Exercise the visible app once: open the supplied BIN/XDF, create two layouts, move/zoom/minimize different tables/tools, switch layouts, close/reopen the app, accept BIN+XDF restore, and verify the selected layout returns with the same documents and data hash.
- [x] Update `AGENTS.md` with verified results and scan the plan against every spec section before reporting completion.

### Follow-up: modal and priority-window z-order regression (2026-09-24)

- [x] Reproduce workspace-dialog and command-palette overlap with a focused table.
- [x] Make prompts/palette outrank dock windows, and make Debug/loading overlay
  yield while priority UI is open.
- [x] Verify formatting and all `tuner-app` tests (333 passed).
- [x] Verify the optimized configuration with `cargo check --release -p tuner-app`.
- [x] Rebuild the release executable after closing it; the initial build attempt
  was blocked while Windows held the executable open.

### Follow-up: manual table resize persistence (2026-09-24)

- [x] Reproduce resize → save → reopen with a simulated real corner drag.
- [x] Detect egui window edge/corner drags and disable content auto-fit for the
  user-sized border.
- [x] Verify formatting, app/workspace tests, and the optimized release build.
