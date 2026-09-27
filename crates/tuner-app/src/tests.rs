use super::*;
use crate::project_file::TunerProjectFile;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tuner_xdf::{CategoryReferenceMode, ParameterKind};

static TEST_SEQUENCE: AtomicUsize = AtomicUsize::new(1);

#[test]
fn native_window_title_displays_app_version() {
    assert_eq!(APP_TITLE, "TunerNook v0.1.5");
    assert_eq!(APP_NAME, "TunerNook");
}

impl TunerApp {
    fn open_compare_with_test_documents(&mut self) {
        let (xdf, bin) = column_major_fixture();
        self.workspace.set_documents(
            Some(bin.clone()),
            Some(xdf.clone()),
            Some(PathBuf::from("destination.bin")),
            Some(PathBuf::from("definition.xdf")),
        );
        self.compare.source_bin = Some(BinDocument::from_bytes(vec![10, 40, 40, 50, 50, 80]));
        self.compare.source_bin_path = Some(PathBuf::from("source.bin"));
        self.compare.window_open = true;
    }
}

fn app_with_matching_compare_documents() -> TunerApp {
    let mut app = TunerApp::headless();
    app.open_compare_with_test_documents();
    app
}

fn agent_ipc_request(value: Value) -> ui_ipc::UiIpcRequest {
    serde_json::from_value(value).unwrap()
}

fn rendered_text_rect(output: &egui::FullOutput, exact: &str) -> Option<egui::Rect> {
    fn in_shape(shape: &egui::Shape, exact: &str) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Text(text) => {
                (text.galley.job.text == exact).then(|| text.visual_bounding_rect())
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| in_shape(shape, exact)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .find_map(|clipped| in_shape(&clipped.shape, exact))
}

fn rendered_fill_count(output: &egui::FullOutput, fill: egui::Color32) -> usize {
    fn count(shape: &egui::Shape, fill: egui::Color32) -> usize {
        match shape {
            egui::Shape::Rect(rect) => usize::from(rect.fill == fill),
            egui::Shape::Vec(shapes) => shapes.iter().map(|shape| count(shape, fill)).sum(),
            _ => 0,
        }
    }
    output
        .shapes
        .iter()
        .map(|clipped| count(&clipped.shape, fill))
        .sum()
}

fn rendered_circle_fill_count(output: &egui::FullOutput, fill: egui::Color32) -> usize {
    fn count(shape: &egui::Shape, fill: egui::Color32) -> usize {
        match shape {
            egui::Shape::Circle(circle) => usize::from(circle.fill == fill),
            egui::Shape::Vec(shapes) => shapes.iter().map(|shape| count(shape, fill)).sum(),
            _ => 0,
        }
    }
    output
        .shapes
        .iter()
        .map(|clipped| count(&clipped.shape, fill))
        .sum()
}

fn render_toolbar_frame(
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
        egui::CentralPanel::default().show(ctx, |ui| app.show_toolbar(ui));
        school_help::show_help_window(ctx, app.preferences.school_me_mode);
    })
}

fn render_browser_contents_frame(
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
        egui::CentralPanel::default().show(ctx, |ui| app.show_browser_contents(ui));
        school_help::show_help_window(ctx, app.preferences.school_me_mode);
    })
}

fn click_browser_text(
    app: &mut TunerApp,
    label: &str,
    context: &egui::Context,
    screen: egui::Rect,
    time: &mut f64,
) {
    let output = render_browser_contents_frame(app, context, screen, *time, Vec::new());
    let point = rendered_text_rect(&output, label).map(|rect| rect.center());
    output.drop_without_applying_deltas();
    let point = point.unwrap_or_else(|| panic!("browser should render {label:?}"));
    for pressed in [true, false] {
        *time += 0.1;
        let output = render_browser_contents_frame(
            app,
            context,
            screen,
            *time,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        output.drop_without_applying_deltas();
    }
}

fn render_workspace_canvas_frame(
    app: &mut TunerApp,
    context: &egui::Context,
    screen: egui::Rect,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(screen);
    input.events = events;
    context.run_ui(input, |ctx| {
        let frame_context = ctx.clone();
        egui::CentralPanel::default().show(ctx, |ui| {
            app.show_workspace_canvas(ui, &frame_context);
        });
        school_help::show_help_window(ctx, app.preferences.school_me_mode);
    })
}

fn render_workspace_canvas_frame_with_response_id(
    app: &mut TunerApp,
    context: &egui::Context,
    screen: egui::Rect,
    events: Vec<egui::Event>,
) -> (egui::FullOutput, egui::Id) {
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(screen);
    input.events = events;
    let mut response_id = None;
    let output = context.run_ui(input, |ctx| {
        let frame_context = ctx.clone();
        egui::CentralPanel::default().show(ctx, |ui| {
            response_id = Some(ui.id().with("workspace-background-context-menu"));
            app.show_workspace_canvas(ui, &frame_context);
        });
        school_help::show_help_window(ctx, app.preferences.school_me_mode);
    });
    (
        output,
        response_id.expect("canvas menu response ID should be captured"),
    )
}

fn click_toolbar_label(
    app: &mut TunerApp,
    label: &str,
    context: &egui::Context,
    screen: egui::Rect,
    time: &mut f64,
) {
    let output = render_toolbar_frame(app, context, screen, *time, Vec::new());
    let point = rendered_text_rect(&output, label)
        .unwrap_or_else(|| panic!("toolbar/menu should render {label:?}"))
        .center();
    output.drop_without_applying_deltas();
    for pressed in [true, false] {
        *time += 0.1;
        let output = render_toolbar_frame(
            app,
            context,
            screen,
            *time,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        output.drop_without_applying_deltas();
    }
}

fn press_shortcut(
    app: &mut TunerApp,
    context: &egui::Context,
    key: egui::Key,
    modifiers: egui::Modifiers,
    shortcut: &str,
) {
    let mut input = egui::RawInput::default();
    input.events = vec![
        egui::Event::ModifiersChanged(modifiers),
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        },
    ];
    let mut key_pressed = false;
    let mut shortcut_matches_input = false;
    let mut wants_keyboard_input = false;
    context
        .run_ui(input, |ctx| {
            key_pressed = ctx.input(|input| input.key_pressed(key));
            shortcut_matches_input = ctx.input(|input| shortcut_matches(input, shortcut));
            wants_keyboard_input = ctx.egui_wants_keyboard_input();
            app.handle_shortcuts(ctx);
        })
        .drop_without_applying_deltas();
    assert!(key_pressed, "egui should receive {key:?}");
    assert!(shortcut_matches_input, "input should match {shortcut}");
    assert!(
        !wants_keyboard_input,
        "no text widget should own keyboard input"
    );

    let mut release = egui::RawInput::default();
    release.events = vec![
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: false,
            repeat: false,
            modifiers,
        },
        egui::Event::ModifiersChanged(egui::Modifiers::default()),
    ];
    context
        .run_ui(release, |_| {})
        .drop_without_applying_deltas();
}

#[test]
fn dock_focus_minimize_restore_preserves_open_table() {
    let xdf = XdfDocument::parse(TWO_TABLE_XDF).unwrap();
    let bin = BinDocument::from_bytes(vec![1, 2]);
    let before = bin.bytes().to_vec();
    let first_semantic = xdf.parameters[0].semantic_id.clone();
    let second_semantic = xdf.parameters[1].semantic_id.clone();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    assert!(app.open_table(&first_semantic));
    let first_key = app.open_tables[0].key.clone();
    assert!(app.open_table(&second_semantic));
    let second_key = app.open_tables[1].key.clone();
    let second_id = WindowId::Table(second_key.clone());
    let second_label = app
        .dock_entries()
        .into_iter()
        .find(|entry| entry.id == second_id)
        .unwrap()
        .label;
    assert!(app.focus_table(&first_key));
    assert_eq!(
        app.project_preferences.dock_state.focused(),
        Some(&WindowId::Table(first_key.clone()))
    );
    assert!(
        !app.project_preferences
            .dock_state
            .is_minimized(&WindowId::Table(second_key.clone())),
        "dock state before rendering: {:?}",
        app.project_preferences.dock_state
    );

    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut time = 0.0;
    click_dock_label(&mut app, &second_label, &context, screen, &mut time);
    assert_eq!(
        app.project_preferences.dock_state.focused(),
        Some(&WindowId::Table(second_key.clone()))
    );
    click_dock_label(&mut app, &second_label, &context, screen, &mut time);
    assert!(app
        .project_preferences
        .dock_state
        .is_minimized(&WindowId::Table(second_key.clone())));
    assert!(app.open_tables.iter().any(|table| table.key == second_key));
    click_dock_label(&mut app, &second_label, &context, screen, &mut time);
    assert!(!app
        .project_preferences
        .dock_state
        .is_minimized(&WindowId::Table(second_key.clone())));
    assert!(app
        .close_window(&WindowId::Table(second_key.clone()))
        .is_ok());
    assert!(!app.open_tables.iter().any(|table| table.key == second_key));
    assert_eq!(
        app.project_preferences.dock_state.focused(),
        Some(&WindowId::Table(first_key))
    );
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        before.as_slice()
    );
}

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
    for label in ["New layout…", "Rename…", "Delete…", "Track"] {
        assert!(
            rendered_text_rect(&menu, label).is_some(),
            "missing {label}"
        );
    }
    menu.drop_without_applying_deltas();
    click_dock_label(&mut app, "Track", &context, screen, &mut time);
    assert_eq!(app.project_preferences.active_workspace_id, track_id);
}

#[test]
fn workspace_dialog_stays_above_the_focused_table_window() {
    let mut app = app_with_project_fixture();
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let table_key = app.open_tables[0].key.clone();
    let table = &mut app.open_tables[0];
    table.memory.x = 300;
    table.memory.y = 300;
    table.memory.width = 700;
    table.memory.height = 500;
    table.memory.fit_to_content = Some(false);
    table.geometry_request = true;
    app.workspace_dialog = Some(WorkspaceDialog::New {
        name: "Layout 2".into(),
    });
    app.operations
        .begin(OperationKind::LoadingXdf, "definition.xdf");
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    for frame in 0..3 {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(frame as f64 * 0.1);
        context
            .run_ui(input, |ctx| {
                app.show_workspace_dialog(ctx);
                app.show_table_window(ctx, screen, &table_key);
                app.show_operation_overlay(ctx, screen);
                app.raise_focused_dock_window(ctx);
            })
            .drop_without_applying_deltas();
    }

    let dialog_layer = egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new((
            "workspace-dialog",
            app.project_preferences.active_workspace_id,
            "new",
        )),
    );
    let table_layer = egui::LayerId::new(
        app.dock_window_order(&WindowId::Table(table_key.clone())),
        egui::Id::new(("table-window", app.project_scope(), table_key.as_str())),
    );
    let dialog_rect = context
        .memory(|memory| memory.area_rect(dialog_layer.id))
        .expect("workspace dialog should have a rect");
    let table_rect = context
        .memory(|memory| memory.area_rect(table_layer.id))
        .expect("focused table should have a rect");
    let overlap = dialog_rect.intersect(table_rect);
    assert!(
        overlap.is_positive(),
        "windows should overlap: {dialog_rect:?} {table_rect:?}"
    );
    assert_eq!(context.layer_id_at(overlap.center()), Some(dialog_layer));
}

#[test]
fn command_palette_stays_above_the_focused_table_window() {
    let mut app = app_with_project_fixture();
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    app.debug_report_open = true;
    app.project_preferences
        .dock_state
        .open_tool(WindowId::DebugReport);
    let table_key = app.open_tables[0].key.clone();
    let table = &mut app.open_tables[0];
    table.memory.x = 300;
    table.memory.y = 300;
    table.memory.width = 700;
    table.memory.height = 500;
    table.memory.fit_to_content = Some(false);
    table.geometry_request = true;
    app.command_palette_open = true;

    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    for frame in 0..3 {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(frame as f64 * 0.1);
        context
            .run_ui(input, |ctx| {
                app.show_command_palette(ctx);
                app.show_debug_report(ctx);
                app.raise_debug_report(ctx);
                app.show_table_window(ctx, screen, &table_key);
                app.raise_focused_dock_window(ctx);
            })
            .drop_without_applying_deltas();
    }

    let palette_id = egui::Id::new(("command-palette", app.project_scope()));
    let palette_layer = egui::LayerId::new(egui::Order::Foreground, palette_id);
    let table_layer = egui::LayerId::new(
        app.dock_window_order(&WindowId::Table(table_key.clone())),
        egui::Id::new(("table-window", app.project_scope(), table_key.as_str())),
    );
    let palette_rect = context
        .memory(|memory| memory.area_rect(palette_id))
        .expect("command palette should have a rect");
    let table_rect = context
        .memory(|memory| memory.area_rect(table_layer.id))
        .expect("focused table should have a rect");
    let overlap = palette_rect.intersect(table_rect);
    assert!(
        overlap.is_positive(),
        "windows should overlap: {palette_rect:?} {table_rect:?}"
    );
    assert_eq!(context.layer_id_at(overlap.center()), Some(palette_layer));
}

#[test]
fn priority_ui_suspends_dock_foreground_for_each_prompt_kind() {
    let mut app = app_with_project_fixture();
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let focused = app
        .project_preferences
        .dock_state
        .focused()
        .unwrap()
        .clone();

    app.workspace_dialog = Some(WorkspaceDialog::New {
        name: String::new(),
    });
    assert_eq!(app.dock_window_order(&focused), egui::Order::Middle);
    app.workspace_dialog = None;

    app.command_palette_open = true;
    assert_eq!(app.dock_window_order(&focused), egui::Order::Middle);
    app.command_palette_open = false;

    app.pending_project_restore = Some(PendingProjectRestore {
        bin_path: PathBuf::from("last.bin"),
        xdf_path: None,
        bin_available: true,
        xdf_available: false,
    });
    assert_eq!(app.dock_window_order(&focused), egui::Order::Middle);
    app.pending_project_restore = None;

    app.pending_xdf_restore = Some(PendingXdfRestore {
        bin_identity: app.project_identity.clone().unwrap_or_default(),
        path: PathBuf::from("last.xdf"),
        available: true,
    });
    assert_eq!(app.dock_window_order(&focused), egui::Order::Middle);
    app.pending_xdf_restore = None;

    app.xdf_editor.close_prompt = true;
    assert_eq!(app.dock_window_order(&focused), egui::Order::Middle);
    assert_eq!(
        app.dock_window_order(&WindowId::DebugReport),
        egui::Order::Middle
    );
}

#[test]
fn workspace_dialogs_create_rename_and_delete_from_the_dock() {
    let mut app = app_with_project_fixture();
    let default_id = app.project_preferences.active_workspace_id;
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut time = 20.0;

    app.workspace_dialog = Some(WorkspaceDialog::New {
        name: "Track".into(),
    });
    for _ in 0..2 {
        time += 0.1;
        render_dock_frame(&mut app, &context, screen, time, Vec::new())
            .drop_without_applying_deltas();
    }
    click_dock_label(&mut app, "Create", &context, screen, &mut time);
    let track_id = app.project_preferences.active_workspace_id;
    assert_ne!(track_id, default_id);
    assert_eq!(app.project_preferences.active_workspace_name, "Track");

    app.workspace_dialog = Some(WorkspaceDialog::Rename {
        id: track_id,
        name: "Street".into(),
    });
    for _ in 0..2 {
        time += 0.1;
        render_dock_frame(&mut app, &context, screen, time, Vec::new())
            .drop_without_applying_deltas();
    }
    click_dock_label(&mut app, "Rename", &context, screen, &mut time);
    assert_eq!(app.project_preferences.active_workspace_name, "Street");

    app.workspace_dialog = Some(WorkspaceDialog::Delete {
        id: track_id,
        name: "Street".into(),
    });
    for _ in 0..2 {
        time += 0.1;
        render_dock_frame(&mut app, &context, screen, time, Vec::new())
            .drop_without_applying_deltas();
    }
    click_dock_label(&mut app, "Delete", &context, screen, &mut time);
    assert_eq!(app.project_preferences.active_workspace_id, default_id);
    assert!(app
        .project_preferences
        .saved_workspace_snapshots
        .iter()
        .all(|snapshot| snapshot.id != track_id));
}

#[test]
fn dock_focus_minimize_restore_preserves_search_state() {
    let mut app = app_with_project_fixture();
    app.search_state.open = true;
    app.search_state.query = "torque".into();
    let search_id = WindowId::Search;
    let label = app
        .dock_entries()
        .into_iter()
        .find(|entry| entry.id == search_id)
        .unwrap()
        .label;
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut time = 0.0;

    click_dock_label(&mut app, &label, &context, screen, &mut time);
    click_dock_label(&mut app, &label, &context, screen, &mut time);
    assert!(app.project_preferences.dock_state.is_minimized(&search_id));
    assert!(app.search_state.open);
    assert_eq!(app.search_state.query, "torque");

    click_dock_label(&mut app, &label, &context, screen, &mut time);
    assert!(!app.project_preferences.dock_state.is_minimized(&search_id));
    assert!(app.search_state.open);
    assert_eq!(app.search_state.query, "torque");
    app.close_window(&search_id).unwrap();
    assert!(!app.search_state.open);
    assert!(!app.dock_entries().iter().any(|entry| entry.id == search_id));
}

#[test]
fn dock_tool_window_request_moves_target_layer_to_top() {
    let mut app = app_with_project_fixture();
    let before_bin = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let before_xdf = app
        .workspace
        .xdf
        .as_ref()
        .unwrap()
        .normalized_fingerprint
        .clone();
    app.hex_editor.window_open = true;
    app.hex_editor.memory.x = 80;
    app.hex_editor.memory.y = 80;
    app.hex_editor.memory.width = 700;
    app.hex_editor.memory.height = 460;
    app.map_finder.memory.window_open = true;
    app.map_finder.memory.x = 80;
    app.map_finder.memory.y = 80;
    app.map_finder.memory.width = 700;
    app.map_finder.memory.height = 460;
    app.compare.window_open = true;
    app.compare.window.x = 80;
    app.compare.window.y = 80;
    app.compare.window.width = 700;
    app.compare.window.height = 460;
    let targets = [WindowId::HexEditor, WindowId::MapFinder, WindowId::Compare];
    for id in &targets {
        assert!(app.dock_entries().iter().any(|entry| &entry.id == id));
    }

    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let overlap_point = egui::pos2(200.0, 150.0);
    let mut time = 1.0;
    let initial = render_calibration_tool_frame(&mut app, &context, screen, time);
    initial.drop_without_applying_deltas();
    let scope = app.project_scope();
    let layers = [
        (
            WindowId::HexEditor,
            egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new(("hex-editor", scope.as_str())),
            ),
        ),
        (
            WindowId::MapFinder,
            egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new(("map-finder", scope.as_str())),
            ),
        ),
        (
            WindowId::Compare,
            egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new(("compare-window", scope.clone())),
            ),
        ),
    ];
    for (id, expected_layer) in layers {
        app.focus_window(&id).unwrap();
        assert_eq!(app.project_preferences.dock_state.focused(), Some(&id));
        match &id {
            WindowId::HexEditor => assert!(app.hex_editor.focus_requested),
            WindowId::MapFinder => assert!(app.map_finder.focus_requested),
            WindowId::Compare => assert!(app.compare.focus_requested),
            _ => unreachable!(),
        }
        if id == WindowId::Compare {
            assert!(!app.map_finder.focus_requested);
        }
        time += 0.1;
        let output = render_calibration_tool_frame(&mut app, &context, screen, time);
        let top_layer = context.layer_id_at(overlap_point);
        output.drop_without_applying_deltas();
        assert_eq!(
            app.project_preferences.dock_state.focused(),
            Some(&id),
            "focus state after rendering {id:?}"
        );
        assert_eq!(top_layer, Some(expected_layer), "{id:?} should be topmost");
    }
    assert!(app.hex_editor.window_open);
    assert!(app.map_finder.memory.window_open);
    assert!(app.compare.window_open);
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), before_bin);
    assert_eq!(
        app.workspace.xdf.as_ref().unwrap().normalized_fingerprint,
        before_xdf
    );
}

#[test]
fn utility_windows_are_dockable_and_focusable() {
    let mut app = app_with_project_fixture();
    app.settings_open = true;
    app.debug_report_open = true;
    app.nooklink_setup_open = true;
    let targets = [
        WindowId::Settings,
        WindowId::DebugReport,
        WindowId::NookLink,
    ];
    for id in &targets {
        assert!(app.dock_entries().iter().any(|entry| &entry.id == id));
        app.focus_window(id).unwrap();
        assert_eq!(app.project_preferences.dock_state.focused(), Some(id));
    }
}

#[test]
fn utility_window_geometry_stays_inside_the_live_safe_area() {
    let mut app = app_with_project_fixture();
    app.settings_open = true;
    app.debug_report_open = true;
    app.nooklink_setup_open = true;
    let safe = egui::Rect::from_min_size(egui::pos2(200.0, 80.0), egui::vec2(700.0, 500.0));
    app.tool_window_bounds = Some(safe);
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    render_utility_windows_frame(&mut app, &context, screen, 1.0).drop_without_applying_deltas();

    for id in ["settings", "debug_report", "nooklink"] {
        let rect = app.project_preferences.utility_window_geometry[id].rect();
        assert!(
            safe.contains_rect(rect),
            "{id} escaped the safe area: {rect:?}"
        );
    }
}

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
    app.project_preferences
        .dock_state
        .open_tool(history_id.clone());
    app.project_preferences.dock_state.focus(history_id.clone());
    assert!(app
        .dock_entries()
        .iter()
        .any(|entry| entry.id == WindowId::XdfEditor));
    assert!(app
        .dock_entries()
        .iter()
        .any(|entry| entry.id == history_id));

    app.minimize_window(&WindowId::XdfEditor).unwrap();
    app.minimize_window(&history_id).unwrap();
    assert!(app.xdf_editor.open);
    assert_eq!(app.xdf_editor.draft, draft);
    assert_eq!(
        app.action_history_table.as_deref(),
        Some(table_key.as_str())
    );
}

#[test]
fn utility_close_uses_dirty_xdf_confirmation_and_cleans_other_targets() {
    let mut app = app_with_project_fixture();
    app.open_xdf_editor().unwrap();
    app.xdf_editor.dirty = true;
    app.close_window(&WindowId::XdfEditor).unwrap();
    assert!(app.xdf_editor.open);
    assert!(app.xdf_editor.close_prompt);
    assert!(app
        .dock_entries()
        .iter()
        .any(|entry| entry.id == WindowId::XdfEditor));

    app.xdf_editor.dirty = false;
    app.close_window(&WindowId::XdfEditor).unwrap();
    assert!(!app.xdf_editor.open);

    app.settings_open = true;
    app.debug_report_open = true;
    app.nooklink_setup_open = true;
    for id in [
        WindowId::Settings,
        WindowId::DebugReport,
        WindowId::NookLink,
    ] {
        app.mark_tool_window_open(id.clone());
        app.close_window(&id).unwrap();
        assert!(!app.dock_entries().iter().any(|entry| entry.id == id));
    }
    assert!(!app.settings_open);
    assert!(!app.debug_report_open);
    assert!(!app.nooklink_setup_open);
}

#[test]
fn project_notepad_is_project_scoped_and_uses_shared_window_controls() {
    let mut app = app_with_project_fixture();
    let bin_before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    app.open_project_notepad();
    let id = WindowId::ProjectNotepad;
    assert!(app.dock_entries().iter().any(|entry| entry.id == id));
    app.project_preferences.project_notepad = "Notes stay with this BIN.".into();
    app.project_preferences.notepad_stay_on_top = true;
    assert_eq!(app.dock_window_order(&id), egui::Order::Foreground);

    app.minimize_window(&id).unwrap();
    assert!(app.project_preferences.dock_state.is_minimized(&id));
    app.restore_window(&id).unwrap();
    assert_eq!(app.project_preferences.dock_state.focused(), Some(&id));
    app.close_window(&id).unwrap();
    assert!(!app.dock_entries().iter().any(|entry| entry.id == id));

    let encoded = serde_json::to_string(&app.project_preferences).unwrap();
    let restored: ProjectPreferences = serde_json::from_str(&encoded).unwrap();
    assert_eq!(restored.project_notepad, "Notes stay with this BIN.");
    assert!(restored.notepad_stay_on_top);
    assert_eq!(restored.bin_identity, app.project_preferences.bin_identity);
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), bin_before);
}

#[test]
fn project_notepad_window_shows_saved_text_and_stay_on_top_control() {
    let mut app = app_with_project_fixture();
    app.project_preferences.project_notepad = "Check the fuel table after the log.".into();
    app.open_project_notepad();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    app.tool_window_bounds = Some(screen);

    let mut labels = (false, false, false);
    for frame in 0..3 {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(f64::from(frame) * 0.1);
        let output = context.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.allocate_space(screen.size());
            });
            app.show_project_notepad_window(ctx);
            app.raise_focused_dock_window(ctx);
        });
        labels = (
            rendered_text_rect(&output, "Project Notepad").is_some(),
            rendered_text_rect(&output, "Stay on top").is_some(),
            rendered_text_rect(&output, "Check the fuel table after the log.").is_some(),
        );
        output.drop_without_applying_deltas();
        if labels.0 && labels.1 && labels.2 {
            break;
        }
    }

    assert_eq!(labels, (true, true, true));
}

#[test]
fn debug_report_yields_to_explicit_dock_focus() {
    let mut app = app_with_project_fixture();
    app.settings_open = true;
    app.project_preferences
        .dock_state
        .open_tool(WindowId::Settings);
    app.project_preferences
        .dock_state
        .restore(&WindowId::Settings);
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut time = 1.0;
    let settings = render_utility_windows_frame(&mut app, &context, screen, time);
    let project_scope = app.project_scope();
    let settings_layer = egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new(("settings-window", project_scope.as_str())),
    );
    assert!(context.memory(|memory| memory.layer_ids().any(|layer| layer == settings_layer)));
    settings.drop_without_applying_deltas();

    app.debug_report_open = true;
    app.project_preferences
        .dock_state
        .open_tool(WindowId::DebugReport);
    app.project_preferences
        .dock_state
        .restore(&WindowId::DebugReport);
    time += 0.1;
    let debug = render_utility_windows_frame(&mut app, &context, screen, time);
    let debug_layer = egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new(("debug-report", project_scope.as_str())),
    );
    assert!(context.memory(|memory| memory.layer_ids().any(|layer| layer == debug_layer)));
    debug.drop_without_applying_deltas();
    assert_ne!(debug_layer, settings_layer);

    app.focus_window(&WindowId::Settings).unwrap();
    time += 0.1;
    let focused = render_utility_windows_frame(&mut app, &context, screen, time);
    let layers = context.memory(|memory| memory.layer_ids().collect::<Vec<_>>());
    focused.drop_without_applying_deltas();
    assert!(layers.contains(&debug_layer));
    assert!(layers.contains(&settings_layer));
    assert!(
        layers.iter().position(|layer| *layer == settings_layer)
            > layers.iter().position(|layer| *layer == debug_layer),
        "explicit settings focus should raise it above the debug report"
    );
    assert_eq!(
        app.project_preferences.dock_state.focused(),
        Some(&WindowId::Settings)
    );
}

#[test]
fn session_only_windows_are_skipped_and_reported_on_project_restore() {
    let mut app = TunerApp::headless();
    let history = WindowId::ActionHistory("old-xdf|old-table".into());
    let mut project = ProjectPreferences::for_identity("bin-a");
    project.dock_state.open_tool(WindowId::XdfEditor);
    project.dock_state.open_tool(history.clone());
    project.dock_state.focus(history);
    let operation = app
        .operations
        .begin(OperationKind::RestoringWorkspace, "bin-a");

    app.install_operation_result(OperationResult::restored(operation, "bin-a", project));

    assert!(!app
        .project_preferences
        .dock_state
        .is_open_tool(&WindowId::XdfEditor));
    assert!(app
        .project_preferences
        .dock_state
        .open_tool_windows
        .iter()
        .all(|id| !matches!(id, WindowId::ActionHistory(_))));
    assert!(!app.xdf_editor.open);
    assert!(app.action_history_table.is_none());
    assert!(app
        .workspace_restore_notice
        .as_deref()
        .is_some_and(|notice| notice.contains("session-only")));
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
        app.show_workspace_dialog(ctx);
        school_help::show_help_window(ctx, app.preferences.school_me_mode);
    })
}

fn render_calibration_tool_frame(
    app: &mut TunerApp,
    context: &egui::Context,
    screen: egui::Rect,
    time: f64,
) -> egui::FullOutput {
    render_calibration_tool_frame_with_events(app, context, screen, time, Vec::new())
}

fn render_calibration_tool_frame_with_events(
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
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.allocate_space(screen.size());
        });
        app.show_compare_window(ctx);
        app.show_hex_editor(ctx, screen);
        app.show_map_finder(ctx, screen);
        app.raise_focused_dock_window(ctx);
        school_help::show_help_window(ctx, app.preferences.school_me_mode);
    })
}

fn render_utility_windows_frame(
    app: &mut TunerApp,
    context: &egui::Context,
    screen: egui::Rect,
    time: f64,
) -> egui::FullOutput {
    render_utility_windows_frame_with_events(app, context, screen, time, Vec::new())
}

fn render_utility_windows_frame_with_events(
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
        app.show_debug_report(ctx);
        app.show_settings(ctx);
        app.show_nooklink_setup(ctx);
        app.raise_debug_report(ctx);
        app.raise_focused_dock_window(ctx);
        school_help::show_help_window(ctx, app.preferences.school_me_mode);
    })
}

fn click_settings_label(
    app: &mut TunerApp,
    label: &str,
    context: &egui::Context,
    screen: egui::Rect,
    time: &mut f64,
) {
    let mut point = None;
    for _ in 0..3 {
        let output = render_utility_windows_frame(app, context, screen, *time);
        point = rendered_text_rect(&output, label).map(|rect| rect.center());
        output.drop_without_applying_deltas();
        if point.is_some() {
            break;
        }
        *time += 0.1;
    }
    let point = point.unwrap_or_else(|| panic!("Settings should render {label:?}"));
    for pressed in [true, false] {
        *time += 0.1;
        let output = render_utility_windows_frame_with_events(
            app,
            context,
            screen,
            *time,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        output.drop_without_applying_deltas();
    }
}

fn click_dock_label(
    app: &mut TunerApp,
    label: &str,
    context: &egui::Context,
    screen: egui::Rect,
    time: &mut f64,
) {
    let output = render_dock_frame(app, context, screen, *time, Vec::new());
    let Some(rect) = rendered_text_rect(&output, label) else {
        let layers = context.memory(|memory| memory.layer_ids().collect::<Vec<_>>());
        output.drop_without_applying_deltas();
        panic!("dock label {label:?} should render; layers={layers:?}");
    };
    let point = rect.center();
    output.drop_without_applying_deltas();
    for pressed in [true, false] {
        *time += 0.1;
        let event = egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let output = render_dock_frame(
            app,
            context,
            screen,
            *time,
            vec![egui::Event::PointerMoved(point), event],
        );
        output.drop_without_applying_deltas();
    }
}

fn rendered_table_cell_center(output: &egui::FullOutput, row: usize, column: usize) -> egui::Pos2 {
    let column_header = rendered_text_rect(output, &format!("C{column}"))
        .expect("table should render the requested column header");
    let row_header = rendered_text_rect(output, &format!("R{row}"))
        .expect("table should render the requested row header");
    egui::pos2(column_header.center().x, row_header.center().y)
}

#[test]
fn nooklink_quick_start_actions_copy_and_report_clipboard_results() {
    let mut app = TunerApp::headless();
    app.nooklink_setup_open = true;
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(screen);
    input.time = Some(0.0);
    context
        .run_ui(input, |ctx| {
            app.show_nooklink_setup_with_clipboard(ctx, |_| Ok(()));
        })
        .drop_without_applying_deltas();

    let mut input = egui::RawInput::default();
    input.screen_rect = Some(screen);
    input.time = Some(1.0);
    let output = context.run_ui(input, |ctx| {
        app.show_nooklink_setup_with_clipboard(ctx, |_| Ok(()));
    });
    let copy_rect = rendered_text_rect(&output, "Copy starter prompt");
    let has_export_button = rendered_text_rect(&output, "Save context file…").is_some();
    output.drop_without_applying_deltas();
    let copy_rect = copy_rect.expect("NookLink setup should show a copy-prompt button");
    assert!(has_export_button);

    let pointer = copy_rect.center();
    let mut copied_prompt = None;
    for (time, pressed, fail_copy) in [
        (1.1, true, false),
        (1.2, false, false),
        (1.3, true, false),
        (1.4, false, true),
    ] {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        input.events = vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::PointerButton {
                pos: pointer,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            },
        ];
        let output = if fail_copy {
            context.run_ui(input, |ctx| {
                app.show_nooklink_setup_with_clipboard(ctx, |_| {
                    Err("simulated clipboard failure".to_string())
                });
            })
        } else {
            context.run_ui(input, |ctx| {
                app.show_nooklink_setup_with_clipboard(ctx, |text| {
                    copied_prompt = Some(text.to_string());
                    Ok(())
                });
            })
        };
        output.drop_without_applying_deltas();
        if time == 1.2 {
            assert_eq!(app.workspace.status.level, StatusLevel::Success);
        }
    }

    let prompt = copied_prompt
        .as_deref()
        .expect("copy button should send the starter prompt to the clipboard");
    assert!(prompt.contains("Attach the NookLink Agent Context file"));
    assert!(prompt.contains("[describe your task]"));
    assert!(
        app.agent_tasks.is_empty(),
        "setup must not start an agent task"
    );
    assert_eq!(app.workspace.status.level, StatusLevel::Warning);
    assert!(app
        .workspace
        .status
        .text
        .contains("simulated clipboard failure"));
}

#[test]
fn nooklink_context_export_matches_the_embedded_file_and_reports_errors() {
    let mut app = TunerApp::headless();
    let export_path = temporary_test_path("md");
    app.save_nooklink_agent_context_to(&export_path).unwrap();
    assert_eq!(
        std::fs::read(&export_path).unwrap(),
        nooklink_quick_start::AGENT_CONTEXT.as_bytes()
    );
    assert_eq!(app.workspace.status.level, StatusLevel::Success);
    std::fs::remove_file(export_path).unwrap();

    let missing_parent = temporary_test_path("missing-directory");
    let failed_path = missing_parent.join("NookLink-Agent-Context.md");
    let error = app
        .save_nooklink_agent_context_to(&failed_path)
        .unwrap_err();
    assert_eq!(app.workspace.status.level, StatusLevel::Warning);
    assert!(app.workspace.status.text.contains(&error.to_string()));
}

#[test]
fn nooklink_setup_is_available_without_starting_or_configuring_an_agent() {
    let mut app = TunerApp::headless();
    assert!(app.agent_tasks.is_empty());
    assert!(!app.nooklink_setup_open);
    assert!(app.ui_ipc_capabilities()["actions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|action| action == "agent_task"));
    app.nooklink_setup_open = true;
    let context = egui::Context::default();
    let mut output = context.run_ui(egui::RawInput::default(), |ctx| {
        app.show_nooklink_setup(ctx)
    });
    output.textures_delta.clear();
    assert!(app.agent_tasks.is_empty());
}

#[test]
fn nooklink_quick_and_long_start_require_the_chosen_path() {
    let mut app = TunerApp::headless();
    let start = agent_ipc_request(
        json!({"action":"agent_task","agent_task":{"kind":"start","capability":"diagnostics","goal":"check"}}),
    );
    let quick = app.handle_ui_ipc_request(&start).data.unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_owned();
    app.start_nooklink_quick_task(&quick).unwrap();
    assert_eq!(
        app.agent_tasks.poll(&quick).unwrap().status,
        AgentTaskStatus::Running
    );
    assert!(app.nooklink_challenge.is_none());

    let long = app.handle_ui_ipc_request(&start).data.unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_owned();
    app.challenge_nooklink_long_task(&long);
    assert_eq!(
        app.agent_tasks.poll(&long).unwrap().status,
        AgentTaskStatus::AwaitingUserApproval
    );
    let answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    assert!(!app.complete_nooklink_challenge("wrong"));
    assert_eq!(
        app.agent_tasks.poll(&long).unwrap().status,
        AgentTaskStatus::AwaitingUserApproval
    );
    assert!(app.complete_nooklink_challenge(&answer));
    assert_eq!(
        app.agent_tasks.poll(&long).unwrap().level,
        Some(AgentTaskLevel::LongComplex)
    );
}

#[test]
fn nooklink_raw_read_challenge_denies_wrong_answer_and_changed_bin() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![1, 2, 3]));
    let start = agent_ipc_request(
        json!({"action":"agent_task","agent_task":{"kind":"start","capability":"diagnostics","goal":"inspect"}}),
    );
    let id = app.handle_ui_ipc_request(&start).data.unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_owned();
    app.start_nooklink_quick_task(&id).unwrap();
    let read = agent_ipc_request(
        json!({"action":"read_bin_bytes","task_id":id,"byte_offset":0,"byte_length":1}),
    );
    assert!(!app.handle_ui_ipc_request(&read).ok);
    app.challenge_nooklink_raw_read(&id);
    let answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    assert!(!app.complete_nooklink_challenge("wrong"));
    assert!(!app.agent_tasks.poll(&id).unwrap().raw_read_authorized);
    app.workspace.bin = Some(BinDocument::from_bytes(vec![4, 5, 6]));
    assert!(!app.complete_nooklink_challenge(&answer));
    assert!(!app.agent_tasks.poll(&id).unwrap().raw_read_authorized);
}

#[test]
fn nooklink_save_as_waits_for_challenge_and_rejects_stale_bin() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![1, 2, 3]));
    let start = agent_ipc_request(
        json!({"action":"agent_task","agent_task":{"kind":"start","capability":"diagnostics","goal":"save"}}),
    );
    let id = app.handle_ui_ipc_request(&start).data.unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_owned();
    app.start_nooklink_quick_task(&id).unwrap();
    let mut save = agent_ipc_request(json!({"action":"save_as","path":"nooklink-output.bin"}));
    assert!(!app.handle_ui_ipc_request(&save).ok);
    save.task_id = Some(id);
    assert!(app.handle_ui_ipc_request(&save).ok);
    assert!(!app.operations.is_busy());
    let answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![9, 8, 7]));
    assert!(!app.complete_nooklink_challenge(&answer));
    assert!(!app.operations.is_busy());
}

#[test]
fn nooklink_running_task_renders_and_raw_read_can_be_authorized() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![0xab, 0xcd]));
    let start = agent_ipc_request(
        json!({"action":"agent_task","agent_task":{"kind":"start","capability":"diagnostics","goal":"inspect"}}),
    );
    let id = app.handle_ui_ipc_request(&start).data.unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_owned();
    app.start_nooklink_quick_task(&id).unwrap();
    app.agent_tasks
        .update_progress(&id, "scan".into(), Some(40), "working".into())
        .unwrap();
    let read = agent_ipc_request(
        json!({"action":"read_bin_bytes","task_id":id,"byte_offset":0,"byte_length":1}),
    );
    assert!(!app.handle_ui_ipc_request(&read).ok);
    app.nooklink_setup_open = true;
    let context = egui::Context::default();
    let mut output = context.run_ui(egui::RawInput::default(), |ctx| {
        app.show_nooklink_setup(ctx)
    });
    output.textures_delta.clear();
    assert_eq!(
        app.agent_tasks.poll(&id).unwrap().progress_percent,
        Some(40)
    );
    assert_eq!(
        app.agent_tasks.poll(&id).unwrap().status,
        AgentTaskStatus::Running
    );
    app.challenge_nooklink_raw_read(&id);
    let answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    assert!(app.complete_nooklink_challenge(&answer));
    assert_eq!(app.handle_ui_ipc_request(&read).data.unwrap()["hex"], "ab");
    app.agent_tasks.cancel(&id).unwrap();
    assert_eq!(
        app.agent_tasks.poll(&id).unwrap().status,
        AgentTaskStatus::Cancelled
    );
}

fn start_quick_xdf_task(app: &mut TunerApp) -> String {
    let start = agent_ipc_request(json!({
        "action":"agent_task",
        "agent_task":{"kind":"start","capability":"diagnostics","goal":"save XDF"}
    }));
    let id = app.handle_ui_ipc_request(&start).data.unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_owned();
    app.agent_tasks
        .approve_start(&id, AgentTaskLevel::Quick)
        .unwrap();
    id
}

fn xdf_authoring_report() -> AgentReport {
    AgentReport {
        summary: "Add a verified table definition".into(),
        findings: vec!["The selected numeric grid appears map-like; verify before use.".into()],
        confidence: Some(0.9),
        evidence: Vec::new(),
        operations: vec![ProposedOperation::XdfAuthoring {
            operations: vec![ProposedXdfOperation::AddParameter {
                definition: ProposedXdfParameter {
                    unique_id: Some("agent-load-map".into()),
                    kind: ProposedXdfParameterKind::Table,
                    title: "Agent Load Map".into(),
                    description: "Proposed by a NookLink XDF authoring task.".into(),
                    category: Some("Fuel".into()),
                    xdf_address: 0x20,
                    element_width_bits: 8,
                    rows: 2,
                    columns: 3,
                    signed: false,
                    endianness: ProposedEndianness::Little,
                    numeric_kind: ProposedNumericKind::Integer,
                    column_major: false,
                    row_stride_bits: 24,
                    column_stride_bits: 8,
                    conversion: Some("X".into()),
                    bit_offset: None,
                    bit_width: None,
                    bit_mask: None,
                    axes: Vec::new(),
                },
            }],
        }],
    }
}

fn start_quick_xdf_authoring_task(app: &mut TunerApp) -> String {
    let start = agent_ipc_request(json!({
        "action":"agent_task",
        "agent_task":{"kind":"start","capability":"xdf_authoring","goal":"create a table definition"}
    }));
    let response = app.handle_ui_ipc_request(&start);
    assert!(response.ok, "{}", response.message);
    let id = response.data.unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_owned();
    app.start_nooklink_quick_task(&id).unwrap();
    id
}

fn submit_xdf_authoring_report(app: &mut TunerApp, task_id: &str) {
    let submit = agent_ipc_request(json!({
        "action":"agent_task",
        "agent_task":{"kind":"submit","task_id":task_id,"report":xdf_authoring_report()}
    }));
    let response = app.handle_ui_ipc_request(&submit);
    assert!(response.ok, "{}", response.message);
}

#[test]
fn nooklink_xdf_authoring_applies_only_to_the_draft_and_save_requires_a_second_challenge() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![0; 0x100]));
    let original_bin = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let task_id = start_quick_xdf_authoring_task(&mut app);
    submit_xdf_authoring_report(&mut app, &task_id);

    app.review_agent_proposal(&task_id, true).unwrap();
    let answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    assert!(app.complete_nooklink_challenge(&answer));
    assert_eq!(
        app.agent_tasks.poll(&task_id).unwrap().status,
        AgentTaskStatus::Applied
    );
    assert_eq!(
        app.xdf_editor.draft.as_ref().unwrap().parameters[0].title,
        "Agent Load Map"
    );
    assert!(app.workspace.xdf.is_none());
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        original_bin.as_slice()
    );

    let path = temporary_test_path("nooklink-xdf-draft-save.xdf");
    let save = agent_ipc_request(json!({"action":"save_xdf_as","path":path,"task_id":task_id}));
    let staged = app.handle_ui_ipc_request(&save);
    assert!(staged.ok, "{}", staged.message);
    assert!(matches!(
        app.nooklink_challenge
            .as_ref()
            .map(|challenge| &challenge.action),
        Some(NookLinkChallengeAction::SaveXdfAs {
            draft_revision: Some(_),
            ..
        })
    ));

    let mut header = app.xdf_editor.draft.as_ref().unwrap().header.clone();
    header.title = Some("User changed the draft".into());
    app.xdf_editor
        .apply_operations(&[XdfAuthoringOperation::SetHeader { header }])
        .unwrap();
    let answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    assert!(!app.complete_nooklink_challenge(&answer));
    assert!(!app.operations.is_busy());
    assert!(!path.exists());
}

#[test]
fn nooklink_xdf_proposal_is_stale_if_the_draft_changes_before_review() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![0; 0x100]));
    let task_id = start_quick_xdf_authoring_task(&mut app);
    submit_xdf_authoring_report(&mut app, &task_id);
    let mut header = app.xdf_editor.draft.as_ref().unwrap().header.clone();
    header.title = Some("Changed while agent was working".into());
    app.xdf_editor
        .apply_operations(&[XdfAuthoringOperation::SetHeader { header }])
        .unwrap();

    assert!(app.review_agent_proposal(&task_id, true).is_err());
    assert_eq!(
        app.agent_tasks.poll(&task_id).unwrap().status,
        AgentTaskStatus::Stale
    );
    assert!(app.nooklink_challenge.is_none());
    assert!(app.xdf_editor.draft.as_ref().unwrap().parameters.is_empty());
}

#[test]
fn nooklink_xdf_authoring_save_challenge_writes_only_after_confirmation() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![0; 0x100]));
    let task_id = start_quick_xdf_authoring_task(&mut app);
    submit_xdf_authoring_report(&mut app, &task_id);
    app.review_agent_proposal(&task_id, true).unwrap();
    let answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    assert!(app.complete_nooklink_challenge(&answer));

    let path = temporary_test_path("nooklink-xdf-draft-confirmed.xdf");
    let save = agent_ipc_request(json!({"action":"save_xdf_as","path":path,"task_id":task_id}));
    assert!(app.handle_ui_ipc_request(&save).ok);
    assert!(!path.exists());
    let answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    assert!(app.complete_nooklink_challenge(&answer));

    let context = egui::Context::default();
    for _ in 0..100 {
        app.poll_background_operations(&context);
        if !app.operations.is_busy() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!app.operations.is_busy());
    let saved = XdfDocument::load(&path).unwrap();
    assert_eq!(saved.parameters[0].title, "Agent Load Map");
    assert!(!app.xdf_editor.is_dirty());
    app.close_window(&WindowId::XdfEditor).unwrap();
    assert!(!app.xdf_editor.close_prompt);
    assert!(!app.xdf_editor.open);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn nooklink_xdf_overwrite_replaces_only_after_the_existing_challenge() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![0; 0x100]));
    let task_id = start_quick_xdf_authoring_task(&mut app);
    submit_xdf_authoring_report(&mut app, &task_id);
    app.review_agent_proposal(&task_id, true).unwrap();
    let apply_answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    assert!(app.complete_nooklink_challenge(&apply_answer));

    let path = temporary_test_path("nooklink-xdf-overwrite.xdf");
    let original = XdfDocument::parse(
        br#"<XDFFORMAT><XDFCONSTANT uniqueid="original"><title>Original</title><EMBEDDEDDATA mmedaddress="0" mmedelementsizebits="8" /></XDFCONSTANT></XDFFORMAT>"#,
    )
    .unwrap();
    std::fs::write(&path, original.to_xdf_text().unwrap()).unwrap();
    let save = agent_ipc_request(json!({"action":"save_xdf_as","path":path,"task_id":task_id}));
    assert!(app.handle_ui_ipc_request(&save).ok);
    assert!(matches!(
        app.nooklink_challenge
            .as_ref()
            .map(|challenge| &challenge.action),
        Some(NookLinkChallengeAction::SaveXdfAs {
            target_existed: true,
            ..
        })
    ));
    assert_eq!(
        XdfDocument::load(&path).unwrap().parameters[0].title,
        "Original"
    );

    let save_answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    assert!(app.complete_nooklink_challenge(&save_answer));
    let context = egui::Context::default();
    for _ in 0..100 {
        app.poll_background_operations(&context);
        if !app.operations.is_busy() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!app.operations.is_busy());
    assert_eq!(
        XdfDocument::load(&path).unwrap().parameters[0].title,
        "Agent Load Map"
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn nooklink_save_xdf_as_requires_running_task_for_active_xdf() {
    let mut app = TunerApp::headless();
    let (xdf, _) = column_major_fixture();
    app.workspace.xdf = Some(xdf);
    let path = temporary_test_path("nooklink-xdf-missing-task.xdf");
    let mut request = agent_ipc_request(json!({"action":"save_xdf_as","path":path}));
    let missing_task = app.handle_ui_ipc_request(&request);
    assert!(!missing_task.ok);
    assert!(missing_task.message.contains("running task ID"));
    assert!(!app.operations.is_busy());

    let id = start_quick_xdf_task(&mut app);
    app.workspace.xdf = Some(XdfDocument::parse(CONVERSION_TABLE_XDF).unwrap());
    request.task_id = Some(id);
    let stale_xdf = app.handle_ui_ipc_request(&request);
    assert!(!stale_xdf.ok);
    assert!(stale_xdf.message.contains("active XDF"));
    assert!(app.nooklink_challenge.is_none());
    assert!(!app.operations.is_busy());
}

#[test]
fn nooklink_save_xdf_as_waits_for_challenge_before_saving() {
    let mut app = TunerApp::headless();
    let (xdf, _) = column_major_fixture();
    app.workspace.xdf = Some(xdf);
    let id = start_quick_xdf_task(&mut app);
    let path = temporary_test_path("nooklink-xdf-confirmed.xdf");
    let mut request = agent_ipc_request(json!({"action":"save_xdf_as","path":path}));
    request.task_id = Some(id);
    let staged = app.handle_ui_ipc_request(&request);
    assert!(staged.ok, "{}", staged.message);
    assert!(!app.operations.is_busy());
    assert!(!path.exists());
    let captured_hash = app.workspace.xdf.as_ref().unwrap().exact_sha256.clone();
    assert!(matches!(
        &app.nooklink_challenge.as_ref().unwrap().action,
        NookLinkChallengeAction::SaveXdfAs {
            task_id,
            path: staged_path,
            xdf_sha256,
            draft_revision: None,
            ..
        } if task_id == request.task_id.as_ref().unwrap()
            && staged_path == &path
            && xdf_sha256 == &captured_hash
    ));
    let answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    let state = app.ui_ipc_state();
    assert!(state.get("challenge").is_none());
    assert!(state.get("answer").is_none());
    assert!(!app.complete_nooklink_challenge("wrong"));
    assert!(!app.operations.is_busy());
    assert!(app.nooklink_challenge.is_some());
    assert!(app.complete_nooklink_challenge(&answer));
    assert_eq!(
        app.operations.active().map(|operation| operation.kind),
        Some(OperationKind::SavingXdf)
    );
    let result = (0..100).find_map(|_| {
        let result = app.operations.take_results().into_iter().next();
        if result.is_none() {
            std::thread::sleep(Duration::from_millis(5));
        }
        result
    });
    app.install_operation_result(result.expect("XDF save worker should finish"));
    assert!(path.is_file());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn nooklink_save_xdf_as_rejects_changed_xdf_after_challenge() {
    let mut app = TunerApp::headless();
    let (xdf, _) = column_major_fixture();
    app.workspace.xdf = Some(xdf);
    let id = start_quick_xdf_task(&mut app);
    let path = temporary_test_path("nooklink-xdf-stale.xdf");
    let mut request = agent_ipc_request(json!({"action":"save_xdf_as","path":path}));
    request.task_id = Some(id);
    assert!(app.handle_ui_ipc_request(&request).ok);
    let answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    app.workspace.xdf = Some(XdfDocument::parse(CONVERSION_TABLE_XDF).unwrap());
    assert!(!app.complete_nooklink_challenge(&answer));
    assert!(!app.operations.is_busy());
    assert!(!path.exists());
}

#[test]
fn nooklink_save_xdf_as_rejects_task_cancelled_after_challenge() {
    let mut app = TunerApp::headless();
    let (xdf, _) = column_major_fixture();
    app.workspace.xdf = Some(xdf);
    let id = start_quick_xdf_task(&mut app);
    let path = temporary_test_path("nooklink-xdf-cancelled.xdf");
    let mut request = agent_ipc_request(json!({"action":"save_xdf_as","path":path}));
    request.task_id = Some(id.clone());
    assert!(app.handle_ui_ipc_request(&request).ok);
    let answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    app.agent_tasks.cancel(&id).unwrap();
    assert!(!app.complete_nooklink_challenge(&answer));
    assert!(!app.operations.is_busy());
    assert!(!path.exists());
}

#[test]
fn nooklink_dispatch_rejects_save_dialog_commands() {
    for (command, action) in [
        ("file.save-as", "save_as"),
        ("file.save-xdf-as", "save_xdf_as"),
    ] {
        let mut app = TunerApp::headless();
        let (xdf, bin) = column_major_fixture();
        app.workspace.xdf = Some(xdf);
        app.workspace.bin = Some(bin);
        app.operations
            .begin(OperationKind::SavingBin, "already busy");
        let request = agent_ipc_request(json!({"action":"dispatch","command":command}));
        let response = app.handle_ui_ipc_request(&request);
        assert!(!response.ok);
        assert!(response.message.contains(action), "{}", response.message);
        assert!(app.operations.is_busy());
    }
}

#[test]
fn nooklink_cannot_save_project_manifest_through_unapproved_dispatch() {
    let bin_path = temporary_test_path("bin");
    let manifest_path = temporary_test_path("tnproj");
    std::fs::write(&bin_path, [1_u8, 2]).unwrap();
    let identity = project_identity_for_path(&bin_path);
    let mut app = TunerApp::headless();
    app.persistence_enabled = false;
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![1, 2])),
        None,
        Some(bin_path.clone()),
        None,
    );
    app.project_identity = Some(identity.clone());
    app.project_preferences = ProjectPreferences::for_identity(&identity);
    app.project_preferences.project_file_path = Some(manifest_path.clone());

    let request = agent_ipc_request(json!({
        "action": "dispatch",
        "command": "file.save-project"
    }));
    let response = app.handle_ui_ipc_request(&request);
    let wrote_manifest = manifest_path.exists();
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_file(&bin_path);

    assert!(
        !response.ok,
        "NookLink should require the explicit UI save flow"
    );
    assert!(
        !wrote_manifest,
        "unapproved NookLink dispatch wrote a manifest"
    );
    assert!(response.message.contains("TunerNook"));
}

#[test]
fn ui_ipc_agent_task_routes_start_progress_poll_and_cancellation() {
    let mut app = app_with_matching_compare_documents();
    let start = agent_ipc_request(
        json!({"action":"agent_task","agent_task":{"kind":"start","capability":"diagnostics","goal":"check mappings"}}),
    );
    let response = app.handle_ui_ipc_request(&start);
    assert!(response.ok, "{}", response.message);
    let task = response.data.unwrap();
    let id = task["task_id"].as_str().unwrap();
    assert_eq!(task["status"], "awaiting_user_approval");
    assert_eq!(
        task["document_identity"]["bin_sha256"],
        sha256_hex(app.workspace.bin.as_ref().unwrap().bytes())
    );
    assert_eq!(
        task["document_identity"]["xdf_sha256"],
        app.workspace.xdf.as_ref().unwrap().exact_sha256
    );
    let id = id.to_string();
    app.agent_tasks
        .approve_start(&id, AgentTaskLevel::Quick)
        .unwrap();
    let progress = agent_ipc_request(
        json!({"action":"agent_task","agent_task":{"kind":"progress","task_id":id,"phase":"scan","progress_percent":40,"message":"working"}}),
    );
    assert!(app.handle_ui_ipc_request(&progress).ok);
    let poll =
        agent_ipc_request(json!({"action":"agent_task","agent_task":{"kind":"poll","task_id":id}}));
    let response = app.handle_ui_ipc_request(&poll);
    assert_eq!(response.data.unwrap()["progress_percent"], 40);
    app.agent_tasks.cancel(&id).unwrap();
    assert!(!app.handle_ui_ipc_request(&progress).ok);
    assert_eq!(
        app.handle_ui_ipc_request(&poll).data.unwrap()["status"],
        "cancelled"
    );
}

#[test]
fn ui_ipc_agent_task_rejects_invalid_messages_and_exposes_safe_state() {
    let mut app = TunerApp::headless();
    let missing = agent_ipc_request(json!({"action":"agent_task"}));
    assert!(!app.handle_ui_ipc_request(&missing).ok);
    assert!(serde_json::from_value::<ui_ipc::UiIpcRequest>(json!({"action":"agent_task","agent_task":{"kind":"start","capability":"unknown","goal":"x"}})).is_err());
    let start = agent_ipc_request(
        json!({"action":"agent_task","token":"local-secret-token","agent_task":{"kind":"start","capability":"diagnostics","goal":"x"}}),
    );
    let task = app.handle_ui_ipc_request(&start).data.unwrap();
    let id = task["task_id"].as_str().unwrap();
    app.agent_tasks
        .approve_start(id, AgentTaskLevel::Quick)
        .unwrap();
    let high = agent_ipc_request(
        json!({"action":"agent_task","agent_task":{"kind":"progress","task_id":id,"phase":"scan","progress_percent":101,"message":"x"}}),
    );
    assert!(!app.handle_ui_ipc_request(&high).ok);
    let state = app.ui_ipc_state();
    assert_eq!(state["agent_tasks"][0]["task_id"], id);
    assert_eq!(state["agent_tasks"][0]["capability"], "diagnostics");
    assert_eq!(state["agent_tasks"][0]["status"], "running");
    assert!(state["agent_tasks"][0]["review_decision"].is_null());
    assert!(!state.to_string().contains("local-secret-token"));
    let actions = app.ui_ipc_capabilities()["actions"]
        .as_array()
        .unwrap()
        .clone();
    assert!(actions.iter().any(|action| action == "agent_task"));
    assert!(actions.iter().any(|action| action == "read_bin_bytes"));
}

#[test]
fn ui_ipc_agent_task_read_bin_bytes_is_bounded_and_exact() {
    let mut app = TunerApp::headless();
    let mut read =
        agent_ipc_request(json!({"action":"read_bin_bytes","byte_offset":1,"byte_length":2}));
    assert!(!app.handle_ui_ipc_request(&read).ok);
    app.workspace.bin = Some(BinDocument::from_bytes(vec![0, 0xab, 0xcd, 0xff]));
    let start = agent_ipc_request(json!({
        "action":"agent_task",
        "agent_task":{"kind":"start","capability":"diagnostics","goal":"inspect"}
    }));
    let id = app.handle_ui_ipc_request(&start).data.unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_string();
    app.agent_tasks
        .approve_start(&id, AgentTaskLevel::Quick)
        .unwrap();
    read.task_id = Some(id.clone());
    assert!(!app.handle_ui_ipc_request(&read).ok);
    let bin_hash = sha256_hex(app.workspace.bin.as_ref().unwrap().bytes());
    app.agent_tasks.allow_raw_reads(&id, &bin_hash).unwrap();
    let response = app.handle_ui_ipc_request(&read);
    assert!(response.ok, "{}", response.message);
    assert_eq!(
        response.data.unwrap(),
        json!({"offset":1,"length":2,"hex":"abcd","bin_sha256":sha256_hex(&[0, 0xab, 0xcd, 0xff]),"dirty":false})
    );
    for (offset, length) in [(0, 0), (usize::MAX, 2), (3, 2), (0, 65_537)] {
        read.byte_offset = Some(offset);
        read.byte_length = Some(length);
        assert!(!app.handle_ui_ipc_request(&read).ok, "{offset}+{length}");
    }
    read.path = Some(PathBuf::from("other.bin"));
    assert!(!app.handle_ui_ipc_request(&read).ok);
}

#[test]
fn ui_ipc_agent_task_state_reflects_submitted_record() {
    let mut app = TunerApp::headless();
    let start = agent_ipc_request(
        json!({"action":"agent_task","agent_task":{"kind":"start","capability":"diagnostics","goal":"x"}}),
    );
    let response = app.handle_ui_ipc_request(&start);
    assert!(response.ok, "{}", response.message);
    let id = response.data.unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_string();
    app.agent_tasks
        .approve_start(&id, AgentTaskLevel::Quick)
        .unwrap();
    let submit = agent_ipc_request(
        json!({"action":"agent_task","agent_task":{"kind":"submit","task_id":id,"report":{"summary":"checked","findings":["one"],"confidence":0.8,"evidence":[],"operations":[]}}}),
    );
    assert!(app.handle_ui_ipc_request(&submit).ok);
    let poll =
        agent_ipc_request(json!({"action":"agent_task","agent_task":{"kind":"poll","task_id":id}}));
    let polled = app.handle_ui_ipc_request(&poll).data.unwrap();
    let state = app.ui_ipc_state();
    assert_eq!(state["agent_tasks"].as_array().unwrap().len(), 1);
    assert_eq!(state["agent_tasks"][0]["task_id"], id);
    assert_eq!(state["agent_tasks"][0]["status"], "awaiting_review");
    assert_eq!(state["agent_tasks"][0]["report"]["summary"], "checked");
    assert_eq!(state["agent_tasks"][0]["report"], polled["report"]);
}

#[test]
fn ui_ipc_agent_task_requires_app_approval_before_running() {
    let mut app = TunerApp::headless();
    let start = agent_ipc_request(json!({
        "action":"agent_task",
        "agent_task":{"kind":"start","capability":"diagnostics","goal":"inspect"}
    }));
    let response = app.handle_ui_ipc_request(&start);
    assert!(response.ok, "{}", response.message);
    let task = response.data.unwrap();
    assert_eq!(task["status"], "awaiting_user_approval");
    let progress = agent_ipc_request(json!({
        "action":"agent_task",
        "agent_task":{"kind":"progress","task_id":task["task_id"],"phase":"scan","progress_percent":1,"message":"working"}
    }));
    assert!(!app.handle_ui_ipc_request(&progress).ok);
    let state = app.handle_ui_ipc_request(&agent_ipc_request(json!({"action":"state"})));
    assert!(state.ok);
    let state = state.data.unwrap().to_string();
    assert!(!state.contains("challenge"));
    assert!(!state.contains("answer"));
}

#[test]
fn ui_ipc_raw_bin_reads_require_task_authorization_and_unchanged_identity() {
    let original_bytes = vec![0x10, 0x20, 0x30, 0x40];
    let original_hash = sha256_hex(&original_bytes);
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(original_bytes.clone()));
    let start = agent_ipc_request(json!({
        "action":"agent_task",
        "agent_task":{"kind":"start","capability":"diagnostics","goal":"inspect bytes"}
    }));
    let started = app.handle_ui_ipc_request(&start).data.unwrap();
    let task_id = started["task_id"].as_str().unwrap().to_string();
    assert_eq!(started["status"], "awaiting_user_approval");

    let pending_task_read = agent_ipc_request(json!({
        "action":"read_bin_bytes","task_id":task_id,"byte_offset":1,"byte_length":2
    }));
    let response = app.handle_ui_ipc_request(&pending_task_read);
    assert!(!response.ok);
    assert!(response.data.is_none());

    let missing_id = agent_ipc_request(json!({
        "action":"read_bin_bytes","byte_offset":1,"byte_length":2
    }));
    let response = app.handle_ui_ipc_request(&missing_id);
    assert!(!response.ok);
    assert!(response.data.is_none());

    let unknown_id = agent_ipc_request(json!({
        "action":"read_bin_bytes","task_id":"unknown-task","byte_offset":1,"byte_length":2
    }));
    assert!(!app.handle_ui_ipc_request(&unknown_id).ok);

    app.agent_tasks
        .approve_start(&task_id, agent_tasks::AgentTaskLevel::Quick)
        .unwrap();
    let request = agent_ipc_request(json!({
        "action":"read_bin_bytes","task_id":task_id,"byte_offset":1,"byte_length":2
    }));
    let response = app.handle_ui_ipc_request(&request);
    assert!(!response.ok);
    assert!(response.data.is_none());
    let record = app.agent_tasks.poll(&task_id).unwrap();
    assert!(record.raw_read_authorization_pending);
    assert!(!record.raw_read_authorized);

    app.agent_tasks
        .allow_raw_reads(&task_id, "different-hash")
        .unwrap_err();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![0xff, 0xee, 0xdd, 0xcc]));
    let changed_bin_read = app.handle_ui_ipc_request(&request);
    assert!(!changed_bin_read.ok);
    assert!(changed_bin_read.data.is_none());
    let changed_hash = sha256_hex(app.workspace.bin.as_ref().unwrap().bytes());
    app.agent_tasks
        .allow_raw_reads(&task_id, &changed_hash)
        .unwrap_err();

    app.workspace.bin = Some(BinDocument::from_bytes(original_bytes));
    app.agent_tasks
        .allow_raw_reads(&task_id, &original_hash)
        .unwrap();
    let response = app.handle_ui_ipc_request(&request);
    assert!(response.ok, "{}", response.message);
    assert_eq!(
        response.data.unwrap(),
        json!({
            "offset":1,"length":2,"hex":"2030","bin_sha256":original_hash,"dirty":false
        })
    );
    app.workspace.bin = None;
    let missing_bin_read = app.handle_ui_ipc_request(&request);
    assert!(!missing_bin_read.ok);
    assert!(missing_bin_read.data.is_none());

    for action in ["state", "hex_state", "map_search_state"] {
        let ordinary_read = agent_ipc_request(json!({"action":action}));
        assert!(app.handle_ui_ipc_request(&ordinary_read).ok, "{action}");
    }
}

fn app_with_compare_documents_and_different_xdfs() -> TunerApp {
    let mut app = app_with_matching_compare_documents();
    let mut source_xdf_bytes = COLUMN_MAJOR_XDF.to_vec();
    source_xdf_bytes.push(b'\n');
    app.compare.source_xdf = Some(XdfDocument::parse(&source_xdf_bytes).unwrap());
    app.compare.source_xdf_path = Some(PathBuf::from("different-definition.xdf"));
    app
}

fn wait_for_operation_result(app: &mut TunerApp) -> OperationResult {
    for _ in 0..200 {
        if let Some(result) = app.operations.take_results().into_iter().next() {
            return result;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("background operation did not produce a result");
}

const COLUMN_MAJOR_XDF: &[u8] = br#"<XDFFORMAT><XDFTABLE uniqueid="map">
    <title>Map</title><XDFAXIS id="z">
      <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8"
        mmedrowcount="2" mmedcolcount="3"
        mmedmajorstridebits="0" mmedminorstridebits="0"
        mmedtypeflags="0x06" />
    </XDFAXIS>
</XDFTABLE></XDFFORMAT>"#;

const AXIS_TABLE_XDF: &[u8] = br#"<XDFFORMAT><XDFTABLE uniqueid="axis-map">
    <title>Axis Map</title>
    <XDFAXIS id="x"><indexcount>2</indexcount>
      <EMBEDDEDDATA mmedaddress="0x10" mmedelementsizebits="8"
        mmedmajorstridebits="8" mmedtypeflags="0x06" />
      <XDFCONVERT><MATH equation="X * 0.5" /></XDFCONVERT>
    </XDFAXIS>
    <XDFAXIS id="y"><indexcount>2</indexcount>
      <EMBEDDEDDATA mmedaddress="0x20" mmedelementsizebits="16"
        mmedmajorstridebits="16" mmedtypeflags="0x06" />
    </XDFAXIS>
    <XDFAXIS id="z"><EMBEDDEDDATA mmedaddress="0x30"
        mmedelementsizebits="8" mmedrowcount="2" mmedcolcount="2"
        mmedmajorstridebits="8" mmedminorstridebits="8" mmedtypeflags="0x06" />
    </XDFAXIS>
</XDFTABLE></XDFFORMAT>"#;

const DESCRIPTIVE_AXIS_XDF: &[u8] = br#"<XDFFORMAT><XDFTABLE uniqueid="descriptive-axis-map">
    <title>Descriptive Axis Map</title>
    <XDFAXIS id="x"><indexcount>2</indexcount>
      <LABEL index="0">Low</LABEL><LABEL index="1">High</LABEL>
    </XDFAXIS>
    <XDFAXIS id="z"><EMBEDDEDDATA mmedaddress="0x30"
      mmedelementsizebits="8" mmedrowcount="1" mmedcolcount="2"
      mmedmajorstridebits="8" mmedminorstridebits="8" mmedtypeflags="0x06" />
    </XDFAXIS>
</XDFTABLE></XDFFORMAT>"#;

const TWO_TABLE_XDF: &[u8] = br#"<XDFFORMAT>
    <XDFTABLE uniqueid="map-a"><title>Map A</title><XDFAXIS id="z">
      <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8"
        mmedrowcount="1" mmedcolcount="1" mmedmajorstridebits="0"
        mmedminorstridebits="0" mmedtypeflags="0x06" />
    </XDFAXIS></XDFTABLE>
    <XDFTABLE uniqueid="map-b"><title>Map B</title><XDFAXIS id="z">
      <EMBEDDEDDATA mmedaddress="0x01" mmedelementsizebits="8"
        mmedrowcount="1" mmedcolcount="1" mmedmajorstridebits="0"
        mmedminorstridebits="0" mmedtypeflags="0x06" />
    </XDFAXIS></XDFTABLE>
</XDFFORMAT>"#;

const LARGE_TABLE_XDF: &[u8] = br#"<XDFFORMAT><XDFTABLE uniqueid="large-map">
    <title>Large Map</title><XDFAXIS id="z">
      <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8"
        mmedrowcount="12" mmedcolcount="12" mmedmajorstridebits="0"
        mmedminorstridebits="0" mmedtypeflags="0x06" />
    </XDFAXIS>
</XDFTABLE></XDFFORMAT>"#;

const TYPE_AWARE_XDF: &[u8] = br#"<XDFFORMAT>
    <XDFCONSTANT uniqueid="scalar"><title>Scalar value</title>
      <XDFCONVERT><MATH equation="2 * X + 1" /></XDFCONVERT>
      <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="16" mmedtypeflags="0x02" />
    </XDFCONSTANT>
    <XDFFLAG uniqueid="enabled"><title>Enabled flag</title>
      <EMBEDDEDDATA mmedaddress="0x02" mmedelementsizebits="8" mmedtypeflags="0x02" />
      <MASK>0x04</MASK>
    </XDFFLAG>
    <XDFBITFIELD uniqueid="mode"><title>Mode bits</title>
      <EMBEDDEDDATA mmedaddress="0x03" mmedelementsizebits="8" mmedtypeflags="0x02" />
      <BITOFFSET>1</BITOFFSET><BITWIDTH>3</BITWIDTH>
    </XDFBITFIELD>
</XDFFORMAT>"#;

const CONVERSION_TABLE_XDF: &[u8] = br#"<XDFFORMAT><XDFTABLE uniqueid="convert-map">
    <title>Converted Map</title><XDFCONVERT><MATH equation="X" /></XDFCONVERT>
    <XDFAXIS id="z"><EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8"
      mmedrowcount="1" mmedcolcount="1" mmedmajorstridebits="0"
      mmedminorstridebits="0" mmedtypeflags="0x06" /></XDFAXIS>
</XDFTABLE></XDFFORMAT>"#;

#[test]
fn axis_header_text_uses_actual_values_and_table_precision() {
    let view = AxisView {
        axis_index: 0,
        index: 0,
        label: None,
        range: Some(ByteRange {
            start: 0x10,
            end: 0x11,
        }),
        raw: Some(RawValue::Unsigned(25)),
        engineering: Some(12.5),
        editable: true,
        error: None,
    };

    assert_eq!(
        table_axis_header_text(&view, TableAxisRole::X, 0, TableDisplay::Engineering, 2,),
        "12.50"
    );
    assert_eq!(
        table_axis_header_text(&view, TableAxisRole::X, 0, TableDisplay::Raw, 2),
        "u25"
    );

    let labeled_view = AxisView {
        label: Some("Low".to_string()),
        raw: None,
        engineering: None,
        editable: false,
        range: None,
        error: None,
        ..view
    };
    assert_eq!(
        table_axis_header_text(
            &labeled_view,
            TableAxisRole::X,
            0,
            TableDisplay::Engineering,
            2,
        ),
        "Low"
    );
}

#[test]
fn table_zoom_scales_content_metrics_without_changing_zoom_bounds() {
    let base = table_ui_scale(100);
    let enlarged = table_ui_scale(150);
    assert_eq!(base.factor, 1.0);
    assert_eq!(enlarged.factor, 1.5);
    assert!(enlarged.cell_width > base.cell_width);
    assert!(enlarged.cell_height > base.cell_height);
    assert!(enlarged.body_font_size > base.body_font_size);
    assert_eq!(table_ui_scale(10).factor, 0.5);
    assert_eq!(table_ui_scale(500).factor, 2.0);
}

#[test]
fn auto_color_ramp_is_monotonic_and_fixed_range_clamps() {
    let settings = TableColorSettings::default();
    let colors = color_ramp_samples(&settings, (0.0, 100.0), &[0.0, 50.0, 100.0]);
    assert_ne!(colors[0], colors[1]);
    assert_ne!(colors[1], colors[2]);

    let mut fixed = settings;
    fixed.mode = TableColorMode::FixedRange;
    fixed.fixed_min = Some(10.0);
    fixed.fixed_max = Some(90.0);
    assert_eq!(
        table_cell_fill(&fixed, 0.0, Some((0.0, 100.0))),
        table_cell_fill(&fixed, 10.0, Some((0.0, 100.0)))
    );
    assert_eq!(
        table_cell_fill(&fixed, 100.0, Some((0.0, 100.0))),
        table_cell_fill(&fixed, 90.0, Some((0.0, 100.0)))
    );
    assert!(table_cell_fill(&fixed, 50.0, None).is_some());
}

#[test]
fn table_color_module_preserves_root_api() {
    let settings = TableColorSettings::default();
    let observed = Some((0.0, 100.0));
    let expected = Some(egui::Color32::from_rgba_unmultiplied(32, 76, 144, 190));

    assert_eq!(
        crate::table_colors::table_cell_fill(&settings, 0.0, observed),
        expected
    );
    assert_eq!(table_cell_fill(&settings, 0.0, observed), expected);
    assert_eq!(
        crate::table_colors::color_range_for_values(&[3.0, f64::NAN, -2.0]),
        Some((-2.0, 3.0))
    );
}

#[test]
fn auto_color_range_ignores_retained_fixed_thresholds() {
    let mut settings = TableColorSettings::default();
    settings.fixed_min = Some(40.0);
    settings.fixed_max = Some(60.0);

    let expected = table_cell_fill(&TableColorSettings::default(), 20.0, Some((0.0, 100.0)));
    assert_eq!(
        table_cell_fill(&settings, 20.0, Some((0.0, 100.0))),
        expected
    );
}

#[test]
fn table_coloring_paints_unselected_cells() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    let fill = egui::Color32::from_rgba_unmultiplied(17, 29, 43, 255);
    let coloring = &mut app.open_tables[0].memory.coloring;
    coloring.low = fill.to_array();
    coloring.middle = fill.to_array();
    coloring.high = fill.to_array();

    fn count_fill(shape: &egui::epaint::Shape, fill: egui::Color32) -> usize {
        match shape {
            egui::epaint::Shape::Rect(rect) => usize::from(rect.fill == fill),
            egui::epaint::Shape::Vec(shapes) => {
                shapes.iter().map(|shape| count_fill(shape, fill)).sum()
            }
            _ => 0,
        }
    }

    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(canvas);
    let mut output = context.run_ui(input, |ctx| {
        app.show_table_window(ctx, canvas, &key);
    });
    output.textures_delta.clear();
    for _ in 0..12 {
        let mut next_input = egui::RawInput::default();
        next_input.screen_rect = Some(canvas);
        output = context.run_ui(next_input, |ctx| {
            app.show_table_window(ctx, canvas, &key);
        });
        output.textures_delta.clear();
    }
    let fill_count: usize = output
        .shapes
        .iter()
        .map(|clipped| count_fill(&clipped.shape, fill))
        .sum();
    assert!(
        fill_count >= 5,
        "expected the range fill on unselected cells, found {fill_count} matching shape(s)"
    );
}

#[test]
fn selected_cell_sweep_changes_only_the_selected_cell_fills() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    let dimensions = app
        .workspace
        .selected_parameter()
        .unwrap()
        .layout
        .dimensions;
    let total_cells = dimensions.rows * dimensions.columns;
    let base = egui::Color32::from_rgb(17, 29, 43);
    let coloring = &mut app.open_tables[0].memory.coloring;
    coloring.low = base.to_array();
    coloring.middle = base.to_array();
    coloring.high = base.to_array();
    app.workspace.select_cell_range(0, 0, 0, 1).unwrap();
    app.preferences.sweep_enabled = true;

    fn count_fill(shape: &egui::epaint::Shape, fill: egui::Color32) -> usize {
        match shape {
            egui::epaint::Shape::Rect(rect) => usize::from(rect.fill == fill),
            egui::epaint::Shape::Vec(shapes) => {
                shapes.iter().map(|shape| count_fill(shape, fill)).sum()
            }
            _ => 0,
        }
    }

    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let mut output = egui::FullOutput::default();
    let mut time = 0.5;
    for _ in 0..13 {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        output = context.run_ui(input, |ctx| {
            app.show_table_window(ctx, screen, &key);
        });
        output.textures_delta.clear();
        time += 1.0 / 60.0;
    }
    let base_count = output
        .shapes
        .iter()
        .map(|clipped| count_fill(&clipped.shape, base))
        .sum::<usize>();
    assert_eq!(base_count, total_cells - 2);
}

#[test]
fn selected_hex_sweep_only_changes_selected_byte_cells() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes((0..64).collect()));
    app.hex_editor.window_open = true;
    app.hex_editor.selection = Some(HexSelection::new(0, 3));
    app.hex_editor.memory.coloring.low = [17, 29, 43, 255];
    app.hex_editor.memory.coloring.middle = [17, 29, 43, 255];
    app.hex_editor.memory.coloring.high = [17, 29, 43, 255];
    app.preferences.sweep_enabled = true;

    let base = egui::Color32::from_rgb(17, 29, 43);
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let mut output = egui::FullOutput::default();
    let mut time = 0.5;
    for _ in 0..13 {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        output = context.run_ui(input, |ctx| app.show_hex_editor(ctx, screen));
        output.textures_delta.clear();
        time += 1.0 / 60.0;
    }

    let base_cells = rendered_fill_count(&output, base);
    assert!(
        base_cells >= 60,
        "unselected hex cells should keep their color"
    );
    assert!(
        base_cells < 64,
        "selected hex cells should receive the sweep tint"
    );
}

#[test]
fn selected_map_finder_cell_sweep_changes_only_its_preview_cell() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![10, 20, 30, 40, 50, 60]));
    app.map_finder.memory.window_open = true;
    app.map_finder.candidates = vec![MapCandidate {
        offset: 0,
        byte_length: 6,
        rows: 2,
        columns: 3,
        display_format: HexDisplayFormat::Unsigned8,
        endianness: HexEndianness::Little,
        score: 90,
        value_range: (0.0, 100.0),
        value_bands: 6,
        axis_suggestions: Vec::new(),
        selected_x_axis: None,
        selected_y_axis: None,
    }];
    app.map_finder.selected_candidate = Some(0);
    app.map_finder.selected_cell = Some((0, 0));
    app.map_finder.memory.coloring.low = [17, 29, 43, 255];
    app.map_finder.memory.coloring.middle = [17, 29, 43, 255];
    app.map_finder.memory.coloring.high = [17, 29, 43, 255];
    app.preferences.sweep_enabled = true;

    let base = egui::Color32::from_rgb(17, 29, 43);
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let mut output = egui::FullOutput::default();
    let mut time = 0.5;
    for _ in 0..13 {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        output = context.run_ui(input, |ctx| app.show_map_finder(ctx, screen));
        output.textures_delta.clear();
        time += 1.0 / 60.0;
    }

    assert_eq!(rendered_fill_count(&output, base), 5);
}

#[test]
fn selected_surface_points_receive_the_shared_sweep_tint() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_surface(&semantic_id));
    app.workspace.select_cell_range(0, 1, 0, 2).unwrap();
    app.preferences.sweep_enabled = true;
    let key = app.open_surfaces[0].key.clone();

    let context = egui::Context::default();
    let selection_fill = context
        .style_of(egui::Theme::Dark)
        .visuals
        .selection
        .bg_fill;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let mut output = egui::FullOutput::default();
    let mut time = 0.5;
    for _ in 0..13 {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        output = context.run_ui(input, |ctx| app.show_surface_window(ctx, &key));
        output.textures_delta.clear();
        time += 1.0 / 60.0;
    }
    let progress = selected_cell_sweep_progress(time - 1.0 / 60.0);
    let (top, left, _, right) = app.workspace.selected_cell_range().unwrap().bounds();
    assert_eq!(top, 0);
    for column in left..=right {
        let tint = selected_cell_sweep_fill(
            selection_fill,
            selected_cell_sweep_intensity(column, left, right, progress),
        );
        assert!(rendered_circle_fill_count(&output, tint) > 0);
    }
}

#[test]
fn compare_graph_selected_points_receive_the_shared_sweep_tint() {
    let mut app = app_with_matching_compare_documents();
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    app.workspace.select_parameter(&semantic_id);
    app.workspace.select_cell_range(0, 1, 0, 2).unwrap();
    app.compare.selected_semantic_id = Some(semantic_id.clone());
    app.compare.selected_cells = Some(CellSelection::new((0, 1), (0, 2)));
    let key = app.compare_cache_key(&semantic_id).unwrap();
    let cells = (0..6)
        .map(|index| CompareCell::from_values(Some(index as f64), Some(index as f64 + 1.0)))
        .collect();
    app.compare.map_cache.insert(
        key,
        Arc::new(CompareMapData::new(
            semantic_id,
            2,
            3,
            vec![0.0, 1.0, 2.0],
            vec![0.0, 1.0],
            cells,
        )),
    );
    app.compare.graph_open = true;
    app.preferences.sweep_enabled = true;

    let context = egui::Context::default();
    let selection_fill = context
        .style_of(egui::Theme::Dark)
        .visuals
        .selection
        .bg_fill;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let mut output = egui::FullOutput::default();
    let mut time = 0.5;
    for _ in 0..13 {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        output = context.run_ui(input, |ctx| app.show_compare_surface(ctx));
        output.textures_delta.clear();
        time += 1.0 / 60.0;
    }
    let progress = selected_cell_sweep_progress(time - 1.0 / 60.0);
    for column in 1..=2 {
        let tint = selected_cell_sweep_fill(
            selection_fill,
            selected_cell_sweep_intensity(column, 1, 2, progress),
        );
        assert!(rendered_circle_fill_count(&output, tint) > 0);
    }
}

#[test]
fn selected_cell_sweep_moves_right_then_left_on_two_second_cycle() {
    assert_eq!(selected_cell_sweep_progress(0.0), 0.0);
    assert_eq!(selected_cell_sweep_progress(0.5), 0.5);
    assert_eq!(selected_cell_sweep_progress(1.0), 1.0);
    assert_eq!(selected_cell_sweep_progress(1.5), 0.5);
    assert_eq!(selected_cell_sweep_progress(2.0), 0.0);
    assert_eq!(selected_cell_sweep_progress(2.25), 0.25);

    let left_first = selected_cell_sweep_intensity(0, 0, 4, 0.0);
    let right_first = selected_cell_sweep_intensity(4, 0, 4, 0.0);
    let left_return = selected_cell_sweep_intensity(0, 0, 4, 1.0);
    let right_return = selected_cell_sweep_intensity(4, 0, 4, 1.0);
    assert!(left_first > right_first);
    assert!(right_return > left_return);
}

#[test]
fn selected_cell_sweep_tints_selected_fill_without_changing_its_base() {
    let base = egui::Color32::from_rgb(24, 36, 48);
    assert_eq!(selected_cell_sweep_fill(base, 0.0), base);
    let tinted = selected_cell_sweep_fill(base, 0.7);
    let expected = blend_table_color(base.to_array(), [120, 215, 255, 255], 0.35);
    assert_eq!(
        tinted, expected,
        "sweep tint should use half its prior strength"
    );
    assert_ne!(tinted, base);
    assert_eq!(tinted.a(), base.a());
}

#[test]
fn table_action_controls_wrap_below_the_edit_row_in_a_narrow_window() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    app.open_tables[0].memory.x = 80;
    app.open_tables[0].memory.y = 80;
    app.open_tables[0].memory.width = 600;
    app.open_tables[0].memory.height = 440;
    app.open_tables[0].geometry_request = true;

    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let mut output = context.run_ui(egui::RawInput::default(), |ctx| {
        app.show_table_window(ctx, canvas, &key);
    });
    output.textures_delta.clear();
    for _ in 0..2 {
        output = context.run_ui(egui::RawInput::default(), |ctx| {
            app.show_table_window(ctx, canvas, &key);
        });
        output.textures_delta.clear();
    }

    let edit_row = rendered_text_rect(&output, "Engineering value:")
        .expect("active table should show its engineering editor");
    let action_row =
        rendered_text_rect(&output, "Ca").expect("active table should show its Copy All action");
    let history_action =
        rendered_text_rect(&output, "H").expect("active table should show its history action");
    let window_id = egui::Id::new(("table-window", "no-project", key.as_str()));
    let window_rect = context
        .memory(|memory| memory.area_rect(window_id))
        .expect("table window should have a geometry");

    assert!(action_row.min.y > edit_row.min.y);
    assert!(history_action.max.x < window_rect.right());
}

#[test]
fn old_table_memory_gets_color_defaults_and_sanitizes_invalid_range() {
    let mut memory: TableWindowMemory = serde_json::from_str(r#"{"width":800}"#).unwrap();
    assert_eq!(memory.coloring.mode, TableColorMode::AutoRange);
    memory.coloring.fixed_min = Some(f64::NAN);
    memory.coloring.fixed_max = Some(1.0);
    memory.sanitize();
    assert!(memory.coloring.fixed_min.is_none());
}

#[test]
fn table_coloring_falls_back_to_raw_when_engineering_value_is_nonfinite() {
    let view = CellView {
        row: 0,
        column: 0,
        range: ByteRange { start: 0, end: 1 },
        raw: RawValue::Unsigned(42),
        engineering: Some(f64::NAN),
        error: Some("conversion failed".into()),
    };
    assert_eq!(
        table_numeric_value(&view, TableDisplay::Engineering),
        Some(42.0)
    );
}

#[test]
fn table_axis_index_prefers_matching_role_ids_and_safe_positions() {
    let explicit = XdfDocument::parse(AXIS_TABLE_XDF).unwrap();
    let parameter = &explicit.parameters[0];
    assert_eq!(table_axis_index(parameter, TableAxisRole::X, 2), Some(0));
    assert_eq!(table_axis_index(parameter, TableAxisRole::Y, 2), Some(1));

    let positional = XdfDocument::parse(
        br#"<XDFFORMAT><XDFTABLE uniqueid="positional-axes">
            <title>Positional axes</title>
            <XDFAXIS id="first"><indexcount>3</indexcount></XDFAXIS>
            <XDFAXIS id="second"><indexcount>2</indexcount></XDFAXIS>
            <XDFAXIS id="z"><EMBEDDEDDATA mmedaddress="0x00"
                mmedelementsizebits="8" mmedrowcount="2" mmedcolcount="3"
                mmedmajorstridebits="0" mmedminorstridebits="0"
                mmedtypeflags="0x06" /></XDFAXIS>
        </XDFTABLE></XDFFORMAT>"#,
    )
    .unwrap();
    let parameter = &positional.parameters[0];
    assert_eq!(table_axis_index(parameter, TableAxisRole::X, 3), Some(0));
    assert_eq!(table_axis_index(parameter, TableAxisRole::Y, 2), Some(1));

    let ambiguous = XdfDocument::parse(
        br#"<XDFFORMAT><XDFTABLE uniqueid="ambiguous-axes">
            <title>Ambiguous axes</title>
            <XDFAXIS id="column"><indexcount>3</indexcount></XDFAXIS>
            <XDFAXIS id="x"><indexcount>3</indexcount></XDFAXIS>
            <XDFAXIS id="row"><indexcount>2</indexcount></XDFAXIS>
            <XDFAXIS id="y"><indexcount>2</indexcount></XDFAXIS>
            <XDFAXIS id="z"><EMBEDDEDDATA mmedaddress="0x00"
                mmedelementsizebits="8" mmedrowcount="2" mmedcolcount="3"
                mmedmajorstridebits="0" mmedminorstridebits="0"
                mmedtypeflags="0x06" /></XDFAXIS>
        </XDFTABLE></XDFFORMAT>"#,
    )
    .unwrap();
    let parameter = &ambiguous.parameters[0];
    assert_eq!(table_axis_index(parameter, TableAxisRole::X, 3), Some(1));
    assert_eq!(table_axis_index(parameter, TableAxisRole::Y, 2), Some(3));

    let z_only = XdfDocument::parse(COLUMN_MAJOR_XDF).unwrap();
    assert_eq!(
        table_axis_index(&z_only.parameters[0], TableAxisRole::X, 3),
        None
    );
    assert_eq!(
        table_axis_index(&z_only.parameters[0], TableAxisRole::Y, 2),
        None
    );
    assert_eq!(
        table_axis_index(&z_only.parameters[0], TableAxisRole::X, 1),
        None
    );
}

#[test]
fn table_geometry_guard_ignores_delayed_rect_until_two_stable_frames() {
    let saved = TableGeometry {
        x: 24,
        y: 32,
        width: 760,
        height: 520,
    };
    let delayed = TableGeometry {
        width: 1_120,
        height: 780,
        ..saved
    };
    let guard = TableGeometryGuard::new(saved);

    let guard = advance_table_geometry_guard(guard, Some(delayed), false).unwrap();
    assert_eq!(guard.stable_observations, 0);
    let guard = advance_table_geometry_guard(guard, Some(saved), false).unwrap();
    assert_eq!(guard.stable_observations, 1);
    let guard = advance_table_geometry_guard(guard, Some(delayed), false).unwrap();
    assert_eq!(guard.stable_observations, 0);
    let guard = advance_table_geometry_guard(guard, Some(saved), false).unwrap();
    assert_eq!(guard.stable_observations, 1);
    assert!(advance_table_geometry_guard(guard, Some(saved), false).is_none());
}

#[test]
fn table_geometry_guard_targets_canvas_clamp_without_replacing_saved_geometry() {
    let saved = TableGeometry {
        x: 420,
        y: 260,
        width: 760,
        height: 520,
    };
    let small_canvas = egui::Rect::from_min_size(egui::pos2(100.0, 80.0), egui::vec2(600.0, 400.0));
    assert_eq!(
        table_effective_geometry(saved, small_canvas),
        TableGeometry {
            x: 100,
            y: 80,
            width: 600,
            height: 400,
        }
    );

    let fitting_canvas =
        egui::Rect::from_min_size(egui::pos2(100.0, 80.0), egui::vec2(1_200.0, 900.0));
    assert_eq!(table_effective_geometry(saved, fitting_canvas), saved);
}

#[test]
fn table_window_production_path_keeps_identity_and_geometry_while_guarded() {
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0; 144])),
        Some(XdfDocument::parse(LARGE_TABLE_XDF).unwrap()),
        None,
        None,
    );
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    app.open_tables[0].memory = TableWindowMemory {
        x: 40,
        y: 50,
        width: 760,
        height: 520,
        position_saved: true,
        zoom_percent: 100,
        scroll_x: 0,
        scroll_y: 0,
        decimal_places: 2,
        coloring: TableColorSettings::default(),
        fit_to_content: Some(false),
    };

    let context = egui::Context::default();
    let canvas_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let window_id = egui::Id::new(("table-window", "no-project", key.as_str()));
    let run_frame = |app: &mut TunerApp, context: &egui::Context| {
        context
            .run_ui(egui::RawInput::default(), |ctx| {
                app.show_table_window(ctx, canvas_rect, &key);
            })
            .drop_without_applying_deltas();
    };

    run_frame(&mut app, &context);
    let saved_geometry = TableGeometry::from_memory(&app.open_tables[0].memory);
    assert!(context
        .memory(|memory| memory.area_rect(window_id))
        .is_some());

    app.open_tables[0].memory.zoom_percent = 200;
    app.table_geometry_guards
        .insert(key.clone(), TableGeometryGuard::new(saved_geometry));
    let pre_show_geometry = context
        .memory(|memory| memory.area_rect(window_id))
        .map(TableGeometry::from_rect);
    assert_eq!(pre_show_geometry, Some(saved_geometry));
    run_frame(&mut app, &context);
    assert_eq!(
        TableGeometry::from_memory(&app.open_tables[0].memory),
        saved_geometry
    );
    assert_eq!(
        context.memory(|memory| memory.area_rect(window_id).map(TableGeometry::from_rect)),
        Some(saved_geometry)
    );
    assert!(app.table_geometry_guards.contains_key(&key));

    let pre_show_geometry = context
        .memory(|memory| memory.area_rect(window_id))
        .map(TableGeometry::from_rect);
    assert_eq!(pre_show_geometry, Some(saved_geometry));
    run_frame(&mut app, &context);
    assert_eq!(
        TableGeometry::from_memory(&app.open_tables[0].memory),
        saved_geometry
    );
    assert_eq!(
        context.memory(|memory| memory.area_rect(window_id).map(TableGeometry::from_rect)),
        Some(saved_geometry)
    );
    assert!(!app.table_geometry_guards.contains_key(&key));

    run_frame(&mut app, &context);
    assert_eq!(
        TableGeometry::from_memory(&app.open_tables[0].memory),
        saved_geometry
    );
    assert!(!app.table_geometry_guards.contains_key(&key));
    assert!(context
        .memory(|memory| memory.area_rect(window_id))
        .is_some());
}

#[test]
fn reopening_table_applies_saved_geometry_to_existing_egui_window_state() {
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0; 144])),
        Some(XdfDocument::parse(LARGE_TABLE_XDF).unwrap()),
        None,
        None,
    );
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    app.open_tables[0].memory = TableWindowMemory {
        x: 80,
        y: 70,
        width: 1_120,
        height: 780,
        ..TableWindowMemory::default()
    };

    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_400.0, 900.0));
    let window_id = egui::Id::new(("table-window", "no-project", key.as_str()));
    context
        .run_ui(egui::RawInput::default(), |ctx| {
            app.show_table_window(ctx, canvas, &key);
        })
        .drop_without_applying_deltas();
    let previous = context
        .memory(|memory| memory.area_rect(window_id))
        .map(TableGeometry::from_rect)
        .expect("table should have an existing egui window state");

    let saved = TableWindowMemory {
        x: 160,
        y: 120,
        width: 480,
        height: 320,
        fit_to_content: Some(false),
        ..app.open_tables[0].memory.clone()
    };
    app.open_tables[0].memory = saved.clone();
    assert!(app.close_table(&key));
    assert!(app.open_table(&semantic_id));
    assert_eq!(app.open_tables[0].memory.width, saved.width);
    assert_eq!(app.open_tables[0].memory.height, saved.height);
    assert!(app.open_tables[0].geometry_request);

    context
        .run_ui(egui::RawInput::default(), |ctx| {
            app.show_table_window(ctx, canvas, &key);
        })
        .drop_without_applying_deltas();
    let reopened = context
        .memory(|memory| memory.area_rect(window_id))
        .map(TableGeometry::from_rect)
        .expect("reopened table should have geometry");
    assert_ne!(reopened, previous);
    assert_eq!(
        reopened,
        TableGeometry::from_memory(&app.open_tables[0].memory)
    );
}

#[test]
fn reopening_zoomed_out_table_fits_saved_content_scale() {
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0; 144])),
        Some(XdfDocument::parse(LARGE_TABLE_XDF).unwrap()),
        None,
        None,
    );
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    let title = app.open_tables[0].title.clone();
    let category = app.open_tables[0].category.clone();
    let summary = summary_from_parameter(
        app.workspace
            .xdf
            .as_ref()
            .unwrap()
            .parameter(&semantic_id)
            .unwrap(),
    );
    let mut saved = app.open_tables[0].memory.clone();
    saved.zoom_percent = 60;
    let expected = table_default_window_size(
        summary.rows,
        summary.columns,
        saved.zoom_percent,
        &format!("{category} · {}", summary_label(&summary)),
    );
    assert!(saved.width as f32 > expected.x);
    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_400.0, 900.0));
    let window_id = egui::Id::new(("table-window", "no-project", key.as_str()));
    context
        .run_ui(egui::RawInput::default(), |ctx| {
            app.show_table_window(ctx, canvas, &key);
        })
        .drop_without_applying_deltas();
    let previous = context
        .memory(|memory| memory.area_rect(window_id))
        .map(TableGeometry::from_rect)
        .expect("table should have its initial geometry");
    app.open_tables[0].memory = saved.clone();
    assert!(app.close_table(&key));
    assert!(app.open_table(&semantic_id));

    assert_eq!(app.open_tables[0].title, title);
    assert_eq!(app.open_tables[0].memory.zoom_percent, 60);
    assert_eq!(
        (
            app.open_tables[0].memory.width,
            app.open_tables[0].memory.height
        ),
        (expected.x as u32, expected.y as u32)
    );
    context
        .run_ui(egui::RawInput::default(), |ctx| {
            app.show_table_window(ctx, canvas, &key);
        })
        .drop_without_applying_deltas();
    let reopened = context
        .memory(|memory| memory.area_rect(window_id))
        .map(TableGeometry::from_rect)
        .expect("reopened table should have geometry");
    assert_ne!(reopened, previous);
    assert_eq!(reopened.width, expected.x as u32);
    assert_eq!(reopened.height, expected.y as u32);
}

#[test]
fn auto_fit_table_window_resizes_live_with_zoom_but_manual_size_stays_fixed() {
    fn draw_frame(
        app: &mut TunerApp,
        context: &egui::Context,
        canvas: egui::Rect,
        key: &str,
        time: &mut f64,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        *time += 0.1;
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        input.time = Some(*time);
        input.events = events;
        context.run_ui(input, |ctx| app.show_table_window(ctx, canvas, key))
    }

    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0; 144])),
        Some(XdfDocument::parse(LARGE_TABLE_XDF).unwrap()),
        None,
        None,
    );
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    assert_eq!(app.open_tables[0].memory.fit_to_content, Some(true));

    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_400.0, 900.0));
    let mut time = 0.0;
    let output = draw_frame(&mut app, &context, canvas, &key, &mut time, Vec::new());
    output.drop_without_applying_deltas();
    let mut output = draw_frame(&mut app, &context, canvas, &key, &mut time, Vec::new());
    let zoom_out = rendered_text_rect(&output, "−").expect("zoom-out control should render");
    output.drop_without_applying_deltas();

    for pressed in [true, false] {
        output = draw_frame(
            &mut app,
            &context,
            canvas,
            &key,
            &mut time,
            vec![
                egui::Event::PointerMoved(zoom_out.center()),
                egui::Event::PointerButton {
                    pos: zoom_out.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        output.drop_without_applying_deltas();
    }

    let xdf = app.workspace.xdf.as_ref().unwrap();
    let parameter = xdf.parameter(&semantic_id).unwrap();
    let summary = summary_from_parameter(parameter);
    let title = format!(
        "{} · {}",
        app.open_tables[0].category,
        summary_label(&summary)
    );
    let expected = table_default_window_size(summary.rows, summary.columns, 90, &title);
    assert_eq!(app.open_tables[0].memory.zoom_percent, 90);
    assert_eq!(
        (
            app.open_tables[0].memory.width,
            app.open_tables[0].memory.height
        ),
        (expected.x as u32, expected.y as u32),
        "auto-fit border should track the zoom used to size the cells"
    );

    output = draw_frame(&mut app, &context, canvas, &key, &mut time, Vec::new());
    output.drop_without_applying_deltas();
    let window_id = egui::Id::new(("table-window", "no-project", key.as_str()));
    let live = context
        .memory(|memory| memory.area_rect(window_id))
        .expect("auto-fit window should retain its geometry");
    assert_eq!(live.width().round() as u32, expected.x as u32);
    assert_eq!(live.height().round() as u32, expected.y as u32);

    app.open_tables[0].memory.fit_to_content = Some(false);
    let output = draw_frame(&mut app, &context, canvas, &key, &mut time, Vec::new());
    let zoom_out =
        rendered_text_rect(&output, "−").expect("zoom-out control should remain available");
    output.drop_without_applying_deltas();
    let manual_size = (
        app.open_tables[0].memory.width,
        app.open_tables[0].memory.height,
    );
    for pressed in [true, false] {
        let output = draw_frame(
            &mut app,
            &context,
            canvas,
            &key,
            &mut time,
            vec![
                egui::Event::PointerMoved(zoom_out.center()),
                egui::Event::PointerButton {
                    pos: zoom_out.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        output.drop_without_applying_deltas();
    }
    assert_eq!(app.open_tables[0].memory.zoom_percent, 80);
    assert_eq!(
        (
            app.open_tables[0].memory.width,
            app.open_tables[0].memory.height
        ),
        manual_size,
        "a manually sized border should not be changed by cell zoom"
    );
}

#[test]
fn reopening_explicitly_resized_table_preserves_its_saved_border() {
    let xdf = XdfDocument::parse(LARGE_TABLE_XDF).unwrap();
    let summary = summary_from_parameter(&xdf.parameters[0]);
    let memory = TableWindowMemory {
        width: 1_080,
        height: 720,
        zoom_percent: 60,
        fit_to_content: Some(false),
        ..TableWindowMemory::default()
    };

    let prepared = prepare_table_memory_for_open(memory.clone(), &summary, "Uncategorized");

    assert_eq!(prepared.width, memory.width);
    assert_eq!(prepared.height, memory.height);
    assert_eq!(prepared.fit_to_content, Some(false));
}

#[test]
fn table_reuses_recent_project_geometry_after_xdf_revision() {
    fn revised_xdf(title: &str) -> XdfDocument {
        let source = std::str::from_utf8(COLUMN_MAJOR_XDF).unwrap();
        let bytes = source.replace("<title>Map</title>", &format!("<title>{title}</title>"));
        XdfDocument::parse(bytes.as_bytes()).unwrap()
    }

    let current_xdf = XdfDocument::parse(COLUMN_MAJOR_XDF).unwrap();
    let recent_xdf = revised_xdf("Recent Map");
    let older_xdf = revised_xdf("Older Map");
    let semantic_id = current_xdf.parameters[0].semantic_id.clone();
    assert_eq!(recent_xdf.parameters[0].semantic_id, semantic_id);
    assert_eq!(older_xdf.parameters[0].semantic_id, semantic_id);
    assert_ne!(
        current_xdf.normalized_fingerprint,
        recent_xdf.normalized_fingerprint
    );

    let recent_key = table_key(&recent_xdf.normalized_fingerprint, &semantic_id);
    let older_key = table_key(&older_xdf.normalized_fingerprint, &semantic_id);
    let recent_memory = TableWindowMemory {
        x: 160,
        y: 120,
        width: 920,
        height: 610,
        zoom_percent: 90,
        fit_to_content: Some(false),
        ..TableWindowMemory::default()
    };
    let older_memory = TableWindowMemory {
        width: 720,
        height: 520,
        zoom_percent: 100,
        fit_to_content: Some(false),
        ..TableWindowMemory::default()
    };

    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0; 6])),
        Some(current_xdf),
        None,
        None,
    );
    let identity = project_identity_for_path(Path::new("C:/tuning/revised-definition.bin"));
    app.project_identity = Some(identity.clone());
    app.project_preferences = ProjectPreferences::for_identity(&identity);
    app.project_preferences
        .table_windows
        .insert(recent_key.clone(), recent_memory.clone());
    app.project_preferences
        .table_windows
        .insert(older_key.clone(), older_memory);
    app.project_preferences.recent_tables = vec![recent_key, older_key];
    app.apply_project_preferences_to_legacy();

    assert!(app.open_table(&semantic_id));
    let opened = &app.open_tables[0].memory;
    assert_eq!(
        (opened.x, opened.y, opened.width, opened.height),
        (160, 120, 920, 610)
    );
    assert_eq!(opened.zoom_percent, 90);
    assert_eq!(opened.fit_to_content, Some(false));

    let current_key = app.open_tables[0].key.clone();
    assert!(app.close_table(&current_key));
    app.preferences.table_windows.insert(
        current_key.clone(),
        TableWindowMemory {
            width: 760,
            height: 480,
            zoom_percent: 110,
            fit_to_content: Some(false),
            ..TableWindowMemory::default()
        },
    );
    assert!(app.open_table(&semantic_id));
    let exact = &app.open_tables[0].memory;
    assert_eq!((exact.width, exact.height), (760, 480));
    assert_eq!(exact.zoom_percent, 110);
}

#[test]
fn table_geometry_fallback_does_not_cross_bin_project() {
    let current_xdf = XdfDocument::parse(COLUMN_MAJOR_XDF).unwrap();
    let old_bytes = std::str::from_utf8(COLUMN_MAJOR_XDF)
        .unwrap()
        .replace("<title>Map</title>", "<title>Other project's Map</title>");
    let old_xdf = XdfDocument::parse(old_bytes.as_bytes()).unwrap();
    let semantic_id = current_xdf.parameters[0].semantic_id.clone();
    let old_key = table_key(&old_xdf.normalized_fingerprint, &semantic_id);

    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0; 6])),
        Some(current_xdf.clone()),
        None,
        None,
    );
    app.preferences.table_windows.insert(
        old_key.clone(),
        TableWindowMemory {
            width: 920,
            height: 610,
            fit_to_content: Some(false),
            ..TableWindowMemory::default()
        },
    );
    app.preferences.recent_tables = vec![old_key];

    let other_identity = project_identity_for_path(Path::new("D:/tuning/another-project.bin"));
    app.project_identity = Some(other_identity.clone());
    app.project_preferences = ProjectPreferences::for_identity(&other_identity);
    app.apply_project_preferences_to_legacy();

    assert!(app.open_table(&semantic_id));
    let summary = summary_from_parameter(&current_xdf.parameters[0]);
    let default = default_table_window_memory(&summary, "Uncategorized", 100);
    assert_eq!(
        (
            app.open_tables[0].memory.width,
            app.open_tables[0].memory.height
        ),
        (default.width, default.height)
    );
}

#[test]
fn manually_resized_table_border_is_saved_and_restored() {
    fn draw_frame(
        app: &mut TunerApp,
        context: &egui::Context,
        canvas: egui::Rect,
        key: &str,
        time: &mut f64,
        events: Vec<egui::Event>,
    ) -> Option<egui::Rect> {
        *time += 0.1;
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        input.time = Some(*time);
        input.events = events;
        let window_id = egui::Id::new(("table-window", "no-project", key));
        let dock_id = WindowId::Table(key.to_owned());
        let layer_id = egui::LayerId::new(app.dock_window_order(&dock_id), window_id);
        let resize_corner_id = egui::Id::new(layer_id)
            .with("edge_drag")
            .with("right_bottom");
        let mut resize_corner = None;
        context
            .run_ui(input, |ctx| {
                app.show_table_window(ctx, canvas, key);
                resize_corner = ctx
                    .read_response(resize_corner_id)
                    .map(|response| response.rect);
            })
            .drop_without_applying_deltas();
        resize_corner
    }

    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0; 144])),
        Some(XdfDocument::parse(LARGE_TABLE_XDF).unwrap()),
        None,
        None,
    );
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    app.open_tables[0].memory.x = 80;
    app.open_tables[0].memory.y = 80;
    app.open_tables[0].memory.width = 900;
    app.open_tables[0].memory.height = 600;
    app.open_tables[0].memory.fit_to_content = Some(true);
    app.open_tables[0].geometry_request = true;

    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_400.0, 900.0));
    let mut time = 0.0;
    let mut resize_corner = None;
    for _ in 0..3 {
        resize_corner = draw_frame(&mut app, &context, canvas, &key, &mut time, Vec::new());
    }

    let window_id = egui::Id::new(("table-window", "no-project", key.as_str()));
    let original_rect = context
        .memory(|memory| memory.area_rect(window_id))
        .expect("table should have its initial border");
    let resize_corner = resize_corner.expect("table should expose its resize corner");
    let start = resize_corner.center();
    let end = start + egui::vec2(100.0, 80.0);
    draw_frame(
        &mut app,
        &context,
        canvas,
        &key,
        &mut time,
        vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    draw_frame(
        &mut app,
        &context,
        canvas,
        &key,
        &mut time,
        vec![egui::Event::PointerMoved(end)],
    );
    draw_frame(
        &mut app,
        &context,
        canvas,
        &key,
        &mut time,
        vec![
            egui::Event::PointerMoved(end),
            egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    let resized = context
        .memory(|memory| memory.area_rect(window_id))
        .expect("resized table should keep its border");
    assert!(resized.width() > original_rect.width());
    assert!(resized.height() > original_rect.height());
    assert_eq!(app.open_tables[0].memory.fit_to_content, Some(false));
    let saved_size = (
        app.open_tables[0].memory.width,
        app.open_tables[0].memory.height,
    );
    assert_eq!(
        saved_size,
        (
            resized.width().round() as u32,
            resized.height().round() as u32
        )
    );

    assert!(app.close_table(&key));
    assert!(app.open_table(&semantic_id));
    assert_eq!(
        (
            app.open_tables[0].memory.width,
            app.open_tables[0].memory.height
        ),
        saved_size
    );
    draw_frame(&mut app, &context, canvas, &key, &mut time, Vec::new());
    let reopened = context
        .memory(|memory| memory.area_rect(window_id))
        .expect("reopened table should have a border");
    assert_eq!(
        (
            reopened.width().round() as u32,
            reopened.height().round() as u32
        ),
        saved_size
    );
}

#[test]
fn manual_resize_is_saved_when_previous_rect_was_canvas_clamped() {
    fn draw_frame(
        app: &mut TunerApp,
        context: &egui::Context,
        screen: egui::Rect,
        canvas: egui::Rect,
        key: &str,
        time: &mut f64,
        events: Vec<egui::Event>,
    ) -> Option<egui::Rect> {
        *time += 0.1;
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(*time);
        input.events = events;
        let window_id = egui::Id::new(("table-window", "no-project", key));
        let dock_id = WindowId::Table(key.to_owned());
        let layer_id = egui::LayerId::new(app.dock_window_order(&dock_id), window_id);
        let resize_corner_id = egui::Id::new(layer_id)
            .with("edge_drag")
            .with("right_bottom");
        let mut resize_corner = None;
        context
            .run_ui(input, |ctx| {
                app.show_table_window(ctx, canvas, key);
                resize_corner = ctx
                    .read_response(resize_corner_id)
                    .map(|response| response.rect);
            })
            .drop_without_applying_deltas();
        resize_corner
    }

    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0; 2])),
        Some(XdfDocument::parse(TWO_TABLE_XDF).unwrap()),
        None,
        None,
    );
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    app.open_tables[0].memory = TableWindowMemory {
        x: 40,
        y: 40,
        width: 1_000,
        height: 700,
        zoom_percent: 100,
        fit_to_content: Some(false),
        position_saved: true,
        ..TableWindowMemory::default()
    };
    app.open_tables[0].geometry_request = true;

    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 800.0));
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let window_id = egui::Id::new(("table-window", "no-project", key.as_str()));
    let mut time = 0.0;
    for _ in 0..3 {
        draw_frame(
            &mut app,
            &context,
            screen,
            canvas,
            &key,
            &mut time,
            Vec::new(),
        );
    }
    let clamped = context
        .memory(|memory| memory.area_rect(window_id))
        .expect("table should clamp to the smaller workspace canvas");
    assert!(clamped.width() < app.open_tables[0].memory.width as f32);
    assert!(clamped.height() < app.open_tables[0].memory.height as f32);

    let resize_corner = draw_frame(
        &mut app,
        &context,
        screen,
        canvas,
        &key,
        &mut time,
        Vec::new(),
    )
    .expect("clamped table should remain user-resizable");
    let start = resize_corner.center();
    let end = start - egui::vec2(40.0, 30.0);
    draw_frame(
        &mut app,
        &context,
        screen,
        canvas,
        &key,
        &mut time,
        vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    draw_frame(
        &mut app,
        &context,
        screen,
        canvas,
        &key,
        &mut time,
        vec![egui::Event::PointerMoved(end)],
    );
    draw_frame(
        &mut app,
        &context,
        screen,
        canvas,
        &key,
        &mut time,
        vec![
            egui::Event::PointerMoved(end),
            egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );

    let resized = context
        .memory(|memory| memory.area_rect(window_id))
        .expect("resized table should keep a live rectangle");
    let saved = &app.open_tables[0].memory;
    assert_eq!(saved.width, resized.width().round() as u32);
    assert_eq!(saved.height, resized.height().round() as u32);
    assert_eq!(saved.fit_to_content, Some(false));
}

#[test]
fn table_window_production_path_clamps_geometry_to_small_canvas_without_capture() {
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        None,
        Some(XdfDocument::parse(LARGE_TABLE_XDF).unwrap()),
        None,
        None,
    );
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    app.open_tables[0].memory = TableWindowMemory {
        x: 420,
        y: 260,
        width: 760,
        height: 520,
        position_saved: true,
        zoom_percent: 200,
        scroll_x: 0,
        scroll_y: 0,
        decimal_places: 2,
        coloring: TableColorSettings::default(),
        fit_to_content: Some(false),
    };
    app.open_tables[0].geometry_request = false;
    let saved_geometry = TableGeometry::from_memory(&app.open_tables[0].memory);
    app.table_geometry_guards
        .insert(key.clone(), TableGeometryGuard::new(saved_geometry));

    let context = egui::Context::default();
    let canvas_rect = egui::Rect::from_min_size(egui::pos2(100.0, 80.0), egui::vec2(600.0, 400.0));
    let window_id = egui::Id::new(("table-window", "no-project", key.as_str()));
    let effective_geometry = table_effective_geometry(saved_geometry, canvas_rect);
    let run_frame = |app: &mut TunerApp, context: &egui::Context| {
        context
            .run_ui(egui::RawInput::default(), |ctx| {
                app.show_table_window(ctx, canvas_rect, &key);
            })
            .drop_without_applying_deltas();
    };

    run_frame(&mut app, &context);
    assert_eq!(
        context.memory(|memory| memory.area_rect(window_id).map(TableGeometry::from_rect)),
        Some(effective_geometry)
    );
    assert_eq!(
        TableGeometry::from_memory(&app.open_tables[0].memory),
        saved_geometry
    );
    assert!(app.table_geometry_guards.contains_key(&key));

    let pre_show_geometry = context
        .memory(|memory| memory.area_rect(window_id))
        .map(TableGeometry::from_rect);
    assert_eq!(pre_show_geometry, Some(effective_geometry));
    run_frame(&mut app, &context);
    assert_eq!(
        context.memory(|memory| memory.area_rect(window_id).map(TableGeometry::from_rect)),
        Some(effective_geometry)
    );
    assert_eq!(
        TableGeometry::from_memory(&app.open_tables[0].memory),
        saved_geometry
    );
    assert!(!app.table_geometry_guards.contains_key(&key));

    run_frame(&mut app, &context);
    assert_eq!(
        TableGeometry::from_memory(&app.open_tables[0].memory),
        saved_geometry
    );
}

#[test]
fn table_window_restores_saved_geometry_after_canvas_grows() {
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        None,
        Some(XdfDocument::parse(LARGE_TABLE_XDF).unwrap()),
        None,
        None,
    );
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    app.open_tables[0].memory = TableWindowMemory {
        x: 420,
        y: 260,
        width: 760,
        height: 520,
        position_saved: true,
        zoom_percent: 200,
        scroll_x: 0,
        scroll_y: 0,
        decimal_places: 2,
        coloring: TableColorSettings::default(),
        fit_to_content: Some(false),
    };
    let saved_geometry = TableGeometry::from_memory(&app.open_tables[0].memory);

    let context = egui::Context::default();
    let small_canvas = egui::Rect::from_min_size(egui::pos2(100.0, 80.0), egui::vec2(600.0, 400.0));
    let large_canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let window_id = egui::Id::new(("table-window", "no-project", key.as_str()));
    let small_geometry = table_effective_geometry(saved_geometry, small_canvas);
    let run_frame = |app: &mut TunerApp, context: &egui::Context, canvas_rect: egui::Rect| {
        context
            .run_ui(egui::RawInput::default(), |ctx| {
                app.show_table_window(ctx, canvas_rect, &key);
            })
            .drop_without_applying_deltas();
    };

    app.table_geometry_guards
        .insert(key.clone(), TableGeometryGuard::new(saved_geometry));
    run_frame(&mut app, &context, small_canvas);
    run_frame(&mut app, &context, small_canvas);
    assert_eq!(
        context.memory(|memory| memory.area_rect(window_id).map(TableGeometry::from_rect)),
        Some(small_geometry)
    );
    assert!(!app.table_geometry_guards.contains_key(&key));
    assert_eq!(
        TableGeometry::from_memory(&app.open_tables[0].memory),
        saved_geometry
    );

    let pre_show_geometry = context
        .memory(|memory| memory.area_rect(window_id))
        .map(TableGeometry::from_rect);
    assert_eq!(pre_show_geometry, Some(small_geometry));
    run_frame(&mut app, &context, large_canvas);
    assert_eq!(
        context.memory(|memory| memory.area_rect(window_id).map(TableGeometry::from_rect)),
        Some(saved_geometry)
    );
    assert_eq!(
        TableGeometry::from_memory(&app.open_tables[0].memory),
        saved_geometry
    );
    assert!(app.table_geometry_guards.contains_key(&key));

    let pre_show_geometry = context
        .memory(|memory| memory.area_rect(window_id))
        .map(TableGeometry::from_rect);
    assert_eq!(pre_show_geometry, Some(saved_geometry));
    run_frame(&mut app, &context, large_canvas);
    assert_eq!(
        context.memory(|memory| memory.area_rect(window_id).map(TableGeometry::from_rect)),
        Some(saved_geometry)
    );
    assert_eq!(
        TableGeometry::from_memory(&app.open_tables[0].memory),
        saved_geometry
    );
    assert!(!app.table_geometry_guards.contains_key(&key));

    run_frame(&mut app, &context, large_canvas);
    assert_eq!(
        TableGeometry::from_memory(&app.open_tables[0].memory),
        saved_geometry
    );
}

#[test]
fn closing_table_clears_its_geometry_guard() {
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        None,
        Some(XdfDocument::parse(LARGE_TABLE_XDF).unwrap()),
        None,
        None,
    );
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    app.table_geometry_guards.insert(
        key.clone(),
        TableGeometryGuard::new(TableGeometry::from_memory(&app.open_tables[0].memory)),
    );

    assert!(app.close_table(&key));
    assert!(!app.table_geometry_guards.contains_key(&key));
}

#[test]
fn document_and_project_changes_clear_geometry_guards() {
    let mut app = TunerApp::headless();
    let guard = TableGeometryGuard::new(TableGeometry {
        x: 0,
        y: 0,
        width: 760,
        height: 520,
    });
    app.table_geometry_guards
        .insert("document-stale".to_string(), guard);
    app.restore_workspace_after_document_change();
    assert!(app.table_geometry_guards.is_empty());

    app.table_geometry_guards
        .insert("project-stale".to_string(), guard);
    app.activate_project_for_path(Path::new("project-a.bin"));
    app.table_geometry_guards
        .insert("project-stale".to_string(), guard);
    app.activate_project_for_path(Path::new("project-b.bin"));
    assert!(app.table_geometry_guards.is_empty());
}

fn temporary_test_path(extension: &str) -> PathBuf {
    let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "tunernook-app-{}-{sequence}.{extension}",
        std::process::id()
    ))
}

#[test]
fn project_manifest_file_commands_are_registered_for_bin_projects() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![1, 2, 3]));

    assert_eq!(app.project_save_route(), ProjectSaveRoute::SaveAs);
    let associated_path = PathBuf::from("project.tnproj");
    app.project_preferences.project_file_path = Some(associated_path.clone());
    assert_eq!(
        app.project_save_route(),
        ProjectSaveRoute::UpdateAssociated(associated_path)
    );

    for (id, label) in [
        ("file.open-project", "Open Project…"),
        ("file.save-project", "Save Project"),
        ("file.save-project-as", "Save Project As…"),
    ] {
        assert!(app
            .registry
            .descriptors()
            .iter()
            .any(|command| { command.id == id && command.label == label }));
    }
    let entries = app.registry.entries(&app.workspace);
    for id in ["file.save-project", "file.save-project-as"] {
        assert!(entries
            .iter()
            .any(|entry| entry.descriptor.id == id && entry.enabled));
    }
}

#[test]
fn project_manifest_save_preserves_dirty_bin_and_associates_manifest() {
    let bin_path = temporary_test_path("bin");
    let xdf_path = temporary_test_path("xdf");
    let manifest_path = temporary_test_path("tnproj");
    let settings_path = temporary_test_path("settings.json");
    std::fs::write(&bin_path, [1_u8, 2, 3, 4]).unwrap();
    std::fs::write(&xdf_path, COLUMN_MAJOR_XDF).unwrap();
    let identity = project_identity_for_path(&bin_path);
    let mut app = TunerApp::headless();
    app.settings_path = settings_path.clone();
    app.persistence_enabled = true;
    let mut bin = BinDocument::from_bytes(vec![1, 2, 3, 4]);
    let mut edit = bin.transaction("dirty before project save");
    edit.write_u8(0, 9).unwrap();
    edit.commit().unwrap();
    let mut preferences = ProjectPreferences::for_identity(&identity);
    preferences.active_workspace_id = 7;
    preferences.active_workspace_name = "Daily tune".into();
    preferences.last_xdf_path = Some(xdf_path.clone());
    let mut named = ProjectPreferences::for_identity(&identity);
    named.hex_window_open = true;
    preferences
        .saved_workspace_snapshots
        .push(WorkspaceSnapshot {
            id: 4,
            name: "Hex review".into(),
            state: WorkspaceViewState::capture(&named),
        });
    app.project_identity = Some(identity.clone());
    app.project_preferences = preferences;
    app.workspace.set_documents(
        Some(bin),
        Some(XdfDocument::parse(COLUMN_MAJOR_XDF).unwrap()),
        Some(bin_path.clone()),
        Some(xdf_path.clone()),
    );
    let edited_bytes = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let undo_depth = app.workspace.bin.as_ref().unwrap().undo_depth();

    app.save_project_manifest(&manifest_path).unwrap();

    let saved: TunerProjectFile =
        TunerProjectFile::from_json(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    assert_eq!(saved.bin_path, bin_path);
    assert_eq!(saved.xdf_path, Some(xdf_path.clone()));
    assert_eq!(saved.preferences.last_xdf_path, saved.xdf_path);
    assert_eq!(saved.preferences.active_workspace_name, "Daily tune");
    assert_eq!(saved.preferences.saved_workspace_snapshots.len(), 1);
    assert_eq!(
        saved.preferences.project_file_path,
        Some(manifest_path.clone())
    );
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        edited_bytes.as_slice()
    );
    assert!(app.workspace.bin.as_ref().unwrap().is_dirty());
    assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), undo_depth);
    assert!(app
        .workspace
        .status
        .text
        .contains("BIN edits remain unsaved"));
    assert!(app.workspace.status.text.contains("Save As"));

    let saved_preferences =
        load_project_preferences(&project_settings_path(&settings_path, &identity), &identity);
    assert_eq!(
        saved_preferences.project_file_path,
        Some(manifest_path.clone())
    );
    assert_eq!(saved_preferences.last_xdf_path, Some(xdf_path.clone()));

    app.project_preferences.active_workspace_name = "Track test".into();
    app.dispatch_command("file.save-project").unwrap();
    let updated =
        TunerProjectFile::from_json(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    assert_eq!(updated.preferences.active_workspace_name, "Track test");

    for path in [&bin_path, &xdf_path, &manifest_path, &settings_path] {
        let _ = std::fs::remove_file(path);
    }
    let _ = std::fs::remove_file(project_settings_path(&settings_path, &identity));
}

#[test]
fn project_manifest_save_refuses_to_replace_bin_even_with_manifest_extension() {
    let bin_path = temporary_test_path("tnproj");
    std::fs::write(&bin_path, [0x11_u8, 0x22, 0x33]).unwrap();
    let identity = project_identity_for_path(&bin_path);
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0x11, 0x22, 0x33])),
        None,
        Some(bin_path.clone()),
        None,
    );
    app.project_identity = Some(identity);

    let result = app.save_project_manifest(&bin_path);

    assert!(result.is_err());
    assert_eq!(std::fs::read(&bin_path).unwrap(), [0x11, 0x22, 0x33]);
    assert_eq!(app.project_preferences.project_file_path, None);
    let _ = std::fs::remove_file(bin_path);
}

fn install_project_open_until_idle(app: &mut TunerApp) {
    for _ in 0..8 {
        if app.operations.active().is_none() {
            break;
        }
        let result = wait_for_operation_result(app);
        app.install_operation_result(result);
    }
}

#[test]
fn open_project_manifest_restores_workspace_association_and_xdf() {
    let bin_path = temporary_test_path("bin");
    let xdf_path = temporary_test_path("xdf");
    let manifest_path = temporary_test_path("tnproj");
    let settings_path = temporary_test_path("settings.json");
    std::fs::write(&bin_path, [10_u8, 20, 30, 40, 50, 60]).unwrap();
    std::fs::write(&xdf_path, COLUMN_MAJOR_XDF).unwrap();
    let identity = project_identity_for_path(&bin_path);
    let mut manifest_preferences = ProjectPreferences::for_identity(&identity);
    manifest_preferences.active_workspace_id = 14;
    manifest_preferences.active_workspace_name = "Track setup".into();
    manifest_preferences.browser_filter = "timing".into();
    manifest_preferences.last_xdf_path = Some(PathBuf::from("outdated.xdf"));
    let manifest = TunerProjectFile {
        format_version: 1,
        bin_path: bin_path.clone(),
        xdf_path: Some(xdf_path.clone()),
        preferences: manifest_preferences,
    };
    std::fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();

    let mut app = TunerApp::headless();
    app.settings_path = settings_path.clone();
    app.persistence_enabled = true;
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![99])),
        Some(XdfDocument::parse(TWO_TABLE_XDF).unwrap()),
        Some(PathBuf::from("old.bin")),
        Some(PathBuf::from("old.xdf")),
    );
    app.project_identity = Some("old-project".into());

    app.open_project_file(manifest_path.clone()).unwrap();
    install_project_open_until_idle(&mut app);

    assert_eq!(app.project_identity.as_deref(), Some(identity.as_str()));
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        &[10, 20, 30, 40, 50, 60]
    );
    assert!(app.workspace.xdf.is_some());
    assert_eq!(app.project_preferences.active_workspace_name, "Track setup");
    assert_eq!(app.project_preferences.browser_filter, "timing");
    assert_eq!(
        app.project_preferences.project_file_path,
        Some(manifest_path.clone())
    );
    assert_eq!(
        app.project_preferences.last_xdf_path,
        Some(xdf_path.clone())
    );

    for path in [&bin_path, &xdf_path, &manifest_path, &settings_path] {
        let _ = std::fs::remove_file(path);
    }
    let _ = std::fs::remove_file(project_settings_path(&settings_path, &identity));
}

#[test]
fn open_project_manifest_with_missing_optional_xdf_restores_bin_only_with_warning() {
    let bin_path = temporary_test_path("bin");
    let missing_xdf = temporary_test_path("missing-xdf");
    let manifest_path = temporary_test_path("tnproj");
    let settings_path = temporary_test_path("settings.json");
    std::fs::write(&bin_path, [10_u8, 20, 30]).unwrap();
    let identity = project_identity_for_path(&bin_path);
    let mut preferences = ProjectPreferences::for_identity(&identity);
    preferences.last_xdf_path = Some(missing_xdf.clone());
    let manifest = TunerProjectFile {
        format_version: 1,
        bin_path: bin_path.clone(),
        xdf_path: Some(missing_xdf.clone()),
        preferences,
    };
    std::fs::write(&manifest_path, manifest.to_json().unwrap()).unwrap();
    let mut app = TunerApp::headless();
    app.settings_path = settings_path.clone();
    app.persistence_enabled = true;
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![99])),
        Some(XdfDocument::parse(TWO_TABLE_XDF).unwrap()),
        Some(PathBuf::from("old.bin")),
        Some(PathBuf::from("old.xdf")),
    );

    app.open_project_file(manifest_path.clone()).unwrap();
    install_project_open_until_idle(&mut app);

    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), &[10, 20, 30]);
    assert!(app.workspace.xdf.is_none());
    assert!(app.workspace.xdf_path.is_none());
    assert!(app.workspace.status.text.contains("missing"));
    assert_eq!(
        app.project_preferences.last_xdf_path,
        Some(missing_xdf.clone())
    );

    for path in [&bin_path, &manifest_path, &settings_path] {
        let _ = std::fs::remove_file(path);
    }
    let _ = std::fs::remove_file(project_settings_path(&settings_path, &identity));
}

#[test]
fn invalid_project_manifest_keeps_current_documents_and_missing_bin_is_rejected() {
    let bin_path = temporary_test_path("bin");
    let manifest_path = temporary_test_path("tnproj");
    let missing_bin_path = temporary_test_path("missing-bin");
    let missing_bin_manifest_path = temporary_test_path("missing-bin-tnproj");
    std::fs::write(&bin_path, [1_u8, 2, 3]).unwrap();
    let identity = project_identity_for_path(&bin_path);
    let mut invalid_preferences = ProjectPreferences::for_identity(&identity);
    invalid_preferences.bin_identity = "different.bin".into();
    let invalid_manifest = TunerProjectFile {
        format_version: 1,
        bin_path: bin_path.clone(),
        xdf_path: None,
        preferences: invalid_preferences,
    };
    std::fs::write(
        &manifest_path,
        serde_json::to_string(&invalid_manifest).unwrap(),
    )
    .unwrap();
    let missing_identity = project_identity_for_path(&missing_bin_path);
    let missing_manifest = TunerProjectFile {
        format_version: 1,
        bin_path: missing_bin_path.clone(),
        xdf_path: None,
        preferences: ProjectPreferences::for_identity(missing_identity),
    };
    std::fs::write(
        &missing_bin_manifest_path,
        missing_manifest.to_json().unwrap(),
    )
    .unwrap();
    let mut app = TunerApp::headless();
    let (xdf, bin) = column_major_fixture();
    app.workspace.set_documents(
        Some(bin),
        Some(xdf),
        Some(PathBuf::from("unchanged.bin")),
        Some(PathBuf::from("unchanged.xdf")),
    );
    let before_bytes = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let before_xdf = app
        .workspace
        .xdf
        .as_ref()
        .unwrap()
        .normalized_fingerprint
        .clone();

    assert!(app.open_project_file(manifest_path.clone()).is_err());
    assert!(app
        .open_project_file(missing_bin_manifest_path.clone())
        .is_err());

    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), before_bytes);
    assert_eq!(
        app.workspace.xdf.as_ref().unwrap().normalized_fingerprint,
        before_xdf
    );
    assert!(app.operations.active().is_none());

    for path in [&bin_path, &manifest_path, &missing_bin_manifest_path] {
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn defaults_expose_stable_commands_and_standard_layout() {
    let preferences = AppPreferences::default();
    let registry = CommandRegistry::core();
    assert_eq!(preferences.layout, LayoutPreset::Standard);
    assert!(preferences.show_browser);
    assert!(registry.descriptors().iter().any(|command| {
        command.id == "file.open-bin" && command.default_shortcut.as_deref() == Some("Ctrl+O")
    }));
    assert!(registry
        .descriptors()
        .iter()
        .any(|command| command.id == "edit.undo"));
}

#[test]
fn ui_ipc_dispatches_through_the_existing_command_registry() {
    let mut app = TunerApp::headless();
    let request = ui_ipc::UiIpcRequest {
        protocol: Some(ui_ipc::UI_IPC_PROTOCOL.to_string()),
        token: None,
        request_id: Some("dispatch-1".to_string()),
        action: "dispatch".to_string(),
        agent_task: None,
        task_id: None,
        command: Some("view.debug-report".to_string()),
        path: None,
        semantic_id: None,
        axis_index: None,
        table_key: None,
        row: None,
        column: None,
        row_end: None,
        column_end: None,
        byte_offset: None,
        byte_length: None,
        hex: None,
        display_format: None,
        endianness: None,
        decimal_places: None,
        map_rows: None,
        map_columns: None,
        map_rows_min: None,
        map_rows_max: None,
        map_columns_min: None,
        map_columns_max: None,
        scan_all: None,
        map_start_address: None,
        map_end_address: None,
        minimum_score: None,
        skip_mapped: None,
        map_every_byte: None,
        map_zoom_percent: None,
        candidate_pane_width: None,
        map_position: None,
        map_size: None,
        candidate_index: None,
        map_axis_index: None,
        axis_role: None,
        color_mode: None,
        color_range: None,
        color_range_scope: None,
        color_low: None,
        color_middle: None,
        color_high: None,
        forward: None,
        edit_value: None,
        clamp: None,
        filter: None,
        query: None,
        formula: None,
        changed_only: None,
        selected_semantic_ids: None,
        window_id: None,
        workspace_id: None,
        workspace_name: None,
    };

    let response = app.handle_ui_ipc_request(&request);

    assert!(response.ok);
    assert_eq!(response.request_id.as_deref(), Some("dispatch-1"));
    assert!(app.debug_report_open);
}

#[test]
fn ui_ipc_axis_assignment_is_explicit_compatible_and_display_only() {
    let mut app = TunerApp::headless();
    app.map_finder.candidates = vec![MapCandidate {
        offset: 16,
        byte_length: 32,
        rows: 4,
        columns: 4,
        display_format: HexDisplayFormat::Unsigned16,
        endianness: HexEndianness::Little,
        score: 90,
        value_range: (0.0, 1.0),
        value_bands: 2,
        axis_suggestions: vec![
            MapAxisSuggestion {
                offset: 0,
                values: vec![100.0, 200.0, 300.0, 400.0],
                score: 100,
                increasing: true,
                matches_columns: true,
                matches_rows: false,
                display_format: HexDisplayFormat::Unsigned16,
                endianness: HexEndianness::Little,
            },
            MapAxisSuggestion {
                offset: 8,
                values: vec![10.0, 20.0, 30.0, 40.0],
                score: 100,
                increasing: true,
                matches_columns: false,
                matches_rows: true,
                display_format: HexDisplayFormat::Unsigned16,
                endianness: HexEndianness::Little,
            },
        ],
        selected_x_axis: None,
        selected_y_axis: None,
    }];

    let mut assign = ui_ipc::UiIpcRequest::for_test("assign_map_axis", "", None, None);
    assign.candidate_index = Some(0);
    assign.map_axis_index = Some(0);
    assign.axis_role = Some("x".to_string());
    let response = app.handle_ui_ipc_request(&assign);
    assert!(response.ok, "{}", response.message);
    let state = response.data.unwrap();
    assert_eq!(state["candidates"][0]["selected_x_axis"], 0);
    assert!(state["candidates"][0]["selected_y_axis"].is_null());

    let mut invalid = ui_ipc::UiIpcRequest::for_test("assign_map_axis", "", None, None);
    invalid.candidate_index = Some(0);
    invalid.map_axis_index = Some(1);
    invalid.axis_role = Some("x".to_string());
    assert!(!app.handle_ui_ipc_request(&invalid).ok);
    assert_eq!(app.map_finder.candidates[0].selected_x_axis, Some(0));

    let mut clear = ui_ipc::UiIpcRequest::for_test("clear_map_axis", "", None, None);
    clear.candidate_index = Some(0);
    clear.axis_role = Some("x".to_string());
    assert!(app.handle_ui_ipc_request(&clear).ok);
    assert_eq!(app.map_finder.candidates[0].selected_x_axis, None);
}

#[test]
fn map_finder_stop_button_requests_cancellation() {
    let mut app = app_with_project_fixture();
    app.map_finder.memory.window_open = true;
    app.focus_window(&WindowId::MapFinder).unwrap();
    let operation_id = app
        .operations
        .begin(OperationKind::SearchingMaps, "test map scan");
    app.map_finder.operation_id = Some(operation_id);
    let cancellation = std::sync::Arc::new(AtomicBool::new(false));
    app.map_finder.cancellation = Some(std::sync::Arc::clone(&cancellation));

    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    app.tool_window_bounds = Some(screen);
    app.map_finder.memory.x = 80;
    app.map_finder.memory.y = 80;
    app.map_finder.memory.width = 700;
    app.map_finder.memory.height = 460;
    let mut time = 1.0;
    let mut stop_point = None;
    let mut title_visible = false;
    for _ in 0..3 {
        let output = render_calibration_tool_frame(&mut app, &context, screen, time);
        stop_point = rendered_text_rect(&output, "Stop").map(|rect| rect.center());
        title_visible = rendered_text_rect(&output, "2D Map Finder").is_some();
        output.drop_without_applying_deltas();
        if stop_point.is_some() {
            break;
        }
        time += 0.1;
    }
    let point = stop_point.unwrap_or_else(|| {
        panic!(
            "Map Finder should show Stop while searching; title_visible={title_visible}, operation_id={:?}, status={:?}",
            app.map_finder.operation_id,
            app.map_finder.status
        )
    });

    for pressed in [true, false] {
        time += 0.1;
        let output = render_calibration_tool_frame_with_events(
            &mut app,
            &context,
            screen,
            time,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        output.drop_without_applying_deltas();
    }

    assert!(cancellation.load(Ordering::Acquire));
    assert_eq!(app.map_finder.status, "Stopping map search…");
}

#[test]
fn operation_overlay_stop_button_cancels_only_map_search() {
    let mut app = TunerApp::headless();
    let operation_id = app
        .operations
        .begin(OperationKind::SearchingMaps, "test map scan");
    app.map_finder.operation_id = Some(operation_id);
    app.map_finder.progress_percent.store(42, Ordering::Release);
    let cancellation = std::sync::Arc::new(AtomicBool::new(false));
    app.map_finder.cancellation = Some(std::sync::Arc::clone(&cancellation));

    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut stop_point = None;
    for frame in 0..3 {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(1.0 + f64::from(frame) * 0.1);
        let output = context.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.allocate_space(screen.size());
            });
            app.show_operation_overlay(ctx, screen);
        });
        stop_point = rendered_text_rect(&output, "Stop").map(|rect| rect.center());
        output.drop_without_applying_deltas();
        if stop_point.is_some() {
            break;
        }
    }
    let stop_point = stop_point.expect("map-search operation overlay should show Stop");

    for (time, pressed) in [(1.1, true), (1.2, false)] {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        input.events = vec![
            egui::Event::PointerMoved(stop_point),
            egui::Event::PointerButton {
                pos: stop_point,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        context
            .run_ui(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.allocate_space(screen.size());
                });
                app.show_operation_overlay(ctx, screen);
            })
            .drop_without_applying_deltas();
    }

    assert!(cancellation.load(Ordering::Acquire));
    assert_eq!(app.map_finder.status, "Stopping map search…");

    let mut input = egui::RawInput::default();
    input.screen_rect = Some(screen);
    input.time = Some(1.3);
    let output = context.run_ui(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.allocate_space(screen.size());
        });
        app.show_operation_overlay(ctx, screen);
    });
    assert!(rendered_text_rect(&output, "Stopping…").is_some());
    output.drop_without_applying_deltas();

    let mut other_app = TunerApp::headless();
    other_app
        .operations
        .begin(OperationKind::LoadingXdf, "definition.xdf");
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(screen);
    input.time = Some(1.4);
    let output = context.run_ui(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.allocate_space(screen.size());
        });
        other_app.show_operation_overlay(ctx, screen);
    });
    assert!(rendered_text_rect(&output, "Stop").is_none());
    output.drop_without_applying_deltas();
}

#[test]
fn ui_ipc_can_cancel_an_active_map_search_and_reject_idle_cancel() {
    let mut app = TunerApp::headless();
    let idle = ui_ipc::UiIpcRequest::for_test("cancel_map_search", "", None, None);
    let idle_response = app.handle_ui_ipc_request(&idle);
    assert!(!idle_response.ok);

    let operation_id = app
        .operations
        .begin(OperationKind::SearchingMaps, "test map scan");
    app.map_finder.operation_id = Some(operation_id);
    let cancellation = std::sync::Arc::new(AtomicBool::new(false));
    app.map_finder.cancellation = Some(std::sync::Arc::clone(&cancellation));

    let request = ui_ipc::UiIpcRequest::for_test("cancel_map_search", "", None, None);
    let response = app.handle_ui_ipc_request(&request);

    assert!(response.ok, "{}", response.message);
    assert!(cancellation.load(Ordering::Acquire));
    assert_eq!(response.data.unwrap()["cancel_requested"], true);
    assert!(app.ui_ipc_capabilities()["actions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|action| action.as_str() == Some("cancel_map_search")));
}

#[test]
fn cancelled_map_search_result_discards_candidates_and_reports_status() {
    let mut app = TunerApp::headless();
    let operation_id = app
        .operations
        .begin(OperationKind::SearchingMaps, "test map scan");
    app.map_finder.operation_id = Some(operation_id);
    app.map_finder.progress_percent.store(34, Ordering::Release);
    let partial_candidate = MapCandidate {
        offset: 0,
        byte_length: 16,
        rows: 4,
        columns: 4,
        display_format: HexDisplayFormat::Unsigned8,
        endianness: HexEndianness::Little,
        score: 90,
        value_range: (0.0, 15.0),
        value_bands: 16,
        axis_suggestions: Vec::new(),
        selected_x_axis: None,
        selected_y_axis: None,
    };

    app.install_operation_result(OperationResult::map_candidates(
        operation_id,
        "test map scan",
        vec![partial_candidate],
        true,
    ));

    assert!(app.map_finder.candidates.is_empty());
    assert!(app.map_finder.operation_id.is_none());
    assert!(app.map_finder.cancellation.is_none());
    assert_eq!(app.map_finder.progress_percent.load(Ordering::Acquire), 34);
    assert_eq!(
        app.map_finder.status,
        "Map scan canceled; partial results discarded."
    );
    assert!(!app.operations.is_busy());
}

#[test]
fn map_search_worker_propagates_stop_to_the_operation_result() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes((0_u8..128).collect()));
    app.map_finder.memory.rows_min = 4;
    app.map_finder.memory.rows_max = 4;
    app.map_finder.memory.columns_min = 4;
    app.map_finder.memory.columns_max = 4;
    app.map_finder.memory.display_format = HexDisplayFormat::Unsigned8;
    app.map_finder.memory.scan_every_byte = true;
    app.map_finder.memory.skip_mapped = false;

    app.start_map_search(true).unwrap();
    let cancellation = std::sync::Arc::clone(app.map_finder.cancellation.as_ref().unwrap());
    cancellation.store(true, Ordering::Release);
    let result = wait_for_operation_result(&mut app);

    assert!(matches!(
        &result.payload,
        Ok(OperationPayload::MapCandidates {
            candidates,
            cancelled: true
        }) if candidates.is_empty()
    ));
    app.install_operation_result(result);
    assert!(app.map_finder.candidates.is_empty());
    assert_eq!(
        app.map_finder.status,
        "Map scan canceled; partial results discarded."
    );
}

#[test]
fn ui_ipc_can_dismiss_the_startup_restore_prompt() {
    let mut app = TunerApp::headless();
    app.pending_project_restore = Some(PendingProjectRestore {
        bin_path: PathBuf::from("last.bin"),
        xdf_path: None,
        bin_available: true,
        xdf_available: false,
    });

    let request = ui_ipc::UiIpcRequest::for_test("dismiss_last_project_prompt", "", None, None);
    let response = app.handle_ui_ipc_request(&request);
    assert!(response.ok);
    assert!(app.pending_project_restore.is_none());

    let capabilities = app.ui_ipc_capabilities();
    let actions = capabilities["actions"].as_array().unwrap();
    assert!(actions
        .iter()
        .any(|action| { action.as_str() == Some("restore_last_project_bin_xdf") }));
}

#[test]
fn ui_ipc_compare_actions_are_discoverable_and_change_mode() {
    let mut app = app_with_matching_compare_documents();
    let capabilities = app.ui_ipc_capabilities();
    let actions = capabilities["actions"].as_array().unwrap();
    assert!(actions.iter().any(|value| value == "open_compare_bin"));
    assert!(actions.iter().any(|value| value == "build_transfer_plan"));

    let request = ui_ipc::UiIpcRequest::for_test("set_compare_mode", "", None, None);
    let response = app.handle_ui_ipc_request(&request);
    assert!(!response.ok, "missing mode must be rejected");
}

#[test]
fn ui_ipc_state_reports_compare_hashes_and_plan_status() {
    let app = app_with_matching_compare_documents();
    let state = app.ui_ipc_state();
    assert!(state["compare"]["source_bin_sha256"].is_string());
    assert!(state["compare"]["destination_bin_sha256"].is_string());
    assert_eq!(state["compare"]["plan_status"], "none");
}

#[test]
fn ui_ipc_open_compare_bin_reveals_the_compare_window() {
    let path = temporary_test_path("compare.bin");
    std::fs::write(&path, [1, 2, 3]).unwrap();
    let mut app = TunerApp::headless();
    let mut request = ui_ipc::UiIpcRequest::for_test("open_compare_bin", "", None, None);
    request.path = Some(path.clone());

    let response = app.handle_ui_ipc_request(&request);

    assert!(response.ok);
    assert!(app.compare.window_open);
    if let Some(operation) = app.operations.active() {
        app.operations.cancel(operation.id, "test cleanup");
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn ui_ipc_workspace_window_actions_are_discoverable_and_document_safe() {
    let mut app = app_with_project_fixture();
    let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let default_id = app.project_preferences.active_workspace_id;
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let table_key = app.open_tables[0].key.clone();
    let table_id = format!("table:{table_key}");

    for (action, minimized) in [
        ("focus_window", false),
        ("minimize_window", true),
        ("restore_window", false),
    ] {
        let response = app.handle_ui_ipc_request(&agent_ipc_request(json!({
            "action": action,
            "window_id": table_id
        })));
        assert!(response.ok, "{}: {}", action, response.message);
        let state = app.ui_ipc_state();
        assert_eq!(
            state["windows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|window| window["id"] == table_id)
                .unwrap()["minimized"],
            json!(minimized)
        );
    }

    let create = agent_ipc_request(json!({
        "action": "workspace_create",
        "workspace_name": "Agent layout"
    }));
    let created = app.handle_ui_ipc_request(&create);
    assert!(created.ok, "{}", created.message);
    let new_id = created.data.unwrap()["workspace_id"].as_u64().unwrap();
    let state = app.ui_ipc_state();
    assert_eq!(state["active_workspace_id"], json!(new_id));
    assert!(state["workspaces"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| { entry["id"] == json!(new_id) && entry["active"] == json!(true) }));
    assert!(!state["windows"].as_array().unwrap().is_empty());
    let actions = app.ui_ipc_capabilities()["actions"]
        .as_array()
        .unwrap()
        .clone();
    for action in [
        "focus_window",
        "minimize_window",
        "restore_window",
        "close_window",
        "workspace_create",
        "workspace_switch",
        "workspace_rename",
        "workspace_delete",
    ] {
        assert!(
            actions.iter().any(|value| value == action),
            "missing {action}"
        );
    }

    let rename = agent_ipc_request(json!({
        "action": "workspace_rename",
        "workspace_id": new_id,
        "workspace_name": "Agent renamed"
    }));
    assert!(app.handle_ui_ipc_request(&rename).ok);
    let switch = agent_ipc_request(json!({
        "action": "workspace_switch",
        "workspace_id": default_id
    }));
    assert!(app.handle_ui_ipc_request(&switch).ok);
    assert_eq!(app.project_preferences.active_workspace_id, default_id);

    let close = agent_ipc_request(json!({
        "action": "close_window",
        "window_id": "table:unknown"
    }));
    assert!(!app.handle_ui_ipc_request(&close).ok);
    let close_open = agent_ipc_request(json!({
        "action": "close_window",
        "window_id": table_id
    }));
    assert!(app.handle_ui_ipc_request(&close_open).ok);
    let delete = agent_ipc_request(json!({
        "action": "workspace_delete",
        "workspace_id": new_id
    }));
    assert!(app.handle_ui_ipc_request(&delete).ok);
    let invalid = agent_ipc_request(json!({
        "action": "focus_window",
        "window_id": "unknown"
    }));
    assert!(!app.handle_ui_ipc_request(&invalid).ok);
    let invalid_workspace = agent_ipc_request(json!({
        "action": "workspace_switch",
        "workspace_id": 999999
    }));
    assert!(!app.handle_ui_ipc_request(&invalid_workspace).ok);
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        before.as_slice()
    );
}

#[test]
fn compare_graph_selection_mirrors_the_active_table_selection() {
    let mut app = TunerApp::headless();
    app.open_compare_with_test_documents();
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    app.workspace.select_parameter(&semantic_id);
    app.workspace.select_cell_range(0, 1, 1, 2).unwrap();
    app.compare.selected_semantic_id = Some(semantic_id.clone());

    app.sync_compare_selection_from_workspace(&semantic_id);

    assert_eq!(
        app.compare.selected_cells,
        app.workspace.selected_cell_range()
    );
}

#[test]
fn compare_debug_report_includes_xdf_hashes_and_plan_state() {
    let app = app_with_matching_compare_documents();
    let report = app.debug_report_text();

    assert!(report.contains("compare_source_xdf_sha256:"));
    assert!(report.contains("compare_destination_xdf_sha256:"));
    assert!(report.contains("transfer_plan: none"));
}

#[test]
fn preferences_round_trip_and_malformed_input_restore_defaults() {
    let mut preferences = AppPreferences::default();
    preferences.theme = ThemeMode::Dark;
    preferences.layout = LayoutPreset::Diagnostics;
    preferences.browser_organization = BrowserOrganization::RecentFirst;
    preferences.browser_collapsed = true;
    preferences.inspector_width = 420;
    preferences.table_windows.insert(
        "xdf|map".into(),
        TableWindowMemory {
            x: 120,
            y: 90,
            width: 900,
            height: 600,
            position_saved: true,
            zoom_percent: 125,
            scroll_x: 10,
            scroll_y: 20,
            decimal_places: 2,
            coloring: TableColorSettings::default(),
            fit_to_content: Some(false),
        },
    );
    preferences
        .shortcuts
        .insert("edit.undo".into(), "Alt+U".into());
    let encoded = preferences_to_json(&preferences).unwrap();
    let decoded = preferences_from_json(&encoded);
    assert_eq!(decoded, preferences);
    assert_eq!(preferences_from_json("not json"), AppPreferences::default());
    assert_eq!(
        preferences_from_json(r#"{"version":1,"unknown":true}"#),
        AppPreferences::default()
    );
}

#[test]
fn default_table_zoom_preference_defaults_clamps_and_round_trips() {
    let defaults = serde_json::to_value(AppPreferences::default()).unwrap();
    assert_eq!(defaults["default_table_zoom_percent"], 100);

    let loaded = preferences_from_json(r#"{"version":5,"default_table_zoom_percent":137}"#);
    assert_eq!(
        serde_json::to_value(loaded).unwrap()["default_table_zoom_percent"],
        137
    );
    let clamped = preferences_from_json(r#"{"version":5,"default_table_zoom_percent":240}"#);
    assert_eq!(
        serde_json::to_value(clamped).unwrap()["default_table_zoom_percent"],
        200
    );
}

#[test]
fn new_tables_use_default_zoom_while_saved_zoom_wins() {
    let (xdf, bin) = column_major_fixture();
    let semantic_id = xdf.parameters[0].semantic_id.clone();
    let key = table_key(&xdf.normalized_fingerprint, &semantic_id);
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    app.project_identity = Some("bin-a".into());
    app.preferences = preferences_from_json(r#"{"version":5,"default_table_zoom_percent":140}"#);

    assert!(app.open_table(&semantic_id));
    assert_eq!(app.open_tables[0].memory.zoom_percent, 140);

    app.close_table(&key);
    app.preferences
        .table_windows
        .get_mut(&key)
        .expect("opening should persist a table memory")
        .zoom_percent = 165;
    assert!(app.open_table(&semantic_id));
    assert_eq!(app.open_tables[0].memory.zoom_percent, 165);
}

#[test]
fn toolbar_default_table_zoom_controls_change_the_preference() {
    let mut app = TunerApp::headless();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 300.0));
    let mut time = 0.0;

    let output = render_toolbar_frame(&mut app, &context, screen, time, Vec::new());
    let has_default_zoom_label = rendered_text_rect(&output, "New table zoom").is_some();
    let has_default_zoom_value = rendered_text_rect(&output, "100%").is_some();
    output.drop_without_applying_deltas();
    assert!(has_default_zoom_label);
    assert!(has_default_zoom_value);

    let preference_zoom = |app: &TunerApp| {
        serde_json::to_value(&app.preferences).unwrap()["default_table_zoom_percent"].clone()
    };
    click_toolbar_label(&mut app, "−", &context, screen, &mut time);
    assert_eq!(preference_zoom(&app), 90);
    click_toolbar_label(&mut app, "+", &context, screen, &mut time);
    assert_eq!(preference_zoom(&app), 100);
}

#[test]
fn legacy_preferences_default_sweep_is_enabled_and_sweep_setting_round_trips() {
    let legacy = preferences_from_json(r#"{"version":3}"#);
    assert!(legacy.sweep_enabled);
    assert_eq!(legacy.version, SETTINGS_VERSION);

    let settings_path = temporary_test_path("json");
    let mut preferences = legacy;
    preferences.sweep_enabled = false;
    save_preferences(&settings_path, &preferences).unwrap();
    assert!(!load_preferences(&settings_path).sweep_enabled);

    preferences.sweep_enabled = true;
    save_preferences(&settings_path, &preferences).unwrap();
    assert!(load_preferences(&settings_path).sweep_enabled);
    std::fs::remove_file(settings_path).unwrap();
}

#[test]
fn school_me_mode_defaults_off_for_new_and_legacy_settings() {
    let defaults = serde_json::to_value(AppPreferences::default()).unwrap();
    assert_eq!(defaults["school_me_mode"], false);

    let legacy = preferences_from_json(r#"{"version":4}"#);
    assert_eq!(legacy.version, SETTINGS_VERSION);
    assert_eq!(
        serde_json::to_value(legacy).unwrap()["school_me_mode"],
        false
    );
}

#[test]
fn school_me_mode_round_trips_and_is_exposed_to_nooklink() {
    let mut app = TunerApp::headless();
    let initial_state = app.ui_ipc_state();
    assert_eq!(initial_state["school_me_mode"], false);

    let result = app.dispatch_command("view.toggle-school-me-mode");
    assert!(result.is_ok(), "toggle command should exist: {result:?}");
    assert_eq!(app.ui_ipc_state()["school_me_mode"], true);

    let serialized = preferences_to_json(&app.preferences).unwrap();
    let restored = preferences_from_json(&serialized);
    assert_eq!(
        serde_json::to_value(restored).unwrap()["school_me_mode"],
        true
    );

    app.settings_open = true;
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut time = 1.0;
    click_settings_label(&mut app, "School-Me Mode", &context, screen, &mut time);
    assert_eq!(app.ui_ipc_state()["school_me_mode"], false);
}

#[test]
fn school_me_command_button_shows_immediate_help_on_hover() {
    let mut app = TunerApp::headless();
    app.preferences.school_me_mode = true;
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let output = render_toolbar_frame(&mut app, &context, screen, 1.0, Vec::new());
    let point = rendered_text_rect(&output, "Open BIN  [Ctrl+O]")
        .expect("toolbar should render the Open BIN command")
        .center();
    output.drop_without_applying_deltas();

    let output = render_toolbar_frame(
        &mut app,
        &context,
        screen,
        1.1,
        vec![egui::Event::PointerMoved(point)],
    );
    output.drop_without_applying_deltas();
    let output = render_toolbar_frame(&mut app, &context, screen, 1.11, Vec::new());
    let school_help_visible = rendered_text_rect(&output, "What it does").is_some();
    output.drop_without_applying_deltas();

    assert!(
        school_help_visible,
        "School-Me Mode should explain an enabled command immediately on hover"
    );
}

#[test]
fn school_me_custom_browser_control_explains_hover() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.preferences.school_me_mode = true;
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0));
    let output = render_browser_contents_frame(&mut app, &context, screen, 1.0, Vec::new());
    let filter_point = rendered_text_rect(&output, "Title, ID, category…")
        .expect("browser filter hint should render")
        .center();
    output.drop_without_applying_deltas();

    let output = render_browser_contents_frame(
        &mut app,
        &context,
        screen,
        1.1,
        vec![egui::Event::PointerMoved(filter_point)],
    );
    output.drop_without_applying_deltas();
    let output = render_browser_contents_frame(&mut app, &context, screen, 1.11, Vec::new());
    let help_visible = rendered_text_rect(&output, "Filter the parameter list").is_some();
    output.drop_without_applying_deltas();

    assert!(
        help_visible,
        "the browser filter should have School-Me help"
    );
}

#[test]
fn school_me_table_cell_explains_selection_copy_and_edit() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.preferences.school_me_mode = true;
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let bin_before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let data_revision_before = app.workspace.data_revision();
    let xdf_hash_before = app.workspace.xdf.as_ref().unwrap().exact_sha256.clone();
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let mut time = 1.0;
    let draw =
        |app: &mut TunerApp, context: &egui::Context, time: f64, events: Vec<egui::Event>| {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(screen);
            input.time = Some(time);
            input.events = events;
            context.run_ui(input, |ctx| {
                app.show_table_window(ctx, screen, &key);
                school_help::show_help_window(ctx, app.preferences.school_me_mode);
            })
        };

    for _ in 0..3 {
        draw(&mut app, &context, time, Vec::new()).drop_without_applying_deltas();
        time += 0.1;
    }
    let output = draw(&mut app, &context, time, Vec::new());
    let cell_point = rendered_text_rect(&output, "10.00")
        .expect("first table cell should render")
        .center();
    output.drop_without_applying_deltas();

    let output = draw(
        &mut app,
        &context,
        time + 0.1,
        vec![egui::Event::PointerMoved(cell_point)],
    );
    output.drop_without_applying_deltas();
    let output = draw(&mut app, &context, time + 0.11, Vec::new());
    let help_visible = rendered_text_rect(&output, "Select a table cell").is_some()
        || rendered_text_rect(&output, "Select several cells").is_some();
    output.drop_without_applying_deltas();

    assert!(
        help_visible,
        "table cells should explain selection and editing"
    );
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), bin_before);
    assert_eq!(app.workspace.data_revision(), data_revision_before);
    assert_eq!(
        app.workspace.xdf.as_ref().unwrap().exact_sha256,
        xdf_hash_before
    );
}

#[test]
fn school_me_disabled_command_explains_block_reason() {
    let mut app = TunerApp::headless();
    app.preferences.school_me_mode = true;
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 800.0));
    let output = render_toolbar_frame(&mut app, &context, screen, 1.0, Vec::new());
    let point = rendered_text_rect(&output, "Save As  [Ctrl+S]")
        .expect("disabled Save As command should render")
        .center();
    output.drop_without_applying_deltas();

    let output = render_toolbar_frame(
        &mut app,
        &context,
        screen,
        1.1,
        vec![egui::Event::PointerMoved(point)],
    );
    output.drop_without_applying_deltas();
    let output = render_toolbar_frame(&mut app, &context, screen, 1.11, Vec::new());
    let help_visible = rendered_text_rect(&output, "requires an open BIN").is_some();
    output.drop_without_applying_deltas();

    assert!(
        help_visible,
        "disabled actions should explain why they cannot run"
    );
}

#[test]
fn school_me_mode_off_preserves_plain_command_ui() {
    let mut app = TunerApp::headless();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 800.0));
    let output = render_toolbar_frame(&mut app, &context, screen, 1.0, Vec::new());
    let point = rendered_text_rect(&output, "Save As  [Ctrl+S]")
        .expect("disabled Save As command should render")
        .center();
    output.drop_without_applying_deltas();

    let output = render_toolbar_frame(
        &mut app,
        &context,
        screen,
        1.1,
        vec![egui::Event::PointerMoved(point)],
    );
    let educational_copy_visible = rendered_text_rect(&output, "What it does").is_some();
    output.drop_without_applying_deltas();

    assert!(
        !educational_copy_visible,
        "School-Me help must remain off by default"
    );
}

#[test]
fn school_me_settings_workspace_and_dock_controls_explain_hover() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.preferences.school_me_mode = true;
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    app.settings_open = true;
    let dock_label = app.dock_entries()[0].label.clone();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));

    let draw_dock = |app: &mut TunerApp, time, events| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        input.events = events;
        context.run_ui(input, |ui| {
            let ctx = ui.ctx().clone();
            app.show_window_dock(ui);
            school_help::show_help_window(&ctx, app.preferences.school_me_mode);
        })
    };
    let output = draw_dock(&mut app, 0.9, Vec::new());
    let workspace_point = rendered_text_rect(&output, "Workspace: Default")
        .expect("workspace selector should render")
        .center();
    output.drop_without_applying_deltas();
    draw_dock(
        &mut app,
        1.0,
        vec![egui::Event::PointerMoved(workspace_point)],
    )
    .drop_without_applying_deltas();
    let output = draw_dock(&mut app, 1.01, Vec::new());
    let workspace_help = rendered_text_rect(&output, "Switch workspaces").is_some();
    output.drop_without_applying_deltas();
    assert!(
        workspace_help,
        "workspace selector should explain saved layouts"
    );

    draw_dock(
        &mut app,
        1.05,
        vec![egui::Event::PointerMoved(egui::pos2(1_180.0, 880.0))],
    )
    .drop_without_applying_deltas();
    let output = draw_dock(&mut app, 1.1, Vec::new());
    let dock_point = rendered_text_rect(&output, &dock_label)
        .expect("open table should render in the dock")
        .center();
    output.drop_without_applying_deltas();
    draw_dock(&mut app, 1.2, vec![egui::Event::PointerMoved(dock_point)])
        .drop_without_applying_deltas();
    let output = draw_dock(&mut app, 1.21, Vec::new());
    let dock_help = rendered_text_rect(&output, "Focus or minimize a window").is_some();
    output.drop_without_applying_deltas();
    assert!(
        dock_help,
        "dock tabs should explain focus and minimize behavior"
    );

    let mut settings_app = TunerApp::headless();
    settings_app.preferences.school_me_mode = true;
    settings_app.settings_open = true;
    settings_app
        .project_preferences
        .dock_state
        .restore(&WindowId::Settings);
    settings_app
        .project_preferences
        .dock_state
        .open_tool(WindowId::Settings);
    settings_app.tool_window_bounds = Some(screen);
    let settings_context = egui::Context::default();
    for time in [1.0, 1.1, 1.2] {
        render_utility_windows_frame(&mut settings_app, &settings_context, screen, time)
            .drop_without_applying_deltas();
    }
    let output = render_utility_windows_frame(&mut settings_app, &settings_context, screen, 1.3);
    let mode_rect = rendered_text_rect(&output, "School-Me Mode");
    output.drop_without_applying_deltas();
    let mode_point = mode_rect
        .expect("Settings should show the School-Me option")
        .center();
    let output = render_utility_windows_frame_with_events(
        &mut settings_app,
        &settings_context,
        screen,
        1.4,
        vec![egui::Event::PointerMoved(mode_point)],
    );
    output.drop_without_applying_deltas();
    let output = render_utility_windows_frame(&mut settings_app, &settings_context, screen, 1.41);
    let setting_help = rendered_text_rect(&output, "Turn School-Me Mode on or off").is_some();
    output.drop_without_applying_deltas();
    assert!(
        setting_help,
        "Settings controls should explain their choices"
    );
}

#[test]
fn school_me_table_axis_header_explains_breakpoints() {
    let xdf = XdfDocument::parse(AXIS_TABLE_XDF).unwrap();
    let mut app = TunerApp::headless();
    app.preferences.school_me_mode = true;
    app.workspace.set_documents(
        Some(BinDocument::from_bytes((0..64).collect())),
        Some(xdf),
        None,
        None,
    );
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let draw = |app: &mut TunerApp, time, events| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        input.events = events;
        context.run_ui(input, |ctx| {
            app.show_table_window(ctx, screen, &key);
            school_help::show_help_window(ctx, app.preferences.school_me_mode);
        })
    };
    for time in [1.0, 1.1, 1.2] {
        draw(&mut app, time, Vec::new()).drop_without_applying_deltas();
    }
    let output = draw(&mut app, 1.3, Vec::new());
    let axis_point = rendered_text_rect(&output, "8.00")
        .expect("mapped axis value should render")
        .center();
    output.drop_without_applying_deltas();
    draw(&mut app, 1.4, vec![egui::Event::PointerMoved(axis_point)]).drop_without_applying_deltas();
    let output = draw(&mut app, 1.41, Vec::new());
    let axis_help = rendered_text_rect(&output, "Select an axis number").is_some();
    output.drop_without_applying_deltas();
    assert!(
        axis_help,
        "axis headers should explain their breakpoint role"
    );
}

#[test]
fn school_me_search_query_explains_search_scope() {
    let mut app = TunerApp::headless();
    app.preferences.school_me_mode = true;
    app.search_state.open = true;
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    app.tool_window_bounds = Some(screen);
    let draw = |app: &mut TunerApp, time, events| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        input.events = events;
        context.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.allocate_space(screen.size());
            });
            app.show_search_window(ctx, screen);
            school_help::show_help_window(ctx, app.preferences.school_me_mode);
        })
    };
    for time in [1.0, 1.1, 1.2] {
        draw(&mut app, time, Vec::new()).drop_without_applying_deltas();
    }
    let output = draw(&mut app, 1.3, Vec::new());
    let query_point =
        rendered_text_rect(&output, "title, ID, category, 0xFF, or engineering value")
            .expect("advanced search query field should render")
            .center();
    output.drop_without_applying_deltas();
    draw(&mut app, 1.4, vec![egui::Event::PointerMoved(query_point)])
        .drop_without_applying_deltas();
    let output = draw(&mut app, 1.41, Vec::new());
    let help_visible = rendered_text_rect(&output, "Search for parameters").is_some();
    output.drop_without_applying_deltas();
    assert!(
        help_visible,
        "advanced search should explain what fields it checks"
    );
}

#[test]
fn school_me_map_finder_format_explains_numeric_interpretation() {
    let mut app = TunerApp::headless();
    app.preferences.school_me_mode = true;
    app.workspace.bin = Some(BinDocument::from_bytes(vec![0; 64]));
    app.map_finder.memory.window_open = true;
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    app.tool_window_bounds = Some(screen);
    let draw = |app: &mut TunerApp, time, events| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        input.events = events;
        context.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.allocate_space(screen.size());
            });
            app.show_map_finder(ctx, screen);
            school_help::show_help_window(ctx, app.preferences.school_me_mode);
        })
    };
    for time in [1.0, 1.1, 1.2] {
        draw(&mut app, time, Vec::new()).drop_without_applying_deltas();
    }
    let output = draw(&mut app, 1.3, Vec::new());
    let format_point = rendered_text_rect(&output, "u16")
        .expect("Map Finder number format should render")
        .center();
    output.drop_without_applying_deltas();
    draw(&mut app, 1.4, vec![egui::Event::PointerMoved(format_point)])
        .drop_without_applying_deltas();
    let output = draw(&mut app, 1.41, Vec::new());
    let help_visible = rendered_text_rect(&output, "Choose number format").is_some();
    output.drop_without_applying_deltas();
    assert!(
        help_visible,
        "Map Finder format should explain its effect on values"
    );
}

#[test]
fn school_me_hex_format_explains_interpretation_without_editing_bytes() {
    let mut app = TunerApp::headless();
    app.preferences.school_me_mode = true;
    app.workspace.bin = Some(BinDocument::from_bytes((0..128).collect()));
    app.hex_editor.window_open = true;
    app.hex_editor.memory.display_format = HexDisplayFormat::Unsigned16;
    let bytes_before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let revision_before = app.workspace.data_revision();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    app.tool_window_bounds = Some(screen);
    let draw = |app: &mut TunerApp, time, events| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        input.events = events;
        context.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.allocate_space(screen.size());
            });
            app.show_hex_editor(ctx, screen);
            school_help::show_help_window(ctx, app.preferences.school_me_mode);
        })
    };
    for time in [1.0, 1.1, 1.2] {
        draw(&mut app, time, Vec::new()).drop_without_applying_deltas();
    }
    let output = draw(&mut app, 1.3, Vec::new());
    let format_point = rendered_text_rect(&output, "u16")
        .expect("Hex Editor display format should render")
        .center();
    output.drop_without_applying_deltas();
    draw(&mut app, 1.4, vec![egui::Event::PointerMoved(format_point)])
        .drop_without_applying_deltas();
    let output = draw(&mut app, 1.41, Vec::new());
    let help_visible = rendered_text_rect(&output, "Read bytes as a number format").is_some();
    output.drop_without_applying_deltas();
    assert!(
        help_visible,
        "Hex format should explain how bytes are interpreted"
    );
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), bytes_before);
    assert_eq!(app.workspace.data_revision(), revision_before);
}

#[test]
fn school_me_xdf_storage_help_does_not_edit_the_definition() {
    let mut app = app_with_project_fixture();
    app.preferences.school_me_mode = true;
    app.open_xdf_editor().unwrap();
    let bin_before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let xdf_hash_before = app.workspace.xdf.as_ref().unwrap().exact_sha256.clone();
    let revision_before = app.xdf_editor.revision;
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_300.0, 900.0));
    app.tool_window_bounds = Some(screen);
    let draw = |app: &mut TunerApp, time, events| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        input.events = events;
        context.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.allocate_space(screen.size());
            });
            app.show_xdf_editor_window(ctx, screen);
            school_help::show_help_window(ctx, app.preferences.school_me_mode);
        })
    };
    for time in [1.0, 1.1, 1.2] {
        draw(&mut app, time, Vec::new()).drop_without_applying_deltas();
    }
    let output = draw(&mut app, 1.3, Vec::new());
    let storage_point = rendered_text_rect(&output, "Layout / Storage")
        .expect("XDF Maker should show its layout/storage section")
        .center();
    output.drop_without_applying_deltas();
    draw(
        &mut app,
        1.4,
        vec![egui::Event::PointerMoved(storage_point)],
    )
    .drop_without_applying_deltas();
    let output = draw(&mut app, 1.41, Vec::new());
    let help_visible =
        rendered_text_rect(&output, "Describe where and how a table is stored").is_some();
    output.drop_without_applying_deltas();
    assert!(
        help_visible,
        "XDF storage controls should explain their fields"
    );
    assert!(!app.xdf_editor.dirty && !app.xdf_editor.form_dirty);
    assert_eq!(app.xdf_editor.revision, revision_before);
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), bin_before);
    assert_eq!(
        app.workspace.xdf.as_ref().unwrap().exact_sha256,
        xdf_hash_before
    );
}

#[test]
fn school_me_surface_graph_explains_gestures_without_editing_values() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.preferences.school_me_mode = true;
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_surface(&semantic_id));
    let key = app.open_surfaces[0].key.clone();
    let bin_before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let revision_before = app.workspace.data_revision();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let draw = |app: &mut TunerApp, time, events| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        input.events = events;
        context.run_ui(input, |ctx| {
            app.show_surface_window(ctx, &key);
            school_help::show_help_window(ctx, app.preferences.school_me_mode);
        })
    };
    for time in [1.0, 1.1, 1.2, 1.3] {
        draw(&mut app, time, Vec::new()).drop_without_applying_deltas();
    }
    let plot_id = egui::Id::new(("surface-plot", key.as_str()));
    let plot_point = context
        .read_response(plot_id)
        .expect("3D surface interaction area should render")
        .rect
        .center();
    draw(&mut app, 1.4, vec![egui::Event::PointerMoved(plot_point)]).drop_without_applying_deltas();
    let output = draw(&mut app, 1.41, Vec::new());
    let help_visible = rendered_text_rect(&output, "Use the 3D surface").is_some()
        || rendered_text_rect(&output, "Select a graph point").is_some();
    output.drop_without_applying_deltas();
    assert!(
        help_visible,
        "the 3D graph should explain its gestures and points"
    );
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), bin_before);
    assert_eq!(app.workspace.data_revision(), revision_before);
}

#[test]
fn school_me_disabled_map_scan_explains_wait_reason() {
    let mut app = TunerApp::headless();
    app.preferences.school_me_mode = true;
    app.workspace.bin = Some(BinDocument::from_bytes(vec![0; 64]));
    app.map_finder.memory.window_open = true;
    app.operations
        .begin(OperationKind::Searching, "Another search");
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    app.tool_window_bounds = Some(screen);
    let draw = |app: &mut TunerApp, time, events| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        input.events = events;
        context.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.allocate_space(screen.size());
            });
            app.show_map_finder(ctx, screen);
            school_help::show_help_window(ctx, app.preferences.school_me_mode);
        })
    };
    for time in [1.0, 1.1, 1.2] {
        draw(&mut app, time, Vec::new()).drop_without_applying_deltas();
    }
    let output = draw(&mut app, 1.3, Vec::new());
    let scan_point = rendered_text_rect(&output, "⌕ Scan Range")
        .expect("disabled Map Finder scan action should render")
        .center();
    output.drop_without_applying_deltas();
    draw(&mut app, 1.4, vec![egui::Event::PointerMoved(scan_point)]).drop_without_applying_deltas();
    let output = draw(&mut app, 1.41, Vec::new());
    let reason_visible = rendered_text_rect(
        &output,
        "Wait for the current background operation to finish.",
    )
    .is_some();
    output.drop_without_applying_deltas();
    assert!(
        reason_visible,
        "disabled scan actions should explain the current blocker"
    );
}

#[test]
fn school_me_nooklink_setup_explains_agent_start_actions() {
    let mut app = TunerApp::headless();
    app.preferences.school_me_mode = true;
    app.nooklink_setup_open = true;
    app.project_preferences
        .dock_state
        .open_tool(WindowId::NookLink);
    app.project_preferences
        .dock_state
        .restore(&WindowId::NookLink);
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    app.tool_window_bounds = Some(screen);
    for time in [1.0, 1.1, 1.2] {
        render_utility_windows_frame(&mut app, &context, screen, time)
            .drop_without_applying_deltas();
    }
    let output = render_utility_windows_frame(&mut app, &context, screen, 1.3);
    let starter_point = rendered_text_rect(&output, "Copy starter prompt")
        .expect("NookLink quick start should expose the starter action")
        .center();
    output.drop_without_applying_deltas();
    render_utility_windows_frame_with_events(
        &mut app,
        &context,
        screen,
        1.4,
        vec![egui::Event::PointerMoved(starter_point)],
    )
    .drop_without_applying_deltas();
    let output = render_utility_windows_frame(&mut app, &context, screen, 1.41);
    let help_visible = rendered_text_rect(&output, "Copy the agent starter prompt").is_some();
    output.drop_without_applying_deltas();
    assert!(
        help_visible,
        "NookLink setup actions should explain their purpose"
    );
}

#[test]
fn school_me_compare_filter_explains_its_scope() {
    let mut app = app_with_matching_compare_documents();
    app.preferences.school_me_mode = true;
    let destination_before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let revision_before = app.workspace.data_revision();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    app.tool_window_bounds = Some(screen);
    let draw = |app: &mut TunerApp, time, events| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        input.events = events;
        context.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.allocate_space(screen.size());
            });
            app.show_compare_window(ctx);
            school_help::show_help_window(ctx, app.preferences.school_me_mode);
        })
    };
    for time in [1.0, 1.1, 1.2] {
        draw(&mut app, time, Vec::new()).drop_without_applying_deltas();
    }
    let output = draw(&mut app, 1.3, Vec::new());
    let filter_point = rendered_text_rect(&output, "Filter compare maps…")
        .expect("Compare window filter should render")
        .center();
    output.drop_without_applying_deltas();
    draw(&mut app, 1.4, vec![egui::Event::PointerMoved(filter_point)])
        .drop_without_applying_deltas();
    let output = draw(&mut app, 1.41, Vec::new());
    let help_visible = rendered_text_rect(&output, "Filter compared parameters").is_some();
    output.drop_without_applying_deltas();
    assert!(help_visible, "Compare should explain the map filter");
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        destination_before
    );
    assert_eq!(app.workspace.data_revision(), revision_before);
}

#[test]
fn sweep_toolbar_toggles_and_saves_app_preference() {
    let mut app = TunerApp::headless();
    let settings_path = temporary_test_path("json");
    app.settings_path = settings_path.clone();
    app.persistence_enabled = true;
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut time = 1.0;

    assert!(app.preferences.sweep_enabled);
    click_toolbar_label(&mut app, "Sweep", &context, screen, &mut time);
    assert!(!app.preferences.sweep_enabled);
    assert!(app.preferences_dirty);
    app.persist_preferences();
    assert!(!load_preferences(&settings_path).sweep_enabled);

    std::fs::remove_file(settings_path).unwrap();
}

#[test]
fn settings_entry_points_focus_one_existing_window() {
    let mut app = TunerApp::headless();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut time = 1.0;
    let settings_id = WindowId::Settings;

    for (menu_or_button, item, focus_shortcuts) in [
        (Some("File"), Some("Settings…"), false),
        (None, None, false),
        (Some("Tools"), Some("Keyboard Shortcuts…"), true),
    ] {
        if let Some(menu) = menu_or_button {
            click_toolbar_label(&mut app, menu, &context, screen, &mut time);
            let item = item.expect("menu item should be present");
            click_toolbar_label(&mut app, item, &context, screen, &mut time);
        } else {
            click_toolbar_label(&mut app, "⚙", &context, screen, &mut time);
        }

        assert!(app.settings_open);
        assert_eq!(
            app.project_preferences.dock_state.focused(),
            Some(&settings_id)
        );
        assert!(app
            .project_preferences
            .dock_state
            .is_open_tool(&settings_id));
        assert_eq!(app.settings_focus_shortcuts, focus_shortcuts);
        app.close_window(&settings_id).unwrap();
        time += 0.2;
    }
}

#[test]
fn legacy_shortcut_overrides_gain_workspace_defaults_without_overwriting_custom_or_empty() {
    let restored = preferences_from_json(
        r#"{
            "version": 4,
            "shortcuts": {
                "edit.undo": "Alt+U",
                "workspace.slot.1": "",
                "workspace.slot.2": "Ctrl+Alt+W"
            }
        }"#,
    );

    assert_eq!(restored.version, SETTINGS_VERSION);
    assert_eq!(restored.shortcuts["edit.undo"], "Alt+U");
    assert_eq!(restored.shortcuts["workspace.slot.1"], "");
    assert_eq!(restored.shortcuts["workspace.slot.2"], "Ctrl+Alt+W");
    assert_eq!(restored.shortcuts["workspace.slot.3"], "Shift+3");
    assert_eq!(restored.shortcuts["workspace.slot.4"], "Shift+4");
}

#[test]
fn workspace_shortcut_chords_parse_digit_keys() {
    assert_eq!(
        parse_shortcut("Shift+1"),
        Some(ParsedShortcut {
            ctrl: false,
            alt: false,
            shift: true,
            key: egui::Key::Num1,
        })
    );
    assert_eq!(
        parse_shortcut("Shift+4"),
        Some(ParsedShortcut {
            ctrl: false,
            alt: false,
            shift: true,
            key: egui::Key::Num4,
        })
    );
    assert_eq!(parse_shortcut("Control+c"), parse_shortcut("Ctrl+C"));
    assert!(parse_shortcut("Shift+NotAKey").is_none());

    let descriptors = CommandRegistry::core().all_descriptors();
    for (slot, shortcut) in [
        (1, "Shift+1"),
        (2, "Shift+2"),
        (3, "Shift+3"),
        (4, "Shift+4"),
    ] {
        let descriptor = descriptors
            .iter()
            .find(|descriptor| descriptor.id == format!("workspace.slot.{slot}"))
            .expect("each workspace slot should be a registered command");
        assert_eq!(descriptor.default_shortcut.as_deref(), Some(shortcut));
    }
}

#[test]
fn workspace_digit_shortcut_matches_physical_key_after_shift_changes_symbol() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![1, 2, 3]));
    app.project_identity = Some("bin-a".into());
    app.project_preferences = ProjectPreferences::for_identity("bin-a");
    app.project_preferences.active_workspace_id = 1;
    app.project_preferences.active_workspace_name = "Layout 2".into();
    app.project_preferences.saved_workspace_snapshots = vec![WorkspaceSnapshot {
        id: 0,
        name: "Default".into(),
        state: WorkspaceViewState::default(),
    }];

    let context = egui::Context::default();
    let send_shifted_key =
        |app: &mut TunerApp, logical: egui::Key, physical: egui::Key, chord: &str| {
            let shift = egui::Modifiers {
                shift: true,
                ..Default::default()
            };
            let mut input = egui::RawInput::default();
            input.events = vec![
                egui::Event::ModifiersChanged(shift),
                egui::Event::Key {
                    key: logical,
                    physical_key: Some(physical),
                    pressed: true,
                    repeat: false,
                    modifiers: shift,
                },
            ];
            let mut matched = false;
            context
                .run_ui(input, |ctx| {
                    matched = ctx.input(|input| shortcut_matches(input, chord));
                    app.handle_shortcuts(ctx);
                })
                .drop_without_applying_deltas();

            let mut release = egui::RawInput::default();
            release.events = vec![
                egui::Event::Key {
                    key: logical,
                    physical_key: Some(physical),
                    pressed: false,
                    repeat: false,
                    modifiers: shift,
                },
                egui::Event::ModifiersChanged(egui::Modifiers::NONE),
            ];
            context
                .run_ui(release, |_| {})
                .drop_without_applying_deltas();
            assert!(matched, "{chord} should match its physical number key");
        };

    send_shifted_key(
        &mut app,
        egui::Key::Exclamationmark,
        egui::Key::Num1,
        "Shift+1",
    );
    assert_eq!(app.project_preferences.active_workspace_id, 0);
}

#[test]
fn workspace_slot_shortcut_respects_slot_availability_and_switch_guard() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![1, 2, 3]));
    app.project_identity = Some("bin-a".into());
    app.project_preferences = ProjectPreferences::for_identity("bin-a");
    app.project_preferences.saved_workspace_snapshots = vec![WorkspaceSnapshot {
        id: 4,
        name: "Track".into(),
        state: WorkspaceViewState::default(),
    }];
    let context = egui::Context::default();
    let shift = egui::Modifiers {
        shift: true,
        ..Default::default()
    };

    assert!(app
        .operation_block_reason(BuiltinCommand::WorkspaceSlot(3))
        .is_some());
    assert_eq!(
        app.preferences
            .shortcuts
            .get("workspace.slot.2")
            .map(String::as_str),
        Some("Shift+2")
    );
    assert_eq!(workspace_id_for_slot(&app.project_preferences, 2), Some(4));
    assert_eq!(
        app.operation_block_reason(BuiltinCommand::WorkspaceSlot(2)),
        None
    );
    press_shortcut(&mut app, &context, egui::Key::Num3, shift, "Shift+3");
    assert_eq!(app.project_preferences.active_workspace_id, 0);

    let busy = app.operations.begin(OperationKind::LoadingBin, "busy.bin");
    press_shortcut(&mut app, &context, egui::Key::Num2, shift, "Shift+2");
    assert_eq!(app.project_preferences.active_workspace_id, 0);
    app.operations.cancel(busy, "test transition");
    assert!(app.operations.active().is_none());

    press_shortcut(&mut app, &context, egui::Key::Num2, shift, "Shift+2");
    assert_eq!(app.project_preferences.active_workspace_id, 4);
}

#[test]
fn shortcut_capture_requires_armed_recorder() {
    let mut app = TunerApp::headless();
    let settings_path = temporary_test_path("settings.json");
    app.settings_path = settings_path.clone();
    app.persistence_enabled = true;
    app.open_settings(true);
    app.shortcut_filter = "edit.undo".into();
    let context = egui::Context::default();
    let original = app.preferences.shortcuts["edit.undo"].clone();

    press_shortcut(
        &mut app,
        &context,
        egui::Key::J,
        egui::Modifiers {
            ctrl: true,
            alt: true,
            ..Default::default()
        },
        "Ctrl+Alt+J",
    );
    assert!(app.shortcut_capture_command.is_none());
    assert_eq!(app.preferences.shortcuts["edit.undo"], original);

    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 800.0));
    let mut time = 1.0;
    click_settings_label(&mut app, "Record", &context, screen, &mut time);
    assert_eq!(app.shortcut_capture_command.as_deref(), Some("edit.undo"));

    press_shortcut(
        &mut app,
        &context,
        egui::Key::J,
        egui::Modifiers {
            ctrl: true,
            alt: true,
            ..Default::default()
        },
        "Ctrl+Alt+J",
    );
    assert!(app.shortcut_capture_command.is_none());
    assert_eq!(app.preferences.shortcuts["edit.undo"], "Ctrl+Alt+J");
    app.flush_preferences_if_idle(&context);
    assert_eq!(
        load_preferences(&settings_path).shortcuts["edit.undo"],
        "Ctrl+Alt+J"
    );
    std::fs::remove_file(settings_path).unwrap();
}

#[test]
fn shortcut_recorder_captures_shifted_digit_from_its_physical_key() {
    let mut app = TunerApp::headless();
    app.shortcut_capture_command = Some("workspace.slot.1".into());
    let context = egui::Context::default();
    let shift = egui::Modifiers {
        shift: true,
        ..Default::default()
    };
    let mut input = egui::RawInput::default();
    input.events = vec![
        egui::Event::ModifiersChanged(shift),
        egui::Event::Key {
            key: egui::Key::Exclamationmark,
            physical_key: Some(egui::Key::Num1),
            pressed: true,
            repeat: false,
            modifiers: shift,
        },
    ];

    context
        .run_ui(input, |ctx| app.handle_shortcuts(ctx))
        .drop_without_applying_deltas();

    assert_eq!(app.preferences.shortcuts["workspace.slot.1"], "Shift+1");
    assert!(app.shortcut_capture_command.is_none());
}

#[test]
fn shortcut_capture_rejects_invalid_or_conflicting_chords() {
    let mut app = TunerApp::headless();
    app.open_settings(true);
    app.shortcut_filter = "edit.redo".into();
    app.preferences
        .shortcuts
        .insert("edit.undo".into(), "Ctrl+Alt+J".into());
    let original_redo = app.preferences.shortcuts["edit.redo"].clone();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 800.0));
    let mut time = 1.0;
    click_settings_label(&mut app, "Record", &context, screen, &mut time);
    assert_eq!(app.shortcut_capture_command.as_deref(), Some("edit.redo"));

    let invalid_key = egui::Key::ArrowDown;
    let mut invalid_input = egui::RawInput::default();
    invalid_input.events = vec![
        egui::Event::ModifiersChanged(egui::Modifiers::NONE),
        egui::Event::Key {
            key: invalid_key,
            physical_key: Some(invalid_key),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
    ];
    context
        .run_ui(invalid_input, |ctx| app.handle_shortcuts(ctx))
        .drop_without_applying_deltas();
    assert_eq!(app.preferences.shortcuts["edit.redo"], original_redo);
    assert!(app.shortcut_feedback.is_some());
    assert_eq!(app.shortcut_capture_command.as_deref(), Some("edit.redo"));

    press_shortcut(
        &mut app,
        &context,
        egui::Key::J,
        egui::Modifiers {
            ctrl: true,
            alt: true,
            ..Default::default()
        },
        "Ctrl+Alt+J",
    );
    assert_eq!(app.preferences.shortcuts["edit.redo"], original_redo);
    assert_eq!(app.shortcut_capture_command.as_deref(), Some("edit.redo"));
    let feedback = app
        .shortcut_feedback
        .clone()
        .expect("conflict should be explained");
    assert!(feedback.contains("Undo"));
    let output = render_utility_windows_frame(&mut app, &context, screen, time + 0.2);
    let feedback_visible = rendered_text_rect(&output, &feedback).is_some();
    output.drop_without_applying_deltas();
    assert!(feedback_visible);
}

#[test]
fn shortcut_reset_clears_override_to_default() {
    let mut app = TunerApp::headless();
    let settings_path = temporary_test_path("settings.json");
    app.settings_path = settings_path.clone();
    app.persistence_enabled = true;
    app.open_settings(true);
    app.shortcut_filter = "edit.undo".into();
    app.preferences
        .shortcuts
        .insert("edit.undo".into(), "Alt+U".into());
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 800.0));
    let mut time = 1.0;

    click_settings_label(&mut app, "Reset", &context, screen, &mut time);

    assert!(!app.preferences.shortcuts.contains_key("edit.undo"));
    let descriptor = app
        .registry
        .all_descriptors()
        .into_iter()
        .find(|descriptor| descriptor.id == "edit.undo")
        .unwrap();
    assert_eq!(app.shortcut_for(&descriptor), Some("Ctrl+Z"));
    assert!(!load_preferences(&settings_path)
        .shortcuts
        .contains_key("edit.undo"));
    std::fs::remove_file(settings_path).unwrap();
}

#[test]
fn shortcut_clear_sets_explicit_unbound_override() {
    let mut app = TunerApp::headless();
    let settings_path = temporary_test_path("settings.json");
    app.settings_path = settings_path.clone();
    app.persistence_enabled = true;
    app.open_settings(true);
    app.shortcut_filter = "edit.undo".into();
    app.preferences
        .shortcuts
        .insert("edit.undo".into(), "Alt+U".into());
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 800.0));
    let mut time = 1.0;

    click_settings_label(&mut app, "Clear", &context, screen, &mut time);

    assert_eq!(
        app.preferences
            .shortcuts
            .get("edit.undo")
            .map(String::as_str),
        Some("")
    );
    let descriptor = app
        .registry
        .all_descriptors()
        .into_iter()
        .find(|descriptor| descriptor.id == "edit.undo")
        .unwrap();
    assert_eq!(app.shortcut_for(&descriptor), None);
    assert_eq!(load_preferences(&settings_path).shortcuts["edit.undo"], "");
    std::fs::remove_file(settings_path).unwrap();
}

#[test]
fn shortcut_editor_lists_every_command_palette_action() {
    let mut app = TunerApp::headless();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_600.0, 5_000.0));
    app.tool_window_bounds = Some(screen);
    app.project_preferences.utility_window_geometry.insert(
        "settings".into(),
        window_geometry::WindowGeometryMemory {
            x: 20,
            y: 20,
            width: 1_400,
            height: 4_800,
        },
    );
    app.open_settings(true);
    let descriptors = app.registry.all_descriptors();
    let context = egui::Context::default();
    let mut time = 1.0;
    let mut output = None;
    for _ in 0..3 {
        let frame = render_utility_windows_frame(&mut app, &context, screen, time);
        if rendered_text_rect(&frame, "Record").is_some() {
            output = Some(frame);
            break;
        }
        frame.drop_without_applying_deltas();
        time += 0.1;
    }
    let output = output.expect("shortcut section should scroll into view");
    let listed_count = descriptors
        .iter()
        .filter(|descriptor| rendered_text_rect(&output, &descriptor.label).is_some())
        .count();
    let slot_mapping_visible = rendered_text_rect(&output, "Shift+1 → Default").is_some();
    output.drop_without_applying_deltas();

    assert_eq!(listed_count, descriptors.len());
    assert!(slot_mapping_visible);
}

#[test]
fn global_preference_edits_preserve_saved_workspace_snapshots_and_document_identity() {
    let settings_path = temporary_test_path("settings.json");
    let bin_path = PathBuf::from("project.bin");
    let xdf_path = PathBuf::from("project.xdf");
    let mut app = TunerApp::headless();
    app.settings_path = settings_path.clone();
    app.persistence_enabled = true;
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![1, 2, 3])),
        Some(XdfDocument::parse(TWO_TABLE_XDF).unwrap()),
        Some(bin_path.clone()),
        Some(xdf_path),
    );
    let identity = project_identity_for_path(&bin_path);
    app.project_identity = Some(identity.clone());
    app.project_preferences = ProjectPreferences::for_identity(&identity);
    let snapshot = WorkspaceSnapshot {
        id: 1,
        name: "Saved layout".into(),
        state: WorkspaceViewState::default(),
    };
    app.project_preferences
        .saved_workspace_snapshots
        .push(snapshot.clone());
    let original_bytes = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let original_revision = app.workspace.data_revision();
    let original_xdf = app
        .workspace
        .xdf
        .as_ref()
        .unwrap()
        .normalized_fingerprint
        .clone();

    app.preferences.sweep_enabled = false;
    app.preferences
        .shortcuts
        .insert("edit.undo".into(), "Alt+U".into());
    app.persist_preferences();

    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), original_bytes);
    assert_eq!(app.workspace.data_revision(), original_revision);
    assert_eq!(
        app.workspace.xdf.as_ref().unwrap().normalized_fingerprint,
        original_xdf
    );
    assert_eq!(
        app.project_preferences.saved_workspace_snapshots,
        vec![snapshot.clone()]
    );
    let project =
        load_project_preferences(&project_settings_path(&settings_path, &identity), &identity);
    assert_eq!(project.saved_workspace_snapshots, vec![snapshot]);
    let preferences = load_preferences(&settings_path);
    assert!(!preferences.sweep_enabled);
    assert_eq!(preferences.shortcuts["edit.undo"], "Alt+U");

    let project_path = project_settings_path(&settings_path, &identity);
    std::fs::remove_file(settings_path).unwrap();
    std::fs::remove_file(project_path).unwrap();
}

#[test]
fn legacy_main_settings_default_to_empty_global_workspace_backgrounds() {
    let restored = preferences_from_json(r#"{"version":2}"#);
    let serialized: Value = serde_json::from_str(&preferences_to_json(&restored).unwrap()).unwrap();

    assert_eq!(restored.version, SETTINGS_VERSION);
    assert_eq!(serialized["workspace_backgrounds"], json!({}));
}

#[test]
fn global_workspace_backgrounds_round_trip_and_sanitize_by_id() {
    let restored = preferences_from_json(
        r#"{
            "version": 2,
            "workspace_backgrounds": {
                "1": {"image":"aa.png", "placement":"Fill", "opacity_percent":42},
                "2": {"image":"../escape.png", "placement":"Fit", "opacity_percent":255}
            }
        }"#,
    );
    let serialized: Value = serde_json::from_str(&preferences_to_json(&restored).unwrap()).unwrap();

    assert_eq!(
        serialized["workspace_backgrounds"]["1"],
        json!({"image":"aa.png", "placement":"Fill", "opacity_percent":42})
    );
    assert_eq!(
        serialized["workspace_backgrounds"]["2"],
        json!({"image":null, "placement":"Fit", "opacity_percent":100})
    );
}

#[test]
fn global_backgrounds_follow_workspace_id_across_projects_not_project_manifests() {
    let root = temporary_test_path("global-workspace-backgrounds");
    std::fs::create_dir_all(&root).unwrap();
    let settings_path = root.join("settings.json");
    let shared = WorkspaceBackgroundSettings {
        image: Some("shared.png".into()),
        placement: WorkspaceBackgroundPlacement::Fill,
        opacity_percent: 53,
    };
    let distinct = WorkspaceBackgroundSettings {
        image: Some("other.png".into()),
        placement: WorkspaceBackgroundPlacement::Fit,
        opacity_percent: 88,
    };
    let mut preferences = AppPreferences::default();
    preferences.workspace_backgrounds.insert(0, shared.clone());
    preferences
        .workspace_backgrounds
        .insert(1, distinct.clone());
    save_preferences(&settings_path, &preferences).unwrap();
    let mut app = TunerApp::headless();
    app.settings_path = settings_path.clone();
    app.preferences = load_preferences(&settings_path);

    app.activate_project_for_path(Path::new("I:/ProjectA/vehicle.bin"));
    app.project_preferences.active_workspace_id = 0;
    let first_project_background = app.preferences.workspace_backgrounds[&0].clone();
    app.activate_project_for_path(Path::new("I:/ProjectB/vehicle.bin"));
    app.project_preferences.active_workspace_id = 0;
    let second_project_background = app.preferences.workspace_backgrounds[&0].clone();
    app.project_preferences.active_workspace_id = 1;
    let other_workspace_background = app.preferences.workspace_backgrounds[&1].clone();

    app.project_preferences = ProjectPreferences::for_identity("delete-layout-test");
    app.project_preferences.active_workspace_id = 0;
    app.workspace.bin = Some(BinDocument::from_bytes(vec![0]));
    let new_workspace_id = app.create_workspace_snapshot("Second layout").unwrap();
    app.delete_workspace_snapshot(0).unwrap();
    let background_after_workspace_delete = app.preferences.workspace_backgrounds[&0].clone();

    let manifest_bin = root.join("manifest.bin");
    let manifest_identity = project_identity_for_path(&manifest_bin);
    let manifest = TunerProjectFile {
        format_version: project_file::FORMAT_VERSION,
        bin_path: manifest_bin,
        xdf_path: None,
        preferences: ProjectPreferences::for_identity(manifest_identity),
    };
    let manifest_value: Value = serde_json::from_str(&manifest.to_json().unwrap()).unwrap();
    let reopened_settings = load_preferences(&settings_path);
    let _ = std::fs::remove_dir_all(root);

    assert_eq!(first_project_background, shared);
    assert_eq!(second_project_background, shared);
    assert_eq!(other_workspace_background, distinct);
    assert_eq!(new_workspace_id, 1);
    assert_eq!(background_after_workspace_delete, shared);
    assert!(manifest_value.get("workspace_backgrounds").is_none());
    assert!(manifest_value["preferences"]
        .get("workspace_backgrounds")
        .is_none());
    assert_eq!(reopened_settings.workspace_backgrounds[&0], shared);
    assert_eq!(reopened_settings.workspace_backgrounds[&1], distinct);
}

#[test]
fn workspace_background_context_menu_opens_on_blank_canvas_without_a_bin() {
    let mut app = TunerApp::headless();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0));
    let point = egui::pos2(800.0, 560.0);
    render_workspace_canvas_frame(&mut app, &context, screen, Vec::new())
        .drop_without_applying_deltas();
    let mut menu_response_id = None;
    let mut press_owned = false;
    for pressed in [true, false] {
        let (output, response_id) = render_workspace_canvas_frame_with_response_id(
            &mut app,
            &context,
            screen,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        menu_response_id = Some(response_id);
        if pressed {
            press_owned = context
                .read_response(response_id)
                .is_some_and(|response| response.is_pointer_button_down_on());
        }
        output.drop_without_applying_deltas();
    }
    let menu_response = context.read_response(menu_response_id.unwrap()).unwrap();
    let canvas_received_secondary_click = menu_response.secondary_clicked();
    let canvas_hovered = menu_response.hovered();
    let pointer_clicked_secondary =
        context.input(|input| input.pointer.button_clicked(egui::PointerButton::Secondary));
    let canvas_response_rect = menu_response.rect;
    let output = render_workspace_canvas_frame(&mut app, &context, screen, Vec::new());
    let actions_visible = ["Set background image…", "Fit", "Fill", "Opacity"]
        .map(|label| rendered_text_rect(&output, label).is_some());
    let remove_visible = rendered_text_rect(&output, "Remove background image").is_some();
    output.drop_without_applying_deltas();

    assert!(
        canvas_received_secondary_click,
        "canvas response {:?} (hovered={canvas_hovered}, press_owned={press_owned}, pointer_clicked={pointer_clicked_secondary}) did not receive right-click at {point:?}",
        canvas_response_rect
    );
    assert!(actions_visible.into_iter().all(|visible| visible));
    assert!(!remove_visible);
}

#[test]
fn missing_workspace_background_stays_replaceable_and_removable_from_canvas_menu() {
    let root = temporary_test_path("workspace-background-missing");
    std::fs::create_dir_all(&root).unwrap();
    let mut app = TunerApp::headless();
    app.settings_path = root.join("settings.json");
    app.preferences.workspace_backgrounds.insert(
        0,
        WorkspaceBackgroundSettings {
            image: Some("missing.png".into()),
            ..WorkspaceBackgroundSettings::default()
        },
    );
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0));
    let point = egui::pos2(800.0, 560.0);
    render_workspace_canvas_frame(&mut app, &context, screen, Vec::new())
        .drop_without_applying_deltas();
    for pressed in [true, false] {
        let output = render_workspace_canvas_frame(
            &mut app,
            &context,
            screen,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        output.drop_without_applying_deltas();
    }
    let output = render_workspace_canvas_frame(&mut app, &context, screen, Vec::new());
    let unavailable = rendered_text_rect(&output, "Background image unavailable").is_some();
    let replace = rendered_text_rect(&output, "Set background image…").is_some();
    let remove = rendered_text_rect(&output, "Remove background image").is_some();
    output.drop_without_applying_deltas();
    let _ = std::fs::remove_dir_all(root);

    assert!(unavailable);
    assert!(replace);
    assert!(remove);
}

#[test]
fn workspace_background_menu_persists_the_fill_placement_choice() {
    let root = temporary_test_path("workspace-background-menu-settings");
    std::fs::create_dir_all(&root).unwrap();
    let settings_path = root.join("settings.json");
    let source_path = root.join("source.png");
    image::RgbaImage::from_pixel(2, 2, image::Rgba([80, 120, 160, 255]))
        .save(&source_path)
        .unwrap();
    let source_bytes = std::fs::read(&source_path).unwrap();
    let image_reference =
        workspace_background::copy_background_asset(&source_path, &settings_path).unwrap();
    let managed_image =
        workspace_background::resolve_asset_path(&settings_path, &image_reference).unwrap();
    let mut app = TunerApp::headless();
    app.persistence_enabled = true;
    app.settings_path = settings_path.clone();
    app.preferences.workspace_backgrounds.insert(
        0,
        WorkspaceBackgroundSettings {
            image: Some(image_reference),
            ..WorkspaceBackgroundSettings::default()
        },
    );
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0));
    let point = egui::pos2(800.0, 560.0);
    render_workspace_canvas_frame(&mut app, &context, screen, Vec::new())
        .drop_without_applying_deltas();
    for pressed in [true, false] {
        render_workspace_canvas_frame(
            &mut app,
            &context,
            screen,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        )
        .drop_without_applying_deltas();
    }
    let menu = render_workspace_canvas_frame(&mut app, &context, screen, Vec::new());
    let fill_center = rendered_text_rect(&menu, "Fill").map(|rect| rect.center());
    menu.drop_without_applying_deltas();
    let fill_center = fill_center.expect("Fill option should be visible in the canvas menu");
    for pressed in [true, false] {
        render_workspace_canvas_frame(
            &mut app,
            &context,
            screen,
            vec![
                egui::Event::PointerMoved(fill_center),
                egui::Event::PointerButton {
                    pos: fill_center,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        )
        .drop_without_applying_deltas();
    }
    let after_fill = app.preferences.workspace_backgrounds.get(&0).cloned();
    render_workspace_canvas_frame(&mut app, &context, screen, Vec::new())
        .drop_without_applying_deltas();
    let blank_canvas_point = egui::pos2(800.0, 560.0);
    for pressed in [true, false] {
        render_workspace_canvas_frame(
            &mut app,
            &context,
            screen,
            vec![
                egui::Event::PointerMoved(blank_canvas_point),
                egui::Event::PointerButton {
                    pos: blank_canvas_point,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        )
        .drop_without_applying_deltas();
    }
    let remove_menu = render_workspace_canvas_frame(&mut app, &context, screen, Vec::new());
    let remove_center =
        rendered_text_rect(&remove_menu, "Remove background image").map(|rect| rect.center());
    let replace_visible = rendered_text_rect(&remove_menu, "Set background image…").is_some();
    remove_menu.drop_without_applying_deltas();
    let remove_visible = remove_center.is_some();
    if let Some(remove_center) = remove_center {
        for pressed in [true, false] {
            render_workspace_canvas_frame(
                &mut app,
                &context,
                screen,
                vec![
                    egui::Event::PointerMoved(remove_center),
                    egui::Event::PointerButton {
                        pos: remove_center,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            )
            .drop_without_applying_deltas();
        }
    }
    let in_memory = app.preferences.workspace_backgrounds[&0].clone();
    let persisted = load_preferences(&settings_path).workspace_backgrounds[&0].clone();
    let managed_image_still_exists = std::fs::read(&managed_image).ok();
    let _ = std::fs::remove_dir_all(root);

    assert_eq!(in_memory.placement, WorkspaceBackgroundPlacement::Fill);
    assert_eq!(persisted.placement, WorkspaceBackgroundPlacement::Fill);
    assert!(after_fill
        .as_ref()
        .is_some_and(|settings| settings.image.is_some()));
    assert!(replace_visible);
    assert!(remove_visible);
    assert_eq!(in_memory.image, None);
    assert_eq!(persisted.image, None);
    assert_eq!(
        managed_image_still_exists.as_deref(),
        Some(source_bytes.as_slice())
    );
}

#[test]
fn selecting_global_workspace_background_needs_no_bin_and_does_not_save_project_state() {
    let root = temporary_test_path("workspace-background-assets");
    std::fs::create_dir_all(&root).unwrap();
    let settings_path = root.join("settings.json");
    let source_path = root.join("original.png");
    image::RgbaImage::from_pixel(2, 2, image::Rgba([80, 120, 160, 255]))
        .save(&source_path)
        .unwrap();
    let _no_bin_source_bytes = std::fs::read(&source_path).unwrap();
    let project_identity = "bin-project-sentinel";
    let project_settings = project_settings_path(&settings_path, project_identity);
    std::fs::create_dir_all(project_settings.parent().unwrap()).unwrap();
    std::fs::write(&project_settings, b"project state must remain untouched").unwrap();
    let mut app = TunerApp::headless();
    app.persistence_enabled = true;
    app.settings_path = settings_path.clone();
    app.project_identity = Some(project_identity.into());
    app.project_preferences = ProjectPreferences::for_identity(project_identity);
    app.project_preferences.active_workspace_id = 0;
    app.preferences_dirty = true;

    let no_bin_result = app.set_workspace_background_image(&source_path);
    let unrelated_preferences_remain_dirty = app.preferences_dirty;
    let bin = BinDocument::from_bytes(vec![1, 2, 3, 4]);
    let xdf = XdfDocument::parse(COLUMN_MAJOR_XDF).unwrap();
    app.workspace.set_documents(
        Some(bin),
        Some(xdf),
        Some(root.join("tune.bin")),
        Some(root.join("defs.xdf")),
    );
    let bin_before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let xdf_hash_before = app.workspace.xdf.as_ref().unwrap().exact_sha256.clone();
    let revision_before = app.workspace.data_revision();
    let second_source_path = root.join("second.png");
    image::RgbaImage::from_pixel(2, 2, image::Rgba([160, 120, 80, 255]))
        .save(&second_source_path)
        .unwrap();
    let source_bytes = std::fs::read(&second_source_path).unwrap();
    let loaded_bin_result = app.set_workspace_background_image(&second_source_path);
    let setting = app.preferences.workspace_backgrounds.get(&0).cloned();
    let saved_preferences = load_preferences(&settings_path);
    let project_bytes = std::fs::read(&project_settings).unwrap();
    let copied_bytes = setting
        .as_ref()
        .and_then(|setting| setting.image.as_deref())
        .and_then(|reference| {
            workspace_background::resolve_asset_path(&settings_path, reference).ok()
        })
        .and_then(|path| std::fs::read(path).ok());
    let _ = std::fs::remove_dir_all(root);

    assert!(
        no_bin_result.is_ok(),
        "a BIN is not required: {no_bin_result:?}"
    );
    assert!(loaded_bin_result.is_ok());
    assert!(setting
        .as_ref()
        .is_some_and(|setting| setting.image.is_some()));
    assert_eq!(
        saved_preferences.workspace_backgrounds,
        app.preferences.workspace_backgrounds
    );
    assert!(unrelated_preferences_remain_dirty);
    assert_eq!(copied_bytes.as_deref(), Some(source_bytes.as_slice()));
    assert_eq!(project_bytes, b"project state must remain untouched");
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), bin_before);
    assert_eq!(
        app.workspace.xdf.as_ref().unwrap().exact_sha256,
        xdf_hash_before
    );
    assert_eq!(app.workspace.data_revision(), revision_before);
}

#[test]
fn workspace_canvas_paints_the_background_for_the_active_numeric_id() {
    let root = temporary_test_path("workspace-background-root");
    std::fs::create_dir_all(&root).unwrap();
    let settings_path = root.join("settings.json");
    let source_path = root.join("source.png");
    image::RgbaImage::from_pixel(2, 2, image::Rgba([80, 120, 160, 255]))
        .save(&source_path)
        .unwrap();
    let reference =
        workspace_background::copy_background_asset(&source_path, &settings_path).unwrap();
    let mut app = TunerApp::headless();
    app.settings_path = settings_path;
    app.project_preferences.active_workspace_id = 7;
    app.preferences.workspace_backgrounds.insert(
        7,
        WorkspaceBackgroundSettings {
            image: Some(reference),
            ..WorkspaceBackgroundSettings::default()
        },
    );

    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0));
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(screen);
    let output = context.run_ui(input, |ctx| {
        let frame_context = ctx.clone();
        egui::CentralPanel::default().show(ctx, |ui| {
            app.show_workspace_canvas(ui, &frame_context);
        });
    });
    fn count_meshes(shape: &egui::Shape) -> usize {
        match shape {
            egui::Shape::Mesh(_) => 1,
            egui::Shape::Vec(shapes) => shapes.iter().map(count_meshes).sum(),
            _ => 0,
        }
    }
    let background_mesh_count = output
        .shapes
        .iter()
        .map(|shape| count_meshes(&shape.shape))
        .sum::<usize>();
    output.drop_without_applying_deltas();

    assert_eq!(background_mesh_count, 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn changing_cell_selection_and_table_position_does_not_reload_canvas_background() {
    let root = temporary_test_path("workspace-background-table-cache");
    std::fs::create_dir_all(&root).unwrap();
    let settings_path = root.join("settings.json");
    let source_path = root.join("background.png");
    image::RgbaImage::from_pixel(2, 2, image::Rgba([60, 90, 120, 255]))
        .save(&source_path)
        .unwrap();
    let reference =
        workspace_background::copy_background_asset(&source_path, &settings_path).unwrap();
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.settings_path = settings_path;
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    app.preferences.workspace_backgrounds.insert(
        0,
        WorkspaceBackgroundSettings {
            image: Some(reference),
            ..WorkspaceBackgroundSettings::default()
        },
    );
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    app.open_tables[0].memory = TableWindowMemory {
        x: 120,
        y: 100,
        width: 760,
        height: 520,
        fit_to_content: Some(false),
        ..TableWindowMemory::default()
    };
    let window_id = egui::Id::new(("table-window", "no-project", key.as_str()));
    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let run_frame = |app: &mut TunerApp| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        let mut canvas_rect = canvas;
        let output = context.run_ui(input, |ctx| {
            let frame_context = ctx.clone();
            egui::CentralPanel::default().show(ctx, |ui| {
                canvas_rect = app.show_workspace_canvas(ui, &frame_context);
            });
            app.show_table_window(ctx, canvas_rect, &key);
        });
        output.drop_without_applying_deltas();
    };
    run_frame(&mut app);
    run_frame(&mut app);
    let first_rect = context
        .memory(|memory| memory.area_rect(window_id))
        .unwrap();
    let cache_counts = workspace_background::cache_work_counts(&app.workspace_background_cache);
    app.workspace.select_cell_range(0, 0, 0, 1).unwrap();
    app.open_tables[0].memory.x += 80;
    app.open_tables[0].memory.y += 30;
    app.open_tables[0].geometry_request = true;
    app.table_geometry_guards.remove(&key);
    run_frame(&mut app);
    let moved_rect = context
        .memory(|memory| memory.area_rect(window_id))
        .unwrap();
    let loads_after_layout_change =
        workspace_background::cache_work_counts(&app.workspace_background_cache);
    let _ = std::fs::remove_dir_all(root);

    assert_ne!(moved_rect.min, first_rect.min);
    assert_eq!(loads_after_layout_change, cache_counts);
}

#[test]
fn recent_bin_history_is_case_insensitive_and_bounded() {
    let mut preferences = AppPreferences::default();
    preferences.recent_bins = vec![
        PathBuf::from("C:/Tuning/first.bin"),
        PathBuf::from("c:/tuning/FIRST.bin"),
    ];
    for index in 0..(RECENT_BIN_LIMIT + 2) {
        preferences
            .recent_bins
            .push(PathBuf::from(format!("C:/Tuning/{index}.bin")));
    }

    let decoded = preferences_from_json(&preferences_to_json(&preferences).unwrap());

    assert!(decoded.recent_bins.len() <= RECENT_BIN_LIMIT);
    assert_eq!(
        decoded
            .recent_bins
            .iter()
            .filter(|path| normalized_path_text(path) == "c:/tuning/first.bin")
            .count(),
        1
    );
}

#[test]
fn recent_xdf_history_is_case_insensitive_and_bounded() {
    let mut preferences = AppPreferences::default();
    preferences.recent_xdfs = vec![
        PathBuf::from("C:/Tuning/first.xdf"),
        PathBuf::from("c:/tuning/FIRST.xdf"),
    ];
    for index in 0..(RECENT_BIN_LIMIT + 2) {
        preferences
            .recent_xdfs
            .push(PathBuf::from(format!("C:/Tuning/{index}.xdf")));
    }

    let decoded = preferences_from_json(&preferences_to_json(&preferences).unwrap());

    assert!(decoded.recent_xdfs.len() <= RECENT_BIN_LIMIT);
    assert_eq!(
        decoded
            .recent_xdfs
            .iter()
            .filter(|path| normalized_path_text(path) == "c:/tuning/first.xdf")
            .count(),
        1
    );
}

#[test]
fn ui_ipc_state_exposes_recent_xdf_paths() {
    let mut app = TunerApp::headless();
    app.preferences.recent_xdfs = vec![PathBuf::from("C:/Tuning/recent.xdf")];

    assert_eq!(
        app.ui_ipc_state()["recent_xdfs"],
        json!(["C:/Tuning/recent.xdf"])
    );
}

#[test]
fn successful_bin_load_adds_the_path_to_recent_history() {
    let mut app = TunerApp::headless();
    let operation = app
        .operations
        .begin(OperationKind::LoadingBin, "recent.bin");
    let mut result = OperationResult::bin(operation, BinDocument::from_bytes(vec![1, 2, 3]));
    result.subject = "C:/Tuning/recent.bin".to_string();

    app.install_operation_result(result);

    assert_eq!(
        app.preferences
            .recent_bins
            .first()
            .map(|path| normalized_path_text(path)),
        Some("c:/tuning/recent.bin".to_string())
    );
}

#[test]
fn startup_restore_reads_the_latest_bin_and_its_project_xdf() {
    let settings_path = temporary_test_path("settings.json");
    let bin_path = temporary_test_path("bin");
    let xdf_path = temporary_test_path("xdf");
    std::fs::write(&bin_path, [1, 2, 3]).unwrap();
    std::fs::write(&xdf_path, COLUMN_MAJOR_XDF).unwrap();

    let mut preferences = AppPreferences::default();
    preferences.recent_bins = vec![bin_path.clone()];
    save_preferences(&settings_path, &preferences).unwrap();
    let identity = project_identity_for_path(&bin_path);
    let mut project = ProjectPreferences::for_identity(&identity);
    project.last_xdf_path = Some(xdf_path.clone());
    let project_path = project_settings_path(&settings_path, &identity);
    save_project_preferences(&project_path, &project).unwrap();

    let pending = pending_project_restore(&settings_path, &load_preferences(&settings_path))
        .expect("the latest BIN should create a startup restore prompt");
    assert_eq!(pending.bin_path, bin_path);
    assert!(pending.bin_available);
    assert_eq!(pending.xdf_path, Some(xdf_path.clone()));
    assert!(pending.xdf_available);

    std::fs::remove_file(settings_path).unwrap();
    std::fs::remove_file(project_path).unwrap();
    std::fs::remove_file(bin_path).unwrap();
    std::fs::remove_file(xdf_path).unwrap();
}

#[test]
fn dismissing_startup_restore_leaves_an_empty_workspace() {
    let mut app = TunerApp::headless();
    app.pending_project_restore = Some(PendingProjectRestore {
        bin_path: PathBuf::from("last.bin"),
        xdf_path: Some(PathBuf::from("last.xdf")),
        bin_available: true,
        xdf_available: true,
    });

    app.start_last_project_restore(StartupRestoreAction::No)
        .unwrap();

    assert!(app.pending_project_restore.is_none());
    assert!(app.pending_startup_load.is_none());
    assert!(app.workspace.bin.is_none());
    assert!(app.workspace.xdf.is_none());
    assert_eq!(
        app.workspace.status.text,
        "Started with an empty workspace."
    );
}

#[test]
fn startup_restore_keeps_bin_only_choice_from_triggering_xdf_prompt() {
    let mut app = TunerApp::headless();
    let bin_path = temporary_test_path("bin");
    app.pending_project_restore = Some(PendingProjectRestore {
        bin_path: bin_path.clone(),
        xdf_path: Some(PathBuf::from("last.xdf")),
        bin_available: true,
        xdf_available: true,
    });

    app.start_last_project_restore(StartupRestoreAction::BinOnly)
        .unwrap();
    let loading_bin = app.operations.active().unwrap().id;
    assert_eq!(app.pending_startup_load, Some(StartupRestoreLoad::BinOnly));
    app.operations.cancel(loading_bin, "test transition");

    let identity = project_identity_for_path(&bin_path);
    let restore = app
        .operations
        .begin(OperationKind::RestoringWorkspace, identity.clone());
    let mut project = ProjectPreferences::for_identity(&identity);
    project.last_xdf_path = Some(PathBuf::from("last.xdf"));
    project.open_table_keys = vec!["old-xdf|old-table".into()];
    app.install_operation_result(OperationResult::restored(restore, identity, project));

    assert!(app.pending_xdf_restore.is_none());
    assert!(app.pending_startup_load.is_none());
    assert!(app.operations.active().is_none());
    assert!(app.open_tables.is_empty());
    assert_eq!(
        app.project_preferences.open_table_keys,
        ["old-xdf|old-table"]
    );
}

fn app_with_project_fixture() -> TunerApp {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    app.project_identity = Some("bin-a".into());
    app.project_preferences = ProjectPreferences::for_identity("bin-a");
    app
}

#[test]
fn switching_workspace_restores_layout_without_changing_bin_or_xdf() {
    let mut app = app_with_project_fixture();
    app.project_preferences.utility_window_geometry.insert(
        "debug_report".into(),
        window_geometry::WindowGeometryMemory {
            x: 120,
            y: 80,
            width: 900,
            height: 640,
        },
    );
    app.compare.window.x = 300;
    let before_bin = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let before_xdf = app
        .workspace
        .xdf
        .as_ref()
        .unwrap()
        .normalized_fingerprint
        .clone();
    let before_revision = app.workspace.data_revision();
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    app.workspace.select_cell_range(0, 0, 1, 1).unwrap();
    let key = app.open_tables[0].key.clone();
    let original = app.project_preferences.active_workspace_id;
    let second = app.create_workspace_snapshot("Diagnostics").unwrap();
    assert_eq!(app.project_preferences.saved_workspace_snapshots.len(), 1);
    assert!(!app
        .project_preferences
        .saved_workspace_snapshots
        .iter()
        .any(|snapshot| snapshot.id == second));

    app.project_preferences.utility_window_geometry.insert(
        "debug_report".into(),
        window_geometry::WindowGeometryMemory {
            x: 720,
            y: 120,
            width: 840,
            height: 600,
        },
    );
    app.compare.window.x = 420;

    app.close_table(&key);
    app.switch_workspace_snapshot(original).unwrap();

    assert_eq!(app.open_tables.len(), 1);
    assert_eq!(app.project_preferences.active_workspace_id, original);
    assert_eq!(
        app.workspace.selected_semantic_id.as_deref(),
        Some(semantic_id.as_str())
    );
    assert_eq!(
        app.workspace.selected_cells,
        Some(CellSelection::new((0, 0), (1, 1)))
    );
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        before_bin.as_slice()
    );
    assert_eq!(
        app.workspace.xdf.as_ref().unwrap().normalized_fingerprint,
        before_xdf
    );
    assert_eq!(app.workspace.data_revision(), before_revision);
    assert_eq!(
        app.project_preferences.utility_window_geometry["debug_report"].x,
        120
    );
    assert_eq!(app.compare.window.x, 300);
    assert_ne!(second, original);
}

#[test]
fn compare_workspace_switch_restores_saved_rect_over_reused_egui_state() {
    let mut app = app_with_project_fixture();
    app.open_compare_with_test_documents();
    app.compare.window_open = true;
    app.project_preferences
        .dock_state
        .open_tool(WindowId::Compare);
    app.project_preferences
        .dock_state
        .restore(&WindowId::Compare);
    let original_id = app.project_preferences.active_workspace_id;
    let original_rect = CompareWindowMemory {
        x: 100,
        y: 90,
        width: 620,
        height: 430,
        position_saved: true,
    };
    app.compare.window = original_rect.clone();
    app.project_preferences.compare_window = original_rect.clone();
    app.create_workspace_snapshot("Compare other").unwrap();

    app.compare.window = CompareWindowMemory {
        x: 650,
        y: 130,
        width: 500,
        height: 390,
        position_saved: true,
    };
    app.project_preferences.compare_window = app.compare.window.clone();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 800.0));
    app.tool_window_bounds = Some(screen);
    let draw = |app: &mut TunerApp, context: &egui::Context, time: f64| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        context
            .run_ui(input, |ctx| app.show_compare_window(ctx))
            .drop_without_applying_deltas();
    };
    draw(&mut app, &context, 1.0);
    let window_id = egui::Id::new(("compare-window", app.project_scope()));
    let other_rect = context
        .memory(|memory| memory.area_rect(window_id))
        .expect("Compare should render in the second workspace");

    app.switch_workspace_snapshot(original_id).unwrap();
    assert_eq!(app.compare.window.x, original_rect.x);
    draw(&mut app, &context, 1.1);
    assert!(!app.compare.geometry_request);
    let restored_rect = context
        .memory(|memory| memory.area_rect(window_id))
        .expect("Compare should remain open after restoring the first workspace");

    assert_eq!(restored_rect.min.x.round() as i32, original_rect.x);
    assert_eq!(restored_rect.min.y.round() as i32, original_rect.y);
    assert_ne!(restored_rect.min, other_rect.min);
}

#[test]
fn workspace_switch_restores_each_manual_table_rect_with_reused_egui_window() {
    fn draw_frame(
        app: &mut TunerApp,
        context: &egui::Context,
        canvas: egui::Rect,
        key: &str,
        time: &mut f64,
    ) {
        *time += 0.1;
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        input.time = Some(*time);
        context
            .run_ui(input, |ctx| app.show_table_window(ctx, canvas, key))
            .drop_without_applying_deltas();
    }

    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0; 144])),
        Some(XdfDocument::parse(LARGE_TABLE_XDF).unwrap()),
        None,
        None,
    );
    app.project_identity = Some("bin-a".into());
    app.project_preferences = ProjectPreferences::for_identity("bin-a");
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    let window_id = egui::Id::new(("table-window", "bin-a", key.as_str()));
    let workspace_a = app.project_preferences.active_workspace_id;

    let rect_a = TableWindowMemory {
        x: 100,
        y: 80,
        width: 900,
        height: 600,
        zoom_percent: 110,
        fit_to_content: Some(false),
        position_saved: true,
        ..TableWindowMemory::default()
    };
    app.open_tables[0].memory = rect_a.clone();
    app.open_tables[0].geometry_request = true;

    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_400.0, 900.0));
    let mut time = 0.0;
    draw_frame(&mut app, &context, canvas, &key, &mut time);
    draw_frame(&mut app, &context, canvas, &key, &mut time);

    let workspace_b = app.create_workspace_snapshot("Compact").unwrap();
    let rect_b = TableWindowMemory {
        x: 180,
        y: 120,
        width: 640,
        height: 400,
        zoom_percent: 90,
        fit_to_content: Some(false),
        position_saved: true,
        ..TableWindowMemory::default()
    };
    app.open_tables[0].memory = rect_b.clone();
    app.open_tables[0].geometry_request = true;
    assert!(app.open_tables[0].geometry_request);
    assert_eq!(app.project_scope(), "bin-a");
    draw_frame(&mut app, &context, canvas, &key, &mut time);
    let first_b = context
        .memory(|memory| memory.area_rect(window_id))
        .expect("workspace B table should retain an egui window");
    assert_eq!((first_b.left(), first_b.top()), (180.0, 120.0));
    draw_frame(&mut app, &context, canvas, &key, &mut time);
    let shown_b = context
        .memory(|memory| memory.area_rect(window_id))
        .expect("workspace B table should retain an egui window");
    assert_eq!((shown_b.width(), shown_b.height()), (640.0, 400.0));
    assert_eq!((shown_b.left(), shown_b.top()), (180.0, 120.0));
    assert_eq!(
        (app.open_tables[0].memory.x, app.open_tables[0].memory.y),
        (180, 120)
    );

    app.switch_workspace_snapshot(workspace_a).unwrap();
    assert_eq!(
        (app.open_tables[0].memory.x, app.open_tables[0].memory.y),
        (100, 80)
    );
    assert_eq!(app.open_tables[0].memory.width, 900);
    assert_eq!(app.open_tables[0].memory.height, 600);
    assert_eq!(app.open_tables[0].memory.fit_to_content, Some(false));
    draw_frame(&mut app, &context, canvas, &key, &mut time);
    let restored_a = context
        .memory(|memory| memory.area_rect(window_id))
        .expect("workspace A table window should have geometry");
    assert_eq!((restored_a.left(), restored_a.top()), (100.0, 80.0));
    assert_eq!((restored_a.width(), restored_a.height()), (900.0, 600.0));

    app.switch_workspace_snapshot(workspace_b).unwrap();
    assert_eq!(
        (app.open_tables[0].memory.x, app.open_tables[0].memory.y),
        (180, 120)
    );
    assert_eq!(app.open_tables[0].memory.width, 640);
    assert_eq!(app.open_tables[0].memory.height, 400);
    assert_eq!(app.open_tables[0].memory.fit_to_content, Some(false));
    draw_frame(&mut app, &context, canvas, &key, &mut time);
    let restored_b = context
        .memory(|memory| memory.area_rect(window_id))
        .expect("workspace B table window should have geometry");
    assert_eq!((restored_b.left(), restored_b.top()), (180.0, 120.0));
    assert_eq!((restored_b.width(), restored_b.height()), (640.0, 400.0));
}

#[test]
fn live_manual_table_size_survives_workspace_switch_and_close_reopen() {
    fn draw_frame(
        app: &mut TunerApp,
        context: &egui::Context,
        canvas: egui::Rect,
        key: &str,
        time: &mut f64,
        events: Vec<egui::Event>,
    ) -> Option<egui::Rect> {
        *time += 0.1;
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        input.time = Some(*time);
        input.events = events;
        let window_id = egui::Id::new(("table-window", "bin-size-lifecycle", key));
        let dock_id = WindowId::Table(key.to_owned());
        let layer_id = egui::LayerId::new(app.dock_window_order(&dock_id), window_id);
        let resize_corner_id = egui::Id::new(layer_id)
            .with("edge_drag")
            .with("right_bottom");
        let mut resize_corner = None;
        context
            .run_ui(input, |ctx| {
                let mut frame = eframe::Frame::_new_kittest();
                eframe::App::ui(app, ctx, &mut frame);
                resize_corner = ctx
                    .ctx()
                    .read_response(resize_corner_id)
                    .map(|response| response.rect);
            })
            .drop_without_applying_deltas();
        resize_corner
    }

    fn draw_stable_frames(
        app: &mut TunerApp,
        context: &egui::Context,
        canvas: egui::Rect,
        key: &str,
        time: &mut f64,
    ) {
        for _ in 0..3 {
            draw_frame(app, context, canvas, key, time, Vec::new());
        }
    }

    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0; 144])),
        Some(XdfDocument::parse(LARGE_TABLE_XDF).unwrap()),
        None,
        None,
    );
    app.project_identity = Some("bin-size-lifecycle".into());
    app.project_preferences = ProjectPreferences::for_identity("bin-size-lifecycle");
    app.preferences.show_diagnostics = false;
    app.project_preferences.show_diagnostics = false;
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    let window_id = egui::Id::new(("table-window", "bin-size-lifecycle", key.as_str()));
    let workspace_a = app.project_preferences.active_workspace_id;
    app.open_tables[0].memory = TableWindowMemory {
        x: 100,
        y: 80,
        width: 900,
        height: 600,
        zoom_percent: 100,
        fit_to_content: Some(false),
        position_saved: true,
        ..TableWindowMemory::default()
    };
    app.open_tables[0].geometry_request = true;

    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_400.0, 900.0));
    let mut time = 0.0;
    draw_stable_frames(&mut app, &context, canvas, &key, &mut time);
    let workspace_b = app.create_workspace_snapshot("Same size").unwrap();
    app.preferences.show_diagnostics = true;
    draw_stable_frames(&mut app, &context, canvas, &key, &mut time);

    app.switch_workspace_snapshot(workspace_a).unwrap();
    draw_stable_frames(&mut app, &context, canvas, &key, &mut time);
    app.switch_workspace_snapshot(workspace_b).unwrap();
    draw_stable_frames(&mut app, &context, canvas, &key, &mut time);
    assert_eq!(
        context.memory(|memory| memory.area_rect(window_id).map(TableGeometry::from_rect)),
        Some(TableGeometry {
            x: 100,
            y: 80,
            width: 900,
            height: 600,
        })
    );

    let resize_corner = draw_frame(&mut app, &context, canvas, &key, &mut time, Vec::new())
        .expect("table should expose a resize corner");
    let start = resize_corner.center();
    let end = start + egui::vec2(40.0, 30.0);
    draw_frame(
        &mut app,
        &context,
        canvas,
        &key,
        &mut time,
        vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    draw_frame(
        &mut app,
        &context,
        canvas,
        &key,
        &mut time,
        vec![egui::Event::PointerMoved(end)],
    );
    draw_frame(
        &mut app,
        &context,
        canvas,
        &key,
        &mut time,
        vec![
            egui::Event::PointerMoved(end),
            egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    let resized = context
        .memory(|memory| memory.area_rect(window_id))
        .map(TableGeometry::from_rect)
        .expect("resized table should retain its rectangle");
    assert_eq!(
        TableGeometry::from_memory(&app.open_tables[0].memory),
        resized
    );
    assert_eq!(app.open_tables[0].memory.fit_to_content, Some(false));

    app.switch_workspace_snapshot(workspace_a).unwrap();
    let saved_b = app
        .project_preferences
        .saved_workspace_snapshots
        .iter()
        .find(|snapshot| snapshot.id == workspace_b)
        .and_then(|snapshot| snapshot.state.table_windows.get(&key))
        .expect("switch should capture workspace B's live table size");
    assert_eq!(TableGeometry::from_memory(saved_b), resized);

    draw_stable_frames(&mut app, &context, canvas, &key, &mut time);
    assert_eq!(
        context.memory(|memory| memory.area_rect(window_id).map(TableGeometry::from_rect)),
        Some(TableGeometry {
            x: 100,
            y: 80,
            width: 900,
            height: 600,
        })
    );
    app.switch_workspace_snapshot(workspace_b).unwrap();
    draw_stable_frames(&mut app, &context, canvas, &key, &mut time);
    assert_eq!(
        context.memory(|memory| memory.area_rect(window_id).map(TableGeometry::from_rect)),
        Some(resized)
    );

    let saved_before_close = app.open_tables[0].memory.clone();
    let titlebar_rect = context
        .memory(|memory| memory.area_rect(window_id))
        .expect("restored table should show its title bar");
    let close_position = egui::pos2(titlebar_rect.right() - 14.0, titlebar_rect.top() + 13.0);
    draw_frame(
        &mut app,
        &context,
        canvas,
        &key,
        &mut time,
        vec![
            egui::Event::PointerMoved(close_position),
            egui::Event::PointerButton {
                pos: close_position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    draw_frame(
        &mut app,
        &context,
        canvas,
        &key,
        &mut time,
        vec![
            egui::Event::PointerMoved(close_position),
            egui::Event::PointerButton {
                pos: close_position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    assert!(
        app.open_tables.is_empty(),
        "title-bar close should close the table"
    );
    assert_eq!(
        app.preferences.table_windows.get(&key),
        Some(&saved_before_close)
    );
    assert!(app.open_table(&semantic_id));
    draw_stable_frames(&mut app, &context, canvas, &key, &mut time);
    assert_eq!(
        context.memory(|memory| memory.area_rect(window_id).map(TableGeometry::from_rect)),
        Some(resized)
    );
}

#[test]
fn workspace_snapshot_names_are_unique_and_last_layout_cannot_be_deleted() {
    let mut app = app_with_project_fixture();
    let default_id = app.project_preferences.active_workspace_id;
    let diagnostics = app.create_workspace_snapshot("Diagnostics").unwrap();
    assert!(app.create_workspace_snapshot(" diagnostics ").is_err());
    app.rename_workspace_snapshot(diagnostics, "Street")
        .unwrap();
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

#[test]
fn workspace_layout_mutations_are_blocked_during_document_operations() {
    let mut app = app_with_project_fixture();
    app.operations.begin(OperationKind::LoadingXdf, "next.xdf");
    let before = app.project_preferences.clone();

    assert!(app.create_workspace_snapshot("Blocked").is_err());
    assert_eq!(
        app.project_preferences.active_workspace_id,
        before.active_workspace_id
    );
    assert_eq!(
        app.project_preferences.active_workspace_name,
        before.active_workspace_name
    );
    assert_eq!(
        app.project_preferences.saved_workspace_snapshots,
        before.saved_workspace_snapshots
    );
}

#[test]
fn startup_restore_loads_xdf_after_bin_workspace_restoration() {
    let mut app = TunerApp::headless();
    let bin_path = temporary_test_path("bin");
    let xdf_path = temporary_test_path("xdf");
    std::fs::write(&xdf_path, COLUMN_MAJOR_XDF).unwrap();
    app.pending_project_restore = Some(PendingProjectRestore {
        bin_path: bin_path.clone(),
        xdf_path: Some(xdf_path.clone()),
        bin_available: true,
        xdf_available: true,
    });

    app.start_last_project_restore(StartupRestoreAction::BinAndXdf)
        .unwrap();
    let loading_bin = app.operations.active().unwrap().id;
    app.operations.cancel(loading_bin, "test transition");
    let (_, bin) = column_major_fixture();
    app.workspace.bin = Some(bin);
    app.workspace.bin_path = Some(bin_path.clone());

    let identity = project_identity_for_path(&bin_path);
    let restore = app
        .operations
        .begin(OperationKind::RestoringWorkspace, identity.clone());
    let (xdf, _) = column_major_fixture();
    let semantic_id = xdf.parameters[0].semantic_id.clone();
    let key = table_key(&xdf.normalized_fingerprint, &semantic_id);
    let mut project = ProjectPreferences::for_identity(&identity);
    project.active_workspace_id = 7;
    project.active_workspace_name = "Track".into();
    project.last_xdf_path = Some(xdf_path.clone());
    project.open_table_keys = vec![key.clone()];
    project.active_table_key = Some(key.clone());
    project.selected_semantic_id = Some(semantic_id.clone());
    project.selected_cell = (1, 1);
    project.selected_cells = Some(CellSelection::new((1, 0), (1, 1)));
    app.install_operation_result(OperationResult::restored(
        restore,
        identity.clone(),
        project.clone(),
    ));
    assert_eq!(app.project_preferences.active_workspace_id, 7);
    assert!(app.open_tables.is_empty());
    assert_eq!(app.project_preferences.open_table_keys, [key.clone()]);

    let active = app
        .operations
        .active()
        .expect("the remembered XDF should load after BIN restoration");
    assert_eq!(active.kind, OperationKind::LoadingXdf);
    assert_eq!(active.subject, xdf_path.display().to_string());
    assert!(app.pending_xdf_restore.is_none());
    let loaded_xdf = XdfDocument::load(&xdf_path).unwrap();
    assert_eq!(
        loaded_xdf.normalized_fingerprint,
        xdf.normalized_fingerprint
    );
    app.install_operation_result(OperationResult {
        id: active.id,
        generation: 0,
        subject: xdf_path.display().to_string(),
        payload: Ok(OperationPayload::Xdf(loaded_xdf)),
    });
    let restore = app
        .operations
        .active()
        .expect("loading the remembered XDF restores its project layout")
        .id;
    app.operations
        .cancel(restore, "complete startup restore in test");
    let restore = app
        .operations
        .begin(OperationKind::RestoringWorkspace, identity.clone());
    app.install_operation_result(OperationResult::restored(restore, identity, project));
    assert_eq!(
        app.open_tables.len(),
        1,
        "restore notice: {:?}; expected key: {}; saved table keys: {:?}; xdf fingerprint: {:?}",
        app.workspace_restore_notice,
        key,
        app.project_preferences.open_table_keys,
        app.workspace
            .xdf
            .as_ref()
            .map(|xdf| &xdf.normalized_fingerprint)
    );
    assert_eq!(app.open_tables[0].key, key);
    assert_eq!(app.project_preferences.active_workspace_name, "Track");
    assert_eq!(
        app.workspace.selected_semantic_id.as_deref(),
        Some(semantic_id.as_str())
    );
    assert_eq!(
        app.workspace.selected_cells,
        Some(CellSelection::new((1, 0), (1, 1)))
    );
    assert!(app
        .operations
        .active()
        .is_some_and(|active| { active.kind == OperationKind::Validating }));
    std::fs::remove_file(xdf_path).unwrap();
}

#[test]
fn preferences_file_replacement_round_trips_without_leaving_swap_files() {
    let path = temporary_test_path("json");
    let mut preferences = AppPreferences::default();
    save_preferences(&path, &preferences).unwrap();
    preferences.browser_collapsed = true;
    save_preferences(&path, &preferences).unwrap();
    assert_eq!(load_preferences(&path), preferences);
    assert!(!path
        .with_file_name(format!(
            "{}.tmp-{}",
            path.file_name().unwrap().to_string_lossy(),
            std::process::id()
        ))
        .exists());
    assert!(!path
        .with_file_name(format!(
            "{}.backup-{}",
            path.file_name().unwrap().to_string_lossy(),
            std::process::id()
        ))
        .exists());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn project_preferences_round_trip_is_identity_scoped() {
    let path = temporary_test_path("project.json");
    let identity = "c:/tuning/scga05_cal.bin";
    let mut project = ProjectPreferences::for_identity(identity);
    project.layout = LayoutPreset::DataEntry;
    project.show_editor = false;
    project.diagnostics_height = 240;
    project.browser_filter = "torque".into();
    project.category_state_initialized = true;
    project.collapsed_categories = vec!["AT".into()];
    project.favorite_tables = vec!["xdf|favorite".into()];
    project.recent_tables = vec!["xdf|recent".into()];
    project.browser_collapsed = true;
    project.inspector_width = 500;
    project.project_notepad = "Keep this tune conservative.".into();
    project.notepad_stay_on_top = true;
    project.table_windows.insert(
        "xdf|favorite".into(),
        TableWindowMemory {
            x: 120,
            y: 90,
            width: 900,
            height: 600,
            position_saved: true,
            zoom_percent: 125,
            scroll_x: 10,
            scroll_y: 20,
            decimal_places: 2,
            coloring: TableColorSettings::default(),
            fit_to_content: Some(false),
        },
    );
    project
        .tab_orders
        .insert("xdf".into(), vec!["xdf|favorite".into()]);
    project.open_table_keys = vec!["xdf|favorite".into()];
    project.active_table_key = Some("xdf|favorite".into());

    save_project_preferences(&path, &project).unwrap();
    assert_eq!(load_project_preferences(&path, identity), project);

    let other_identity = "d:/other/scga05_cal.bin";
    let other = load_project_preferences(&path, other_identity);
    assert_eq!(other, ProjectPreferences::for_identity(other_identity));

    std::fs::remove_file(path).unwrap();
}

#[test]
fn agent_task_history_migrates_old_project_json_and_revision() {
    let mut old = serde_json::to_value(ProjectPreferences::for_identity("bin-a")).unwrap();
    old.as_object_mut().unwrap().remove("agent_task_history");
    old.as_object_mut().unwrap().remove("project_notepad");
    old.as_object_mut().unwrap().remove("notepad_stay_on_top");
    old["version"] = json!(8);
    let restored = project_preferences_from_json(&old.to_string(), "bin-a");
    assert_eq!(
        serde_json::to_value(&restored).unwrap()["agent_task_history"],
        json!([])
    );
    assert_eq!(restored.project_notepad, "");
    assert!(!restored.notepad_stay_on_top);

    old["agent_task_history"] = json!([{
        "task_id": "task-1", "capability": "diagnostics", "document_identity": {
            "bin_sha256": "bin-hash", "xdf_sha256": "xdf-hash"
        }, "status": "accepted", "summary": "Diagnostics task",
        "evidence": [], "decision": "accepted", "applied_operations": []
    }]);
    let restored = project_preferences_from_json(&old.to_string(), "bin-a");
    let encoded = serde_json::to_value(restored).unwrap();
    assert_eq!(
        encoded["agent_task_history"][0]["document_identity"]["workspace_data_revision"],
        0
    );
    assert_eq!(encoded["version"], json!(PROJECT_SETTINGS_VERSION));
}

#[test]
fn agent_task_history_round_trip_is_project_scoped_bounded_and_secret_free() {
    let settings_path = temporary_test_path("agent-history-settings.json");
    let first = format!("bin-a-{}", settings_path.display());
    let second = format!("bin-b-{}", settings_path.display());
    let first_path = project_settings_path(&settings_path, &first);
    let second_path = project_settings_path(&settings_path, &second);
    let mut value = serde_json::to_value(ProjectPreferences::for_identity(&first)).unwrap();
    value["agent_task_history"] = json!((0..51).map(|index| json!({
        "task_id": format!("task-{index}"), "capability": "diagnostics",
        "document_identity": {"bin_sha256": "bin-hash", "xdf_sha256": "xdf-hash", "workspace_data_revision": 7},
        "status": "applied", "summary": "Diagnostics task", "evidence": [{
            "summary": "", "semantic_id": "fuel-map", "byte_range": [16, 18]
        }], "decision": "accepted", "applied_operations": ["set_engineering_cell:fuel-map:0:0"]
    })).collect::<Vec<_>>());
    value["token"] = json!("local-secret-token");
    let loaded: ProjectPreferences = serde_json::from_value(value).unwrap();
    save_project_preferences(&first_path, &loaded).unwrap();
    let saved = std::fs::read_to_string(&first_path).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&saved).unwrap()["agent_task_history"]
            .as_array()
            .unwrap()
            .len(),
        50
    );
    let history = serde_json::to_value(load_project_preferences(&first_path, &first)).unwrap()
        ["agent_task_history"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(history.len(), 50);
    assert_eq!(history[0]["task_id"], "task-1");
    assert_eq!(
        history[49]["document_identity"]["workspace_data_revision"],
        7
    );
    assert!(!saved.contains("local-secret-token"));
    assert!(!saved.contains("\"token\""));
    assert_eq!(
        serde_json::to_value(load_project_preferences(&second_path, &second)).unwrap()
            ["agent_task_history"],
        json!([])
    );
    std::fs::remove_file(first_path).unwrap();
}

#[test]
fn agent_task_history_ipc_failure_updates_project_and_safe_debug_report() {
    let mut app = TunerApp::headless();
    app.project_identity = Some("bin-a".into());
    app.project_preferences = ProjectPreferences::for_identity("bin-a");
    app.nooklink_answer = "private challenge answer marker".into();
    let start = agent_ipc_request(json!({"action":"agent_task","agent_task":{
        "kind":"start","capability":"diagnostics","goal":"private goal token"
    }}));
    let task_id = app.handle_ui_ipc_request(&start).data.unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_string();
    app.start_nooklink_quick_task(&task_id).unwrap();
    let fail = agent_ipc_request(json!({"action":"agent_task","agent_task":{
        "kind":"fail","task_id":task_id,"message":"private failure token"
    }}));
    assert!(app.handle_ui_ipc_request(&fail).ok);
    assert_eq!(app.project_preferences.agent_task_history.len(), 1);
    let item = &app.project_preferences.agent_task_history[0];
    assert_eq!(item.task_id, task_id);
    assert_eq!(item.status, AgentTaskStatus::Failed);
    let persisted = serde_json::to_string(&app.project_preferences).unwrap();
    let debug = app.debug_report_text();
    for secret in [
        "private goal token",
        "private failure token",
        "private challenge answer marker",
    ] {
        assert!(!persisted.contains(secret));
        assert!(!debug.contains(secret));
    }
    assert!(debug.contains(&task_id));
    assert!(debug.contains("failed"));
}

#[test]
fn agent_task_history_started_without_project_does_not_attach_to_later_project() {
    let mut app = TunerApp::headless();
    let start = agent_ipc_request(json!({"action":"agent_task","agent_task":{
        "kind":"start","capability":"diagnostics","goal":"inspect"
    }}));
    let task_id = app.handle_ui_ipc_request(&start).data.unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_string();
    app.start_nooklink_quick_task(&task_id).unwrap();
    app.project_identity = Some("bin-b".into());
    app.project_preferences = ProjectPreferences::for_identity("bin-b");
    let fail = agent_ipc_request(json!({"action":"agent_task","agent_task":{
        "kind":"fail","task_id":task_id,"message":"failed"
    }}));
    assert!(app.handle_ui_ipc_request(&fail).ok);
    assert!(app.project_preferences.agent_task_history.is_empty());
}

#[test]
fn agent_task_history_late_failure_stays_with_originating_project() {
    let settings_path = temporary_test_path("agent-history-switch-settings.json");
    let first = format!("bin-a-{}", settings_path.display());
    let second = format!("bin-b-{}", settings_path.display());
    let mut app = TunerApp::headless();
    app.persistence_enabled = true;
    app.settings_path = settings_path.clone();
    app.project_identity = Some(first.clone());
    app.project_preferences = ProjectPreferences::for_identity(&first);
    let start = agent_ipc_request(json!({"action":"agent_task","agent_task":{
        "kind":"start","capability":"diagnostics","goal":"inspect"
    }}));
    let task_id = app.handle_ui_ipc_request(&start).data.unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_string();
    app.start_nooklink_quick_task(&task_id).unwrap();
    app.project_identity = Some(second.clone());
    app.project_preferences = ProjectPreferences::for_identity(&second);
    let fail = agent_ipc_request(json!({"action":"agent_task","agent_task":{
        "kind":"fail","task_id":task_id,"message":"failed"
    }}));
    assert!(app.handle_ui_ipc_request(&fail).ok);
    assert!(app.project_preferences.agent_task_history.is_empty());
    let old_path = project_settings_path(&settings_path, &first);
    let old = load_project_preferences(&old_path, &first);
    assert_eq!(old.agent_task_history.len(), 1);
    assert_eq!(old.agent_task_history[0].task_id, task_id);
    std::fs::remove_file(old_path).unwrap();
}

#[test]
fn history_omits_untrusted_evidence_text_and_semantic_ids() {
    const SECRET_MARKER: &str = "challenge-secret-marker-7f91";
    let mut app = TunerApp::headless();
    app.project_identity = Some("privacy-bin".into());
    app.project_preferences = ProjectPreferences::for_identity("privacy-bin");
    let start = agent_ipc_request(json!({"action":"agent_task","agent_task":{
        "kind":"start","capability":"diagnostics","goal":"private goal"
    }}));
    let task_id = app.handle_ui_ipc_request(&start).data.unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_string();
    app.start_nooklink_quick_task(&task_id).unwrap();
    let submit = agent_ipc_request(json!({"action":"agent_task","agent_task":{
        "kind":"submit","task_id":task_id,"report":{
            "summary":"private report", "findings":[], "confidence":0.7, "operations":[],
            "evidence":[
                {"summary":SECRET_MARKER,"semantic_id":SECRET_MARKER,"byte_range":[8,12]},
                {"summary":"invalid range","semantic_id":"untrusted-id","byte_range":[12,8]}
            ]
        }
    }}));
    assert!(app.handle_ui_ipc_request(&submit).ok);
    app.review_agent_proposal(&task_id, true).unwrap();

    let history = &app.project_preferences.agent_task_history[0];
    let serialized = serde_json::to_string(history).unwrap();
    let debug = app.debug_report_text();
    assert!(!serialized.contains(SECRET_MARKER));
    assert!(!debug.contains(SECRET_MARKER));
    assert!(history
        .evidence
        .iter()
        .all(|evidence| evidence.semantic_id.is_none()));
    assert_eq!(history.evidence.len(), 1);
    assert_eq!(history.evidence[0].byte_range, Some([8, 12]));
}

#[test]
fn nooklink_start_rejects_busy_workspace_and_pending_project_transitions() {
    let start = || {
        agent_ipc_request(json!({"action":"agent_task","agent_task":{
            "kind":"start","capability":"diagnostics","goal":"inspect"
        }}))
    };
    let mut cases = Vec::new();

    let mut busy = TunerApp::headless();
    busy.project_identity = Some("previous-bin".into());
    busy.operations
        .begin(OperationKind::RestoringWorkspace, "next-bin");
    cases.push(busy);

    let mut project_prompt = TunerApp::headless();
    project_prompt.project_identity = Some("previous-bin".into());
    project_prompt.pending_project_restore = Some(PendingProjectRestore {
        bin_path: PathBuf::from("next.bin"),
        xdf_path: None,
        bin_available: true,
        xdf_available: false,
    });
    cases.push(project_prompt);

    let mut startup_pending = TunerApp::headless();
    startup_pending.project_identity = Some("previous-bin".into());
    startup_pending.pending_startup_load = Some(StartupRestoreLoad::BinOnly);
    cases.push(startup_pending);

    for mut app in cases {
        let response = app.handle_ui_ipc_request(&start());
        assert!(!response.ok);
        assert!(response.message.to_lowercase().contains("retry"));
        assert!(app.agent_tasks.is_empty());
        assert!(app.agent_tasks.take_history().is_empty());
        assert_eq!(app.project_identity.as_deref(), Some("previous-bin"));
    }
}

#[test]
fn inactive_project_history_write_failure_requeues_and_deduplicates_retry() {
    let root = temporary_test_path("agent-history-write-retry");
    std::fs::create_dir(&root).unwrap();
    let projects_path = root.join("projects");
    std::fs::write(&projects_path, "blocks directory creation").unwrap();
    let settings_path = root.join("settings.json");
    let origin = format!("origin-{}", root.display());
    let current = format!("current-{}", root.display());
    let mut app = TunerApp::headless();
    app.persistence_enabled = true;
    app.settings_path = settings_path.clone();
    app.project_identity = Some(origin.clone());
    app.project_preferences = ProjectPreferences::for_identity(&origin);
    let start = agent_ipc_request(json!({"action":"agent_task","agent_task":{
        "kind":"start","capability":"diagnostics","goal":"inspect"
    }}));
    let task_id = app.handle_ui_ipc_request(&start).data.unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_string();
    app.start_nooklink_quick_task(&task_id).unwrap();
    app.project_identity = Some(current.clone());
    app.project_preferences = ProjectPreferences::for_identity(&current);
    let fail = agent_ipc_request(json!({"action":"agent_task","agent_task":{
        "kind":"fail","task_id":task_id,"message":"failed"
    }}));
    assert!(app.handle_ui_ipc_request(&fail).ok);

    std::fs::remove_file(&projects_path).unwrap();
    std::fs::create_dir(&projects_path).unwrap();
    let history_path = project_settings_path(&settings_path, &origin);
    app.flush_agent_task_history();
    let retried = load_project_preferences(&history_path, &origin);
    assert_eq!(retried.agent_task_history.len(), 1);
    assert_eq!(retried.agent_task_history[0].task_id, task_id);
    app.flush_agent_task_history();
    assert_eq!(
        load_project_preferences(&history_path, &origin)
            .agent_task_history
            .len(),
        1
    );
    std::fs::remove_file(history_path).unwrap();
    std::fs::remove_dir(projects_path).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn inactive_history_retains_project_mapping_until_retry_succeeds() {
    let root = temporary_test_path("agent-history-mapping-retry");
    std::fs::create_dir(&root).unwrap();
    let projects_path = root.join("projects");
    std::fs::write(&projects_path, "blocks directory creation").unwrap();
    let settings_path = root.join("settings.json");
    let origin = format!("origin-{}", root.display());
    let mut app = TunerApp::headless();
    app.persistence_enabled = true;
    app.settings_path = settings_path.clone();
    app.project_identity = Some(origin.clone());
    app.project_preferences = ProjectPreferences::for_identity(&origin);
    let start = agent_ipc_request(json!({"action":"agent_task","agent_task":{
        "kind":"start","capability":"diagnostics","goal":"inspect"
    }}));
    let task_id = app.handle_ui_ipc_request(&start).data.unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_string();
    app.start_nooklink_quick_task(&task_id).unwrap();
    app.project_identity = Some("different-current-project".into());
    app.project_preferences = ProjectPreferences::for_identity("different-current-project");
    let fail = agent_ipc_request(json!({"action":"agent_task","agent_task":{
        "kind":"fail","task_id":task_id,"message":"failed"
    }}));
    assert!(app.handle_ui_ipc_request(&fail).ok);

    for _ in 0..150 {
        let id = app
            .agent_tasks
            .start(
                AgentCapability::Diagnostics,
                "fill bounded task store".into(),
                DocumentIdentity::default(),
            )
            .unwrap();
        app.agent_tasks
            .approve_start(&id, AgentTaskLevel::Quick)
            .unwrap();
        app.agent_tasks.fail(&id, "done".into()).unwrap();
    }
    let original_was_evicted = app.agent_tasks.poll(&task_id).is_err();
    app.flush_agent_task_history();
    let mapping_retained = app.agent_task_projects.contains_key(&task_id);

    std::fs::remove_file(&projects_path).unwrap();
    std::fs::create_dir(&projects_path).unwrap();
    app.flush_agent_task_history();
    let history_path = project_settings_path(&settings_path, &origin);
    let history_len = if history_path.exists() {
        load_project_preferences(&history_path, &origin)
            .agent_task_history
            .len()
    } else {
        0
    };
    if history_path.exists() {
        std::fs::remove_file(history_path).unwrap();
    }
    std::fs::remove_dir(projects_path).unwrap();
    std::fs::remove_dir(root).unwrap();
    assert!(original_was_evicted);
    assert!(mapping_retained);
    assert_eq!(history_len, 1);
}

#[test]
fn project_history_deduplicates_task_status_decision_on_retry() {
    let make_item = || AgentTaskHistoryItem {
        task_id: "task-7".into(),
        capability: AgentCapability::Diagnostics,
        document_identity: DocumentIdentity::default(),
        status: AgentTaskStatus::Failed,
        summary: "Diagnostics task".into(),
        evidence: Vec::new(),
        decision: Some("failed".into()),
        applied_operations: Vec::new(),
    };
    let mut preferences = ProjectPreferences::for_identity("bin-idempotence");
    preferences.agent_task_history = vec![make_item(), make_item()];
    preferences.sanitize();
    assert_eq!(preferences.agent_task_history.len(), 1);
    assert_eq!(preferences.agent_task_history[0].task_id, "task-7");
}

#[test]
fn debug_report_shows_decision_safe_ranges_and_applied_operation() {
    const SECRET_MARKER: &str = "private-debug-marker-93ab";
    let (xdf, bin) = column_major_fixture();
    let semantic_id = xdf.parameters[0].semantic_id.clone();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    app.project_identity = Some("debug-history-bin".into());
    app.project_preferences = ProjectPreferences::for_identity("debug-history-bin");
    let start = agent_ipc_request(json!({"action":"agent_task","agent_task":{
        "kind":"start","capability":"diagnostics","goal":SECRET_MARKER
    }}));
    let task_id = app.handle_ui_ipc_request(&start).data.unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_string();
    app.start_nooklink_quick_task(&task_id).unwrap();
    let mut report = agent_proposal_report(&semantic_id);
    report.summary = SECRET_MARKER.into();
    report.evidence.push(AgentEvidence {
        summary: SECRET_MARKER.into(),
        semantic_id: Some(SECRET_MARKER.into()),
        byte_range: Some([16, 18]),
    });
    let submit = agent_ipc_request(json!({"action":"agent_task","agent_task":{
        "kind":"submit","task_id":task_id,"report":report
    }}));
    assert!(app.handle_ui_ipc_request(&submit).ok);
    app.review_agent_proposal(&task_id, true).unwrap();
    let answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    assert!(app.complete_nooklink_challenge(&answer));

    let debug = app.debug_report_text();
    assert!(debug.contains("decision=accepted"));
    assert!(debug.contains("evidence=[0x10..0x12]"));
    assert!(debug.contains(&format!("set_engineering_cell:{semantic_id}:0:0")));
    assert!(!debug.contains(SECRET_MARKER));
}

#[test]
fn compare_preferences_round_trip_and_sanitize_missing_values() {
    let mut preferences = ProjectPreferences::for_identity("bin-a");
    preferences.compare_bin_path = Some(PathBuf::from("C:/tuning/source.bin"));
    preferences.compare_xdf_path = Some(PathBuf::from("C:/tuning/source.xdf"));
    preferences.compare_mode = CompareValueMode::PercentDelta;
    preferences.compare_filter = "torque".to_string();
    preferences.compare_changed_only = true;
    preferences.compare_selected_semantic_id = Some("table:map".to_string());
    preferences.compare_window.width = 1;
    preferences.compare_window.height = 10_000;

    let encoded = serde_json::to_string(&preferences).unwrap();
    let decoded: ProjectPreferences = serde_json::from_str(&encoded).unwrap();
    let mut sanitized = decoded.clone();
    sanitized.sanitize();

    assert_eq!(sanitized.compare_mode, CompareValueMode::PercentDelta);
    assert_eq!(sanitized.compare_filter, "torque");
    assert!(sanitized.compare_window.width >= 420);
    assert!(sanitized.compare_window.height <= 1_800);
}

#[test]
fn compare_state_is_isolated_by_bin_identity() {
    let settings_path = temporary_test_path("settings.json");
    let first = project_identity_for_path(Path::new("C:/tuning/first.bin"));
    let second = project_identity_for_path(Path::new("C:/tuning/second.bin"));
    let first_path = project_settings_path(&settings_path, &first);
    let second_path = project_settings_path(&settings_path, &second);

    let mut first_preferences = ProjectPreferences::for_identity(&first);
    first_preferences.compare_bin_path = Some(PathBuf::from("source.bin"));
    save_project_preferences(&first_path, &first_preferences).unwrap();

    let loaded_second = load_project_preferences(&second_path, &second);
    assert_eq!(loaded_second.compare_bin_path, None);

    std::fs::remove_file(first_path).unwrap();
}

#[test]
fn compare_map_snapshot_reads_source_and_destination_values() {
    let (xdf, destination) = column_major_fixture();
    let source = BinDocument::from_bytes(vec![10, 40, 40, 50, 50, 80]);
    let data = build_compare_map_data(&xdf, &xdf, &source, &destination, "table:uid:map").unwrap();

    assert_eq!(data.value(0, 0, CompareValueMode::Destination), Some(10.0));
    assert_eq!(data.value(0, 0, CompareValueMode::Source), Some(10.0));
    assert_eq!(
        data.value(0, 1, CompareValueMode::AbsoluteDelta),
        Some(10.0)
    );
}

#[test]
fn compare_snapshot_prefers_explicit_axis_roles_over_axis_order() {
    let xdf = XdfDocument::parse(
        br#"<XDFFORMAT><XDFTABLE uniqueid="role-map">
            <title>Role Map</title>
            <XDFAXIS id="z"><EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8"
                mmedrowcount="2" mmedcolcount="2" mmedmajorstridebits="8"
                mmedminorstridebits="8" mmedtypeflags="0x06" /></XDFAXIS>
            <XDFAXIS id="y"><indexcount>2</indexcount>
                <EMBEDDEDDATA mmedaddress="0x20" mmedelementsizebits="8"
                mmedmajorstridebits="8" mmedtypeflags="0x06" /></XDFAXIS>
            <XDFAXIS id="x"><indexcount>2</indexcount>
                <EMBEDDEDDATA mmedaddress="0x10" mmedelementsizebits="8"
                mmedmajorstridebits="8" mmedtypeflags="0x06" /></XDFAXIS>
        </XDFTABLE></XDFFORMAT>"#,
    )
    .unwrap();
    let mut bytes = vec![0; 0x40];
    bytes[0x10] = 3;
    bytes[0x11] = 5;
    bytes[0x20] = 7;
    bytes[0x21] = 9;
    let bin = BinDocument::from_bytes(bytes);

    let data = build_compare_map_data(&xdf, &xdf, &bin, &bin, "table:uid:role-map").unwrap();

    assert_eq!(data.x, vec![3.0, 5.0]);
    assert_eq!(data.y, vec![7.0, 9.0]);
}

#[test]
fn compare_source_replacement_changes_cache_identity_and_stale_results_are_ignored() {
    let mut app = TunerApp::headless();
    let first = app
        .operations
        .begin(OperationKind::LoadingCompareBin, "first.bin");
    let second = app
        .operations
        .begin(OperationKind::LoadingCompareBin, "second.bin");

    app.install_compare_result(OperationResult::compare_bin(
        first,
        "first.bin",
        BinDocument::from_bytes(vec![1]),
    ));
    assert!(app.compare.source_bin.is_none());

    app.install_compare_result(OperationResult::compare_bin(
        second,
        "second.bin",
        BinDocument::from_bytes(vec![2]),
    ));
    assert_eq!(app.compare.source_bin.as_ref().unwrap().bytes(), &[2]);
    assert_eq!(
        app.compare.source_bin_sha256.as_deref(),
        Some("dbc1b4c900ffe48d575b5da5c638040125f65db0fe3e24494b76ea986457d986")
    );
}

#[test]
fn compare_cache_entries_share_immutable_map_data() {
    let mut compare = CompareWorkspaceState::default();
    let key = CompareCacheKey {
        source_bin_sha256: "source".to_string(),
        destination_revision: 1,
        source_xdf_fingerprint: "source-xdf".to_string(),
        destination_xdf_fingerprint: "destination-xdf".to_string(),
        semantic_id: "table:uid:map".to_string(),
        mode: CompareValueMode::Source,
    };
    let map = Arc::new(CompareMapData::new(
        key.semantic_id.clone(),
        1,
        1,
        vec![0.0],
        vec![0.0],
        vec![CompareCell::from_values(Some(10.0), Some(20.0))],
    ));
    compare.map_cache.insert(key.clone(), map.clone());

    let table_read = compare.map_cache.get(&key).unwrap().clone();
    let surface_read = compare.map_cache.get(&key).unwrap().clone();

    assert!(Arc::ptr_eq(&table_read, &surface_read));
    assert_eq!(table_read.value(0, 0, CompareValueMode::Source), Some(20.0));
}

#[test]
fn compare_window_renders_without_starting_document_work() {
    let context = egui::Context::default();
    let mut app = TunerApp::headless();
    app.compare.window_open = true;

    context
        .run_ui(egui::RawInput::default(), |ctx| {
            app.show_compare_window(ctx);
        })
        .drop_without_applying_deltas();

    assert!(app.operations.active().is_none());
    let id = egui::Id::new(("compare-window", app.project_scope()));
    assert!(context.memory(|memory| memory.area_rect(id).is_some()));
}

#[test]
fn table_compare_mode_is_scoped_to_the_selected_table() {
    assert_eq!(
        effective_table_compare_mode(
            true,
            Some("table:map"),
            "table:other",
            CompareValueMode::Source,
        ),
        CompareValueMode::Destination
    );
    assert_eq!(
        effective_table_compare_mode(
            true,
            Some("table:map"),
            "table:map",
            CompareValueMode::PercentDelta,
        ),
        CompareValueMode::PercentDelta
    );
    assert_eq!(
        effective_table_compare_mode(
            false,
            Some("table:map"),
            "table:map",
            CompareValueMode::Source,
        ),
        CompareValueMode::Destination
    );
}

#[test]
fn ready_compare_table_skips_destination_views() {
    assert!(table_needs_destination_views(
        ParameterKind::Table,
        CompareValueMode::Destination,
        true,
    ));
    assert!(!table_needs_destination_views(
        ParameterKind::Table,
        CompareValueMode::Source,
        true,
    ));
    assert!(!table_needs_destination_views(
        ParameterKind::Table,
        CompareValueMode::AbsoluteDelta,
        true,
    ));
    assert!(table_needs_destination_views(
        ParameterKind::Table,
        CompareValueMode::PercentDelta,
        false,
    ));
    assert!(!table_needs_destination_views(
        ParameterKind::Constant,
        CompareValueMode::Destination,
        false,
    ));
}

#[test]
fn pending_compare_table_shows_placeholders_instead_of_destination_values() {
    let (xdf, bin) = column_major_fixture();
    let semantic_id = xdf.parameters[0].semantic_id.clone();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    assert!(app.open_table(&semantic_id));
    app.compare.source_bin = Some(BinDocument::from_bytes(vec![0; 6]));
    app.compare.selected_semantic_id = Some(semantic_id.clone());
    app.compare.mode = CompareValueMode::Source;

    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let key = app.open_tables[0].key.clone();
    let mut first_input = egui::RawInput::default();
    first_input.screen_rect = Some(canvas);
    let mut output = context.run_ui(first_input, |ctx| {
        app.show_table_window(ctx, canvas, &key);
    });
    output.textures_delta.clear();
    for frame in 0..3 {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        input.time = Some(frame as f64 * 0.1);
        output = context.run_ui(input, |ctx| app.show_table_window(ctx, canvas, &key));
        if frame < 2 {
            output.textures_delta.clear();
        }
    }

    let placeholder_visible = rendered_text_rect(&output, "…").is_some();
    let destination_value_visible = rendered_text_rect(&output, "10.00").is_some();
    output.textures_delta.clear();
    assert!(placeholder_visible);
    assert!(!destination_value_visible);
}

#[test]
fn surface_color_range_streams_finite_values() {
    assert_eq!(
        color_range_for_iter([-4.0, f64::NAN, 10.0]),
        Some((-4.0, 10.0))
    );
    assert_eq!(color_range_for_iter([]), None);
}

#[test]
fn table_compare_mode_requests_the_selected_snapshot() {
    let mut app = app_with_matching_compare_documents();

    app.set_table_compare_mode("table:uid:map", CompareValueMode::Source)
        .unwrap();

    assert_eq!(app.compare.mode, CompareValueMode::Source);
    assert_eq!(
        app.compare.selected_semantic_id.as_deref(),
        Some("table:uid:map")
    );
    assert_eq!(
        app.operations.active().map(|operation| operation.kind),
        Some(OperationKind::BuildingCompareMap)
    );
    app.operations
        .cancel(app.operations.active().unwrap().id, "test transition");
}

#[test]
fn compare_selection_focuses_the_matching_parameter_without_duplicate_tables() {
    let mut app = TunerApp::headless();
    app.open_compare_with_test_documents();

    assert!(app.focus_compare_semantic("table:uid:map"));
    assert!(app.focus_compare_semantic("table:uid:map"));
    assert_eq!(
        app.compare.selected_semantic_id.as_deref(),
        Some("table:uid:map")
    );
    assert_eq!(app.open_tables.len(), 1);
}

#[test]
fn mismatched_compare_xdf_hash_blocks_transfer_plan() {
    let mut app = app_with_compare_documents_and_different_xdfs();

    app.build_transfer_plan(std::collections::BTreeSet::from([
        "table:uid:map".to_string()
    ]))
    .unwrap();
    let result = wait_for_operation_result(&mut app);
    app.install_compare_result(result);

    let plan = app.compare.transfer_plan.as_ref().unwrap();
    assert!(plan.is_blocked());
    assert!(plan
        .issues
        .iter()
        .any(|issue| { issue.code == tuner_transfer::TransferIssueCode::XdfHashMismatch }));
}

#[test]
fn approved_transfer_applies_as_one_destination_undo_entry() {
    let mut app = app_with_matching_compare_documents();
    let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();

    app.build_transfer_plan(std::collections::BTreeSet::from([
        "table:uid:map".to_string()
    ]))
    .unwrap();
    let result = wait_for_operation_result(&mut app);
    app.install_compare_result(result);
    assert!(app.compare.transfer_plan.as_ref().unwrap().is_ready());
    app.apply_transfer_plan().unwrap();

    assert_ne!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        before.as_slice()
    );
    assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), 1);
    assert!(app.workspace.undo().is_ok());
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        before.as_slice()
    );
}

#[test]
fn stale_transfer_plan_does_not_mutate_destination() {
    let mut app = app_with_matching_compare_documents();
    app.build_transfer_plan(std::collections::BTreeSet::from([
        "table:uid:map".to_string()
    ]))
    .unwrap();
    let result = wait_for_operation_result(&mut app);
    app.install_compare_result(result);
    let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let destination_byte = before[0].wrapping_add(1);
    let mut transaction = app.workspace.bin.as_mut().unwrap().transaction("test");
    transaction.write_u8(0, destination_byte).unwrap();
    transaction.commit().unwrap();

    let error = app.apply_transfer_plan().unwrap_err();
    assert!(error.message.contains("changed"));
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes()[0],
        destination_byte
    );
}

#[test]
fn conversion_override_key_includes_xdf_identity_and_target_kind() {
    let parameter = ConversionTarget::parameter("constant:uid:1");
    let axis = ConversionTarget::axis("table:uid:2", 1);

    assert_eq!(parameter.key("xdf-a"), "xdf-a:parameter:constant:uid:1");
    assert_eq!(axis.key("xdf-a"), "xdf-a:axis:table:uid:2:1");
    assert_ne!(parameter.key("xdf-a"), parameter.key("xdf-b"));
}

#[test]
fn project_preferences_round_trip_conversion_overrides_and_old_json_defaults_empty() {
    let mut preferences = ProjectPreferences::for_identity("bin");
    preferences.conversion_overrides.insert(
        "xdf-a:parameter:constant:uid:1".to_string(),
        "X * 2".to_string(),
    );

    let encoded = serde_json::to_string(&preferences).unwrap();
    let decoded: ProjectPreferences = serde_json::from_str(&encoded).unwrap();
    assert_eq!(
        decoded.conversion_overrides,
        preferences.conversion_overrides
    );

    let old: ProjectPreferences = serde_json::from_str(
        r#"{"version":2,"bin_identity":"bin","layout":"Standard","show_browser":true,"show_editor":true,"show_inspector":true,"show_diagnostics":true,"browser_organization":"Categories","catalog_sort":"Title","catalog_sort_direction":"Ascending","browser_filter":"","category_state_initialized":false,"known_categories":[],"collapsed_categories":[],"favorite_tables":[],"recent_tables":[],"browser_collapsed":false,"inspector_collapsed":false,"browser_width":300,"inspector_width":320,"diagnostics_height":150,"table_windows":{},"tab_orders":{},"open_table_keys":[],"active_table_key":null,"search_state":{"open":false,"query":"","match_mode":"Contains","field_scope":"Metadata","sort_key":"Title","sort_direction":"Ascending","result_limit":100,"window":{"x":0,"y":0,"width":900,"height":600,"zoom_percent":100}}}"#,
    )
    .unwrap();
    assert!(old.conversion_overrides.is_empty());
}

#[test]
fn conversion_override_changes_only_effective_xdf_and_can_be_reset() {
    let xdf = XdfDocument::parse(CONVERSION_TABLE_XDF).unwrap();
    let id = xdf.parameters[0].semantic_id.clone();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10])),
        Some(xdf),
        None,
        None,
    );

    let target = ConversionTarget::parameter(id.clone());
    app.apply_conversion_override(target.clone(), "X * 2")
        .unwrap();

    assert_eq!(
        app.workspace
            .source_xdf
            .as_ref()
            .unwrap()
            .parameter(&id)
            .unwrap()
            .conversion
            .as_deref(),
        Some("X")
    );
    assert_eq!(
        app.workspace
            .xdf
            .as_ref()
            .unwrap()
            .parameter(&id)
            .unwrap()
            .conversion
            .as_deref(),
        Some("X * 2")
    );
    assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), 0);

    app.reset_conversion_override(target).unwrap();
    assert_eq!(
        app.workspace
            .xdf
            .as_ref()
            .unwrap()
            .parameter(&id)
            .unwrap()
            .conversion
            .as_deref(),
        Some("X")
    );
}

#[test]
fn conversion_override_from_another_xdf_fingerprint_is_ignored() {
    let xdf = XdfDocument::parse(CONVERSION_TABLE_XDF).unwrap();
    let id = xdf.parameters[0].semantic_id.clone();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10])),
        Some(xdf),
        None,
        None,
    );
    app.project_preferences.conversion_overrides.insert(
        ConversionTarget::parameter(id.clone()).key("different-xdf"),
        "X * 4".to_string(),
    );

    assert_eq!(app.apply_project_conversion_overrides(), 0);
    assert_eq!(
        app.workspace
            .xdf
            .as_ref()
            .unwrap()
            .parameter(&id)
            .unwrap()
            .conversion
            .as_deref(),
        Some("X")
    );
}

#[test]
fn axis_conversion_override_uses_axis_identity_without_touching_bin() {
    let xdf = XdfDocument::parse(AXIS_TABLE_XDF).unwrap();
    let id = xdf.parameters[0].semantic_id.clone();
    let mut app = TunerApp::headless();
    let before = vec![0_u8; 0x40];
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(before.clone())),
        Some(xdf),
        None,
        None,
    );

    let target = ConversionTarget::axis(id.clone(), 0);
    app.apply_conversion_override(target.clone(), "X + 3")
        .unwrap();
    assert_eq!(
        app.workspace
            .xdf
            .as_ref()
            .unwrap()
            .parameter(&id)
            .unwrap()
            .axes[0]
            .conversion
            .as_deref(),
        Some("X + 3")
    );
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        before.as_slice()
    );
    assert!(app.project_preferences.conversion_overrides.contains_key(
        &target.key(
            &app.workspace
                .source_xdf
                .as_ref()
                .unwrap()
                .normalized_fingerprint
        )
    ));
}

#[test]
fn conversion_override_is_auto_saved_in_the_bin_project_preferences() {
    let settings_path = temporary_test_path("settings.json");
    let bin_path = temporary_test_path("project.bin");
    let xdf = XdfDocument::parse(CONVERSION_TABLE_XDF).unwrap();
    let id = xdf.parameters[0].semantic_id.clone();
    let mut app = TunerApp::headless();
    app.settings_path = settings_path.clone();
    app.persistence_enabled = true;
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10])),
        Some(xdf),
        Some(bin_path.clone()),
        None,
    );
    app.activate_project_for_path(&bin_path);
    app.apply_conversion_override(ConversionTarget::parameter(id), "X * 2")
        .unwrap();

    let project_path =
        project_settings_path(&settings_path, app.project_identity.as_deref().unwrap());
    let saved = load_project_preferences(&project_path, app.project_identity.as_deref().unwrap());
    assert_eq!(
        saved
            .conversion_overrides
            .values()
            .next()
            .map(String::as_str),
        Some("X * 2")
    );

    std::fs::remove_file(settings_path).unwrap();
    std::fs::remove_file(project_path).unwrap();
}

#[test]
fn conversion_preview_reports_evaluation_and_inversion_for_candidate_formula() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    let preview = app.conversion_preview(ConversionTarget::parameter(id), "X * 2");
    assert!(preview.parse_error.is_none());
    assert!(preview.invertible);
    assert_eq!(preview.samples[0].engineering, Some(0.0));
}

#[test]
fn invalid_conversion_override_does_not_replace_effective_formula_or_project_state() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    let before = app
        .workspace
        .xdf
        .as_ref()
        .unwrap()
        .parameter(&id)
        .unwrap()
        .conversion
        .clone();
    assert!(app
        .apply_conversion_override(ConversionTarget::parameter(id.clone()), "X +")
        .is_err());
    assert_eq!(
        app.workspace
            .xdf
            .as_ref()
            .unwrap()
            .parameter(&id)
            .unwrap()
            .conversion,
        before
    );
    assert!(app.project_preferences.conversion_overrides.is_empty());
}

#[test]
fn formula_editor_and_save_xdf_commands_are_registered_with_contextual_availability() {
    let registry = CommandRegistry::core();
    let state = WorkspaceState::default();
    let entries = registry.entries(&state);
    let formula = entries
        .iter()
        .find(|entry| entry.descriptor.id == "view.formula-editor")
        .expect("formula editor command should be registered");
    let save = entries
        .iter()
        .find(|entry| entry.descriptor.id == "file.save-xdf-as")
        .expect("XDF Save As command should be registered");
    assert!(!formula.enabled);
    assert!(formula.reason.contains("XDF"));
    assert!(!save.enabled);
    assert!(save.reason.contains("XDF"));
}

fn write_new_xdf(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    std::io::Write::write_all(&mut file, bytes)
}

#[test]
fn xdf_save_as_refuses_existing_target_and_reparses_new_output() {
    let source_path = temporary_test_path("source.xdf");
    let target_path = temporary_test_path("output.xdf");
    std::fs::write(&source_path, CONVERSION_TABLE_XDF).unwrap();
    let document = XdfDocument::load(&source_path).unwrap();
    let original = std::fs::read(&source_path).unwrap();

    let output = document.to_xdf_text().unwrap();
    write_new_xdf(&target_path, output.as_bytes()).unwrap();
    assert!(XdfDocument::load(&target_path).is_ok());
    assert_eq!(std::fs::read(&source_path).unwrap(), original);
    assert!(write_new_xdf(&target_path, output.as_bytes()).is_err());

    std::fs::remove_file(source_path).unwrap();
    std::fs::remove_file(target_path).unwrap();
}

#[test]
fn ui_ipc_conversion_actions_inspect_apply_reset_and_queue_xdf_save() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();

    let capabilities = app.handle_ui_ipc_request(&ui_ipc::UiIpcRequest::for_test(
        "capabilities",
        "",
        None,
        None,
    ));
    let actions = capabilities
        .data
        .as_ref()
        .and_then(|data| data.get("actions"))
        .and_then(Value::as_array)
        .expect("capabilities should contain actions");
    for action in [
        "get_conversion",
        "set_conversion_override",
        "reset_conversion_override",
        "save_xdf_as",
    ] {
        assert!(actions.iter().any(|value| value == action));
    }

    let get = app.handle_ui_ipc_request(&ui_ipc::UiIpcRequest::for_test(
        "get_conversion",
        &id,
        None,
        None,
    ));
    assert!(get.ok);
    let set = app.handle_ui_ipc_request(&ui_ipc::UiIpcRequest::for_test(
        "set_conversion_override",
        &id,
        None,
        Some("X * 2"),
    ));
    assert!(set.ok);
    assert_eq!(
        app.workspace
            .xdf
            .as_ref()
            .unwrap()
            .parameter(&id)
            .unwrap()
            .conversion
            .as_deref(),
        Some("X * 2")
    );
    let reset = app.handle_ui_ipc_request(&ui_ipc::UiIpcRequest::for_test(
        "reset_conversion_override",
        &id,
        None,
        None,
    ));
    assert!(reset.ok);

    let output = temporary_test_path("ipc-output.xdf");
    let mut save = ui_ipc::UiIpcRequest::for_test("save_xdf_as", &id, None, None);
    save.path = Some(output.clone());
    save.task_id = Some(start_quick_xdf_task(&mut app));
    let queued = app.handle_ui_ipc_request(&save);
    assert!(queued.ok, "{}", queued.message);
    assert!(!app.operations.is_busy());
    assert!(!output.exists());
    let answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    assert!(app.complete_nooklink_challenge(&answer));
    assert_eq!(
        app.operations.active().map(|operation| operation.kind),
        Some(OperationKind::SavingXdf)
    );
    let result = (0..100).find_map(|_| {
        let result = app.operations.take_results().into_iter().next();
        if result.is_none() {
            std::thread::sleep(Duration::from_millis(5));
        }
        result
    });
    app.install_operation_result(result.expect("XDF save worker should finish"));
    assert!(output.is_file());
    std::fs::remove_file(output).unwrap();
}

#[test]
fn project_preferences_remember_last_xdf_without_breaking_old_files() {
    let mut preferences = ProjectPreferences::for_identity("bin-a");
    preferences.last_xdf_path = Some(PathBuf::from("C:/calibration/a.xdf"));

    let encoded = serde_json::to_string(&preferences).unwrap();
    let decoded: ProjectPreferences = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.last_xdf_path, preferences.last_xdf_path);

    let mut old_value = serde_json::to_value(ProjectPreferences::for_identity("bin-a")).unwrap();
    old_value.as_object_mut().unwrap().remove("last_xdf_path");
    let old: ProjectPreferences = serde_json::from_value(old_value).unwrap();
    assert_eq!(old.last_xdf_path, None);
}

#[test]
fn path_normalization_respects_host_separator_semantics() {
    assert_eq!(
        normalize_path_text(r"C:\Calibration\SAME.XDF", true),
        "c:/calibration/same.xdf"
    );
    assert_eq!(
        normalize_path_text(r"folder\name.xdf", false),
        r"folder\name.xdf"
    );
}

#[test]
fn last_xdf_prompt_is_project_scoped_and_use_starts_background_load() {
    let path = temporary_test_path("xdf");
    std::fs::write(&path, COLUMN_MAJOR_XDF).unwrap();

    let mut app = TunerApp::headless();
    app.workspace.xdf = Some(XdfDocument::parse(COLUMN_MAJOR_XDF).unwrap());
    app.workspace.xdf_path = Some(PathBuf::from("current.xdf"));
    app.project_identity = Some("bin-a".to_string());
    app.project_preferences = ProjectPreferences::for_identity("bin-a");
    app.project_preferences.last_xdf_path = Some(path.clone());

    let restore_id = app
        .operations
        .begin(OperationKind::RestoringWorkspace, "bin-a");
    let project = app.project_preferences.clone();
    app.install_operation_result(OperationResult::restored(restore_id, "bin-a", project));

    let pending = app
        .pending_xdf_restore
        .as_ref()
        .expect("restoration should offer the remembered XDF");
    assert_eq!(pending.bin_identity, "bin-a");
    assert_eq!(pending.path, path);
    assert!(pending.available);

    app.use_last_xdf().unwrap();
    let active = app
        .operations
        .active()
        .expect("Use last XDF should start a background operation");
    assert_eq!(active.kind, OperationKind::LoadingXdf);
    assert_eq!(active.subject, path.display().to_string());
    assert!(app.pending_xdf_restore.is_none());

    let result = (0..100).find_map(|_| {
        let result = app.operations.take_results().into_iter().next();
        if result.is_none() {
            std::thread::sleep(Duration::from_millis(5));
        }
        result
    });
    app.install_operation_result(result.expect("XDF worker should finish"));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn successful_xdf_load_persists_last_path_but_failures_do_not_replace_it() {
    let settings_path = temporary_test_path("settings.json");
    let bin_path = temporary_test_path("bin");
    let old_path = PathBuf::from("old.xdf");
    let xdf_path = temporary_test_path("xdf");
    std::fs::write(&xdf_path, COLUMN_MAJOR_XDF).unwrap();
    let canonical_xdf_path = std::fs::canonicalize(&xdf_path).unwrap();
    let identity = project_identity_for_path(&bin_path);

    let mut app = TunerApp::headless();
    app.persistence_enabled = true;
    app.settings_path = settings_path.clone();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![1, 2, 3]));
    app.workspace.bin_path = Some(bin_path);
    app.project_identity = Some(identity.clone());
    app.project_preferences = ProjectPreferences::for_identity(&identity);
    app.project_preferences.last_xdf_path = Some(old_path.clone());

    let failed = app
        .operations
        .begin(OperationKind::LoadingXdf, "failed.xdf");
    app.install_operation_result(OperationResult::error(failed, "failed.xdf", "read failed"));
    assert_eq!(
        app.project_preferences.last_xdf_path,
        Some(old_path.clone())
    );
    assert!(app.preferences.recent_xdfs.is_empty());

    let cancelled = app
        .operations
        .begin(OperationKind::LoadingXdf, "cancelled.xdf");
    app.operations.cancel(cancelled, "cancelled");
    assert_eq!(app.project_preferences.last_xdf_path, Some(old_path));
    assert!(app.preferences.recent_xdfs.is_empty());

    let loaded = app
        .operations
        .begin(OperationKind::LoadingXdf, xdf_path.display().to_string());
    let mut result = OperationResult::xdf(loaded, XdfDocument::load(&xdf_path).unwrap());
    result.subject = xdf_path.display().to_string();
    app.install_operation_result(result);

    assert_eq!(
        app.project_preferences.last_xdf_path,
        Some(xdf_path.clone())
    );
    assert_eq!(
        app.preferences.recent_xdfs.first(),
        Some(&canonical_xdf_path)
    );
    assert_eq!(
        load_preferences(&settings_path).recent_xdfs.first(),
        Some(&canonical_xdf_path)
    );
    let saved =
        load_project_preferences(&project_settings_path(&settings_path, &identity), &identity);
    assert_eq!(saved.last_xdf_path, Some(xdf_path.clone()));

    let restore = (0..100).find_map(|_| {
        let result = app.operations.take_results().into_iter().next();
        if result.is_none() {
            std::thread::sleep(Duration::from_millis(5));
        }
        result
    });
    if let Some(restore) = restore {
        app.install_operation_result(restore);
    }
    std::fs::remove_file(xdf_path).unwrap();
    std::fs::remove_file(project_settings_path(&settings_path, &identity)).unwrap();
}

#[test]
fn new_bin_load_clears_pending_xdf_restore_state() {
    let mut app = TunerApp::headless();
    app.pending_xdf_restore = Some(PendingXdfRestore {
        bin_identity: "old-bin".to_string(),
        path: PathBuf::from("old.xdf"),
        available: true,
    });
    let loading = app.operations.begin(OperationKind::LoadingBin, "new.bin");
    let mut result = OperationResult::bin(loading, BinDocument::from_bytes(vec![1, 2, 3]));
    result.subject = "new.bin".to_string();

    app.install_operation_result(result);

    assert!(app.pending_xdf_restore.is_none());
}

#[test]
fn restored_last_xdf_prompt_uses_canonical_paths_and_marks_missing_files() {
    let existing = temporary_test_path("xdf");
    std::fs::write(&existing, COLUMN_MAJOR_XDF).unwrap();
    let alias = existing
        .parent()
        .unwrap()
        .join(".")
        .join(existing.file_name().unwrap());

    let mut app = TunerApp::headless();
    app.project_identity = Some("bin-a".to_string());
    app.workspace.xdf_path = Some(alias);
    app.project_preferences = ProjectPreferences::for_identity("bin-a");
    app.project_preferences.last_xdf_path = Some(existing.clone());
    let restore_id = app
        .operations
        .begin(OperationKind::RestoringWorkspace, "bin-a");
    app.install_operation_result(OperationResult::restored(
        restore_id,
        "bin-a",
        app.project_preferences.clone(),
    ));
    assert!(app.pending_xdf_restore.is_none());

    let missing = PathBuf::from("missing-last.xdf");
    app.workspace.xdf_path = Some(PathBuf::from("current.xdf"));
    app.project_preferences.last_xdf_path = Some(missing.clone());
    let restore_id = app
        .operations
        .begin(OperationKind::RestoringWorkspace, "bin-a");
    app.install_operation_result(OperationResult::restored(
        restore_id,
        "bin-a",
        app.project_preferences.clone(),
    ));
    let pending = app
        .pending_xdf_restore
        .as_ref()
        .expect("a different remembered path should prompt");
    assert_eq!(pending.path, missing);
    assert!(!pending.available);

    if cfg!(windows) {
        app.workspace.xdf_path = Some(PathBuf::from("c:/calibration/same.xdf"));
        app.project_preferences.last_xdf_path = Some(PathBuf::from("C:\\Calibration\\SAME.XDF"));
        let restore_id = app
            .operations
            .begin(OperationKind::RestoringWorkspace, "bin-a");
        app.install_operation_result(OperationResult::restored(
            restore_id,
            "bin-a",
            app.project_preferences.clone(),
        ));
        assert!(app.pending_xdf_restore.is_none());
    }

    std::fs::remove_file(existing).unwrap();
}

#[test]
fn last_xdf_prompt_rejects_missing_or_stale_requests_without_loading() {
    let mut app = TunerApp::headless();
    app.project_identity = Some("bin-a".to_string());
    app.workspace.xdf_path = Some(PathBuf::from("current.xdf"));
    app.project_preferences = ProjectPreferences::for_identity("bin-a");
    app.project_preferences.last_xdf_path = Some(PathBuf::from("missing-last.xdf"));
    let restore_id = app
        .operations
        .begin(OperationKind::RestoringWorkspace, "bin-a");
    app.install_operation_result(OperationResult::restored(
        restore_id,
        "bin-a",
        app.project_preferences.clone(),
    ));

    let error = app.use_last_xdf().unwrap_err();
    assert!(error.message.contains("missing"));
    assert!(app.operations.active().is_none());
    assert!(!app.pending_xdf_restore.as_ref().unwrap().available);

    app.project_identity = Some("bin-b".to_string());
    let error = app.use_last_xdf().unwrap_err();
    assert!(error.message.contains("stale"));
    assert!(app.operations.active().is_none());
    assert!(app.pending_xdf_restore.is_none());
}

#[test]
fn keep_current_xdf_dismisses_only_the_prompt() {
    let xdf = XdfDocument::parse(COLUMN_MAJOR_XDF).unwrap();
    let fingerprint = xdf.normalized_fingerprint.clone();
    let mut app = TunerApp::headless();
    app.workspace.xdf = Some(xdf);
    app.workspace.xdf_path = Some(PathBuf::from("current.xdf"));
    app.pending_xdf_restore = Some(PendingXdfRestore {
        bin_identity: "bin-a".to_string(),
        path: PathBuf::from("remembered.xdf"),
        available: false,
    });

    app.keep_current_xdf();

    assert!(app.pending_xdf_restore.is_none());
    assert_eq!(
        app.workspace
            .xdf
            .as_ref()
            .map(|xdf| xdf.normalized_fingerprint.as_str()),
        Some(fingerprint.as_str())
    );
    assert_eq!(app.workspace.xdf_path, Some(PathBuf::from("current.xdf")));
}

#[test]
fn resetting_preferences_clears_pending_xdf_restore_state() {
    let mut app = TunerApp::headless();
    app.project_identity = Some("bin-a".to_string());
    app.project_preferences = ProjectPreferences::for_identity("bin-a");
    app.project_preferences.last_xdf_path = Some(PathBuf::from("remembered.xdf"));
    app.pending_xdf_restore = Some(PendingXdfRestore {
        bin_identity: "bin-a".to_string(),
        path: PathBuf::from("remembered.xdf"),
        available: false,
    });

    app.reset_all_preferences();

    assert!(app.pending_xdf_restore.is_none());
    assert_eq!(app.project_preferences.last_xdf_path, None);
}

#[test]
fn xdf_reuse_prompt_renders_without_starting_document_work() {
    let context = egui::Context::default();
    let mut app = TunerApp::headless();
    app.project_identity = Some("bin-a".to_string());
    app.pending_xdf_restore = Some(PendingXdfRestore {
        bin_identity: "bin-a".to_string(),
        path: PathBuf::from("missing.xdf"),
        available: false,
    });

    context
        .run_ui(egui::RawInput::default(), |ctx| {
            app.show_xdf_reuse_prompt(ctx);
        })
        .drop_without_applying_deltas();

    let prompt_id = egui::Id::new(("xdf-reuse-prompt", "bin-a"));
    assert!(context.memory(|memory| memory.area_rect(prompt_id).is_some()));
    let prompt_layer = egui::LayerId::new(egui::Order::Foreground, prompt_id);
    assert!(context.memory(|memory| memory.layer_ids().any(|layer| layer == prompt_layer)));
    assert!(app.operations.active().is_none());
    assert!(app.pending_xdf_restore.is_some());
}

#[test]
fn startup_restore_prompt_renders_as_a_foreground_window() {
    let context = egui::Context::default();
    let mut app = TunerApp::headless();
    app.pending_project_restore = Some(PendingProjectRestore {
        bin_path: PathBuf::from("last.bin"),
        xdf_path: Some(PathBuf::from("last.xdf")),
        bin_available: false,
        xdf_available: false,
    });

    context
        .run_ui(egui::RawInput::default(), |ctx| {
            app.show_startup_restore_prompt(ctx);
        })
        .drop_without_applying_deltas();

    let id = egui::Id::new("startup-project-restore");
    let layer = egui::LayerId::new(egui::Order::Foreground, id);
    assert!(context.memory(|memory| memory.area_rect(id).is_some()));
    assert!(context.memory(|memory| memory.layer_ids().any(|candidate| candidate == layer)));
    assert!(app.pending_project_restore.is_some());
    assert!(app.operations.active().is_none());
}

#[test]
fn formula_editor_renders_for_selected_parameter_without_starting_operation() {
    let context = egui::Context::default();
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    app.workspace.select_parameter(&id);

    context
        .run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.show_inspector_contents(ui);
            });
        })
        .drop_without_applying_deltas();

    assert_eq!(
        app.formula_editor_target,
        Some(ConversionTarget::parameter(id))
    );
    assert_eq!(app.formula_editor_draft, "X");
    assert!(app.operations.active().is_none());
}

#[test]
fn new_project_categories_are_collapsed_by_default() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);

    app.activate_project_for_path(Path::new("C:/tuning/new-project.bin"));

    assert!(app.project_preferences.category_state_initialized);
    assert_eq!(
        app.project_preferences.collapsed_categories,
        vec!["Uncategorized"]
    );
}

#[test]
fn resetting_project_categories_rebuilds_saved_state_and_requests_ui_sync() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    app.project_identity = Some("project".to_string());
    app.project_preferences = ProjectPreferences::for_identity("project");
    app.project_preferences.category_state_initialized = true;
    app.project_preferences.known_categories = vec!["Old category".to_string()];
    app.project_preferences.collapsed_categories = vec!["Old category".to_string()];
    app.apply_project_preferences_to_legacy();

    app.reset_all_preferences();

    assert!(app.category_state_sync_pending);
    assert!(app.project_preferences.category_state_initialized);
    assert_eq!(
        app.project_preferences.collapsed_categories,
        vec!["Uncategorized"]
    );
    assert_eq!(app.preferences.collapsed_categories, vec!["Uncategorized"]);
}

#[test]
fn restoring_project_categories_requests_ui_sync_even_when_saved_catalog_is_unchanged() {
    let (xdf, _) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(None, Some(xdf), None, None);
    let operation = app
        .operations
        .begin(OperationKind::RestoringWorkspace, "project");
    let mut project = ProjectPreferences::for_identity("project");
    project.category_state_initialized = true;
    project.known_categories = vec!["Uncategorized".to_string()];
    project.collapsed_categories = vec!["Uncategorized".to_string()];

    app.install_operation_result(OperationResult::restored(operation, "project", project));

    assert!(app.category_state_sync_pending);
    assert_eq!(app.preferences.collapsed_categories, vec!["Uncategorized"]);
}

#[test]
fn browser_applies_pending_saved_category_state_to_existing_egui_memory() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    app.project_identity = Some("project".to_string());
    app.project_preferences = ProjectPreferences::for_identity("project");
    app.project_preferences.category_state_initialized = true;
    app.project_preferences.known_categories = vec!["Uncategorized".to_string()];
    app.project_preferences.collapsed_categories.clear();
    app.apply_project_preferences_to_legacy();
    app.category_state_sync_pending = false;

    let context = egui::Context::default();
    let mut browser_ui_id = None;
    context
        .run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                browser_ui_id = Some(ui.id());
                app.show_browser_contents(ui);
            });
        })
        .drop_without_applying_deltas();
    let browser_content_id = browser_ui_id
        .expect("browser should have a stable UI ID")
        .with(egui::IdSalt::new("child"))
        .with(egui::IdSalt::new("child"));
    let header_id = category_header_id(browser_content_id, "project", "Uncategorized");
    assert!(
        egui::collapsing_header::CollapsingState::load(&context, header_id)
            .expect("browser category state should be persisted")
            .is_open()
    );

    app.preferences.collapsed_categories = vec!["Uncategorized".to_string()];
    app.category_state_sync_pending = true;
    context
        .run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.show_browser_contents(ui);
            });
        })
        .drop_without_applying_deltas();

    assert!(!app.category_state_sync_pending);
    assert!(
        !egui::collapsing_header::CollapsingState::load(&context, header_id)
            .expect("browser category state should remain persisted")
            .is_open()
    );
}

#[test]
fn project_state_is_isolated_by_bin_identity() {
    let (xdf, bin) = column_major_fixture();
    let settings_path = temporary_test_path("settings.json");
    let first_path = Path::new("C:/tuning/first.bin");
    let second_path = Path::new("D:/tuning/second.bin");
    let first_identity = project_identity_for_path(first_path);
    let second_identity = project_identity_for_path(second_path);
    let first_settings = project_settings_path(&settings_path, &first_identity);
    let second_settings = project_settings_path(&settings_path, &second_identity);

    let mut first = ProjectPreferences::for_identity(&first_identity);
    first.category_state_initialized = true;
    first.collapsed_categories = vec!["AT".into()];
    first.browser_width = 410;
    save_project_preferences(&first_settings, &first).unwrap();
    let mut second = ProjectPreferences::for_identity(&second_identity);
    second.category_state_initialized = true;
    second.browser_width = 520;
    save_project_preferences(&second_settings, &second).unwrap();

    let mut app = TunerApp::headless();
    app.settings_path = settings_path;
    app.persistence_enabled = true;
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let original_bytes = app.workspace.bin.as_ref().unwrap().bytes().to_vec();

    app.activate_project_for_path(first_path);
    assert_eq!(app.project_preferences.browser_width, 410);
    app.activate_project_for_path(second_path);
    assert_eq!(app.project_preferences.browser_width, 520);
    app.activate_project_for_path(first_path);
    assert_eq!(app.project_preferences.browser_width, 410);
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), original_bytes);

    std::fs::remove_file(first_settings).unwrap();
    std::fs::remove_file(second_settings).unwrap();
}

#[test]
fn project_state_restores_open_table_layout() {
    let bin_path = Path::new("C:/tuning/restorable.bin");
    let settings_path = temporary_test_path("settings.json");
    let mut app = TunerApp::headless();
    app.settings_path = settings_path.clone();
    app.persistence_enabled = true;
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10, 20])),
        Some(XdfDocument::parse(TWO_TABLE_XDF).unwrap()),
        None,
        None,
    );
    app.activate_project_for_path(bin_path);
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&id));
    let key = app.open_tables[0].key.clone();
    app.open_tables[0].memory = TableWindowMemory {
        x: 140,
        y: 110,
        width: 880,
        height: 640,
        position_saved: true,
        zoom_percent: 135,
        scroll_x: 44,
        scroll_y: 88,
        decimal_places: 2,
        coloring: TableColorSettings::default(),
        fit_to_content: Some(false),
    };
    app.active_table_key = Some(key.clone());
    app.persist_current_project();

    let mut restored = TunerApp::headless();
    restored.settings_path = settings_path;
    restored.persistence_enabled = true;
    restored.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10, 20])),
        Some(XdfDocument::parse(TWO_TABLE_XDF).unwrap()),
        None,
        None,
    );
    restored.activate_project_for_path(bin_path);

    assert_eq!(restored.open_tables.len(), 1);
    assert_eq!(restored.open_tables[0].key, key);
    assert_eq!(restored.open_tables[0].memory.x, 140);
    assert_eq!(restored.open_tables[0].memory.y, 110);
    assert_eq!(restored.open_tables[0].memory.width, 880);
    assert_eq!(restored.open_tables[0].memory.height, 640);
    assert_eq!(restored.open_tables[0].memory.zoom_percent, 135);
    assert_eq!(restored.open_tables[0].memory.scroll_x, 44);
    assert_eq!(restored.open_tables[0].memory.scroll_y, 88);
    assert_eq!(restored.active_table_key.as_deref(), Some(key.as_str()));

    let project_path = project_settings_path(
        &restored.settings_path,
        &project_identity_for_path(bin_path),
    );
    std::fs::remove_file(project_path).unwrap();
    std::fs::remove_file(restored.settings_path).unwrap();
}

#[test]
fn scrollable_frame_policy_keeps_long_content_inside_resizable_frames() {
    let browser = scroll_frame_policy(ScrollFrameKind::Browser);
    assert!(browser.horizontal);
    assert!(browser.vertical);
    assert_eq!(browser.auto_shrink, [false, false]);

    let inspector = scroll_frame_policy(ScrollFrameKind::Inspector);
    assert!(inspector.horizontal);
    assert!(inspector.vertical);
    assert_eq!(inspector.auto_shrink, [false, false]);

    let table = scroll_frame_policy(ScrollFrameKind::Table);
    assert!(table.horizontal);
    assert!(table.vertical);
    assert_eq!(table.auto_shrink, [false, false]);
}

#[test]
fn browser_and_inspector_frames_scroll_long_labels_horizontally() {
    assert!(scroll_frame_policy(ScrollFrameKind::Browser).horizontal);
    assert!(scroll_frame_policy(ScrollFrameKind::Inspector).horizontal);
}

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
    assert_eq!(
        migrated.browser_organization,
        BrowserOrganization::Categories
    );
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
        position_saved: true,
        zoom_percent: 250,
        scroll_x: 12,
        scroll_y: 34,
        decimal_places: 2,
        coloring: TableColorSettings::default(),
        fit_to_content: Some(false),
    };
    memory.sanitize();
    assert_eq!(memory.zoom_percent, 200);
    let encoded = serde_json::to_string(&memory).unwrap();
    assert_eq!(
        serde_json::from_str::<TableWindowMemory>(&encoded).unwrap(),
        memory
    );
}

#[test]
fn typed_window_geometry_marks_defaults_unsaved_and_legacy_rectangles_saved() {
    let defaults = [
        serde_json::to_value(TableWindowMemory::default()).unwrap(),
        serde_json::to_value(HexWindowMemory::default()).unwrap(),
        serde_json::to_value(MapFinderMemory::default()).unwrap(),
        serde_json::to_value(SearchWindowMemory::default()).unwrap(),
        serde_json::to_value(CompareWindowMemory::default()).unwrap(),
        serde_json::to_value(SurfaceViewMemory::default()).unwrap(),
    ];
    assert!(defaults
        .iter()
        .all(|memory| memory["position_saved"] == false));

    let legacy = serde_json::json!({
        "x": 333,
        "y": 222,
        "width": 840,
        "height": 560
    });
    let saved = [
        serde_json::to_value(serde_json::from_value::<TableWindowMemory>(legacy.clone()).unwrap())
            .unwrap(),
        serde_json::to_value(serde_json::from_value::<HexWindowMemory>(legacy.clone()).unwrap())
            .unwrap(),
        serde_json::to_value(serde_json::from_value::<MapFinderMemory>(legacy.clone()).unwrap())
            .unwrap(),
        serde_json::to_value(serde_json::from_value::<SearchWindowMemory>(legacy.clone()).unwrap())
            .unwrap(),
        serde_json::to_value(
            serde_json::from_value::<CompareWindowMemory>(legacy.clone()).unwrap(),
        )
        .unwrap(),
        serde_json::to_value(serde_json::from_value::<SurfaceViewMemory>(legacy).unwrap()).unwrap(),
    ];
    assert!(saved.iter().all(|memory| memory["position_saved"] == true));
}

#[test]
fn legacy_default_tool_rectangles_are_fresh_but_custom_rectangles_remain_saved() {
    let mut legacy = serde_json::to_value(ProjectPreferences::default()).unwrap();
    legacy["version"] = 12.into();
    legacy["dock_state"]["open_tool_windows"] = serde_json::json!([]);
    legacy["compare_window"]
        .as_object_mut()
        .unwrap()
        .remove("position_saved");
    let mut defaults: ProjectPreferences = serde_json::from_value(legacy.clone()).unwrap();
    defaults.sanitize();
    assert_eq!(
        serde_json::to_value(defaults.compare_window).unwrap()["position_saved"],
        false
    );

    legacy["compare_window"]["x"] = 260.into();
    let mut custom: ProjectPreferences = serde_json::from_value(legacy).unwrap();
    custom.sanitize();
    assert_eq!(
        serde_json::to_value(custom.compare_window).unwrap()["position_saved"],
        true
    );
}

#[test]
fn fresh_table_windows_cascade_inside_the_editor_canvas() {
    let xdf = XdfDocument::parse(TWO_TABLE_XDF).unwrap();
    let ids: Vec<_> = xdf
        .parameters
        .iter()
        .map(|parameter| parameter.semantic_id.clone())
        .collect();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10, 20])),
        Some(xdf),
        None,
        None,
    );
    app.project_identity = Some("bin-a".into());
    app.project_preferences = ProjectPreferences::for_identity("bin-a");
    for id in &ids {
        assert!(app.open_table(id));
    }
    let editor = egui::Rect::from_min_size(egui::pos2(320.0, 48.0), egui::vec2(880.0, 690.0));
    app.initial_window_bounds = Some(editor);
    let keys: Vec<_> = app
        .open_tables
        .iter()
        .map(|table| table.key.clone())
        .collect();
    let shell = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 800.0));
    let context = egui::Context::default();
    let mut rects = Vec::new();

    for key in &keys {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(shell);
        let output = context.run_ui(input, |ctx| app.show_table_window(ctx, shell, key));
        output.drop_without_applying_deltas();
        let id = egui::Id::new(("table-window", app.project_scope(), key.as_str()));
        rects.push(
            context
                .memory(|memory| memory.area_rect(id))
                .expect("fresh table should have a visible rect"),
        );
    }

    assert!(rects.iter().all(|rect| editor.contains_rect(*rect)));
    assert_ne!(rects[0].min, rects[1].min);
}

#[test]
fn table_precision_defaults_to_two_and_clamps_old_or_invalid_values() {
    assert_eq!(TableWindowMemory::default().decimal_places, 2);
    let mut memory = TableWindowMemory {
        decimal_places: 9,
        ..TableWindowMemory::default()
    };
    memory.sanitize();
    assert_eq!(memory.decimal_places, 8);
    let old: TableWindowMemory = serde_json::from_str(
        r#"{"x":0,"y":0,"width":760,"height":520,"zoom_percent":100,"scroll_x":0,"scroll_y":0}"#,
    )
    .unwrap();
    assert_eq!(old.decimal_places, 2);
    assert_eq!(format_f64_with_precision(14.0, 2), "14.00");
    assert_eq!(format_f64(14.0), "14.00");
    assert_eq!(raw_value_text(RawValue::Unsigned(14)), "u14");
}

#[test]
fn workspace_axis_selection_reads_and_edits_the_axis_target() {
    let xdf = XdfDocument::parse(AXIS_TABLE_XDF).unwrap();
    let mut bytes = vec![0; 0x40];
    bytes[0x11] = 20;
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(BinDocument::from_bytes(bytes)), Some(xdf), None, None);
    let id = workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(workspace.select_parameter(&id));
    workspace.select_axis(0, 1).unwrap();
    let view = workspace.selected_axis_view().unwrap();
    assert_eq!(view.raw, Some(RawValue::Unsigned(20)));
    assert_eq!(view.engineering, Some(10.0));
    assert!(view.editable);
    workspace.apply_engineering_text("12.50").unwrap();
    assert_eq!(
        workspace.selected_axis_view().unwrap().raw,
        Some(RawValue::Unsigned(25))
    );
    workspace.undo().unwrap();
    assert_eq!(
        workspace.selected_axis_view().unwrap().raw,
        Some(RawValue::Unsigned(20))
    );
    workspace.select_cell(0, 0).unwrap();
    assert_eq!(workspace.selected_axis, None);
}

#[test]
fn axis_header_batch_matches_scalar_views() {
    let assert_batch_matches_scalar = |workspace: &WorkspaceState, id: &str, axis_index| {
        let batched = workspace.axis_views_for(id, axis_index);
        for (index, view) in batched.iter().enumerate() {
            assert_eq!(view, &workspace.axis_view_for(id, axis_index, index));
        }
        batched
    };
    let mut bytes = vec![0; 0x40];
    bytes[0x10] = 2;
    bytes[0x11] = 20;
    bytes[0x20] = 10;
    bytes[0x22] = 30;
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(
        Some(BinDocument::from_bytes(bytes)),
        Some(XdfDocument::parse(AXIS_TABLE_XDF).unwrap()),
        None,
        None,
    );
    let id = workspace.filtered_parameters("")[0].semantic_id.clone();
    let x_views = assert_batch_matches_scalar(&workspace, &id, 0);
    let y_views = assert_batch_matches_scalar(&workspace, &id, 1);
    assert_eq!(x_views[1].as_ref().unwrap().engineering, Some(10.0));
    assert_eq!(
        y_views[1].as_ref().unwrap().raw,
        Some(RawValue::Unsigned(30))
    );
    assert!(workspace.axis_view_for(&id, 0, 2).is_err());

    let invalid_xdf = String::from_utf8(AXIS_TABLE_XDF.to_vec())
        .unwrap()
        .replace("X * 0.5", "X / 0");
    let mut invalid = WorkspaceState::default();
    invalid.set_documents(
        Some(BinDocument::from_bytes(vec![0; 0x40])),
        Some(XdfDocument::parse(invalid_xdf.as_bytes()).unwrap()),
        None,
        None,
    );
    let invalid_id = invalid.filtered_parameters("")[0].semantic_id.clone();
    let invalid_views = assert_batch_matches_scalar(&invalid, &invalid_id, 0);
    assert!(invalid_views[0].as_ref().unwrap().error.is_some());

    let mut missing_bin = WorkspaceState::default();
    missing_bin.set_documents(
        None,
        Some(XdfDocument::parse(AXIS_TABLE_XDF).unwrap()),
        None,
        None,
    );
    let missing_id = missing_bin.filtered_parameters("")[0].semantic_id.clone();
    let missing_views = assert_batch_matches_scalar(&missing_bin, &missing_id, 0);
    assert!(missing_views[0]
        .as_ref()
        .unwrap()
        .error
        .as_deref()
        .unwrap()
        .contains("BIN"));

    let mut no_storage = WorkspaceState::default();
    no_storage.set_documents(
        Some(BinDocument::from_bytes(vec![0; 0x40])),
        Some(XdfDocument::parse(DESCRIPTIVE_AXIS_XDF).unwrap()),
        None,
        None,
    );
    let no_storage_id = no_storage.filtered_parameters("")[0].semantic_id.clone();
    let no_storage_views = assert_batch_matches_scalar(&no_storage, &no_storage_id, 0);
    assert!(!no_storage_views[0].as_ref().unwrap().editable);
}

#[test]
fn descriptive_axis_view_is_labeled_and_has_no_synthetic_storage() {
    let xdf = XdfDocument::parse(DESCRIPTIVE_AXIS_XDF).unwrap();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0; 0x40])),
        Some(xdf),
        None,
        None,
    );
    let id = workspace.filtered_parameters("")[0].semantic_id.clone();
    let view = workspace.axis_view_for(&id, 0, 1).unwrap();
    assert_eq!(view.label.as_deref(), Some("High"));
    assert_eq!(view.range, None);
    assert_eq!(view.raw, None);
    assert_eq!(view.engineering, None);
    assert!(!view.editable);
    workspace.select_parameter(&id);
    workspace.select_axis(0, 1).unwrap();
    assert!(workspace.apply_engineering_text("12.5").is_err());
    assert_eq!(workspace.bin.as_ref().unwrap().bytes()[0x11], 0);
}

#[test]
fn axis_view_reports_missing_bin_and_conversion_failures() {
    let xdf = XdfDocument::parse(AXIS_TABLE_XDF).unwrap();
    let mut without_bin = WorkspaceState::default();
    without_bin.set_documents(None, Some(xdf), None, None);
    let id = without_bin.filtered_parameters("")[0].semantic_id.clone();
    let missing_bin = without_bin.axis_view_for(&id, 0, 1).unwrap();
    assert!(missing_bin.error.as_deref().unwrap().contains("BIN"));

    let invalid_conversion = String::from_utf8(AXIS_TABLE_XDF.to_vec())
        .unwrap()
        .replace("X * 0.5", "X / 0");
    let mut conversion_failure = WorkspaceState::default();
    conversion_failure.set_documents(
        Some(BinDocument::from_bytes({
            let mut bytes = vec![0; 0x40];
            bytes[0x11] = 20;
            bytes
        })),
        Some(XdfDocument::parse(invalid_conversion.as_bytes()).unwrap()),
        None,
        None,
    );
    let id = conversion_failure.filtered_parameters("")[0]
        .semantic_id
        .clone();
    let view = conversion_failure.axis_view_for(&id, 0, 1).unwrap();
    assert!(view.error.is_some());
    assert_eq!(view.raw, Some(RawValue::Unsigned(20)));
}

#[test]
fn axis_write_rounds_fractional_raw_value_and_rejects_out_of_width_atomically() {
    let xdf = XdfDocument::parse(AXIS_TABLE_XDF).unwrap();
    let mut bytes = vec![0; 0x40];
    bytes[0x11] = 20;
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(BinDocument::from_bytes(bytes)), Some(xdf), None, None);
    let id = workspace.filtered_parameters("")[0].semantic_id.clone();
    workspace.select_parameter(&id);
    workspace.select_axis(0, 1).unwrap();
    let before = workspace.bin.as_ref().unwrap().bytes().to_vec();
    let result = workspace.apply_engineering_text("12.25").unwrap();
    assert_eq!(result.raw_value, RawValue::Unsigned(25));
    assert_eq!(result.stored_engineering, 12.5);
    assert!(workspace.status.text.contains("12.250000"));
    assert!(workspace.status.text.contains("12.500000"));
    assert_eq!(workspace.bin.as_ref().unwrap().undo_depth(), 1);

    let after_rounding = workspace.bin.as_ref().unwrap().bytes().to_vec();
    assert!(workspace.apply_engineering_text("128").is_err());
    assert_eq!(workspace.status.level, StatusLevel::Error);
    assert_eq!(
        workspace.bin.as_ref().unwrap().bytes(),
        after_rounding.as_slice()
    );
    assert_eq!(workspace.bin.as_ref().unwrap().undo_depth(), 1);

    workspace.undo().unwrap();
    assert_eq!(workspace.bin.as_ref().unwrap().bytes(), before.as_slice());
}

#[test]
fn command_availability_explains_missing_documents() {
    let state = WorkspaceState::default();
    let registry = CommandRegistry::core();
    let apply = registry
        .entries(&state)
        .into_iter()
        .find(|entry| entry.descriptor.id == "edit.apply-cell")
        .unwrap();
    assert!(!apply.enabled);
    assert!(apply.reason.contains("BIN"));
}

fn column_major_fixture() -> (XdfDocument, BinDocument) {
    let xdf = XdfDocument::parse(COLUMN_MAJOR_XDF).unwrap();
    (xdf, BinDocument::from_bytes(vec![10, 20, 30, 40, 50, 60]))
}

fn agent_proposal_app() -> (TunerApp, String, String) {
    let (xdf, bin) = column_major_fixture();
    let semantic_id = xdf.parameters[0].semantic_id.clone();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let identity = DocumentIdentity {
        bin_sha256: Some(sha256_hex(app.workspace.bin.as_ref().unwrap().bytes())),
        xdf_sha256: Some(app.workspace.xdf.as_ref().unwrap().exact_sha256.clone()),
        workspace_data_revision: app.workspace.data_revision(),
    };
    let task_id = app
        .agent_tasks
        .start(
            AgentCapability::Diagnostics,
            "review cells".into(),
            identity,
        )
        .unwrap();
    app.agent_tasks
        .approve_start(&task_id, AgentTaskLevel::Quick)
        .unwrap();
    (app, task_id, semantic_id)
}

fn agent_proposal_report(semantic_id: &str) -> AgentReport {
    AgentReport {
        summary: "Check fuel map".into(),
        findings: vec!["Cell may need adjustment".into()],
        confidence: Some(0.7),
        evidence: Vec::new(),
        operations: vec![ProposedOperation::SetEngineeringCell {
            semantic_id: semantic_id.into(),
            row: 0,
            column: 0,
            engineering_value: 77.0,
        }],
    }
}

fn conversion_agent_proposal_app(
    with_override: bool,
) -> (TunerApp, String, String, ConversionTarget) {
    let xdf = XdfDocument::parse(CONVERSION_TABLE_XDF).unwrap();
    let semantic_id = xdf.parameters[0].semantic_id.clone();
    let target = ConversionTarget::parameter(semantic_id.clone());
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10])),
        Some(xdf),
        None,
        None,
    );
    if with_override {
        app.apply_conversion_override(target.clone(), "X * 2")
            .unwrap();
    }
    let identity = DocumentIdentity {
        bin_sha256: Some(sha256_hex(app.workspace.bin.as_ref().unwrap().bytes())),
        xdf_sha256: Some(app.workspace.xdf.as_ref().unwrap().exact_sha256.clone()),
        workspace_data_revision: app.workspace.data_revision(),
    };
    let task_id = app
        .agent_tasks
        .start(
            AgentCapability::Diagnostics,
            "review conversion".into(),
            identity,
        )
        .unwrap();
    app.agent_tasks
        .approve_start(&task_id, AgentTaskLevel::Quick)
        .unwrap();
    (app, task_id, semantic_id, target)
}

fn conversion_agent_proposal(semantic_id: &str) -> AgentReport {
    AgentReport {
        summary: "Review conversion mapping".into(),
        findings: Vec::new(),
        confidence: Some(0.8),
        evidence: Vec::new(),
        operations: vec![ProposedOperation::SetEngineeringCell {
            semantic_id: semantic_id.into(),
            row: 0,
            column: 0,
            engineering_value: 80.0,
        }],
    }
}

#[test]
fn agent_proposal_set_override_while_challenged_marks_stale_without_writing() {
    let (mut app, task_id, semantic_id, target) = conversion_agent_proposal_app(false);
    app.agent_tasks
        .submit(&task_id, conversion_agent_proposal(&semantic_id))
        .unwrap();
    app.review_agent_proposal(&task_id, true).unwrap();
    let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let undo_depth = app.workspace.bin.as_ref().unwrap().undo_depth();
    app.apply_conversion_override(target, "X * 2").unwrap();
    let answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    assert!(!app.complete_nooklink_challenge(&answer));
    assert_eq!(
        app.agent_tasks.poll(&task_id).unwrap().status,
        AgentTaskStatus::Stale
    );
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), before);
    assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), undo_depth);
}

#[test]
fn agent_proposal_reset_override_while_challenged_marks_stale_without_writing() {
    let (mut app, task_id, semantic_id, target) = conversion_agent_proposal_app(true);
    app.agent_tasks
        .submit(&task_id, conversion_agent_proposal(&semantic_id))
        .unwrap();
    app.review_agent_proposal(&task_id, true).unwrap();
    let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let undo_depth = app.workspace.bin.as_ref().unwrap().undo_depth();
    app.reset_conversion_override(target).unwrap();
    let answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    assert!(!app.complete_nooklink_challenge(&answer));
    assert_eq!(
        app.agent_tasks.poll(&task_id).unwrap().status,
        AgentTaskStatus::Stale
    );
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), before);
    assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), undo_depth);
}

#[test]
fn agent_proposal_edit_label_tracks_pending_and_applied_status() {
    assert_eq!(
        agent_proposal_edit_label(AgentTaskStatus::AwaitingReview),
        "Proposed ROM edits (not yet applied):"
    );
    assert_eq!(
        agent_proposal_edit_label(AgentTaskStatus::Applied),
        "Applied ROM edits:"
    );
}

#[test]
fn agent_proposal_stale_bin_or_xdf_blocks_review_without_writing() {
    for changed_bin in [true, false] {
        let (mut app, task_id, semantic_id) = agent_proposal_app();
        app.agent_tasks
            .submit(&task_id, agent_proposal_report(&semantic_id))
            .unwrap();
        if changed_bin {
            let mut transaction = app
                .workspace
                .bin
                .as_mut()
                .unwrap()
                .transaction("native edit");
            transaction.write_bytes(0, &[11]).unwrap();
            transaction.commit().unwrap();
        } else {
            app.workspace.xdf = Some(XdfDocument::parse(TWO_TABLE_XDF).unwrap());
        }
        let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
        let undo_depth = app.workspace.bin.as_ref().unwrap().undo_depth();
        assert!(app.review_agent_proposal(&task_id, true).is_err());
        assert_eq!(
            app.agent_tasks.poll(&task_id).unwrap().status,
            AgentTaskStatus::Stale
        );
        assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), before);
        assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), undo_depth);
        assert!(app.nooklink_challenge.is_none());
    }
}

#[test]
fn agent_proposal_invalid_late_cell_rolls_back_all_bytes_and_history() {
    let (mut app, _, semantic_id) = agent_proposal_app();
    let mut operations = agent_proposal_report(&semantic_id).operations;
    operations.push(ProposedOperation::SetEngineeringCell {
        semantic_id,
        row: 999,
        column: 0,
        engineering_value: 88.0,
    });
    let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let undo_depth = app.workspace.bin.as_ref().unwrap().undo_depth();
    assert!(app.workspace.preview_agent_operations(&operations).is_err());
    assert!(app.workspace.apply_agent_operations(&operations).is_err());
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), before);
    assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), undo_depth);
}

#[test]
fn agent_proposal_wrong_answer_never_writes_and_correct_answer_is_one_undoable_edit() {
    let (mut app, task_id, semantic_id) = agent_proposal_app();
    let mut report = agent_proposal_report(&semantic_id);
    report
        .operations
        .push(ProposedOperation::SetEngineeringCell {
            semantic_id,
            row: 1,
            column: 0,
            engineering_value: 88.0,
        });
    app.agent_tasks.submit(&task_id, report).unwrap();
    app.review_agent_proposal(&task_id, true).unwrap();
    let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    assert!(!app.complete_nooklink_challenge("wrong"));
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), before);
    assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), 0);
    assert_eq!(
        app.agent_tasks.poll(&task_id).unwrap().status,
        AgentTaskStatus::AwaitingReview
    );
    assert!(app.complete_nooklink_challenge(&answer));
    assert_eq!(
        app.agent_tasks.poll(&task_id).unwrap().status,
        AgentTaskStatus::Applied
    );
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes()[0], 77);
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes()[1], 88);
    assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), 1);
    app.workspace.undo().unwrap();
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), before);
}

#[test]
fn agent_proposal_submit_marks_changed_identity_stale() {
    for changed_bin in [true, false] {
        let (mut app, task_id, semantic_id) = agent_proposal_app();
        if changed_bin {
            app.workspace.bin = Some(BinDocument::from_bytes(vec![11, 20, 30, 40, 50, 60]));
        } else {
            app.workspace.xdf = Some(XdfDocument::parse(TWO_TABLE_XDF).unwrap());
        }
        let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
        let submit = agent_ipc_request(json!({
            "action": "agent_task",
            "agent_task": {"kind": "submit", "task_id": task_id, "report": agent_proposal_report(&semantic_id)}
        }));
        let response = app.handle_ui_ipc_request(&submit);
        assert!(response.ok, "{}", response.message);
        assert_eq!(response.data.unwrap()["status"], "stale");
        assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), before);
        assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), 0);
    }
}

#[test]
fn agent_proposal_invalid_submit_and_challenge_time_change_never_write() {
    let (mut app, task_id, semantic_id) = agent_proposal_app();
    let mut report = agent_proposal_report(&semantic_id);
    report
        .operations
        .push(ProposedOperation::SetEngineeringCell {
            semantic_id: semantic_id.clone(),
            row: 999,
            column: 0,
            engineering_value: 12.0,
        });
    let submit = agent_ipc_request(json!({
        "action": "agent_task", "agent_task": {"kind": "submit", "task_id": task_id, "report": report}
    }));
    assert_eq!(
        app.handle_ui_ipc_request(&submit).data.unwrap()["status"],
        "stale"
    );
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        &[10, 20, 30, 40, 50, 60]
    );
    assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), 0);

    let (mut app, task_id, semantic_id) = agent_proposal_app();
    app.agent_tasks
        .submit(&task_id, agent_proposal_report(&semantic_id))
        .unwrap();
    app.review_agent_proposal(&task_id, true).unwrap();
    let answer = app
        .nooklink_challenge
        .as_ref()
        .unwrap()
        .expected_answer
        .to_string();
    app.workspace.xdf = Some(XdfDocument::parse(TWO_TABLE_XDF).unwrap());
    assert!(!app.complete_nooklink_challenge(&answer));
    assert_eq!(
        app.agent_tasks.poll(&task_id).unwrap().status,
        AgentTaskStatus::Stale
    );
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        &[10, 20, 30, 40, 50, 60]
    );
    assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), 0);
}

#[test]
fn agent_proposal_reject_and_read_only_accept_preserve_bin_history() {
    let (mut app, task_id, semantic_id) = agent_proposal_app();
    app.agent_tasks
        .submit(&task_id, agent_proposal_report(&semantic_id))
        .unwrap();
    app.review_agent_proposal(&task_id, false).unwrap();
    assert_eq!(
        app.agent_tasks.poll(&task_id).unwrap().status,
        AgentTaskStatus::Rejected
    );
    assert!(app.nooklink_challenge.is_none());
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        &[10, 20, 30, 40, 50, 60]
    );
    assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), 0);

    let (mut app, task_id, _) = agent_proposal_app();
    app.agent_tasks
        .submit(
            &task_id,
            AgentReport {
                summary: "No changes needed".into(),
                findings: Vec::new(),
                confidence: None,
                evidence: Vec::new(),
                operations: Vec::new(),
            },
        )
        .unwrap();
    app.review_agent_proposal(&task_id, true).unwrap();
    assert_eq!(
        app.agent_tasks.poll(&task_id).unwrap().status,
        AgentTaskStatus::Accepted
    );
    assert!(app.nooklink_challenge.is_none());
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        &[10, 20, 30, 40, 50, 60]
    );
    assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), 0);
}

#[test]
fn cell_range_selection_normalizes_drag_and_select_all() {
    let (xdf, bin) = column_major_fixture();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);

    workspace
        .select_cell_range(1, 2, 0, 1)
        .expect("drag range should be valid");
    let selection = workspace.selected_cell_range().unwrap();
    assert_eq!(selection.bounds(), (0, 1, 1, 2));
    assert!(selection.contains(0, 1));
    assert!(selection.contains(1, 2));
    assert!(!selection.contains(0, 0));

    workspace.select_all_cells().unwrap();
    assert_eq!(
        workspace.selected_cell_range().unwrap().bounds(),
        (0, 0, 1, 2)
    );
    assert_eq!(workspace.selected_cell, (0, 0));
}

#[test]
fn selected_cells_copy_as_displayed_grid_and_paste_as_one_edit() {
    let (xdf, bin) = column_major_fixture();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);
    workspace.select_cell_range(0, 1, 1, 2).unwrap();

    assert_eq!(
        workspace
            .copy_selected_cells(TableDisplay::Engineering, 2)
            .unwrap(),
        "30.00\t50.00\n40.00\t60.00"
    );
    assert_eq!(workspace.paste_cells("101\t102\n103\t104").unwrap(), 4);
    assert_eq!(
        workspace.bin.as_ref().unwrap().bytes(),
        &[10, 20, 101, 103, 102, 104]
    );
    assert_eq!(workspace.bin.as_ref().unwrap().undo_depth(), 1);
}

#[test]
fn invalid_multicell_paste_leaves_bin_and_undo_history_unchanged() {
    let (xdf, bin) = column_major_fixture();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);
    workspace.select_cell_range(0, 1, 1, 2).unwrap();
    let before = workspace.bin.as_ref().unwrap().bytes().to_vec();

    assert!(workspace.paste_cells("101\t102\n103").is_err());
    assert_eq!(workspace.bin.as_ref().unwrap().bytes(), before.as_slice());
    assert_eq!(workspace.bin.as_ref().unwrap().undo_depth(), 0);
}

#[test]
fn selected_cell_operations_apply_as_one_engineering_edit_and_undo_together() {
    let (xdf, bin) = column_major_fixture();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);
    workspace.select_cell_range(0, 1, 1, 2).unwrap();

    assert_eq!(workspace.add_to_selected_cells(10.0).unwrap(), 4);
    assert_eq!(
        workspace.bin.as_ref().unwrap().bytes(),
        &[10, 20, 40, 50, 60, 70]
    );
    assert_eq!(workspace.bin.as_ref().unwrap().undo_depth(), 1);

    workspace.undo().unwrap();
    assert_eq!(
        workspace.bin.as_ref().unwrap().bytes(),
        &[10, 20, 30, 40, 50, 60]
    );
    assert_eq!(workspace.bin.as_ref().unwrap().redo_depth(), 1);
}

#[test]
fn engineering_apply_fills_the_highlighted_range_as_one_undoable_edit() {
    let (xdf, bin) = column_major_fixture();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);
    workspace.select_cell_range(0, 1, 1, 2).unwrap();

    workspace.apply_engineering_text("0").unwrap();

    assert_eq!(
        workspace.bin.as_ref().unwrap().bytes(),
        &[10, 20, 0, 0, 0, 0]
    );
    assert_eq!(workspace.bin.as_ref().unwrap().undo_depth(), 1);
    assert_eq!(
        workspace.selected_cell_range().unwrap().bounds(),
        (0, 1, 1, 2)
    );
    assert_eq!(workspace.edit_text, "0");
    workspace.undo().unwrap();
    assert_eq!(
        workspace.bin.as_ref().unwrap().bytes(),
        &[10, 20, 30, 40, 50, 60]
    );
}

#[test]
#[ignore = "requires the local-only SCGa05 calibration BIN/XDF fixture"]
fn engineering_apply_rounds_integer_map_fill_and_reports_stored_value() {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Test bin and xdf");
    let xdf = XdfDocument::load(fixture_dir.join("SCGa05_cal.xdf")).unwrap();
    let bin = BinDocument::load(fixture_dir.join("SCGa05_cal.bin")).unwrap();
    let parameter = xdf
        .parameters
        .iter()
        .find(|parameter| parameter.title == "Turbo Max Pressure Ratio")
        .expect("the supplied fixture should include the reproduced map");
    assert_eq!(parameter.layout.element_width_bits, 16);
    assert_eq!(
        parameter.layout.storage.numeric_kind,
        tuner_xdf::NumericKind::Integer
    );
    let semantic_id = parameter.semantic_id.clone();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);
    assert!(workspace.select_parameter(&semantic_id));
    workspace.select_cell_range(6, 6, 7, 7).unwrap();
    let before = workspace.bin.as_ref().unwrap().bytes().to_vec();

    let result = workspace.apply_engineering_text("3.20").unwrap();

    assert_eq!(result.requested_engineering, 3.2);
    assert_eq!(result.raw_value, RawValue::Unsigned(13_107));
    assert_eq!(result.stored_engineering, 13_107.0 / 4096.0);
    for row in 6..=7 {
        for column in 6..=7 {
            assert_eq!(
                workspace
                    .selected_parameter()
                    .unwrap()
                    .read_raw_cell(workspace.bin.as_ref().unwrap(), row, column)
                    .unwrap(),
                RawValue::Unsigned(13_107)
            );
        }
    }
    assert_ne!(workspace.bin.as_ref().unwrap().bytes(), before.as_slice());
    assert_eq!(workspace.bin.as_ref().unwrap().undo_depth(), 1);
    assert!(workspace.status.text.contains("3.200000"));
    assert!(workspace.status.text.contains("3.199951"));
    assert!(workspace.status.text.contains("u13107"));

    workspace.undo().unwrap();
    assert_eq!(workspace.bin.as_ref().unwrap().bytes(), before.as_slice());

    let before_overflow = workspace.bin.as_ref().unwrap().bytes().to_vec();
    let undo_depth = workspace.bin.as_ref().unwrap().undo_depth();
    assert!(workspace.apply_engineering_text("20").is_err());
    assert_eq!(workspace.status.level, StatusLevel::Error);
    assert_eq!(
        workspace.bin.as_ref().unwrap().bytes(),
        before_overflow.as_slice()
    );
    assert_eq!(workspace.bin.as_ref().unwrap().undo_depth(), undo_depth);
}

#[test]
fn surface_engineering_values_commit_as_one_edit_and_undo_together() {
    let (xdf, bin) = column_major_fixture();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);
    let id = workspace.filtered_parameters("")[0].semantic_id.clone();
    workspace.select_parameter(&id);
    workspace.select_cell_range(0, 0, 1, 1).unwrap();

    let count = workspace
        .apply_engineering_cell_values(&[
            (0, 0, 101.0),
            (0, 1, 102.0),
            (1, 0, 103.0),
            (1, 1, 104.0),
        ])
        .unwrap();

    assert_eq!(count, 4);
    assert_eq!(
        workspace.bin.as_ref().unwrap().bytes(),
        &[101, 103, 102, 104, 50, 60]
    );
    assert_eq!(workspace.bin.as_ref().unwrap().undo_depth(), 1);
    workspace.undo().unwrap();
    assert_eq!(
        workspace.bin.as_ref().unwrap().bytes(),
        &[10, 20, 30, 40, 50, 60]
    );
}

#[test]
fn failed_engineering_batch_rolls_back_cells_written_before_overflow() {
    let (xdf, bin) = column_major_fixture();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = workspace.filtered_parameters("")[0].semantic_id.clone();
    workspace.select_parameter(&semantic_id);
    let before = workspace.bin.as_ref().unwrap().bytes().to_vec();

    assert!(workspace
        .apply_engineering_cell_values(&[(0, 0, 101.0), (0, 1, 300.0)])
        .is_err());

    assert_eq!(workspace.status.level, StatusLevel::Error);
    assert_eq!(workspace.bin.as_ref().unwrap().bytes(), before.as_slice());
    assert_eq!(workspace.bin.as_ref().unwrap().undo_depth(), 0);
}

#[test]
fn invalid_selected_cell_operation_is_atomic() {
    let (xdf, bin) = column_major_fixture();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);
    workspace.select_cell_range(0, 1, 1, 2).unwrap();
    let before = workspace.bin.as_ref().unwrap().bytes().to_vec();

    assert!(workspace
        .apply_cell_operation(CellOperation::Clamp {
            min: 100.0,
            max: 10.0,
        })
        .is_err());
    assert_eq!(workspace.bin.as_ref().unwrap().bytes(), before.as_slice());
    assert_eq!(workspace.bin.as_ref().unwrap().undo_depth(), 0);
}

#[test]
fn cell_commands_are_registered_and_use_the_shared_selection_state() {
    let registry = CommandRegistry::core();
    for id in [
        "edit.copy-cells",
        "edit.copy-cells-engineering",
        "edit.copy-cells-raw",
        "edit.paste-cells",
        "edit.select-all-cells",
        "edit.fill-cells",
        "edit.zero-cells",
        "edit.add-cells",
        "edit.multiply-cells",
        "edit.clamp-cells",
    ] {
        assert!(
            registry
                .descriptors()
                .iter()
                .any(|command| command.id == id),
            "missing stable command {id}"
        );
    }
}

#[test]
fn cell_commands_dispatch_copy_transform_and_paste_requests() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    app.workspace.select_cell_range(0, 1, 1, 2).unwrap();

    app.dispatch_command("edit.copy-cells").unwrap();
    assert_eq!(
        app.pending_clipboard_text.as_deref(),
        Some("30.00\t50.00\n40.00\t60.00")
    );

    app.workspace.edit_text = "10".to_string();
    app.dispatch_command("edit.add-cells").unwrap();
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        &[10, 20, 40, 50, 60, 70]
    );

    app.dispatch_command("edit.paste-cells").unwrap();
    assert_eq!(
        app.pending_grid_paste_table.as_deref(),
        app.active_table_key.as_deref()
    );
}

#[test]
fn edit_menu_action_history_opens_history_for_the_active_table() {
    let mut app = app_with_project_fixture();
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let table_key = app.active_table_key.clone().unwrap();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
    let mut time = 0.0;

    click_toolbar_label(&mut app, "Edit", &context, screen, &mut time);
    let output = render_toolbar_frame(&mut app, &context, screen, time, Vec::new());
    let history_point = rendered_text_rect(&output, "Action History…").map(|rect| rect.center());
    output.drop_without_applying_deltas();
    let history_point = history_point.expect("Edit menu should expose Action History");
    for pressed in [true, false] {
        time += 0.1;
        render_toolbar_frame(
            &mut app,
            &context,
            screen,
            time,
            vec![
                egui::Event::PointerMoved(history_point),
                egui::Event::PointerButton {
                    pos: history_point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        )
        .drop_without_applying_deltas();
    }

    let id = WindowId::ActionHistory(table_key.clone());
    assert_eq!(
        app.action_history_table.as_deref(),
        Some(table_key.as_str())
    );
    assert!(app.project_preferences.dock_state.is_open_tool(&id));
}

#[test]
fn clipboard_parser_round_trips_integer_and_binary32_raw_tokens() {
    assert_eq!(
        parse_clipboard_cell("u42").unwrap(),
        ClipboardCell::Raw(RawValue::Unsigned(42))
    );
    assert_eq!(
        parse_clipboard_cell("i-7").unwrap(),
        ClipboardCell::Raw(RawValue::Signed(-7))
    );
    assert_eq!(
        parse_clipboard_cell("f32 1.0 (0x3F800000)").unwrap(),
        ClipboardCell::Raw(RawValue::Float32Bits(0x3F80_0000))
    );
}

#[test]
fn table_pointer_press_requires_focus_raise_even_without_child_click() {
    assert!(table_pointer_should_raise(true, true));
    assert!(!table_pointer_should_raise(false, true));
    assert!(!table_pointer_should_raise(true, false));
}

#[test]
fn releasing_another_table_does_not_cancel_cell_drag() {
    let mut drag = Some(CellDragState {
        table_key: "table-a".to_string(),
        anchor: (1, 2),
        focus: (2, 3),
    });

    assert!(take_cell_drag_for_table(&mut drag, "table-b").is_none());
    assert!(drag.is_some());
    assert_eq!(
        take_cell_drag_for_table(&mut drag, "table-a").map(|drag| drag.focus),
        Some((2, 3))
    );
    assert!(drag.is_none());
}

#[test]
fn releasing_another_surface_does_not_cancel_surface_selection_drag() {
    let mut drag = Some(SurfaceSelectionDrag {
        surface_key: "surface-a".to_string(),
        start: egui::pos2(10.0, 20.0),
    });

    assert!(take_surface_selection_drag(&mut drag, "surface-b").is_none());
    assert!(drag.is_some());
    assert_eq!(
        take_surface_selection_drag(&mut drag, "surface-a").map(|drag| drag.start),
        Some(egui::pos2(10.0, 20.0))
    );
    assert!(drag.is_none());
}

#[test]
fn table_window_routes_ctrl_a_from_a_real_egui_key_event() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let mut first_input = egui::RawInput::default();
    first_input.screen_rect = Some(canvas);
    context
        .run_ui(first_input, |ctx| {
            app.show_table_window(ctx, canvas, &key);
        })
        .drop_without_applying_deltas();
    let window_id = egui::Id::new(("table-window", "no-project", key.as_str()));
    let pointer = context
        .memory(|memory| memory.area_rect(window_id))
        .expect("table window should have a geometry")
        .center();
    let mut key_input = egui::RawInput::default();
    key_input.screen_rect = Some(canvas);
    key_input.events = vec![
        egui::Event::PointerMoved(pointer),
        egui::Event::ModifiersChanged(egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        }),
        egui::Event::Key {
            key: egui::Key::A,
            physical_key: Some(egui::Key::A),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            },
        },
    ];
    context
        .run_ui(key_input, |ctx| {
            app.show_table_window(ctx, canvas, &key);
        })
        .drop_without_applying_deltas();

    assert_eq!(
        app.workspace.selected_cell_range().unwrap().bounds(),
        (0, 0, 1, 2)
    );
}

#[test]
fn table_window_routes_copy_and_paste_events_through_egui() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    app.workspace.select_cell_range(0, 1, 1, 2).unwrap();
    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let run_frame = |app: &mut TunerApp, input: egui::RawInput| {
        context.run_ui(input, |ctx| {
            app.show_table_window(ctx, canvas, &key);
        })
    };
    let mut first_input = egui::RawInput::default();
    first_input.screen_rect = Some(canvas);
    run_frame(&mut app, first_input).drop_without_applying_deltas();
    let window_id = egui::Id::new(("table-window", "no-project", key.as_str()));
    let pointer = context
        .memory(|memory| memory.area_rect(window_id))
        .expect("table window should have a geometry")
        .center();
    let modifiers = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    let mut copy_input = egui::RawInput::default();
    copy_input.screen_rect = Some(canvas);
    copy_input.events = vec![
        egui::Event::PointerMoved(pointer),
        egui::Event::ModifiersChanged(modifiers),
        egui::Event::Key {
            key: egui::Key::C,
            physical_key: Some(egui::Key::C),
            pressed: true,
            repeat: false,
            modifiers,
        },
    ];
    let copy_output = run_frame(&mut app, copy_input);
    assert!(copy_output
        .platform_output
        .commands
        .contains(&egui::OutputCommand::CopyText(
            "30.00\t50.00\n40.00\t60.00".to_string()
        )));
    copy_output.drop_without_applying_deltas();

    let mut paste_input = egui::RawInput::default();
    paste_input.screen_rect = Some(canvas);
    paste_input.events = vec![
        egui::Event::PointerMoved(pointer),
        egui::Event::Paste("101\t102\n103\t104".to_string()),
    ];
    run_frame(&mut app, paste_input).drop_without_applying_deltas();
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        &[10, 20, 101, 103, 102, 104]
    );
    assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), 1);
}

#[test]
fn table_window_routes_native_copy_event_through_egui() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    app.workspace.select_cell_range(0, 1, 1, 2).unwrap();
    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let mut first_input = egui::RawInput::default();
    first_input.screen_rect = Some(canvas);
    context
        .run_ui(first_input, |ctx| {
            app.show_table_window(ctx, canvas, &key);
        })
        .drop_without_applying_deltas();

    let window_id = egui::Id::new(("table-window", "no-project", key.as_str()));
    let pointer = context
        .memory(|memory| memory.area_rect(window_id))
        .expect("table window should have a geometry")
        .center();
    let modifiers = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    let mut copy_input = egui::RawInput::default();
    copy_input.screen_rect = Some(canvas);
    copy_input.events = vec![
        egui::Event::PointerMoved(pointer),
        egui::Event::ModifiersChanged(modifiers),
        egui::Event::Copy,
    ];
    let output = context.run_ui(copy_input, |ctx| {
        app.show_table_window(ctx, canvas, &key);
    });
    assert!(output
        .platform_output
        .commands
        .contains(&egui::OutputCommand::CopyText(
            "30.00\t50.00\n40.00\t60.00".to_string()
        )));
    output.drop_without_applying_deltas();
}

#[test]
fn clicking_visible_table_title_bar_raises_that_table_above_previous_active() {
    let xdf = XdfDocument::parse(TWO_TABLE_XDF).unwrap();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10, 20])),
        Some(xdf),
        None,
        None,
    );
    let ids: Vec<_> = app
        .workspace
        .filtered_parameters("")
        .into_iter()
        .map(|parameter| parameter.semantic_id)
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(app.open_table(&ids[0]));
    assert!(app.open_table(&ids[1]));
    let first_key = app.open_tables[0].key.clone();
    let second_key = app.open_tables[1].key.clone();
    app.open_tables[0].memory = TableWindowMemory {
        x: 100,
        y: 100,
        width: 520,
        height: 320,
        ..TableWindowMemory::default()
    };
    app.open_tables[1].memory = TableWindowMemory {
        x: 140,
        y: 140,
        width: 520,
        height: 320,
        ..TableWindowMemory::default()
    };
    app.active_table_key = Some(second_key.clone());

    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let run_frame = |app: &mut TunerApp, events: Vec<egui::Event>| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        input.events = events;
        context
            .run_ui(input, |ctx| {
                app.show_table_window(ctx, canvas, &first_key);
                app.show_table_window(ctx, canvas, &second_key);
            })
            .drop_without_applying_deltas();
    };
    run_frame(&mut app, Vec::new());
    run_frame(&mut app, Vec::new());
    run_frame(&mut app, Vec::new());

    let first_id = egui::Id::new(("table-window", "no-project", first_key.as_str()));
    let second_id = egui::Id::new(("table-window", "no-project", second_key.as_str()));
    let first_rect = context
        .memory(|memory| memory.area_rect(first_id))
        .expect("first table should have a geometry");
    let title_bar_point = first_rect.min + egui::vec2(100.0, 14.0);
    assert_eq!(
        context.layer_id_at(title_bar_point),
        Some(egui::LayerId::new(egui::Order::Middle, first_id))
    );

    run_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(title_bar_point),
            egui::Event::PointerButton {
                pos: title_bar_point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );

    assert_eq!(app.active_table_key.as_deref(), Some(first_key.as_str()));
    assert_eq!(
        context.layer_id_at(title_bar_point),
        Some(egui::LayerId::new(egui::Order::Middle, first_id))
    );
    assert_ne!(
        context.layer_id_at(title_bar_point),
        Some(egui::LayerId::new(egui::Order::Middle, second_id))
    );
}

#[test]
fn resizing_overlapping_unfocused_table_keeps_its_resize_drag() {
    let xdf = XdfDocument::parse(TWO_TABLE_XDF).unwrap();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10, 20])),
        Some(xdf),
        None,
        None,
    );
    app.project_identity = Some("overlapping-resize".into());
    app.project_preferences = ProjectPreferences::for_identity("overlapping-resize");
    let semantic_ids: Vec<_> = app
        .workspace
        .filtered_parameters("")
        .into_iter()
        .map(|parameter| parameter.semantic_id)
        .collect();
    assert_eq!(semantic_ids.len(), 2);
    assert!(app.open_table(&semantic_ids[0]));
    assert!(app.open_table(&semantic_ids[1]));
    let first_key = app.open_tables[0].key.clone();
    let second_key = app.open_tables[1].key.clone();
    app.open_tables[0].memory = TableWindowMemory {
        x: 100,
        y: 100,
        width: 600,
        height: 450,
        fit_to_content: Some(false),
        position_saved: true,
        ..TableWindowMemory::default()
    };
    app.open_tables[1].memory = TableWindowMemory {
        x: 450,
        y: 350,
        width: 500,
        height: 350,
        fit_to_content: Some(false),
        position_saved: true,
        ..TableWindowMemory::default()
    };
    app.open_tables[0].geometry_request = true;
    app.open_tables[1].geometry_request = true;
    assert!(app.focus_table(&first_key));

    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let first_id = egui::Id::new(("table-window", "overlapping-resize", first_key.as_str()));
    let second_id = egui::Id::new(("table-window", "overlapping-resize", second_key.as_str()));
    let second_layer = egui::LayerId::new(
        app.dock_window_order(&WindowId::Table(second_key.clone())),
        second_id,
    );
    let resize_corner_id = egui::Id::new(second_layer)
        .with("edge_drag")
        .with("right_bottom");
    let mut time = 0.0;
    let draw_frame = |app: &mut TunerApp, events: Vec<egui::Event>, time: &mut f64| {
        *time += 0.1;
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        input.time = Some(*time);
        input.events = events;
        let mut resize_corner = None;
        context
            .run_ui(input, |ctx| {
                app.show_table_window(ctx, canvas, &first_key);
                app.show_table_window(ctx, canvas, &second_key);
                app.raise_focused_dock_window(ctx);
                resize_corner = ctx
                    .read_response(resize_corner_id)
                    .map(|response| response.rect);
            })
            .drop_without_applying_deltas();
        resize_corner
    };
    for _ in 0..3 {
        draw_frame(&mut app, Vec::new(), &mut time);
    }

    let first_before = context
        .memory(|memory| memory.area_rect(first_id))
        .expect("underlapping table should be laid out");
    let second_before = context
        .memory(|memory| memory.area_rect(second_id))
        .expect("front table should be laid out");
    let resize_corner = draw_frame(&mut app, Vec::new(), &mut time)
        .expect("front table should expose its resize corner");
    let start = resize_corner.center();
    assert!(
        first_before.intersects(second_before),
        "test tables must overlap"
    );
    assert_eq!(context.layer_id_at(start), Some(second_layer));

    draw_frame(&mut app, vec![egui::Event::PointerMoved(start)], &mut time);
    draw_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        &mut time,
    );
    let end = start + egui::vec2(40.0, 30.0);
    draw_frame(&mut app, vec![egui::Event::PointerMoved(end)], &mut time);
    draw_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(end),
            egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        &mut time,
    );

    let first_after = context
        .memory(|memory| memory.area_rect(first_id))
        .expect("underlapping table should remain open");
    let second_after = context
        .memory(|memory| memory.area_rect(second_id))
        .expect("front table should remain open");
    assert_eq!(app.active_table_key.as_deref(), Some(second_key.as_str()));
    assert_eq!(first_after, first_before);
    assert!(second_after.width() > second_before.width());
    assert!(second_after.height() > second_before.height());
}

#[test]
fn right_clicking_a_table_cell_selects_it_and_opens_the_cells_menu() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let run_frame = |app: &mut TunerApp, events: Vec<egui::Event>| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        input.events = events;
        context
            .run_ui(input, |ctx| {
                let frame_context = ctx.clone();
                egui::CentralPanel::default().show(ctx, |ui| {
                    app.show_workspace_canvas(ui, &frame_context);
                });
                app.show_table_window(ctx, canvas, &key);
            })
            .drop_without_applying_deltas();
    };
    run_frame(&mut app, Vec::new());
    run_frame(&mut app, Vec::new());
    run_frame(&mut app, Vec::new());

    let output = context.run_ui(egui::RawInput::default(), |ctx| {
        app.show_table_window(ctx, canvas, &key);
    });
    let cell_point = rendered_table_cell_center(&output, 1, 2);
    output.drop_without_applying_deltas();
    run_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(cell_point),
            egui::Event::PointerButton {
                pos: cell_point,
                button: egui::PointerButton::Secondary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    run_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(cell_point),
            egui::Event::PointerButton {
                pos: cell_point,
                button: egui::PointerButton::Secondary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    run_frame(&mut app, Vec::new());

    let mut input = egui::RawInput::default();
    input.screen_rect = Some(canvas);
    let menu_output = context.run_ui(input, |ctx| {
        let frame_context = ctx.clone();
        egui::CentralPanel::default().show(ctx, |ui| {
            app.show_workspace_canvas(ui, &frame_context);
        });
        app.show_table_window(ctx, canvas, &key);
    });
    let existing_cells_menu = rendered_text_rect(&menu_output, "Engineering transforms").is_some();
    let workspace_menu = rendered_text_rect(&menu_output, "Set background image…").is_some();
    menu_output.drop_without_applying_deltas();

    assert_eq!(
        app.workspace.selected_cell_range().unwrap().bounds(),
        (1, 2, 1, 2)
    );
    assert_eq!(
        context
            .layer_id_at(cell_point + egui::vec2(20.0, 20.0))
            .map(|layer| layer.order),
        Some(egui::Order::Foreground)
    );
    assert!(existing_cells_menu);
    assert!(!workspace_menu);
}

#[test]
fn table_window_routes_primary_drag_to_a_rectangular_selection() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let run_frame = |app: &mut TunerApp, events: Vec<egui::Event>| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        input.events = events;
        context
            .run_ui(input, |ctx| {
                app.show_table_window(ctx, canvas, &key);
            })
            .drop_without_applying_deltas();
    };
    run_frame(&mut app, Vec::new());
    run_frame(&mut app, Vec::new());
    run_frame(&mut app, Vec::new());
    let output = context.run_ui(egui::RawInput::default(), |ctx| {
        app.show_table_window(ctx, canvas, &key);
    });
    let start = rendered_table_cell_center(&output, 0, 0);
    let end = rendered_table_cell_center(&output, 1, 2);
    output.drop_without_applying_deltas();
    run_frame(&mut app, vec![egui::Event::PointerMoved(start)]);
    run_frame(
        &mut app,
        vec![egui::Event::PointerButton {
            pos: start,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    run_frame(&mut app, vec![egui::Event::PointerMoved(end)]);
    run_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(end),
            egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );

    assert_eq!(
        app.workspace.selected_cell_range().unwrap().bounds(),
        (0, 0, 1, 2)
    );
}

#[test]
fn table_window_does_not_route_paste_while_editor_has_focus() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let run_frame = |app: &mut TunerApp, events: Vec<egui::Event>| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        input.events = events;
        context
            .run_ui(input, |ctx| {
                app.show_table_window(ctx, canvas, &key);
            })
            .drop_without_applying_deltas();
    };
    run_frame(&mut app, Vec::new());
    run_frame(&mut app, Vec::new());
    run_frame(&mut app, Vec::new());
    let editor_id = egui::Id::new(("table-engineering-input", "no-project", key.as_str()));
    let editor = context
        .read_response(editor_id)
        .expect("table editor should have an interactive response")
        .rect
        .center();
    run_frame(&mut app, vec![egui::Event::PointerMoved(editor)]);
    run_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(editor),
            egui::Event::PointerButton {
                pos: editor,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    run_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(editor),
            egui::Event::PointerButton {
                pos: editor,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    assert!(context.egui_wants_keyboard_input());
    let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();

    run_frame(&mut app, vec![egui::Event::Paste("101\t102".to_string())]);

    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        before.as_slice()
    );
    assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), 0);
}

#[test]
fn workspace_filters_selects_and_reads_column_major_cells() {
    let (xdf, bin) = column_major_fixture();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);
    assert_eq!(workspace.filtered_parameters("map").len(), 1);
    let id = workspace.filtered_parameters("map")[0].semantic_id.clone();
    assert!(workspace.select_parameter(&id));
    workspace.select_cell(0, 1).unwrap();
    assert_eq!(
        workspace.selected_cell_view().unwrap().raw,
        RawValue::Unsigned(30)
    );
}

#[test]
fn workspace_edit_undo_redo_and_rejected_edit_are_atomic() {
    let (xdf, bin) = column_major_fixture();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);
    let id = workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(workspace.select_parameter(&id));
    workspace.select_cell(1, 2).unwrap();
    workspace.apply_engineering_text("99").unwrap();
    assert_eq!(
        workspace.bin.as_ref().unwrap().bytes(),
        &[10, 20, 30, 40, 50, 99]
    );
    assert!(workspace.bin.as_ref().unwrap().is_dirty());
    workspace.undo().unwrap();
    assert_eq!(
        workspace.bin.as_ref().unwrap().bytes(),
        &[10, 20, 30, 40, 50, 60]
    );
    workspace.redo().unwrap();
    assert_eq!(
        workspace.bin.as_ref().unwrap().bytes(),
        &[10, 20, 30, 40, 50, 99]
    );
    let before = workspace.bin.as_ref().unwrap().bytes().to_vec();
    let undo_depth = workspace.bin.as_ref().unwrap().undo_depth();
    assert!(workspace.apply_engineering_text("not-a-number").is_err());
    assert_eq!(workspace.bin.as_ref().unwrap().bytes(), before.as_slice());
    assert_eq!(workspace.bin.as_ref().unwrap().undo_depth(), undo_depth);
}

#[test]
fn raw_scalar_and_bitfield_edits_are_storage_aware_and_undoable() {
    let xdf = XdfDocument::parse(TYPE_AWARE_XDF).unwrap();
    assert_eq!(xdf.parameters[0].kind, ParameterKind::Constant);
    assert_eq!(xdf.parameters[1].kind, ParameterKind::Flag);
    assert_eq!(xdf.parameters[2].kind, ParameterKind::BitField);
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0, 0, 0xA0, 0xA0])),
        Some(xdf),
        None,
        None,
    );

    let scalar = workspace
        .filtered_parameters("scalar")
        .into_iter()
        .next()
        .unwrap()
        .semantic_id;
    workspace.select_parameter(&scalar);
    workspace.apply_raw_text("0x1234").unwrap();
    assert_eq!(workspace.bin.as_ref().unwrap().bytes()[..2], [0x34, 0x12]);
    workspace.undo().unwrap();
    assert_eq!(workspace.bin.as_ref().unwrap().bytes()[..2], [0, 0]);

    let flag = workspace
        .filtered_parameters("enabled")
        .into_iter()
        .next()
        .unwrap()
        .semantic_id;
    workspace.select_parameter(&flag);
    assert_eq!(
        workspace.selected_cell_view().unwrap().raw,
        RawValue::Unsigned(0)
    );
    workspace.apply_raw_text("0x1").unwrap();
    assert_eq!(workspace.bin.as_ref().unwrap().bytes()[2], 0xA4);

    let mode = workspace
        .filtered_parameters("mode")
        .into_iter()
        .next()
        .unwrap()
        .semantic_id;
    workspace.select_parameter(&mode);
    workspace.apply_raw_text("0x5").unwrap();
    assert_eq!(workspace.bin.as_ref().unwrap().bytes()[3], 0xAA);
    assert_eq!(
        raw_editor_text(
            workspace.xdf.as_ref().unwrap().parameter(&mode).unwrap(),
            workspace.selected_cell_view().unwrap().raw,
        ),
        "0x5"
    );
}

#[test]
fn hex_grid_click_does_not_scroll_but_external_navigation_does() {
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0_u8; 4096])),
        None,
        Some(PathBuf::from("memory.bin")),
        None,
    );
    app.hex_editor.window_open = true;

    // Clicking a visible byte (the in-grid release path) must not request
    // a scroll: scrolling to a visible byte re-anchors its row to the top
    // of the viewport, which read as a random jump.
    app.select_hex_range(0x0180, 1, false).unwrap();
    assert_eq!(app.hex_editor.selection.unwrap().bounds(), (0x0180, 0x0181));
    assert_eq!(app.hex_editor.address, 0x0180);
    assert!(app.hex_editor.scroll_to_address.is_none());

    // External navigation to bytes that may be off-screen must scroll.
    app.select_hex_range(0x0400, 2, true).unwrap();
    assert_eq!(app.hex_editor.scroll_to_address, Some(0x0400));

    // The wire path (NookLink) keeps scrolling.
    let request = agent_ipc_request(json!({
        "action":"select_hex_range","byte_offset":2048,"byte_length":1
    }));
    app.handle_ui_ipc_request(&request);
    assert_eq!(app.hex_editor.scroll_to_address, Some(2048));

    // Writing from the in-window edit field must not scroll either.
    app.hex_editor.scroll_to_address = None;
    app.write_hex_at(0x0180, "AB", false).unwrap();
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes()[0x0180], 0xAB);
    assert!(app.hex_editor.scroll_to_address.is_none());
}

#[test]
fn hex_editor_auto_range_uses_visible_values_instead_of_hidden_bin_outliers() {
    let mut bytes = Vec::new();
    for index in 0..1_600 {
        let value = if index < 200 {
            index as f32
        } else {
            1.0e30_f32
        };
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(bytes));
    app.hex_editor.window_open = true;
    app.hex_editor.memory.display_format = HexDisplayFormat::Float32;
    app.hex_editor.memory.endianness = HexEndianness::Little;

    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    for _ in 0..3 {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        let mut output = context.run_ui(input, |ctx| app.show_hex_editor(ctx, canvas));
        output.textures_delta.clear();
    }

    assert_eq!(
        app.hex_editor.memory.auto_range_scope,
        HexAutoRangeScope::VisiblePercentile
    );
    let (_, visible_maximum) = app
        .hex_editor
        .observed_color_range
        .expect("rendered Hex Editor should record its visible value range");
    assert!(visible_maximum < 100.0, "{visible_maximum}");

    app.hex_editor.memory.auto_range_scope = HexAutoRangeScope::WholeBin;
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(canvas);
    let mut output = context.run_ui(input, |ctx| app.show_hex_editor(ctx, canvas));
    output.textures_delta.clear();
    let (_, whole_bin_maximum) = app
        .hex_editor
        .observed_color_range
        .expect("whole-BIN coloring should report its range");
    assert!(whole_bin_maximum > 1.0e20, "{whole_bin_maximum}");
}

#[test]
fn ui_ipc_can_select_whole_bin_hex_auto_range() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![0; 128]));
    let mut request = ui_ipc::UiIpcRequest::for_test("set_hex_coloring", "", None, None);
    request.color_mode = Some("auto_range".to_string());
    request.color_range_scope = Some("whole_bin".to_string());

    let response = app.handle_ui_ipc_request(&request);

    assert!(response.ok, "{}", response.message);
    assert_eq!(response.data.unwrap()["color_range_scope"], "whole_bin");
    assert_eq!(
        app.hex_editor.memory.auto_range_scope,
        HexAutoRangeScope::WholeBin
    );
}

#[test]
fn finder_inspect_and_typed_hex_search_select_the_candidate_value() {
    let candidate_offset = 0x1E_F8F0;
    let value_offset = candidate_offset + (9 * 10) * 4;
    let mut bytes = vec![0; 0x20_0000];
    bytes[value_offset..value_offset + 4].copy_from_slice(&6010.0_f32.to_be_bytes());

    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(bytes));
    app.map_finder.candidates.push(MapCandidate {
        offset: candidate_offset,
        byte_length: 400,
        rows: 10,
        columns: 10,
        display_format: HexDisplayFormat::Float32,
        endianness: HexEndianness::Big,
        score: 93,
        value_range: (0.0, 6010.0),
        value_bands: 2,
        axis_suggestions: Vec::new(),
        selected_x_axis: None,
        selected_y_axis: None,
    });
    app.map_finder.selected_cell = Some((9, 0));

    app.inspect_map_candidate_in_hex(0).unwrap();

    assert_eq!(app.hex_editor.address, value_offset);
    assert_eq!(
        app.hex_editor.memory.display_format,
        HexDisplayFormat::Float32
    );
    assert_eq!(app.hex_editor.memory.endianness, HexEndianness::Big);
    assert_eq!(
        app.hex_editor.selection.unwrap().bounds(),
        (value_offset, value_offset + 4)
    );
    assert_eq!(
        decode_hex_value(
            app.workspace.bin.as_ref().unwrap().bytes(),
            value_offset,
            HexDisplayFormat::Float32,
            HexEndianness::Big,
        ),
        Some(HexValue::Float(6010.0))
    );

    assert_eq!(app.search_hex("6010", true).unwrap(), Some(value_offset));
    assert_eq!(app.hex_editor.address, value_offset);
    assert_eq!(app.hex_editor.scroll_to_address, Some(value_offset));
}

#[test]
fn raw_editor_parser_interprets_signed_hex_as_storage_bits() {
    let xdf = XdfDocument::parse(
        br#"<XDFFORMAT><XDFCONSTANT uniqueid="signed"><title>Signed</title>
            <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="16" mmedtypeflags="0x03" />
        </XDFCONSTANT></XDFFORMAT>"#,
    )
    .unwrap();
    let parameter = &xdf.parameters[0];
    assert_eq!(
        parse_raw_editor_value("0xFFFF", parameter).unwrap(),
        RawValue::Signed(-1)
    );
    assert_eq!(
        parse_raw_editor_value("-2", parameter).unwrap(),
        RawValue::Signed(-2)
    );
}

#[test]
fn workspace_loads_bin_and_xdf_independently_and_validates_when_paired() {
    let bin_path = temporary_test_path("bin");
    let xdf_path = temporary_test_path("xdf");
    std::fs::write(&bin_path, [10, 20, 30, 40, 50, 60]).unwrap();
    std::fs::write(&xdf_path, COLUMN_MAJOR_XDF).unwrap();

    let mut workspace = WorkspaceState::default();
    workspace.open_bin(&bin_path).unwrap();
    assert!(workspace.bin.is_some());
    assert!(workspace.xdf.is_none());
    workspace.open_xdf(&xdf_path).unwrap();
    assert!(workspace.xdf.is_some());
    assert_eq!(workspace.validation.as_ref().unwrap().issue_count(), 0);

    std::fs::remove_file(bin_path).unwrap();
    std::fs::remove_file(xdf_path).unwrap();
}

#[test]
#[ignore = "requires the local-only SCGa05 calibration BIN/XDF fixture"]
fn workspace_loads_the_supplied_fixture_pair_and_reports_its_shape() {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("Test bin and xdf");
    let bin_path = fixture_dir.join("SCGa05_cal.bin");
    let xdf_path = fixture_dir.join("SCGa05_cal.xdf");
    assert!(
        bin_path.is_file(),
        "missing fixture BIN: {}",
        bin_path.display()
    );
    assert!(
        xdf_path.is_file(),
        "missing fixture XDF: {}",
        xdf_path.display()
    );

    let mut workspace = WorkspaceState::default();
    workspace.open_bin(&bin_path).unwrap();
    workspace.open_xdf(&xdf_path).unwrap();
    let xdf = workspace.xdf.as_ref().unwrap();
    assert_eq!(xdf.parameters.len(), 2_915);
    assert_eq!(xdf.categories.len(), 57);
    assert_eq!(
        xdf.parameters
            .iter()
            .map(|parameter| parameter.axes.len())
            .sum::<usize>(),
        8_745
    );
    assert!(xdf
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "unsupported-storage"));
    assert_eq!(workspace.bin.as_ref().unwrap().len(), 654_336);
    assert!(workspace.validation.as_ref().unwrap().is_valid());
    assert!(workspace.debug_report().contains("column_major"));
}

#[test]
#[ignore = "requires the local-only SCGa05 calibration BIN/XDF fixture"]
fn supplied_fixture_uses_tunerpro_category_positions_and_nested_counts() {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("Test bin and xdf");
    let xdf = XdfDocument::load(fixture_dir.join("SCGa05_cal.xdf")).unwrap();
    assert_eq!(
        xdf.category_reference_mode,
        CategoryReferenceMode::OneBasedPosition
    );

    let tree = build_category_tree(xdf.parameters.iter().map(summary_from_parameter).collect());
    let root = |name: &str| tree.iter().find(|node| node.name == name).unwrap();
    assert_eq!(root("Airflow").total_count, 100);
    assert_eq!(root("Fuel").total_count, 261);
    assert_eq!(root("MPI").total_count, 88);
    assert_eq!(root("Limiter").total_count, 91);
    assert_eq!(root("Torque Model").total_count, 52);
    assert_eq!(root("Engine Diagnostics").total_count, 1_504);
    assert!(root("Limiter")
        .children
        .iter()
        .any(|child| child.name == "RPM"));
    assert!(!xdf
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "missing-category"));
}

#[test]
fn grouped_parameters_are_category_ready() {
    let (xdf, bin) = column_major_fixture();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);
    let groups = workspace.grouped_parameters("");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].parameters.len(), 1);
    assert_eq!(groups[0].parameters[0].title, "Map");
}

#[test]
fn app_catalog_cache_tracks_the_current_xdf_and_filter() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);

    assert_eq!(
        app.filtered_catalog_parameters("")
            .iter()
            .map(|summary| summary.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Map"]
    );
    assert_eq!(app.filtered_catalog_parameters("does-not-exist").len(), 0);

    let replacement = XdfDocument::parse(
        br#"<XDFFORMAT><XDFCONSTANT uniqueid="constant"><title>Constant</title>
            <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8" />
        </XDFCONSTANT></XDFFORMAT>"#,
    )
    .unwrap();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0])),
        Some(replacement),
        None,
        None,
    );
    assert_eq!(
        app.filtered_catalog_parameters("")
            .iter()
            .map(|summary| summary.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Constant"]
    );
}

#[test]
fn browser_catalog_tree_refreshes_when_sort_preferences_change() {
    let xdf = XdfDocument::parse(
        br#"<XDFFORMAT><XDFTABLE uniqueid="z"><title>Zulu</title>
                <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8" mmedrowcount="1" mmedcolcount="1" />
            </XDFTABLE><XDFTABLE uniqueid="a"><title>Alpha</title>
                <EMBEDDEDDATA mmedaddress="0x01" mmedelementsizebits="8" mmedrowcount="1" mmedcolcount="1" />
            </XDFTABLE></XDFFORMAT>"#,
    )
    .unwrap();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0, 0])),
        Some(xdf),
        None,
        None,
    );

    assert_eq!(
        app.browser_catalog_tree("")[0]
            .parameters
            .iter()
            .map(|summary| summary.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Alpha", "Zulu"]
    );
    app.preferences.catalog_sort = CatalogSortKey::Address;
    app.preferences.catalog_sort_direction = SortDirection::Descending;
    assert_eq!(
        app.browser_catalog_tree("")[0]
            .parameters
            .iter()
            .map(|summary| summary.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Alpha", "Zulu"]
    );
}

#[test]
fn drawer_collapse_commands_do_not_change_documents() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let bin_before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    app.dispatch_command("view.collapse-browser").unwrap();
    app.dispatch_command("view.collapse-inspector").unwrap();
    assert!(app.preferences.browser_collapsed);
    assert!(app.preferences.inspector_collapsed);
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        bin_before.as_slice()
    );
}

#[test]
fn opening_tables_creates_unique_tabs_and_focuses_existing_tab() {
    let xdf = XdfDocument::parse(TWO_TABLE_XDF).unwrap();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10, 20])),
        Some(xdf),
        None,
        None,
    );
    let ids: Vec<_> = app
        .workspace
        .filtered_parameters("")
        .into_iter()
        .map(|parameter| parameter.semantic_id)
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(app.open_table(&ids[0]));
    assert!(app.open_table(&ids[1]));
    assert!(app.open_table(&ids[0]));
    assert_eq!(app.open_tables.len(), 2);
    assert_eq!(
        app.active_table_key.as_deref(),
        Some(app.open_tables[0].key.as_str())
    );
}

#[test]
fn new_table_window_size_scales_with_table_dimensions() {
    let small_xdf = XdfDocument::parse(COLUMN_MAJOR_XDF).unwrap();
    let large_xdf = XdfDocument::parse(LARGE_TABLE_XDF).unwrap();
    let mut small = TunerApp::headless();
    small.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0; 6])),
        Some(small_xdf),
        None,
        None,
    );
    let small_id = small.workspace.filtered_parameters("")[0]
        .semantic_id
        .clone();
    assert!(small.open_table(&small_id));
    let small_memory = small.open_tables[0].memory.clone();

    let mut large = TunerApp::headless();
    large.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0; 144])),
        Some(large_xdf),
        None,
        None,
    );
    let large_id = large.workspace.filtered_parameters("")[0]
        .semantic_id
        .clone();
    assert!(large.open_table(&large_id));
    let large_memory = large.open_tables[0].memory.clone();

    assert!(large_memory.width > small_memory.width);
    assert!(large_memory.height > small_memory.height);
}

#[test]
fn reset_table_layout_recalculates_dimension_aware_default_size() {
    let xdf = XdfDocument::parse(LARGE_TABLE_XDF).unwrap();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0; 144])),
        Some(xdf),
        None,
        None,
    );
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_table(&semantic_id));

    app.reset_table_layout();

    assert!(app.open_tables[0].memory.width > 760);
}

#[test]
fn reset_workspace_windows_changes_only_active_geometry() {
    let xdf = XdfDocument::parse(TWO_TABLE_XDF).unwrap();
    let xdf_identity = xdf.normalized_fingerprint.clone();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10, 20, 30, 40])),
        Some(xdf),
        Some(PathBuf::from("project.bin")),
        Some(PathBuf::from("project.xdf")),
    );
    app.project_identity = Some("project".into());
    app.project_preferences = ProjectPreferences::for_identity("project");
    app.project_preferences.last_xdf_path = Some(PathBuf::from("project.xdf"));
    let parameters = app.workspace.filtered_parameters("");
    let open_semantic_id = parameters[0].semantic_id.clone();
    let closed_semantic_id = parameters[1].semantic_id.clone();
    assert!(app.open_table(&open_semantic_id));
    assert!(app.open_surface(&open_semantic_id));

    let open_key = app.open_tables[0].key.clone();
    let closed_key = table_key(&xdf_identity, &closed_semantic_id);
    let table = &mut app.open_tables[0];
    table.memory.x = 530;
    table.memory.y = 370;
    table.memory.width = 1_120;
    table.memory.height = 840;
    table.memory.zoom_percent = 135;
    table.memory.scroll_x = 17;
    table.memory.scroll_y = 29;
    table.memory.decimal_places = 4;
    table.memory.coloring.mode = TableColorMode::FixedRange;
    table.memory.coloring.fixed_min = Some(-8.0);
    table.memory.coloring.fixed_max = Some(88.0);
    table.memory.fit_to_content = Some(false);
    let mut closed_memory = TableWindowMemory::default();
    closed_memory.x = 810;
    closed_memory.y = 610;
    closed_memory.width = 1_000;
    closed_memory.height = 700;
    closed_memory.zoom_percent = 125;
    closed_memory.decimal_places = 3;
    closed_memory.scroll_x = 7;
    closed_memory.scroll_y = 11;
    closed_memory.coloring.fixed_min = Some(5.0);
    closed_memory.coloring.fixed_max = Some(55.0);
    closed_memory.fit_to_content = Some(false);
    app.preferences
        .table_windows
        .insert(closed_key.clone(), closed_memory.clone());
    app.project_preferences
        .table_windows
        .insert(closed_key.clone(), closed_memory);

    let open_surface = &mut app.open_surfaces[0];
    open_surface.memory.x = 520;
    open_surface.memory.y = 340;
    open_surface.memory.width = 1_200;
    open_surface.memory.height = 900;
    open_surface.memory.yaw = 23.0;
    open_surface.memory.zoom_percent = 145;

    app.search_state.open = true;
    app.search_state.query = "boost*".into();
    app.search_state.window.x = 550;
    app.search_state.window.y = 410;
    app.search_state.window.width = 1_100;
    app.search_state.window.height = 900;
    app.search_state.window.zoom_percent = 135;
    app.hex_editor.window_open = true;
    app.hex_editor.memory.x = 600;
    app.hex_editor.memory.y = 420;
    app.hex_editor.memory.width = 1_250;
    app.hex_editor.memory.height = 880;
    app.hex_editor.memory.display_format = HexDisplayFormat::Float32;
    app.hex_editor.memory.search_query = "6010".into();
    app.map_finder.memory.window_open = true;
    app.map_finder.memory.x = 620;
    app.map_finder.memory.y = 430;
    app.map_finder.memory.width = 1_300;
    app.map_finder.memory.height = 950;
    app.map_finder.memory.rows_min = 6;
    app.compare.window_open = true;
    app.compare.graph_open = true;
    app.compare.filter = "torque".into();
    app.compare.mode = CompareValueMode::AbsoluteDelta;
    app.compare.window.x = 700;
    app.compare.window.y = 450;
    app.compare.window.width = 1_350;
    app.compare.window.height = 960;
    app.compare.surface_windows.insert(
        "compare-map".into(),
        SurfaceViewMemory {
            x: 750,
            y: 470,
            width: 1_150,
            height: 850,
            yaw: 17.0,
            ..SurfaceViewMemory::default()
        },
    );
    app.project_preferences.utility_window_geometry.insert(
        "debug_report".into(),
        window_geometry::WindowGeometryMemory {
            x: 710,
            y: 460,
            width: 1_200,
            height: 900,
        },
    );
    app.debug_report_open = true;
    let table_window = WindowId::Table(open_key.clone());
    let search_window = WindowId::Search;
    app.project_preferences.dock_state.minimize(&table_window);
    app.project_preferences
        .dock_state
        .open_tool(search_window.clone());
    app.project_preferences
        .dock_state
        .focus(search_window.clone());
    app.project_preferences.dock_state.minimize(&search_window);

    let mut inactive_preferences = ProjectPreferences::for_identity("project");
    let mut inactive_table = TableWindowMemory::default();
    inactive_table.x = 900;
    inactive_table.y = 720;
    inactive_preferences
        .table_windows
        .insert("inactive|table".into(), inactive_table);
    let inactive_snapshot = WorkspaceSnapshot {
        id: 2,
        name: "Other layout".into(),
        state: WorkspaceViewState::capture(&inactive_preferences),
    };
    app.project_preferences.saved_workspace_snapshots = vec![inactive_snapshot.clone()];
    let dock_state = app.project_preferences.dock_state.clone();
    let mut edit = app
        .workspace
        .bin
        .as_mut()
        .unwrap()
        .transaction("dirty before reset");
    edit.write_u8(0, 11).unwrap();
    edit.commit().unwrap();
    let before_bytes = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let before_revision = app.workspace.data_revision();
    let before_selection = (
        app.workspace.selected_semantic_id.clone(),
        app.workspace.selected_cell,
        app.workspace.selected_cells,
        app.workspace.selected_axis,
    );
    let before_dirty = app.workspace.bin.as_ref().unwrap().is_dirty();

    assert!(app
        .registry
        .descriptors()
        .iter()
        .any(|command| command.id == "view.reset-workspace-windows"));
    app.dispatch_command("view.reset-workspace-windows")
        .unwrap();

    let open_memory = &app.project_preferences.table_windows[&open_key];
    assert_eq!((open_memory.x, open_memory.y), (80, 80));
    assert!(open_memory.fit_to_content.unwrap());
    assert!(open_memory.width < 760 && open_memory.height < 520);
    assert_eq!(open_memory.zoom_percent, 135);
    assert_eq!((open_memory.scroll_x, open_memory.scroll_y), (17, 29));
    assert_eq!(open_memory.decimal_places, 4);
    assert_eq!(open_memory.coloring.fixed_min, Some(-8.0));
    assert_eq!(open_memory.coloring.fixed_max, Some(88.0));
    let closed_after = &app.project_preferences.table_windows[&closed_key];
    assert_eq!((closed_after.x, closed_after.y), (80, 80));
    assert!(closed_after.fit_to_content.unwrap());
    assert!(closed_after.width < 1_000 && closed_after.height < 700);
    assert_eq!(closed_after.zoom_percent, 125);
    assert_eq!((closed_after.scroll_x, closed_after.scroll_y), (7, 11));
    assert_eq!(closed_after.decimal_places, 3);
    assert_eq!(closed_after.coloring.fixed_min, Some(5.0));
    assert!(app.open_tables[0].geometry_request);
    assert!(app.open_surfaces[0].geometry_request);
    assert_eq!(app.open_surfaces[0].memory.yaw, 23.0);
    assert_eq!(app.open_surfaces[0].memory.zoom_percent, 145);

    assert_eq!(
        (app.search_state.window.x, app.search_state.window.y),
        (140, 100)
    );
    assert_eq!(app.search_state.window.zoom_percent, 135);
    assert_eq!(app.search_state.query, "boost*");
    assert_eq!(
        (app.hex_editor.memory.x, app.hex_editor.memory.y),
        (120, 90)
    );
    assert_eq!(
        app.hex_editor.memory.display_format,
        HexDisplayFormat::Float32
    );
    assert_eq!(app.hex_editor.memory.search_query, "6010");
    assert_eq!(
        (app.map_finder.memory.x, app.map_finder.memory.y),
        (140, 110)
    );
    assert_eq!(app.map_finder.memory.rows_min, 6);
    assert_eq!((app.compare.window.x, app.compare.window.y), (160, 100));
    assert_eq!(app.compare.filter, "torque");
    assert_eq!(app.compare.mode, CompareValueMode::AbsoluteDelta);
    assert_eq!(app.compare.surface_windows["compare-map"].yaw, 17.0);
    assert_eq!(
        app.project_preferences.utility_window_geometry["debug_report"],
        window_geometry::WindowGeometryMemory::default()
    );
    assert_eq!(
        app.project_preferences.saved_workspace_snapshots,
        vec![inactive_snapshot]
    );
    assert_eq!(app.project_preferences.dock_state, dock_state);
    assert_eq!(app.open_tables.len(), 1);
    assert_eq!(app.open_surfaces.len(), 1);
    assert!(app.search_state.open);
    assert!(app.hex_editor.window_open);
    assert!(app.map_finder.memory.window_open);
    assert!(app.compare.window_open && app.compare.graph_open);
    assert!(app.workspace_geometry_reset_pending);
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), before_bytes);
    assert_eq!(app.workspace.bin.as_ref().unwrap().is_dirty(), before_dirty);
    assert_eq!(app.workspace.data_revision(), before_revision);
    assert_eq!(
        app.workspace.xdf.as_ref().unwrap().normalized_fingerprint,
        xdf_identity
    );
    assert_eq!(
        (
            app.workspace.selected_semantic_id.clone(),
            app.workspace.selected_cell,
            app.workspace.selected_cells,
            app.workspace.selected_axis,
        ),
        before_selection
    );
}

#[test]
fn reset_workspace_windows_is_available_from_arrange_without_open_tables() {
    let mut app = app_with_project_fixture();
    assert!(app.open_tables.is_empty());
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 800.0));
    let mut time = 0.0;
    click_dock_label(&mut app, "Arrange", &context, screen, &mut time);
    let menu = render_dock_frame(&mut app, &context, screen, time, Vec::new());

    assert!(rendered_text_rect(&menu, "Reset Workspace Windows").is_some());
    menu.drop_without_applying_deltas();
}

#[test]
fn favorites_section_renders_filtered_links_from_current_xdf() {
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10, 20])),
        Some(XdfDocument::parse(TWO_TABLE_XDF).unwrap()),
        None,
        None,
    );
    let summaries = app.workspace.filtered_parameters("");
    let map_a = summaries
        .iter()
        .find(|summary| summary.title == "Map A")
        .unwrap();
    let map_b = summaries
        .iter()
        .find(|summary| summary.title == "Map B")
        .unwrap();
    let fingerprint = app
        .workspace
        .xdf
        .as_ref()
        .unwrap()
        .normalized_fingerprint
        .clone();
    app.preferences.favorite_tables = vec![
        table_key(&fingerprint, &map_a.semantic_id),
        table_key(&fingerprint, &map_b.semantic_id),
        "different-xdf|stale-table".into(),
    ];
    app.preferences.collapsed_categories = vec!["Uncategorized".into()];
    app.workspace.filter = "Map A".into();

    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 800.0));
    let output = render_browser_contents_frame(&mut app, &context, screen, 0.0, Vec::new());
    let header_visible = rendered_text_rect(&output, "Favorites").is_some();
    let map_a_visible = rendered_text_rect(&output, &summary_label(map_a)).is_some();
    let map_b_visible = rendered_text_rect(&output, &summary_label(map_b)).is_some();
    output.drop_without_applying_deltas();
    assert!(header_visible);
    assert!(map_a_visible);
    assert!(!map_b_visible);

    app.workspace.filter.clear();
    let output = render_browser_contents_frame(&mut app, &context, screen, 0.1, Vec::new());
    let map_a_visible = rendered_text_rect(&output, &summary_label(map_a)).is_some();
    let map_b_visible = rendered_text_rect(&output, &summary_label(map_b)).is_some();
    output.drop_without_applying_deltas();
    assert!(map_a_visible);
    assert!(map_b_visible);
}

#[test]
fn unfavoriting_removes_link_but_keeps_open_table() {
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10, 20])),
        Some(XdfDocument::parse(TWO_TABLE_XDF).unwrap()),
        None,
        None,
    );
    let summary = app.workspace.filtered_parameters("")[0].clone();
    let fingerprint = app
        .workspace
        .xdf
        .as_ref()
        .unwrap()
        .normalized_fingerprint
        .clone();
    let key = table_key(&fingerprint, &summary.semantic_id);
    app.preferences.favorite_tables.push(key.clone());
    app.preferences.collapsed_categories = vec!["Uncategorized".into()];
    app.workspace.filter = summary.title.clone();
    assert!(app.open_table(&summary.semantic_id));
    assert!(app.open_tables[0].pinned);
    let bytes_before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 800.0));
    let label = summary_label(&summary);
    let mut time = 0.0;

    click_browser_text(&mut app, &label, &context, screen, &mut time);
    assert_eq!(
        app.open_tables.len(),
        1,
        "favorite should link to the open table"
    );
    assert_eq!(app.active_table_key.as_deref(), Some(key.as_str()));
    assert_eq!(
        app.project_preferences.dock_state.focused(),
        Some(&WindowId::Table(key.clone()))
    );

    assert!(app.toggle_table_pin(&key));
    assert!(!app.open_tables[0].pinned);
    assert!(!app.preferences.favorite_tables.contains(&key));
    let output = render_browser_contents_frame(&mut app, &context, screen, time + 0.1, Vec::new());
    let favorite_link_visible = rendered_text_rect(&output, &label).is_some();
    output.drop_without_applying_deltas();
    assert!(!favorite_link_visible);
    assert_eq!(app.open_tables.len(), 1);
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), bytes_before);
}

#[test]
fn favorites_recents_tab_order_and_close_actions_are_ui_only() {
    let xdf = XdfDocument::parse(TWO_TABLE_XDF).unwrap();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10, 20])),
        Some(xdf),
        None,
        None,
    );
    let ids: Vec<_> = app
        .workspace
        .filtered_parameters("")
        .into_iter()
        .map(|parameter| parameter.semantic_id)
        .collect();
    assert!(app.open_table(&ids[0]));
    assert!(app.open_table(&ids[1]));
    let first_key = app.open_tables[0].key.clone();
    let second_key = app.open_tables[1].key.clone();
    app.toggle_favorite(&first_key);
    assert!(app.preferences.favorite_tables.contains(&first_key));
    assert_eq!(app.preferences.recent_tables.first(), Some(&second_key));
    app.focus_table(&first_key);
    assert_eq!(app.preferences.recent_tables.first(), Some(&first_key));
    assert!(app.move_table_tab(&second_key, 0));
    assert_eq!(app.open_tables[0].key, second_key);
    assert!(app.close_other_tables());
    assert_eq!(app.open_tables.len(), 1);
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), [10, 20]);
}

#[test]
fn quick_switch_can_open_a_favorite_table_that_is_not_already_open() {
    let xdf = XdfDocument::parse(TWO_TABLE_XDF).unwrap();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10, 20])),
        Some(xdf),
        None,
        None,
    );
    let summary = app.workspace.filtered_parameters("")[1].clone();
    let key = table_key(
        &app.workspace.xdf.as_ref().unwrap().normalized_fingerprint,
        &summary.semantic_id,
    );
    app.preferences.favorite_tables.push(key);
    app.quick_switch_query = summary.title;
    app.activate_quick_switch();
    assert_eq!(app.open_tables.len(), 1);
    assert_eq!(app.open_tables[0].semantic_id, summary.semantic_id);
}

#[test]
fn resetting_table_layout_resets_scroll_restore_state() {
    let xdf = XdfDocument::parse(TWO_TABLE_XDF).unwrap();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10, 20])),
        Some(xdf),
        None,
        None,
    );
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    app.open_table(&id);
    app.open_tables[0].memory.scroll_x = 100;
    app.open_tables[0].memory.scroll_y = 200;
    app.open_tables[0].restore_scroll = false;
    app.reset_table_layout();
    assert_eq!(app.open_tables[0].memory.scroll_x, 0);
    assert_eq!(app.open_tables[0].memory.scroll_y, 0);
    assert!(app.open_tables[0].restore_scroll);
}

#[test]
fn table_organization_changes_tabs_without_mutating_documents() {
    let xdf = XdfDocument::parse(TWO_TABLE_XDF).unwrap();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10, 20])),
        Some(xdf),
        None,
        None,
    );
    let ids: Vec<_> = app
        .workspace
        .filtered_parameters("")
        .into_iter()
        .map(|parameter| parameter.semantic_id)
        .collect();
    app.open_table(&ids[0]);
    app.open_table(&ids[1]);
    let bytes = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    app.cascade_tables();
    app.tile_tables();
    app.reset_table_layout();
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        bytes.as_slice()
    );
}

#[test]
fn debug_report_contains_document_identity_and_selection_details() {
    let (xdf, bin) = column_major_fixture();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);
    let report = workspace.debug_report();
    assert!(report.contains("[BIN]"));
    assert!(report.contains("[XDF]"));
    assert!(report.contains("[MAPPING VALIDATION]"));
    assert!(report.contains("[SELECTED PARAMETER]"));
    assert!(report.contains("column_major"));
    assert!(report.contains("selected_cell:"));
}

#[test]
fn app_debug_report_includes_axis_targets_precision_and_xdf_reuse_state() {
    let xdf = XdfDocument::parse(AXIS_TABLE_XDF).unwrap();
    let mut bytes = vec![0; 0x40];
    bytes[0x11] = 20;
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(bytes)),
        Some(xdf),
        Some(PathBuf::from("calibration.bin")),
        Some(PathBuf::from("current.xdf")),
    );
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    app.workspace.select_parameter(&semantic_id);
    assert!(app.open_table(&semantic_id));
    app.workspace.select_axis(0, 1).unwrap();
    app.open_tables[0].memory.decimal_places = 2;
    app.open_tables[0].memory.coloring.low = [1, 2, 3, 4];
    app.project_identity = Some("project-bin".to_string());
    app.project_preferences = ProjectPreferences::for_identity("project-bin");
    app.project_preferences.last_xdf_path = Some(PathBuf::from("remembered.xdf"));
    app.pending_xdf_restore = Some(PendingXdfRestore {
        bin_identity: "project-bin".to_string(),
        path: PathBuf::from("remembered.xdf"),
        available: false,
    });

    let report = app.debug_report_text();

    assert!(report.contains("selected_axis:"));
    assert!(report.contains("stride_bits:"));
    assert!(report.contains("range:"));
    assert!(report.contains("raw: u20"));
    assert!(report.contains("engineering: 10.00"));
    assert!(report.contains("decimal_places: 2"));
    assert!(report.contains("palette=([1, 2, 3, 4]"));
    assert!(report.contains("remembered_xdf: remembered.xdf"));
    assert!(report.contains("pending_xdf_restore: bin_identity=project-bin"));
    assert!(report.contains("path=remembered.xdf"));
}

#[test]
fn debug_report_uses_the_stable_dock_layer() {
    let context = egui::Context::default();
    let mut app = TunerApp::headless();
    app.debug_report_open = true;
    context
        .run_ui(egui::RawInput::default(), |ui| {
            app.show_debug_report(ui.ctx());
        })
        .drop_without_applying_deltas();
    let id = egui::Id::new(("debug-report", app.project_scope()));
    let expected_layer = egui::LayerId::new(egui::Order::Middle, id);
    assert!(context.memory(|memory| memory.layer_ids().any(|layer| layer == expected_layer)));
}

#[test]
fn debug_report_stays_above_the_operation_overlay() {
    let context = egui::Context::default();
    let mut app = TunerApp::headless();
    app.debug_report_open = true;
    app.operations
        .begin(OperationKind::LoadingXdf, "definition.xdf");
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0));
    context
        .run_ui(egui::RawInput::default(), |ui| {
            app.show_operation_overlay(ui.ctx(), canvas);
            app.show_debug_report(ui.ctx());
            app.raise_debug_report(ui.ctx());
        })
        .drop_without_applying_deltas();
    let project_scope = app.project_scope();
    let debug_layer = egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new(("debug-report", project_scope.as_str())),
    );
    let overlay_layer = egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new(("background-operation", project_scope.as_str())),
    );
    let layers: Vec<_> = context.memory(|memory| memory.layer_ids().collect());
    let debug_position = layers.iter().position(|layer| *layer == debug_layer);
    let overlay_position = layers.iter().position(|layer| *layer == overlay_layer);
    assert!(
        debug_position > overlay_position,
        "debug report should remain above the operation overlay: debug={debug_position:?}, overlay={overlay_position:?}, layers={layers:?}"
    );
}

#[test]
fn successful_bin_edits_undo_and_redo_invalidate_search_results() {
    let xdf = XdfDocument::parse(TWO_TABLE_XDF).unwrap();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10, 20])),
        Some(xdf),
        None,
        None,
    );
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    app.workspace.select_parameter(&id);
    app.search_state.open = true;
    app.search_results = search_snapshot(
        app.workspace.xdf.as_ref().unwrap(),
        app.workspace.bin.as_ref().map(BinDocument::bytes),
        &SearchState::for_query("10", SearchFieldScope::RawValues),
    );
    assert!(!app.search_results.is_empty());

    app.workspace.edit_text = "11".to_string();
    app.dispatch_command("edit.apply-cell").unwrap();
    assert!(app.search_results.is_empty());
    assert!(app.search_debounce_until.is_some());

    app.search_debounce_until = None;
    app.search_results = vec![SearchResult {
        summary: summary_from_parameter(&app.workspace.xdf.as_ref().unwrap().parameters[0]),
        matches: Vec::new(),
        diagnostics: Vec::new(),
    }];
    app.dispatch_command("edit.undo").unwrap();
    assert!(app.search_results.is_empty());

    app.search_debounce_until = None;
    app.search_results = vec![SearchResult {
        summary: summary_from_parameter(&app.workspace.xdf.as_ref().unwrap().parameters[0]),
        matches: Vec::new(),
        diagnostics: Vec::new(),
    }];
    app.dispatch_command("edit.redo").unwrap();
    assert!(app.search_results.is_empty());
}

#[test]
fn app_debug_report_includes_search_and_operation_state() {
    let mut app = TunerApp::headless();
    app.search_state = SearchState::for_query("pedal", SearchFieldScope::Title);
    app.search_results = Vec::new();
    app.operations
        .begin(OperationKind::LoadingXdf, "definition.xdf");

    let report = app.debug_report_text();

    assert!(report.contains("[APP OPERATIONS]"));
    assert!(report.contains("active: Interpreting XDF"));
    assert!(report.contains("[SEARCH]"));
    assert!(report.contains("query: \"pedal\""));
}

#[test]
fn save_as_creates_new_file_and_refuses_existing_target() {
    let (xdf, bin) = column_major_fixture();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);
    let path = temporary_test_path("bin");

    workspace.save_as(&path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), [10, 20, 30, 40, 50, 60]);
    assert!(workspace.save_as(&path).is_err());

    std::fs::remove_file(path).unwrap();
}

#[test]
fn layout_presets_and_command_dispatch_are_separate_from_documents() {
    let mut app = TunerApp::headless();
    let before = app.workspace.bin.is_none();
    app.apply_layout_preset(LayoutPreset::Diagnostics);
    assert_eq!(app.preferences.layout, LayoutPreset::Diagnostics);
    assert!(app.preferences.show_diagnostics);
    assert!(!app.preferences.show_editor);
    assert_eq!(app.workspace.bin.is_none(), before);
    app.dispatch_command("view.toggle-browser").unwrap();
    assert!(!app.preferences.show_browser);
    assert_eq!(app.workspace.bin.is_none(), before);
    app.dispatch_command("view.browser-recent-first").unwrap();
    assert_eq!(
        app.preferences.browser_organization,
        BrowserOrganization::RecentFirst
    );
    app.dispatch_command("view.browser-categories").unwrap();
    assert_eq!(
        app.preferences.browser_organization,
        BrowserOrganization::Categories
    );
    app.dispatch_command("view.debug-report").unwrap();
    assert!(app.debug_report_open);
}

#[test]
fn ctrl_f_search_command_opens_persistent_window_state() {
    let mut app = TunerApp::headless();
    let before = app.workspace.bin.as_ref().map(|bin| bin.bytes().to_vec());
    app.dispatch_command("view.search").unwrap();
    assert!(app.search_state.open);
    assert_eq!(
        app.workspace.bin.as_ref().map(|bin| bin.bytes().to_vec()),
        before
    );
}

#[test]
fn project_defaults_collapse_nested_category_paths_and_persist_catalog_sort() {
    let mut preferences = ProjectPreferences::default();
    preferences.collapsed_categories = vec!["Limiter/ RPM".into()];
    preferences.catalog_sort = CatalogSortKey::Bytes;
    preferences.catalog_sort_direction = SortDirection::Descending;
    let encoded = serde_json::to_string(&preferences).unwrap();
    let restored: ProjectPreferences = serde_json::from_str(&encoded).unwrap();
    assert_eq!(restored.catalog_sort, CatalogSortKey::Bytes);
    assert_eq!(restored.catalog_sort_direction, SortDirection::Descending);
    assert_eq!(restored.collapsed_categories, vec!["Limiter/ RPM"]);
}

#[test]
fn saved_category_state_overrides_an_existing_egui_header_state() {
    let context = egui::Context::default();
    let project_scope = "project";
    let category_key = "Fuel";
    let mut header_id = None;
    let mut parent_ui_id = None;

    context
        .run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                parent_ui_id = Some(ui.id());
                let response = egui::CollapsingHeader::new(category_key)
                    .id_salt(("parameter-category", project_scope, category_key))
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.label("body");
                    });
                header_id = Some(response.header_response.id);
            });
        })
        .drop_without_applying_deltas();
    let header_id = header_id.expect("category header should have an ID");
    assert_eq!(
        header_id,
        category_header_id(
            parent_ui_id.expect("category parent should have an ID"),
            project_scope,
            category_key,
        )
    );
    sync_category_header_state(&context, header_id, false);
    context
        .run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::CollapsingHeader::new(category_key)
                    .id_salt(("parameter-category", project_scope, category_key))
                    .default_open(false)
                    .show(ui, |_| {});
            });
        })
        .drop_without_applying_deltas();

    let state = egui::collapsing_header::CollapsingState::load(&context, header_id)
        .expect("category header state should be persisted");
    assert!(
        !state.is_open(),
        "saved collapsed state must close an already-open category"
    );
}

#[test]
fn saved_nested_category_state_overrides_existing_egui_header_state() {
    let context = egui::Context::default();
    let project_scope = "project";
    let parent_key = "Parent";
    let child_key = "Parent/Child";
    let mut parent_ui_id = None;
    let mut parent_header_id = None;
    let mut child_header_id = None;

    context
        .run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                parent_ui_id = Some(ui.id());
                let parent = egui::CollapsingHeader::new(parent_key)
                    .id_salt(("parameter-category", project_scope, parent_key))
                    .default_open(true)
                    .show(ui, |ui| {
                        let child = egui::CollapsingHeader::new(child_key)
                            .id_salt(("parameter-category", project_scope, child_key))
                            .default_open(true)
                            .show(ui, |ui| {
                                ui.label("body");
                            });
                        child_header_id = Some(child.header_response.id);
                    });
                parent_header_id = Some(parent.header_response.id);
            });
        })
        .drop_without_applying_deltas();

    let parent_ui_id = parent_ui_id.expect("category parent should have an ID");
    let parent_header_id = parent_header_id.expect("parent header should have an ID");
    let child_header_id = child_header_id.expect("child header should have an ID");
    assert_eq!(
        parent_header_id,
        category_header_id(parent_ui_id, project_scope, parent_key)
    );
    assert_eq!(
        child_header_id,
        category_header_id(
            category_body_ui_id(parent_ui_id, parent_header_id),
            project_scope,
            child_key,
        )
    );

    let nodes = vec![CategoryNode {
        key: parent_key.to_string(),
        name: parent_key.to_string(),
        path: vec![parent_key.to_string()],
        total_count: 1,
        parameters: Vec::new(),
        children: vec![CategoryNode {
            key: child_key.to_string(),
            name: "Child".to_string(),
            path: vec![parent_key.to_string(), "Child".to_string()],
            total_count: 1,
            parameters: Vec::new(),
            children: Vec::new(),
        }],
    }];
    let collapsed = vec![parent_key.to_string(), child_key.to_string()];
    sync_category_header_states(&context, parent_ui_id, &nodes, project_scope, &collapsed);

    assert!(
        !egui::collapsing_header::CollapsingState::load(&context, parent_header_id)
            .expect("parent category state should be persisted")
            .is_open()
    );
    assert!(
        !egui::collapsing_header::CollapsingState::load(&context, child_header_id)
            .expect("child category state should be persisted")
            .is_open()
    );
}

#[test]
fn search_result_favorite_action_uses_existing_table_key_without_editing_bin() {
    let xdf = XdfDocument::parse(TWO_TABLE_XDF).unwrap();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10, 20])),
        Some(xdf),
        None,
        None,
    );
    let summary = app.workspace.filtered_parameters("")[0].clone();
    let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    app.favorite_search_result(&summary);
    let key = table_key(
        &app.workspace.xdf.as_ref().unwrap().normalized_fingerprint,
        &summary.semantic_id,
    );
    assert!(app.preferences.favorite_tables.contains(&key));
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), before);
}

#[test]
fn stale_load_cannot_replace_newer_document_and_failure_preserves_previous_state() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![1]));
    let old = app.operations.begin(OperationKind::LoadingBin, "old.bin");
    let new = app.operations.begin(OperationKind::LoadingBin, "new.bin");
    app.install_operation_result(OperationResult::bin(old, BinDocument::from_bytes(vec![2])));
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), &[1]);
    app.install_operation_result(OperationResult::error(new, "new.bin", "read failed"));
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), &[1]);
    assert_eq!(app.workspace.status.level, StatusLevel::Error);
}

#[test]
fn installing_loaded_bin_restores_workspace_and_starts_validation() {
    let (xdf, _) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(None, Some(xdf), None, None);
    let loading = app
        .operations
        .begin(OperationKind::LoadingBin, "fixture.bin");

    app.install_operation_result(OperationResult::bin(
        loading,
        BinDocument::from_bytes(vec![10, 20, 30, 40, 50, 60]),
    ));

    assert_eq!(
        app.workspace.bin.as_ref().map(BinDocument::bytes),
        Some(&[10, 20, 30, 40, 50, 60][..])
    );
    assert_eq!(
        app.operations.active().map(|operation| operation.kind),
        Some(OperationKind::Validating)
    );
}

#[test]
fn path_based_document_load_uses_async_workspace_restore_before_validation() {
    let (xdf, _) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(None, Some(xdf), None, None);
    let loading = app
        .operations
        .begin(OperationKind::LoadingBin, "fixture.bin");
    let mut loaded = OperationResult::bin(
        loading,
        BinDocument::from_bytes(vec![10, 20, 30, 40, 50, 60]),
    );
    loaded.subject = "C:/tuning/fixture.bin".to_string();

    app.install_operation_result(loaded);

    assert_eq!(
        app.operations.active().map(|operation| operation.kind),
        Some(OperationKind::RestoringWorkspace)
    );
    let restored = (0..100).find_map(|_| {
        let result = app.operations.take_results().into_iter().next();
        if result.is_none() {
            std::thread::sleep(Duration::from_millis(5));
        }
        result
    });
    app.install_operation_result(restored.expect("workspace restore should finish"));
    assert_eq!(
        app.operations.active().map(|operation| operation.kind),
        Some(OperationKind::Validating)
    );
    assert_eq!(
        app.project_identity.as_deref(),
        Some("c:/tuning/fixture.bin")
    );
}

#[test]
fn installing_written_result_reports_the_background_output() {
    let mut app = TunerApp::headless();
    let operation = app.operations.begin(OperationKind::SavingBin, "edited.bin");

    app.install_operation_result(OperationResult::written(operation, "edited.bin"));

    assert_eq!(app.workspace.status.level, StatusLevel::Success);
    assert!(app.workspace.status.text.contains("edited.bin"));
    assert!(!app.operations.is_busy());
}

#[test]
fn document_edit_commands_are_blocked_during_background_work() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    app.workspace.select_parameter(&id);
    app.workspace.select_cell(0, 0).unwrap();
    app.workspace.apply_engineering_text("99").unwrap();
    let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();
    app.operations.begin(OperationKind::LoadingXdf, "next.xdf");

    assert!(app.dispatch_command("edit.undo").is_err());
    assert_eq!(app.workspace.bin.as_ref().unwrap().bytes(), before);
    assert!(app.workspace.status.text.contains("background operation"));
}

#[test]
fn search_start_is_blocked_until_document_loading_finishes() {
    let mut app = TunerApp::headless();
    app.operations
        .begin(OperationKind::LoadingXdf, "definition.xdf");

    assert!(app.dispatch_command("view.search").is_err());
    assert!(!app.search_state.open);
}

#[test]
fn surface_preferences_round_trip_and_sanitize_extreme_values() {
    let mut preferences = ProjectPreferences::for_identity("bin");
    preferences.surface_windows.insert(
        "xdf|map".to_string(),
        SurfaceViewMemory {
            width: u32::MAX,
            height: u32::MAX,
            zoom_percent: u16::MAX,
            pitch: f32::NAN,
            fixed_min: Some(9.0),
            fixed_max: Some(1.0),
            ..SurfaceViewMemory::default()
        },
    );
    preferences.open_surface_keys = vec!["xdf|map".to_string(), "xdf|map".to_string()];
    preferences.sanitize();

    let memory = preferences.surface_windows.get("xdf|map").unwrap();
    assert_eq!(memory.width, 2_400);
    assert_eq!(memory.height, 1_800);
    assert_eq!(memory.zoom_percent, 300);
    assert_eq!(memory.pitch, 34.0);
    assert!(memory.fixed_max.is_none());
    assert_eq!(preferences.open_surface_keys, vec!["xdf|map"]);

    let encoded = serde_json::to_string(&preferences).unwrap();
    let decoded: ProjectPreferences = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, preferences);

    let old: ProjectPreferences = serde_json::from_value(json!({
        "version": 3,
        "bin_identity": "bin",
    }))
    .unwrap();
    assert!(old.surface_windows.is_empty());
    assert!(old.open_surface_keys.is_empty());
}

#[test]
fn surface_open_state_is_project_scoped_by_table_key() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    let fingerprint = app
        .workspace
        .xdf
        .as_ref()
        .unwrap()
        .normalized_fingerprint
        .clone();
    let key = table_key(&fingerprint, &id);

    app.project_preferences.open_surface_keys = vec!["other-xdf|other-table".to_string()];
    app.restore_project_surfaces();
    assert!(app.open_surfaces.is_empty());

    let mut saved_memory = SurfaceViewMemory::default();
    saved_memory.x = 333;
    saved_memory.y = 222;
    app.project_preferences
        .surface_windows
        .insert(key.clone(), saved_memory);
    app.project_preferences.open_surface_keys = vec![key.clone()];
    app.restore_project_surfaces();
    assert_eq!(app.open_surfaces.len(), 1);
    assert_eq!(app.open_surfaces[0].memory.x, 333);
    assert_eq!(app.open_surfaces[0].memory.y, 222);

    app.close_surface(&key);
    assert!(app.open_surface(&id));
    assert_eq!(app.open_surfaces.len(), 1);
    assert_eq!(app.active_surface_key.as_deref(), Some(key.as_str()));
}

#[test]
fn surface_snapshot_uses_actual_axes_and_engineering_values() {
    let xdf = XdfDocument::parse(AXIS_TABLE_XDF).unwrap();
    let mut bytes = vec![0; 0x40];
    bytes[0x10] = 4;
    bytes[0x11] = 8;
    bytes[0x20] = 10;
    bytes[0x22] = 20;
    bytes[0x30..0x34].copy_from_slice(&[30, 40, 50, 60]);
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(BinDocument::from_bytes(bytes)), Some(xdf), None, None);
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();

    let data = app.surface_data_for(&id).unwrap();

    assert_eq!((data.rows, data.columns), (2, 2));
    assert_eq!(data.x, vec![2.0, 4.0]);
    assert_eq!(data.y, vec![10.0, 20.0]);
    assert_eq!(
        data.values,
        vec![Some(30.0), Some(40.0), Some(40.0), Some(50.0)]
    );
}

#[test]
fn table_cell_batch_snapshot_reads_each_cell_once_and_preserves_values() {
    let (xdf, bin) = column_major_fixture();
    let mut workspace = WorkspaceState::default();
    workspace.set_documents(Some(bin), Some(xdf), None, None);
    let parameter = &workspace.xdf.as_ref().unwrap().parameters[0];

    let views = workspace.cell_views_for(parameter);

    assert_eq!(views.len(), 6);
    assert_eq!(views[0].as_ref().unwrap().engineering, Some(10.0));
    assert_eq!(views[5].as_ref().unwrap().engineering, Some(60.0));
}

#[test]
fn surface_snapshot_falls_back_to_indexes_and_raw_values() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();

    let data = app.surface_data_for(&id).unwrap();

    assert_eq!(data.x, vec![0.0, 1.0, 2.0]);
    assert_eq!(data.y, vec![0.0, 1.0]);
    assert_eq!(
        data.values,
        vec![
            Some(10.0),
            Some(30.0),
            Some(50.0),
            Some(20.0),
            Some(40.0),
            Some(60.0),
        ]
    );
}

#[test]
fn cached_surface_snapshot_refreshes_after_a_workspace_edit() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();

    let first = app.cached_surface_data_for(&id).unwrap();
    assert_eq!(first.value(0, 0), Some(10.0));

    app.workspace.select_parameter(&id);
    app.workspace.select_cell(0, 0).unwrap();
    app.workspace.apply_engineering_text("99").unwrap();

    let second = app.cached_surface_data_for(&id).unwrap();
    assert_eq!(second.value(0, 0), Some(99.0));
    assert_eq!(first.value(0, 0), Some(10.0));
}

#[test]
fn open_surface_focuses_existing_surface_without_duplicates() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();

    assert!(app.open_surface(&id));
    assert!(app.open_surface(&id));
    assert_eq!(app.open_surfaces.len(), 1);
    assert_eq!(
        app.active_surface_key,
        app.open_surfaces.first().map(|surface| surface.key.clone())
    );

    assert!(app.reset_surface_view(&id));
    assert!(app.close_surface(&app.active_surface_key.clone().unwrap()));
    assert!(app.open_surfaces.is_empty());
}

#[test]
fn clicking_visible_surface_title_bar_raises_it_above_previous_active() {
    let xdf = XdfDocument::parse(TWO_TABLE_XDF).unwrap();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10, 20])),
        Some(xdf),
        None,
        None,
    );
    let ids: Vec<_> = app
        .workspace
        .filtered_parameters("")
        .into_iter()
        .map(|parameter| parameter.semantic_id)
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(app.open_surface(&ids[0]));
    assert!(app.open_surface(&ids[1]));
    let first_key = app.open_surfaces[0].key.clone();
    let second_key = app.open_surfaces[1].key.clone();
    app.open_surfaces[0].memory = SurfaceViewMemory {
        x: 100,
        y: 100,
        width: 520,
        height: 320,
        ..SurfaceViewMemory::default()
    };
    app.open_surfaces[1].memory = SurfaceViewMemory {
        x: 140,
        y: 140,
        width: 520,
        height: 320,
        ..SurfaceViewMemory::default()
    };
    app.active_surface_key = Some(second_key.clone());

    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let run_frame = |app: &mut TunerApp, events: Vec<egui::Event>| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        input.events = events;
        context
            .run_ui(input, |ctx| app.show_surface_windows(ctx, canvas))
            .drop_without_applying_deltas();
    };
    run_frame(&mut app, Vec::new());
    run_frame(&mut app, Vec::new());
    run_frame(&mut app, Vec::new());

    let first_id = egui::Id::new(("surface-window", first_key.as_str()));
    let second_id = egui::Id::new(("surface-window", second_key.as_str()));
    let first_rect = context
        .memory(|memory| memory.area_rect(first_id))
        .expect("first surface should have a geometry");
    let title_bar_point = first_rect.min + egui::vec2(100.0, 14.0);
    assert_eq!(
        context.layer_id_at(title_bar_point),
        Some(egui::LayerId::new(egui::Order::Middle, first_id))
    );

    run_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(title_bar_point),
            egui::Event::PointerButton {
                pos: title_bar_point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    run_frame(&mut app, Vec::new());

    assert_eq!(app.active_surface_key.as_deref(), Some(first_key.as_str()));
    assert_eq!(
        context.layer_id_at(title_bar_point),
        Some(egui::LayerId::new(egui::Order::Middle, first_id))
    );
    assert_ne!(
        context.layer_id_at(title_bar_point),
        Some(egui::LayerId::new(egui::Order::Middle, second_id))
    );
}

#[test]
fn surface_command_is_available_for_a_table_and_renders_headless() {
    let context = egui::Context::default();
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    app.workspace.select_parameter(&id);
    let entry = app
        .registry
        .entries(&app.workspace)
        .into_iter()
        .find(|entry| entry.descriptor.id == "view.3d-surface")
        .expect("3D surface command should be registered");
    assert!(entry.enabled, "{}", entry.reason);

    app.dispatch_command("view.3d-surface").unwrap();
    context
        .run_ui(egui::RawInput::default(), |ctx| {
            app.show_surface_windows(
                ctx,
                egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 700.0)),
            );
        })
        .drop_without_applying_deltas();

    assert_eq!(app.open_surfaces.len(), 1);
    assert!(app.operations.active().is_none());
}

#[test]
#[ignore = "requires the local-only SCGa05 calibration BIN/XDF fixture"]
fn zoomed_surface_faces_are_clipped_to_the_inner_chart_rect() {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Test bin and xdf");
    let xdf = XdfDocument::load(fixture_dir.join("SCGa05_cal.xdf")).unwrap();
    let bin = BinDocument::load(fixture_dir.join("SCGa05_cal.bin")).unwrap();
    let parameter = xdf
        .parameters
        .iter()
        .find(|parameter| parameter.title == "Airflow to Torque VVL 0 Intake 0 Exhaust 0")
        .expect("the supplied fixture should include the graph from the report");
    let semantic_id = parameter.semantic_id.clone();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    assert!(app.open_surface(&semantic_id));
    let key = app.open_surfaces[0].key.clone();
    let memory = &mut app.open_surfaces[0].memory;
    memory.x = 80;
    memory.y = 60;
    memory.width = 1_200;
    memory.height = 900;
    memory.position_saved = true;
    memory.yaw = 61.47992;
    memory.pitch = 11.319976;
    memory.zoom_percent = 199;
    memory.wireframe = true;

    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_800.0, 1_200.0));
    app.initial_window_bounds = Some(screen);
    app.tool_window_bounds = Some(screen);
    let mut output = None;
    for (index, time) in [1.0, 1.1, 1.2].into_iter().enumerate() {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        let frame = context.run_ui(input, |ctx| app.show_surface_window(ctx, &key));
        if index == 2 {
            output = Some(frame);
        } else {
            frame.drop_without_applying_deltas();
        }
    }
    let output = output.expect("last surface frame should be retained");
    let plot_id = egui::Id::new(("surface-plot", key.as_str()));
    let plot_rect = context
        .read_response(plot_id)
        .expect("surface plot should render")
        .rect;
    let chart_rect = plot_rect.shrink2(egui::vec2(76.0, 50.0));
    let data = app.surface_data_for(&semantic_id).unwrap();
    let range = surface_range_for_memory(&data, &app.open_surfaces[0].memory).unwrap();
    let faces = project_surface_geometry(&data, &app.open_surfaces[0].memory, chart_rect, range).1;
    let projection_overflows = faces
        .iter()
        .flat_map(|face| face.points)
        .any(|point| !chart_rect.contains(point));

    let face_clips: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Path(path)
                if path.closed
                    && path.points.len() == 3
                    && faces
                        .iter()
                        .any(|face| face.points.as_slice() == path.points.as_slice()) =>
            {
                Some(clipped.clip_rect)
            }
            _ => None,
        })
        .collect();
    let faces_are_clipped = face_clips
        .iter()
        .all(|clip_rect| chart_rect.contains_rect(*clip_rect));
    let faces_were_rendered = !face_clips.is_empty();
    output.drop_without_applying_deltas();

    assert!(
        projection_overflows,
        "the saved high-zoom view should reproduce geometry extending past the chart"
    );
    assert!(faces_were_rendered, "surface face fills should be emitted");
    assert!(
        faces_are_clipped,
        "surface faces must be clipped to the chart instead of the full plot"
    );
}

#[test]
fn surface_marker_click_selects_the_referenced_cell() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_surface(&semantic_id));
    let key = app.open_surfaces[0].key.clone();
    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let run_frame = |app: &mut TunerApp, events: Vec<egui::Event>| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        input.events = events;
        context
            .run_ui(input, |ctx| app.show_surface_windows(ctx, canvas))
            .drop_without_applying_deltas();
    };
    run_frame(&mut app, Vec::new());
    run_frame(&mut app, Vec::new());

    let plot_id = egui::Id::new(("surface-plot", key.as_str()));
    let plot_rect = context
        .read_response(plot_id)
        .expect("surface plot should have an interaction region")
        .rect;
    let data = app.surface_data_for(&semantic_id).unwrap();
    let range = surface_range_for_memory(&data, &app.open_surfaces[0].memory).unwrap();
    let points = project_surface_points(
        &data,
        &app.open_surfaces[0].memory,
        plot_rect.shrink2(egui::vec2(76.0, 50.0)),
        range,
    );
    let point = points
        .iter()
        .find(|point| (point.row, point.column) == (0, 2))
        .copied()
        .expect("test surface should have the target cell");
    run_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(point.position),
            egui::Event::PointerButton {
                pos: point.position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    run_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(point.position),
            egui::Event::PointerButton {
                pos: point.position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );

    assert_eq!(
        app.workspace.selected_cell_range().unwrap().bounds(),
        (0, 2, 0, 2)
    );
    assert_eq!(app.open_tables.len(), 1);
    assert_eq!(app.active_surface_key.as_deref(), Some(key.as_str()));
    assert!(app.active_table_key.is_none());
}

#[test]
fn surface_background_click_clears_the_surface_cell_selection() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_surface(&semantic_id));
    app.workspace.select_cell_range(0, 0, 1, 2).unwrap();
    let key = app.open_surfaces[0].key.clone();
    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let run_frame = |app: &mut TunerApp, events: Vec<egui::Event>| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        input.events = events;
        context
            .run_ui(input, |ctx| app.show_surface_windows(ctx, canvas))
            .drop_without_applying_deltas();
    };
    run_frame(&mut app, Vec::new());
    run_frame(&mut app, Vec::new());
    let plot_id = egui::Id::new(("surface-plot", key.as_str()));
    let plot_rect = context
        .read_response(plot_id)
        .expect("surface plot should have an interaction region")
        .rect;
    let click = plot_rect.left_top() + egui::vec2(4.0, 4.0);
    run_frame(&mut app, vec![egui::Event::PointerMoved(click)]);
    run_frame(
        &mut app,
        vec![egui::Event::PointerButton {
            pos: click,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    run_frame(
        &mut app,
        vec![egui::Event::PointerButton {
            pos: click,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );

    assert!(app.workspace.selected_cell_range().is_none());
}

#[test]
fn focusing_surface_preserves_selection_for_the_same_table() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_surface(&semantic_id));
    app.workspace.select_cell_range(0, 0, 1, 2).unwrap();
    let key = app.open_surfaces[0].key.clone();

    assert!(app.focus_surface(&key));
    assert_eq!(
        app.workspace.selected_cell_range().unwrap().bounds(),
        (0, 0, 1, 2)
    );
}

#[test]
fn surface_primary_drag_selects_a_logical_cell_rectangle() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_surface(&semantic_id));
    let key = app.open_surfaces[0].key.clone();
    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let run_frame = |app: &mut TunerApp, events: Vec<egui::Event>| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        input.events = events;
        context
            .run_ui(input, |ctx| app.show_surface_windows(ctx, canvas))
            .drop_without_applying_deltas();
    };
    run_frame(&mut app, Vec::new());
    run_frame(&mut app, Vec::new());

    let plot_id = egui::Id::new(("surface-plot", key.as_str()));
    let plot_rect = context
        .read_response(plot_id)
        .expect("surface plot should have an interaction region")
        .rect;
    let data = app.surface_data_for(&semantic_id).unwrap();
    let range = surface_range_for_memory(&data, &app.open_surfaces[0].memory).unwrap();
    let points = project_surface_points(
        &data,
        &app.open_surfaces[0].memory,
        plot_rect.shrink2(egui::vec2(76.0, 50.0)),
        range,
    );
    let start = egui::pos2(
        points
            .iter()
            .map(|point| point.position.x)
            .fold(f32::INFINITY, f32::min)
            - 4.0,
        points
            .iter()
            .map(|point| point.position.y)
            .fold(f32::INFINITY, f32::min)
            - 4.0,
    );
    let end = egui::pos2(
        points
            .iter()
            .map(|point| point.position.x)
            .fold(f32::NEG_INFINITY, f32::max)
            + 4.0,
        points
            .iter()
            .map(|point| point.position.y)
            .fold(f32::NEG_INFINITY, f32::max)
            + 4.0,
    );
    run_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    run_frame(&mut app, vec![egui::Event::PointerMoved(end)]);
    run_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(end),
            egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );

    assert_eq!(
        app.workspace.selected_cell_range().unwrap().bounds(),
        (0, 0, 1, 2)
    );
}

#[test]
fn surface_value_drag_commits_only_selected_cells_as_one_edit() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_surface(&semantic_id));
    app.workspace.select_cell_range(0, 0, 0, 1).unwrap();
    assert_eq!(
        app.workspace.selected_cell_range().unwrap().bounds(),
        (0, 0, 0, 1)
    );
    let key = app.open_surfaces[0].key.clone();
    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let run_frame = |app: &mut TunerApp, events: Vec<egui::Event>| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        input.events = events;
        context
            .run_ui(input, |ctx| app.show_surface_windows(ctx, canvas))
            .drop_without_applying_deltas();
    };
    run_frame(&mut app, Vec::new());
    run_frame(&mut app, Vec::new());

    let plot_id = egui::Id::new(("surface-plot", key.as_str()));
    let plot_rect = context
        .read_response(plot_id)
        .expect("surface plot should have an interaction region")
        .rect;
    let data = app.surface_data_for(&semantic_id).unwrap();
    let range = surface_range_for_memory(&data, &app.open_surfaces[0].memory).unwrap();
    let chart_rect = plot_rect.shrink2(egui::vec2(76.0, 50.0));
    let point = project_surface_points(&data, &app.open_surfaces[0].memory, chart_rect, range)
        .into_iter()
        .find(|point| (point.row, point.column) == (0, 0))
        .expect("test surface should have the selected anchor cell");
    let end = point.position - egui::vec2(0.0, 80.0);
    let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();

    run_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(point.position),
            egui::Event::PointerButton {
                pos: point.position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    run_frame(&mut app, vec![egui::Event::PointerMoved(end)]);
    assert!(
        app.surface_value_drag.is_some(),
        "dragging a selected surface point should enter value-edit mode"
    );
    assert_eq!(
        app.surface_value_drag.as_ref().unwrap().selection.bounds(),
        (0, 0, 0, 1)
    );
    run_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(end),
            egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );

    let after = app.workspace.bin.as_ref().unwrap().bytes();
    assert_ne!(after[0], before[0], "selected cell should be edited");
    assert_ne!(after[2], before[2], "selected cell should be edited");
    assert_eq!(&after[1], &before[1], "unselected cell must not be edited");
    assert_eq!(
        &after[3..],
        &before[3..],
        "unselected cells must not be edited"
    );
    assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), 1);

    app.workspace.undo().unwrap();
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes(),
        before.as_slice()
    );
}

#[test]
fn surface_soft_pull_drag_falls_off_across_selected_cells_only() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_surface(&semantic_id));
    app.workspace.select_cell_range(0, 0, 0, 2).unwrap();
    let key = app.open_surfaces[0].key.clone();
    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let run_frame = |app: &mut TunerApp, events: Vec<egui::Event>| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        input.events = events;
        context
            .run_ui(input, |ctx| app.show_surface_windows(ctx, canvas))
            .drop_without_applying_deltas();
    };
    run_frame(&mut app, Vec::new());
    run_frame(&mut app, Vec::new());

    let plot_id = egui::Id::new(("surface-plot", key.as_str()));
    let plot_rect = context
        .read_response(plot_id)
        .expect("surface plot should have an interaction region")
        .rect;
    let data = app.surface_data_for(&semantic_id).unwrap();
    let range = surface_range_for_memory(&data, &app.open_surfaces[0].memory).unwrap();
    let anchor = project_surface_points(
        &data,
        &app.open_surfaces[0].memory,
        plot_rect.shrink2(egui::vec2(76.0, 50.0)),
        range,
    )
    .into_iter()
    .find(|point| (point.row, point.column) == (0, 0))
    .expect("test surface should have the selected anchor cell");
    let end = anchor.position - egui::vec2(0.0, 80.0);
    let before = app.workspace.bin.as_ref().unwrap().bytes().to_vec();

    run_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(anchor.position),
            egui::Event::PointerButton {
                pos: anchor.position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::Key {
                key: egui::Key::S,
                physical_key: Some(egui::Key::S),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    run_frame(&mut app, vec![egui::Event::PointerMoved(end)]);
    run_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(end),
            egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );

    let after = app.workspace.bin.as_ref().unwrap().bytes();
    let anchor_delta = i16::from(after[0]) - i16::from(before[0]);
    let neighbor_delta = i16::from(after[4]) - i16::from(before[4]);
    assert!(anchor_delta > 0, "soft-pull anchor should move upward");
    assert!(
        neighbor_delta > 0,
        "selected neighbor should receive falloff"
    );
    assert!(anchor_delta > neighbor_delta);
    assert_eq!(&after[1], &before[1], "unselected cell must not be edited");
    assert_eq!(&after[3], &before[3], "unselected cell must not be edited");
    assert_eq!(&after[5], &before[5], "unselected cell must not be edited");
    assert_eq!(app.workspace.bin.as_ref().unwrap().undo_depth(), 1);
}

#[test]
fn surface_secondary_drag_reverses_orbit_direction() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let semantic_id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    assert!(app.open_surface(&semantic_id));
    let key = app.open_surfaces[0].key.clone();
    let context = egui::Context::default();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let run_frame = |app: &mut TunerApp, events: Vec<egui::Event>| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(canvas);
        input.events = events;
        context
            .run_ui(input, |ctx| app.show_surface_windows(ctx, canvas))
            .drop_without_applying_deltas();
    };
    run_frame(&mut app, Vec::new());
    run_frame(&mut app, Vec::new());

    let plot_id = egui::Id::new(("surface-plot", key.as_str()));
    let plot_rect = context
        .read_response(plot_id)
        .expect("surface plot should have an interaction region")
        .rect;
    let start = plot_rect.center();
    let end = start + egui::vec2(40.0, 20.0);
    let initial_yaw = app.open_surfaces[0].memory.yaw;
    let initial_pitch = app.open_surfaces[0].memory.pitch;

    run_frame(
        &mut app,
        vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Secondary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    run_frame(&mut app, vec![egui::Event::PointerMoved(end)]);

    assert!(app.open_surfaces[0].memory.yaw < initial_yaw);
    assert!(app.open_surfaces[0].memory.pitch > initial_pitch);
}

#[test]
fn ui_ipc_surface_actions_open_focus_reset_and_report_state() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace
        .set_documents(Some(bin), Some(xdf), None, None);
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();

    let capabilities = app.ui_ipc_capabilities();
    let actions = capabilities["actions"].as_array().unwrap();
    for action in ["open_surface", "focus_surface", "reset_surface_view"] {
        assert!(actions.iter().any(|value| value == action));
    }

    let open = app.handle_ui_ipc_request(&ui_ipc::UiIpcRequest::for_test(
        "open_surface",
        &id,
        None,
        None,
    ));
    assert!(open.ok);
    let key = app.active_surface_key.clone().unwrap();
    let focus = app.handle_ui_ipc_request(&ui_ipc::UiIpcRequest::for_test(
        "focus_surface",
        &id,
        None,
        None,
    ));
    assert!(focus.ok);
    let reset = app.handle_ui_ipc_request(&ui_ipc::UiIpcRequest::for_test(
        "reset_surface_view",
        &id,
        None,
        None,
    ));
    assert!(reset.ok);
    assert_eq!(app.active_surface_key.as_deref(), Some(key.as_str()));
    assert_eq!(app.ui_ipc_state()["surface"]["open_count"], 1);
}

#[test]
fn ui_ipc_type_aware_actions_open_focus_and_apply_raw_values() {
    let xdf = XdfDocument::parse(TYPE_AWARE_XDF).unwrap();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0, 0, 0xA0, 0xA0])),
        Some(xdf),
        None,
        None,
    );
    let scalar = app
        .workspace
        .filtered_parameters("scalar")
        .into_iter()
        .next()
        .unwrap()
        .semantic_id;
    let capabilities = app.ui_ipc_capabilities();
    for action in [
        "open_parameter",
        "focus_parameter",
        "set_parameter_value",
        "set_parameter_raw",
    ] {
        assert!(capabilities["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == action));
    }

    let open = app.handle_ui_ipc_request(&ui_ipc::UiIpcRequest::for_test(
        "open_parameter",
        &scalar,
        None,
        None,
    ));
    assert!(open.ok);
    assert_eq!(app.ui_ipc_state()["open_tables"][0]["kind"], "constant");

    let mut set_raw = ui_ipc::UiIpcRequest::for_test("set_parameter_raw", &scalar, None, None);
    set_raw.edit_value = Some("0x1234".to_string());
    let applied = app.handle_ui_ipc_request(&set_raw);
    assert!(applied.ok, "{}", applied.message);
    assert_eq!(
        app.workspace.bin.as_ref().unwrap().bytes()[..2],
        [0x34, 0x12]
    );

    let mut focus = ui_ipc::UiIpcRequest::for_test("focus_parameter", &scalar, None, None);
    focus.row = Some(0);
    focus.column = Some(0);
    let focused = app.handle_ui_ipc_request(&focus);
    assert!(focused.ok, "{}", focused.message);
    assert_eq!(app.workspace.selected_cell, (0, 0));
}

#[test]
fn scalar_window_renders_type_aware_engineering_and_raw_inputs() {
    let xdf = XdfDocument::parse(TYPE_AWARE_XDF).unwrap();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![0, 0, 0, 0])),
        Some(xdf),
        None,
        None,
    );
    let semantic_id = app
        .workspace
        .filtered_parameters("scalar")
        .into_iter()
        .next()
        .unwrap()
        .semantic_id;
    assert!(app.open_table(&semantic_id));
    let key = app.open_tables[0].key.clone();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let context = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(canvas);
    context
        .run_ui(input, |ctx| app.show_table_window(ctx, canvas, &key))
        .drop_without_applying_deltas();
    assert!(context
        .read_response(egui::Id::new(("parameter-engineering-input", key.as_str())))
        .is_some());
    assert!(context
        .read_response(egui::Id::new(("parameter-raw-input", key.as_str())))
        .is_some());
}

#[test]
fn xdf_maker_command_opens_an_in_memory_new_xdf_editor() {
    let mut app = TunerApp::headless();
    app.dispatch_command("tools.xdf-maker-editor").unwrap();
    assert!(app.xdf_editor.open);
    assert!(app.xdf_editor.draft.as_ref().unwrap().parameters.is_empty());
}

#[test]
fn tools_menu_opens_the_xdf_maker_editor() {
    let mut app = TunerApp::headless();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let draw_toolbar = |app: &mut TunerApp, time, events| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        input.events = events;
        context.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.show_toolbar(ui));
        })
    };

    assert!(app.preferences.sweep_enabled);
    let output = draw_toolbar(&mut app, 0.0, Vec::new());
    let tools_rect = rendered_text_rect(&output, "Tools");
    output.drop_without_applying_deltas();
    let tools_point = tools_rect.expect("toolbar should show Tools").center();
    for (time, pressed) in [(0.1, true), (0.2, false)] {
        let output = draw_toolbar(
            &mut app,
            time,
            vec![
                egui::Event::PointerMoved(tools_point),
                egui::Event::PointerButton {
                    pos: tools_point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        output.drop_without_applying_deltas();
    }

    let output = draw_toolbar(&mut app, 0.3, Vec::new());
    let editor_rect = rendered_text_rect(&output, "XDF Maker / Editor");
    let notepad_available_from_tools = rendered_text_rect(&output, "Project Notepad…").is_some();
    output.drop_without_applying_deltas();
    assert!(notepad_available_from_tools);
    let editor_point = editor_rect
        .expect("Tools menu should include XDF Maker / Editor")
        .center();
    for (time, pressed) in [(0.4, true), (0.5, false)] {
        let output = draw_toolbar(
            &mut app,
            time,
            vec![
                egui::Event::PointerMoved(editor_point),
                egui::Event::PointerButton {
                    pos: editor_point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        output.drop_without_applying_deltas();
    }

    assert!(app.xdf_editor.open);
}

#[test]
fn selected_cell_sweep_toolbar_button_toggles_animation() {
    let mut app = TunerApp::headless();
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let draw_toolbar = |app: &mut TunerApp, time, events| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        input.events = events;
        context.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.show_toolbar(ui));
        })
    };

    let output = draw_toolbar(&mut app, 0.0, Vec::new());
    let sweep_point = rendered_text_rect(&output, "Sweep")
        .expect("main toolbar should expose the selected-cell sweep toggle")
        .center();
    output.drop_without_applying_deltas();
    for (time, pressed) in [(0.1, true), (0.2, false)] {
        let output = draw_toolbar(
            &mut app,
            time,
            vec![
                egui::Event::PointerMoved(sweep_point),
                egui::Event::PointerButton {
                    pos: sweep_point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        output.drop_without_applying_deltas();
    }
    assert!(!app.preferences.sweep_enabled);

    let output = draw_toolbar(&mut app, 0.3, Vec::new());
    let sweep_point = rendered_text_rect(&output, "Sweep")
        .expect("sweep toggle should remain in the toolbar")
        .center();
    output.drop_without_applying_deltas();
    for (time, pressed) in [(0.4, true), (0.5, false)] {
        let output = draw_toolbar(
            &mut app,
            time,
            vec![
                egui::Event::PointerMoved(sweep_point),
                egui::Event::PointerButton {
                    pos: sweep_point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        output.drop_without_applying_deltas();
    }
    assert!(app.preferences.sweep_enabled);
}

#[test]
fn recent_xdf_file_menu_opens_a_selected_definition() {
    let xdf_path = temporary_test_path("xdf");
    std::fs::write(&xdf_path, COLUMN_MAJOR_XDF).unwrap();
    let file_label = xdf_path.file_name().unwrap().to_string_lossy().into_owned();
    let mut app = TunerApp::headless();
    app.preferences.recent_xdfs = vec![xdf_path.clone()];

    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let draw_toolbar = |app: &mut TunerApp, time, events| {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        input.events = events;
        context.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.show_toolbar(ui));
        })
    };
    let mut time = 0.0;
    let mut click_text = |app: &mut TunerApp, text: &str| {
        let output = draw_toolbar(app, time, Vec::new());
        time += 0.1;
        let point = rendered_text_rect(&output, text)
            .unwrap_or_else(|| panic!("toolbar/menu should show {text:?}"))
            .center();
        output.drop_without_applying_deltas();
        for pressed in [true, false] {
            let output = draw_toolbar(
                app,
                time,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            output.drop_without_applying_deltas();
            time += 0.1;
        }
    };

    click_text(&mut app, "File");
    click_text(&mut app, "Recent XDFs");
    click_text(&mut app, &file_label);

    let active = app
        .operations
        .active()
        .expect("recent XDF should start loading");
    assert_eq!(active.kind, OperationKind::LoadingXdf);
    let result = (0..100).find_map(|_| {
        let result = app.operations.take_results().into_iter().next();
        if result.is_none() {
            std::thread::sleep(Duration::from_millis(5));
        }
        result
    });
    app.install_operation_result(result.expect("recent XDF worker should finish"));
    assert_eq!(app.workspace.xdf_path.as_deref(), Some(xdf_path.as_path()));

    click_text(&mut app, "File");
    click_text(&mut app, "Recent XDFs");
    click_text(&mut app, "Clear Recent XDFs");
    assert!(app.preferences.recent_xdfs.is_empty());
    std::fs::remove_file(xdf_path).unwrap();
}

#[test]
fn map_finder_prefills_a_reviewable_xdf_definition_with_selected_axis() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![0; 256]));
    app.map_finder.candidates.push(MapCandidate {
        offset: 0x20,
        byte_length: 8,
        rows: 2,
        columns: 2,
        display_format: HexDisplayFormat::Unsigned16,
        endianness: HexEndianness::Little,
        score: 91,
        value_range: (0.0, 100.0),
        value_bands: 4,
        axis_suggestions: vec![MapAxisSuggestion {
            offset: 0x10,
            values: vec![0.0, 500.0],
            score: 88,
            increasing: true,
            matches_columns: true,
            matches_rows: false,
            display_format: HexDisplayFormat::Unsigned8,
            endianness: HexEndianness::Little,
        }],
        selected_x_axis: Some(0),
        selected_y_axis: None,
    });

    app.open_xdf_editor_from_candidate(0).unwrap();
    assert!(app.xdf_editor.draft.as_ref().unwrap().parameters.is_empty());
    assert_eq!(app.xdf_editor.form.address_text, "0x20");
    assert_eq!(app.xdf_editor.form.definition.dimensions.rows, 2);
    assert_eq!(app.xdf_editor.form.definition.dimensions.columns, 2);
    assert_eq!(app.xdf_editor.form.definition.element_width_bits, 16);
    assert_eq!(app.xdf_editor.form.definition.row_stride_bits, 32);
    assert_eq!(app.xdf_editor.form.definition.column_stride_bits, 16);
    assert_eq!(app.xdf_editor.form.definition.axes[0].id, "x");
    assert_eq!(app.xdf_editor.form.definition.axes[0].address, Some(0x10));
    assert_eq!(app.xdf_editor.form.definition.axes[0].stride_bits, 8);
}

#[test]
fn hex_selection_prefills_a_byte_table_without_guessing_shape() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![0; 256]));
    app.hex_editor.selection = Some(HexSelection::new(0x30, 0x37));

    app.open_xdf_editor_from_hex_selection().unwrap();
    assert_eq!(app.xdf_editor.form.address_text, "0x30");
    assert_eq!(app.xdf_editor.form.definition.dimensions.rows, 1);
    assert_eq!(app.xdf_editor.form.definition.dimensions.columns, 8);
    assert_eq!(app.xdf_editor.form.definition.element_width_bits, 8);
}

#[test]
fn xdf_editor_window_renders_a_new_draft_without_a_loaded_xdf() {
    let mut app = TunerApp::headless();
    app.open_xdf_editor().unwrap();
    let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(canvas);
    egui::Context::default()
        .run_ui(input, |context| app.show_xdf_editor_window(context, canvas))
        .drop_without_applying_deltas();
    assert!(app.xdf_editor.open);
    assert!(app.xdf_editor.draft.as_ref().unwrap().parameters.is_empty());
}

#[test]
fn xdf_editor_blocks_applying_ranges_outside_the_loaded_bin() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![0; 16]));
    app.open_xdf_editor().unwrap();
    app.xdf_editor.begin_new_definition();
    app.xdf_editor.form.address_text = "0x20".into();
    assert!(app.preview_and_apply_xdf_editor_form().is_err());
    assert!(app.xdf_editor.draft.as_ref().unwrap().parameters.is_empty());
    assert!(app.xdf_editor.undo.is_empty());
}

#[test]
fn resolving_dirty_new_xdf_prompt_by_discard_opens_the_requested_empty_draft() {
    let mut app = TunerApp::headless();
    app.open_xdf_editor().unwrap();
    app.xdf_editor.begin_new_definition();
    app.preview_and_apply_xdf_editor_form().unwrap();
    assert!(app.start_new_xdf_draft().is_err());
    assert!(app.xdf_editor.pending_new_xdf);

    app.discard_xdf_editor();
    assert!(app.xdf_editor.open);
    assert!(!app.xdf_editor.pending_new_xdf);
    assert!(app.xdf_editor.draft.as_ref().unwrap().parameters.is_empty());
}

#[test]
fn resolving_dirty_new_xdf_prompt_by_save_opens_new_draft_after_save() {
    let mut app = TunerApp::headless();
    app.open_xdf_editor().unwrap();
    app.xdf_editor.begin_new_definition();
    app.preview_and_apply_xdf_editor_form().unwrap();
    assert!(app.start_new_xdf_draft().is_err());
    let path = temporary_test_path("saved-before-new-xdf.xdf");
    let snapshot = app.xdf_editor.draft.clone().unwrap();
    app.start_xdf_editor_save(snapshot, path.clone(), true, false)
        .unwrap();

    let context = egui::Context::default();
    for _ in 0..100 {
        app.poll_background_operations(&context);
        if !app.operations.is_busy() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!app.operations.is_busy());
    assert!(path.is_file());
    assert!(app.xdf_editor.open);
    assert!(app.xdf_editor.draft.as_ref().unwrap().parameters.is_empty());
    assert!(!app.xdf_editor.pending_new_xdf);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn ui_ipc_reports_and_prefills_the_xdf_authoring_draft() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![0; 64]));
    let capabilities =
        app.handle_ui_ipc_request(&agent_ipc_request(json!({"action":"capabilities"})));
    assert!(capabilities.data.unwrap()["actions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|action| action == "create_xdf_from_hex_selection"));
    let opened = app.handle_ui_ipc_request(&agent_ipc_request(json!({"action":"open_xdf_editor"})));
    assert!(opened.ok, "{}", opened.message);
    app.hex_editor.selection = Some(HexSelection::new(0x10, 0x17));
    let created = app.handle_ui_ipc_request(&agent_ipc_request(
        json!({"action":"create_xdf_from_hex_selection"}),
    ));
    assert!(created.ok, "{}", created.message);
    assert_eq!(
        created.data.unwrap()["pending_definition"]["dimensions"],
        json!([1, 8])
    );
}

#[test]
fn toolbar_reflows_and_canvas_guidance_tracks_document_state() {
    let mut app = TunerApp::headless();
    let context = egui::Context::default();
    let mut time = 0.0;
    for width in [360.0, 520.0, 900.0] {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 220.0));
        let output = render_toolbar_frame(&mut app, &context, screen, time, Vec::new());
        let mut failures = Vec::new();
        for label in [
            "File",
            "Edit",
            "View",
            "Tools",
            "Open BIN  [Ctrl+O]",
            "Open XDF  [Ctrl+Shift+O]",
            "Save Project",
            "Save As  [Ctrl+S]",
            "Undo  [Ctrl+Z]",
            "Redo  [Ctrl+Y]",
            "BIN: none",
            "XDF: none",
        ] {
            match rendered_text_rect(&output, label) {
                Some(rect) if rect.left() >= screen.left() && rect.right() <= screen.right() => {}
                Some(rect) => failures.push(format!("{label:?} outside width {width}: {rect:?}")),
                None => failures.push(format!("toolbar omitted {label:?} at width {width}")),
            }
        }
        output.drop_without_applying_deltas();
        assert!(failures.is_empty(), "{}", failures.join("; "));
        time += 0.2;
    }

    let narrow_screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(360.0, 800.0));
    click_toolbar_label(&mut app, "File", &context, narrow_screen, &mut time);
    let file_menu = render_toolbar_frame(&mut app, &context, narrow_screen, time, Vec::new());
    let has_save_project_as = rendered_text_rect(&file_menu, "Save Project As…").is_some();
    file_menu.drop_without_applying_deltas();
    assert!(has_save_project_as);

    fn render_canvas(app: &mut TunerApp) -> egui::FullOutput {
        let context = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0));
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        context.run_ui(input, |ctx| {
            let frame_context = ctx.clone();
            egui::CentralPanel::default().show(ctx, |ui| {
                app.show_workspace_canvas(ui, &frame_context);
            });
        })
    }

    let mut empty = TunerApp::headless();
    let output = render_canvas(&mut empty);
    let empty_actions = (
        rendered_text_rect(&output, "Open BIN  [Ctrl+O]").is_some(),
        rendered_text_rect(&output, "Open XDF  [Ctrl+Shift+O]").is_some(),
    );
    output.drop_without_applying_deltas();
    assert!(empty_actions.0 && empty_actions.1);

    let mut bin_only = TunerApp::headless();
    bin_only.workspace.bin = Some(BinDocument::from_bytes(vec![1, 2, 3]));
    bin_only.workspace.bin_path = Some(PathBuf::from("project.bin"));
    let output = render_canvas(&mut bin_only);
    let bin_only_guidance = (
        rendered_text_rect(&output, "Open XDF  [Ctrl+Shift+O]").is_some(),
        rendered_text_rect(&output, "Open BIN  [Ctrl+O]").is_none(),
        rendered_text_rect(&output, "Open an XDF to browse parameters").is_some(),
    );
    output.drop_without_applying_deltas();
    assert!(bin_only_guidance.0 && bin_only_guidance.1 && bin_only_guidance.2);

    let mut xdf_only = TunerApp::headless();
    xdf_only.workspace.xdf = Some(XdfDocument::parse(COLUMN_MAJOR_XDF).unwrap());
    xdf_only.workspace.xdf_path = Some(PathBuf::from("project.xdf"));
    let output = render_canvas(&mut xdf_only);
    let xdf_only_guidance = (
        rendered_text_rect(&output, "Open BIN  [Ctrl+O]").is_some(),
        rendered_text_rect(&output, "Open XDF  [Ctrl+Shift+O]").is_none(),
        rendered_text_rect(&output, "Open a BIN to validate mappings and edit values").is_some(),
    );
    output.drop_without_applying_deltas();
    assert!(xdf_only_guidance.0 && xdf_only_guidance.1 && xdf_only_guidance.2);

    let mut paired = TunerApp::headless();
    paired.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![1, 2, 3])),
        Some(XdfDocument::parse(COLUMN_MAJOR_XDF).unwrap()),
        Some(PathBuf::from("project.bin")),
        Some(PathBuf::from("project.xdf")),
    );
    let output = render_canvas(&mut paired);
    let paired_guidance = (
        rendered_text_rect(&output, "Choose a parameter from the categorized browser.").is_some(),
        rendered_text_rect(&output, "Open BIN  [Ctrl+O]").is_none(),
        rendered_text_rect(&output, "Open XDF  [Ctrl+Shift+O]").is_none(),
    );
    output.drop_without_applying_deltas();
    assert!(paired_guidance.0 && paired_guidance.1 && paired_guidance.2);
}

#[test]
fn search_guidance_labels_distinguish_filter_wildcard_and_bin_values() {
    let mut app = TunerApp::headless();
    app.workspace.xdf = Some(XdfDocument::parse(COLUMN_MAJOR_XDF).unwrap());
    app.workspace.xdf_path = Some(PathBuf::from("project.xdf"));
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_200.0, 900.0));
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(screen);
    let browser = context.run_ui(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.show_browser_contents(ui));
    });
    let has_list_filter = rendered_text_rect(&browser, "List filter").is_some();
    browser.drop_without_applying_deltas();
    assert!(has_list_filter);

    let search_context = egui::Context::default();
    fn draw_search_frame(
        app: &mut TunerApp,
        context: &egui::Context,
        screen: egui::Rect,
        time: f64,
    ) -> egui::FullOutput {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.time = Some(time);
        context.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |_| {});
            app.show_search_window(ctx, screen);
        })
    }

    app.search_state.open = true;
    app.project_preferences
        .dock_state
        .open_tool(WindowId::Search);
    app.project_preferences
        .dock_state
        .restore(&WindowId::Search);
    app.search_state.query = "10".into();
    app.search_state.match_mode = SearchMatchMode::Wildcard;
    app.search_state.field_scope = SearchFieldScope::RawValues;
    for time in [1.0, 1.2, 1.4] {
        draw_search_frame(&mut app, &search_context, screen, time).drop_without_applying_deltas();
    }
    let no_bin = draw_search_frame(&mut app, &search_context, screen, 1.6);
    let no_bin_guidance = [
        "Advanced Search",
        "Wildcard: * = any text, ? = one character",
        "BIN required for value search; metadata matches remain available.",
    ]
    .map(|text| rendered_text_rect(&no_bin, text).is_some());
    no_bin.drop_without_applying_deltas();
    assert_eq!(no_bin_guidance, [true, true, true]);

    app.workspace.bin = Some(BinDocument::from_bytes(vec![10, 20, 30]));
    let with_bin = draw_search_frame(&mut app, &search_context, screen, 1.8);
    let value_notice_with_bin = rendered_text_rect(
        &with_bin,
        "BIN required for value search; metadata matches remain available.",
    )
    .is_some();
    with_bin.drop_without_applying_deltas();
    assert!(!value_notice_with_bin);

    app.workspace.bin = None;
    app.search_state.field_scope = SearchFieldScope::Metadata;
    let metadata_only = draw_search_frame(&mut app, &search_context, screen, 2.0);
    let metadata_guidance = (
        rendered_text_rect(
            &metadata_only,
            "BIN required for value search; metadata matches remain available.",
        )
        .is_none(),
        rendered_text_rect(&metadata_only, "Advanced Search").is_some(),
    );
    metadata_only.drop_without_applying_deltas();
    assert!(metadata_guidance.0 && metadata_guidance.1);
}

#[test]
fn unsaved_bin_and_diagnostics_status_is_actionable() {
    let mut app = TunerApp::headless();
    app.workspace.bin = Some(BinDocument::from_bytes(vec![1, 2, 3, 4]));
    let context = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 220.0));
    let clean = render_toolbar_frame(&mut app, &context, screen, 0.0, Vec::new());
    let clean_status = rendered_text_rect(&clean, "BIN: 4 bytes").is_some();
    let clean_legacy_wording = rendered_text_rect(&clean, "BIN: 4 bytes (clean)").is_some()
        || rendered_text_rect(&clean, "BIN: 4 bytes (dirty)").is_some();
    clean.drop_without_applying_deltas();
    assert!(clean_status);
    assert!(!clean_legacy_wording);

    let mut edit = app
        .workspace
        .bin
        .as_mut()
        .unwrap()
        .transaction("status regression");
    edit.write_u8(0, 9).unwrap();
    edit.commit().unwrap();
    let dirty = render_toolbar_frame(&mut app, &context, screen, 0.2, Vec::new());
    let dirty_message = rendered_text_rect(&dirty, "Unsaved BIN edits").is_some();
    let save_guidance = rendered_text_rect(&dirty, "Use BIN Save As to export").is_some();
    dirty.drop_without_applying_deltas();
    assert!(dirty_message);
    assert!(save_guidance);

    fn render_diagnostics(app: &mut TunerApp) -> egui::FullOutput {
        let context = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 300.0));
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        context.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.show_diagnostics(ui));
        })
    }

    let mut healthy = TunerApp::headless();
    healthy.workspace.set_documents(
        Some(BinDocument::from_bytes(vec![10, 20, 30, 40, 50, 60])),
        Some(XdfDocument::parse(COLUMN_MAJOR_XDF).unwrap()),
        Some(PathBuf::from("clean.bin")),
        Some(PathBuf::from("clean.xdf")),
    );
    healthy.workspace.validation = Some(
        healthy
            .workspace
            .xdf
            .as_ref()
            .unwrap()
            .validate_against(healthy.workspace.bin.as_ref().unwrap().bytes()),
    );
    let healthy_output = render_diagnostics(&mut healthy);
    let healthy_summary =
        rendered_text_rect(&healthy_output, "BIN/XDF mappings healthy · 0 issues").is_some();
    let healthy_has_scroll_prompt = rendered_text_rect(
        &healthy_output,
        "Pair a BIN and XDF to validate mapped ranges.",
    )
    .is_some();
    healthy_output.drop_without_applying_deltas();
    assert!(healthy_summary);
    assert!(!healthy_has_scroll_prompt);

    let out_of_range_xdf = XdfDocument::parse(
        br#"<XDFFORMAT><XDFCONSTANT uniqueid="outside"><title>Out of range table</title>
          <EMBEDDEDDATA mmedaddress="0x10" mmedelementsizebits="8" />
        </XDFCONSTANT></XDFFORMAT>"#,
    )
    .unwrap();
    let mut warning = TunerApp::headless();
    warning.workspace.bin = Some(BinDocument::from_bytes(vec![1]));
    let mut out_of_range_xdf = out_of_range_xdf;
    out_of_range_xdf.diagnostics.push(tuner_xdf::XdfDiagnostic {
        severity: tuner_xdf::DiagnosticSeverity::Warning,
        code: "legacy-formula".into(),
        path: "XDFCONSTANT".into(),
        message: "Legacy formula needs review.".into(),
    });
    warning.workspace.xdf = Some(out_of_range_xdf);
    warning.workspace.validation = Some(
        warning
            .workspace
            .xdf
            .as_ref()
            .unwrap()
            .validate_against(warning.workspace.bin.as_ref().unwrap().bytes()),
    );
    let error_output = render_diagnostics(&mut warning);
    let warning_count =
        rendered_text_rect(&error_output, "⚠ 2 workspace issues · review below").is_some();
    let issue_summary = rendered_text_rect(&error_output, "Out of range table").is_some();
    error_output.drop_without_applying_deltas();
    assert!(warning_count);
    assert!(issue_summary);
}
