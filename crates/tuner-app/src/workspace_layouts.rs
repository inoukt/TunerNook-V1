use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    AxisSelection, BrowserOrganization, CatalogSortKey, CellSelection, CompareValueMode,
    CompareWindowMemory, HexSelection, HexWindowMemory, LayoutPreset, MapFinderMemory,
    ProjectPreferences, SearchState, SortDirection, SurfaceViewMemory, TableWindowMemory,
    WindowDockState,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkspaceSnapshot {
    pub id: u64,
    pub name: String,
    pub state: WorkspaceViewState,
}

pub fn workspace_id_for_slot(project: &ProjectPreferences, slot: u8) -> Option<u64> {
    let index = usize::from(slot.checked_sub(1)?);
    let mut ids = Vec::with_capacity(project.saved_workspace_snapshots.len() + 1);
    ids.push(project.active_workspace_id);
    ids.extend(
        project
            .saved_workspace_snapshots
            .iter()
            .map(|snapshot| snapshot.id),
    );
    ids.sort_unstable();
    ids.dedup();
    ids.get(index).copied()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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
    pub dock_state: WindowDockState,
    pub selected_semantic_id: Option<String>,
    pub selected_cell: (usize, usize),
    pub selected_cells: Option<CellSelection>,
    pub selected_axis: Option<AxisSelection>,
    pub utility_window_geometry: BTreeMap<String, crate::window_geometry::WindowGeometryMemory>,
}

impl Default for WorkspaceViewState {
    fn default() -> Self {
        Self::capture(&ProjectPreferences::default())
    }
}

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
            dock_state: p.dock_state.clone(),
            selected_semantic_id: p.selected_semantic_id.clone(),
            selected_cell: p.selected_cell,
            selected_cells: p.selected_cells,
            selected_axis: p.selected_axis,
            utility_window_geometry: p.utility_window_geometry.clone(),
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
        p.dock_state = self.dock_state.clone();
        p.selected_semantic_id = self.selected_semantic_id.clone();
        p.selected_cell = self.selected_cell;
        p.selected_cells = self.selected_cells;
        p.selected_axis = self.selected_axis;
        p.utility_window_geometry = self.utility_window_geometry.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex::HexDisplayFormat;
    use crate::{TableColorSettings, WindowId};

    #[test]
    fn workspace_shortcut_slots_use_stable_numeric_id_order() {
        let mut project = ProjectPreferences::for_identity("bin-a");
        project.active_workspace_id = 7;
        project.saved_workspace_snapshots = [9, 1, 3]
            .into_iter()
            .map(|id| WorkspaceSnapshot {
                id,
                name: format!("Workspace {id}"),
                state: WorkspaceViewState::default(),
            })
            .collect();

        assert_eq!(workspace_id_for_slot(&project, 0), None);
        assert_eq!(workspace_id_for_slot(&project, 1), Some(1));
        assert_eq!(workspace_id_for_slot(&project, 2), Some(3));
        assert_eq!(workspace_id_for_slot(&project, 3), Some(7));
        assert_eq!(workspace_id_for_slot(&project, 4), Some(9));
        assert_eq!(workspace_id_for_slot(&project, 5), None);

        project.active_workspace_id = 3;
        project.saved_workspace_snapshots = [7, 9, 1]
            .into_iter()
            .map(|id| WorkspaceSnapshot {
                id,
                name: format!("Workspace {id}"),
                state: WorkspaceViewState::default(),
            })
            .collect();
        assert_eq!(workspace_id_for_slot(&project, 1), Some(1));
        assert_eq!(workspace_id_for_slot(&project, 2), Some(3));
        assert_eq!(workspace_id_for_slot(&project, 3), Some(7));
        assert_eq!(workspace_id_for_slot(&project, 4), Some(9));
    }

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
                position_saved: true,
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
        for field in [
            "active_workspace_id",
            "active_workspace_name",
            "saved_workspace_snapshots",
            "dock_state",
            "selected_semantic_id",
            "selected_cell",
            "selected_cells",
            "selected_axis",
            "utility_window_geometry",
        ] {
            old.as_object_mut().unwrap().remove(field);
        }

        let restored = crate::project_preferences_from_json(&old.to_string(), "bin-a");
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
                position_saved: true,
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
        project.selected_axis = Some(AxisSelection {
            axis_index: 0,
            index: 2,
        });
        project.dock_state.focus(WindowId::NookLink);
        let snapshot = WorkspaceSnapshot {
            id: 1,
            name: "Wide map".into(),
            state: WorkspaceViewState::capture(&project),
        };
        project.saved_workspace_snapshots.push(snapshot.clone());
        let encoded = serde_json::to_string(&project).unwrap();
        let loaded: ProjectPreferences = serde_json::from_str(&encoded).unwrap();
        assert_eq!(loaded.saved_workspace_snapshots[0], snapshot);

        let mut restored = ProjectPreferences::for_identity("bin-b");
        restored
            .conversion_overrides
            .insert("global".into(), "X".into());
        snapshot.state.apply_to(&mut restored);
        assert_eq!(restored.table_windows["xdf|table:a"].zoom_percent, 125);
        assert_eq!(restored.table_windows["xdf|table:a"].scroll_y, 90);
        assert_eq!(
            restored.hex_window.display_format,
            HexDisplayFormat::Float32
        );
        assert_eq!(restored.selected_cell, (3, 4));
        assert_eq!(
            restored.selected_cells,
            Some(CellSelection::new((1, 2), (3, 4)))
        );
        assert_eq!(restored.dock_state.focused(), Some(&WindowId::NookLink));
        assert_eq!(restored.bin_identity, "bin-b");
        assert_eq!(restored.conversion_overrides["global"], "X");
    }

    #[test]
    fn invalid_workspace_snapshot_names_and_duplicate_ids_are_reported_and_skipped() {
        let mut project = ProjectPreferences::for_identity("bin-a");
        let state = WorkspaceViewState::capture(&project);
        project.saved_workspace_snapshots = vec![
            WorkspaceSnapshot {
                id: 0,
                name: "Duplicate active".into(),
                state: state.clone(),
            },
            WorkspaceSnapshot {
                id: 1,
                name: "Track".into(),
                state: state.clone(),
            },
            WorkspaceSnapshot {
                id: 2,
                name: " track ".into(),
                state,
            },
        ];

        project.sanitize();

        assert_eq!(project.saved_workspace_snapshots.len(), 1);
        assert_eq!(project.saved_workspace_snapshots[0].id, 1);
        assert_eq!(project.saved_workspace_snapshots[0].name, "Track");
        assert_eq!(project.workspace_restore_warnings.len(), 2);
    }

    #[test]
    fn utility_window_geometry_round_trips_with_workspace_without_duplicating_existing_memories() {
        let mut project = ProjectPreferences::for_identity("bin-a");
        project.utility_window_geometry.insert(
            "xdf_editor".into(),
            crate::window_geometry::WindowGeometryMemory {
                x: 120,
                y: 80,
                width: 920,
                height: 640,
            },
        );
        project.utility_window_geometry.insert(
            "history:xdf|table:main".into(),
            crate::window_geometry::WindowGeometryMemory {
                x: 24,
                y: 32,
                width: 520,
                height: 400,
            },
        );
        let state = WorkspaceViewState::capture(&project);
        let mut restored = ProjectPreferences::for_identity("bin-b");
        state.apply_to(&mut restored);

        assert_eq!(
            restored.utility_window_geometry,
            project.utility_window_geometry
        );
        assert_eq!(restored.hex_window, project.hex_window);
        assert!(!restored.utility_window_geometry.contains_key("hex_editor"));
        assert_eq!(
            WorkspaceViewState::capture(&restored).utility_window_geometry,
            project.utility_window_geometry
        );
    }

    #[test]
    fn utility_window_geometry_sanitization_drops_unknown_targets_and_bounds_sizes() {
        let mut project = ProjectPreferences::for_identity("bin-a");
        project.utility_window_geometry.insert(
            "hex_editor".into(),
            crate::window_geometry::WindowGeometryMemory {
                x: 0,
                y: 0,
                width: 1,
                height: 99_999,
            },
        );
        project.utility_window_geometry.insert(
            "xdf_editor".into(),
            crate::window_geometry::WindowGeometryMemory {
                x: 0,
                y: 0,
                width: 1,
                height: 99_999,
            },
        );
        project.sanitize();

        assert!(!project.utility_window_geometry.contains_key("hex_editor"));
        let memory = &project.utility_window_geometry["xdf_editor"];
        assert!(memory.width >= 320);
        assert!(memory.height <= 10_000);
    }
}
