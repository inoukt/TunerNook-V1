# TunerNook Desktop App Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a runnable, customizable Windows-first `tuner-app` desktop shell over the verified TunerNook backend.

**Architecture:** Keep all BIN/XDF loading, validation, cell access, editing, undo/redo, and Save As operations behind a testable `WorkspaceState` in the new app crate. Build the egui view as a thin intent dispatcher over that state. Centralize workflow actions in a stable command registry and keep layout/display preferences separate from calibration data so future modules can contribute commands and panels without changing the safety boundary.

**Tech Stack:** Stable Rust 1.98.1, `eframe` 0.36.2 with the native `glow` backend, `egui`, `rfd` 0.17.2, `serde`, `serde_json`, and the existing `tuner-core`/`tuner-xdf` crates.

**Spec:** `docs/superpowers/specs/2026-09-13-desktop-app-shell-design.md`

## Global Constraints

- Use `eframe = { version = "0.36.2", default-features = false, features = ["default_fonts", "glow"] }` and `rfd = { version = "0.17.2", default-features = false }`.
- Keep `tuner-core` and `tuner-xdf` dependency-free and unchanged except where a compile-time integration adjustment is strictly required.
- UI code never mutates a BIN byte slice directly; every edit uses `ParameterDefinition::write_engineering_cell` inside one committed core transaction.
- BIN/XDF loading is independent and never writes the input files; Save As must use the existing no-overwrite `BinDocument::save_as` path.
- Settings contain UI/workflow state only, use versioned JSON, ignore unknown fields, and fall back to defaults when malformed.
- The first slice registers safe in-process extension descriptors only; it does not load arbitrary dynamic libraries or execute untrusted code.
- The supplied `Test bin and xdf` files are read-only fixtures and their SHA-256 values must remain unchanged.
- All headless behavior has unit tests; GUI startup is checked with a Windows smoke run after compilation.
- Add-on: the shell includes a first-class debug report window with a stable
  command ID, clipboard copy, and safe no-overwrite text export. It reports
  document identity, parser and mapping diagnostics, selection/storage state,
  edit history, and UI customization state without automatic upload.

---

### Task 1: Scaffold the application crate and customization model

**Files:**

- Modify: `Cargo.toml`
- Create: `crates/tuner-app/Cargo.toml`
- Create: `crates/tuner-app/src/lib.rs`
- Create: `crates/tuner-app/src/main.rs`
- Test: `crates/tuner-app/src/lib.rs`

**Interfaces:**

- Consumes: workspace crates `tuner-core` and `tuner-xdf`.
- Produces: `AppPreferences`, `PanelId`, `LayoutPreset`, `ThemeMode`,
  `UiDensity`, `TableDisplay`, `CommandDescriptor`, `CommandRegistry`,
  `AppExtension`, and a native `TunerApp` entry point.

- [x] **Step 1: Add the crate and dependency declarations.**

  Add `crates/tuner-app` to the workspace members and create this manifest:

  ```toml
  [package]
  name = "tuner-app"
  version = "0.1.0"
  edition = "2021"

  [dependencies]
  eframe = { version = "0.36.2", default-features = false, features = ["default_fonts", "glow"] }
  rfd = { version = "0.17.2", default-features = false }
  serde = { version = "1.0", features = ["derive"] }
  serde_json = "1.0"
  tuner-core = { path = "../tuner-core" }
  tuner-xdf = { path = "../tuner-xdf" }
  ```

  Keep `tuner-cli` as the workspace default member; `cargo build --workspace`
  must include the new desktop binary.

- [x] **Step 2: Write failing customization tests.**

  Add tests that establish stable IDs, default layout state, settings round
  trips, malformed-settings fallback, and disabled-command explanations:

  ```rust
  #[test]
  fn defaults_expose_stable_commands_and_standard_layout() {
      let preferences = AppPreferences::default();
      let registry = CommandRegistry::core();
      assert_eq!(preferences.layout, LayoutPreset::Standard);
      assert!(preferences.show_browser);
      assert!(registry.descriptors().iter().any(|command| {
          command.id == "file.open-bin" && command.default_shortcut.as_deref() == Some("Ctrl+O")
      }));
      assert!(registry.descriptors().iter().any(|command| command.id == "edit.undo"));
  }

  #[test]
  fn preferences_round_trip_and_malformed_input_restore_defaults() {
      let mut preferences = AppPreferences::default();
      preferences.theme = ThemeMode::Dark;
      preferences.layout = LayoutPreset::Diagnostics;
      preferences.shortcuts.insert("edit.undo".into(), "Alt+U".into());
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
  ```

- [x] **Step 3: Run the new tests and confirm the scaffold is missing the requested types.**

  Run `cargo test -p tuner-app`.

  Expected result: compilation fails because the new types and crate source do
  not exist yet. Do not weaken the assertions to make this step pass.

- [x] **Step 4: Implement the preference and command contracts.**

  In `crates/tuner-app/src/lib.rs`, define versioned serde preferences with
  these exact fields and defaults:

  ```rust
  #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
  pub enum LayoutPreset { Standard, DataEntry, Diagnostics }

  #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
  pub enum ThemeMode { System, Light, Dark }

  #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
  pub enum UiDensity { Compact, Comfortable }

  #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
  pub enum TableDisplay { Engineering, Raw }

  #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
  pub struct AppPreferences {
      pub version: u32,
      pub layout: LayoutPreset,
      pub theme: ThemeMode,
      pub density: UiDensity,
      pub table_display: TableDisplay,
      pub show_browser: bool,
      pub show_editor: bool,
      pub show_inspector: bool,
      pub show_diagnostics: bool,
      pub shortcuts: BTreeMap<String, String>,
  }
  ```

  `Default` uses version `1`, `Standard`, `System`, `Comfortable`,
  `Engineering`, all four panels visible, and the built-in shortcut map.
  `preferences_from_json` must call `serde_json::from_str` and return the
  default on any error; serde's default unknown-field behavior is sufficient
  for forward-compatible settings. `preferences_to_json` uses pretty JSON.

  Define stable `BuiltinCommand` variants for open BIN, open XDF, Save As,
  undo, redo, apply-cell, command palette, reset preferences, each panel
  toggle, and the three layout presets. Define `CommandDescriptor` with owned
  `id`, `label`, `category`, `description`, optional `default_shortcut`, and a
  `BuiltinCommand` target. `CommandRegistry::core()` returns the descriptors
  in deterministic menu order. `CommandRegistry::entries` computes
  `enabled` and a human-readable `reason` from `WorkspaceState`; disabled
  commands remain in the returned list.

  Add the safe extension boundary:

  ```rust
  pub trait AppExtension {
      fn id(&self) -> &str;
      fn commands(&self) -> Vec<CommandDescriptor>;
      fn execute(&mut self, action_id: &str, state: &mut WorkspaceState)
          -> Result<(), String>;
  }
  ```

  `CommandRegistry` may store registered in-process extensions, but the first
  slice registers none and only dispatches built-ins. Extension descriptors
  must use a namespaced ID and cannot bypass `WorkspaceState`.

- [x] **Step 5: Add the native entry point and minimal app shell.**

  `src/main.rs` must only create native options and call the library startup:

  ```rust
  fn main() -> eframe::Result<()> {
      let options = eframe::NativeOptions {
          viewport: eframe::egui::ViewportBuilder::default()
              .with_inner_size([1400.0, 900.0])
              .with_min_inner_size([960.0, 640.0]),
          ..Default::default()
      };
      eframe::run_native(
          "TunerNook",
          options,
          Box::new(|creation| Ok(Box::new(TunerApp::new(creation)))),
      )
  }
  ```

  Define `TunerApp` with default `WorkspaceState`, default preferences,
  `CommandRegistry::core()`, a command-palette query, settings visibility,
  and the resolved settings path. The initial `update` can show the shell
  panels but must not load an implicit file.

- [x] **Step 6: Run customization tests and compile the scaffold.**

  Run `cargo test -p tuner-app` and `cargo check -p tuner-app`.

  Expected result: all customization tests pass and the native shell compiles.

### Task 2: Implement the safe workspace state and cell workflow

**Files:**

- Modify: `crates/tuner-app/src/lib.rs`
- Test: `crates/tuner-app/src/lib.rs`

**Interfaces:**

- Consumes: `BinDocument`, `ParameterDefinition`, `RawValue`,
  `EngineeringWriteResult`, `XdfDocument`, and `ValidationReport`.
- Produces: `WorkspaceState`, `WorkspaceError`, `ParameterSummary`,
  `CellView`, load/selection/edit/undo/redo/save methods, and settings path
  helpers used by the UI and tests.

- [x] **Step 1: Write failing workspace tests against a synthetic XDF.**

  Add a helper that parses this deterministic 2x3 integer column-major table
  and a four-byte BIN:

  ```rust
  fn column_major_fixture() -> (XdfDocument, BinDocument) {
      let xdf = XdfDocument::parse(
          br#"<XDFFORMAT><XDFTABLE uniqueid="map">
              <title>Map</title><XDFAXIS id="z">
                <EMBEDDEDDATA mmedaddress="0x00" mmedelementsizebits="8"
                  mmedrowcount="2" mmedcolcount="3"
                  mmedmajorstridebits="0" mmedminorstridebits="0"
                  mmedtypeflags="0x06" />
              </XDFAXIS>
          </XDFTABLE></XDFFORMAT>"#,
      ).unwrap();
      (xdf, BinDocument::from_bytes(vec![10, 20, 30, 40, 50, 60]))
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
      assert_eq!(workspace.selected_cell_view().unwrap().raw, RawValue::Unsigned(30));
  }

  #[test]
  fn workspace_edit_undo_redo_and_rejected_edit_are_atomic() {
      let (xdf, bin) = column_major_fixture();
      let mut workspace = WorkspaceState::default();
      workspace.set_documents(Some(bin), Some(xdf), None, None);
      let id = workspace.filtered_parameters("")[0].semantic_id.clone();
      workspace.select_parameter(&id);
      workspace.select_cell(1, 2).unwrap();
      workspace.apply_engineering_text("99").unwrap();
      assert_eq!(workspace.bin.as_ref().unwrap().bytes(), &[10, 20, 30, 40, 50, 99]);
      assert!(workspace.bin.as_ref().unwrap().is_dirty());
      workspace.undo().unwrap();
      assert_eq!(workspace.bin.as_ref().unwrap().bytes(), &[10, 20, 30, 40, 50, 60]);
      workspace.redo().unwrap();
      assert_eq!(workspace.bin.as_ref().unwrap().bytes(), &[10, 20, 30, 40, 50, 99]);
      let before = workspace.bin.as_ref().unwrap().bytes().to_vec();
      let undo_depth = workspace.bin.as_ref().unwrap().undo_depth();
      assert!(workspace.apply_engineering_text("not-a-number").is_err());
      assert_eq!(workspace.bin.as_ref().unwrap().bytes(), before.as_slice());
      assert_eq!(workspace.bin.as_ref().unwrap().undo_depth(), undo_depth);
  }
  ```

- [x] **Step 2: Run the workspace tests to verify they fail.**

  Run `cargo test -p tuner-app workspace_ -- --nocapture`.

  Expected result: compilation fails because the state methods are not defined.

- [x] **Step 3: Implement document ownership, validation, filtering, and selection.**

  Define:

  ```rust
  pub struct WorkspaceState {
      pub bin: Option<BinDocument>,
      pub xdf: Option<XdfDocument>,
      pub bin_path: Option<PathBuf>,
      pub xdf_path: Option<PathBuf>,
      pub validation: Option<ValidationReport>,
      pub selected_semantic_id: Option<String>,
      pub selected_cell: (usize, usize),
      pub filter: String,
      pub edit_text: String,
      pub status: StatusMessage,
  }
  ```

  `set_documents` is a testable replacement for file dialogs and recomputes
  validation whenever both documents are present. `open_bin` calls
  `BinDocument::load`; `open_xdf` calls `XdfDocument::load`. Both methods
  replace only their own document, preserve the other document, select the
  first sorted parameter if the old selection is unavailable, and convert all
  errors into `WorkspaceError` plus a visible status message.

  `ParameterSummary` contains owned semantic ID, title, category, and
  `ParameterKind`. `filtered_parameters` matches title, unique ID, semantic ID,
  and category case-insensitively, then sorts by category, title, and semantic
  ID. `select_parameter` validates the ID and resets the selected cell to
  `(0, 0)`. `select_cell` checks the selected parameter's dimensions through
  `cell_range` before changing selection.

- [x] **Step 4: Implement checked cell views and transactional edits.**

  `selected_cell_view` returns an owned `CellView` containing row, column,
  range, optional raw value, optional engineering value, and an optional error
  string. It calls `read_engineering_cell` first and `read_raw_cell` as the
  fallback. Raw-only cells remain visible.

  `apply_engineering_text` parses a finite `f64`, obtains the selected
  parameter, opens `bin.transaction(format!("edit {}", parameter.title))`,
  calls `write_engineering_cell`, and commits only on success. Any parse,
  mapping, conversion, or core error returns before a commit; dropped
  transactions roll back staged bytes. On success it updates `edit_text`,
  recomputes validation, and records a success status. The method must never
  call `write_bytes` or otherwise mutate `BinDocument` directly.

  `undo` and `redo` call the corresponding core methods, refresh validation,
  and report when no history is available. `save_as` calls
  `BinDocument::save_as` without changing the active in-memory source path;
  the core no-overwrite error is surfaced unchanged in the status.

- [x] **Step 5: Implement settings persistence helpers and state tests.**

  Add:

  ```rust
  pub fn settings_path() -> PathBuf;
  pub fn load_preferences(path: &Path) -> AppPreferences;
  pub fn save_preferences(path: &Path, preferences: &AppPreferences) -> io::Result<()>;
  ```

  `settings_path` uses `%APPDATA%\TunerNook\settings.json` on Windows,
  `%XDG_CONFIG_HOME%/TunerNook/settings.json` when available elsewhere, and
  `TunerNook/settings.json` as a deterministic relative fallback. Saving
  creates the parent directory and writes a temporary sibling file before
  renaming it to avoid leaving a partial settings file. Tests use explicit
  temporary paths and never the user settings directory.

- [x] **Step 6: Run the completed state tests.**

  Run:

  ```powershell
  cargo test -p tuner-app workspace_ -- --nocapture
  cargo test -p tuner-app preferences_ -- --nocapture
  ```

  Expected result: all workspace, preference, and atomicity tests pass.

### Task 3: Build the customizable egui workspace UI

**Files:**

- Modify: `crates/tuner-app/src/lib.rs`
- Modify: `crates/tuner-app/src/main.rs` only if startup API needs a compile correction
- Test: `crates/tuner-app/src/lib.rs`

**Interfaces:**

- Consumes: `WorkspaceState`, `CommandRegistry`, `AppPreferences`, and `rfd`.
- Produces: toolbar/menu, browser, editor, inspector, diagnostics panel,
  detailed debug report, command palette, settings view, keyboard shortcuts,
  and layout/theme control.

- [x] **Step 1: Add headless command and layout behavior tests.**

  Test that layout presets only change preference fields and dispatching an
  undo command reaches `WorkspaceState` without changing layout state:

  ```rust
  #[test]
  fn layout_presets_and_command_dispatch_are_separate_from_documents() {
      let mut app = TunerApp::headless();
      let before = app.workspace.bin.is_none();
      app.apply_layout_preset(LayoutPreset::Diagnostics);
      assert_eq!(app.preferences.layout, LayoutPreset::Diagnostics);
      assert!(app.preferences.show_diagnostics);
      assert_eq!(app.workspace.bin.is_none(), before);
      app.dispatch_command("view.toggle-browser");
      assert!(!app.preferences.show_browser);
      assert_eq!(app.workspace.bin.is_none(), before);
  }
  ```

- [x] **Step 2: Implement toolbar, menu, and native file actions.**

  In `TunerApp::update`, apply theme and density, draw an egui top panel, and
  render buttons from the same command registry used by the menu. Open BIN and
  Open XDF call `rfd::FileDialog::new().add_filter(...).pick_file()` and pass
  the selected path to `WorkspaceState::open_bin` or `open_xdf`. Save As calls
  `save_file()` with a default name based on the active BIN stem and delegates
  to `WorkspaceState::save_as`.

  Undo, redo, and apply-cell buttons dispatch through stable command IDs. The
  toolbar shows active BIN/XDF names, dirty state, validation issue count, and
  the latest status severity/text. No button handler touches a byte slice.

- [x] **Step 3: Implement the browser, editor, inspector, and diagnostics views.**

  Render the browser in a resizable left `SidePanel` with a filter `TextEdit`,
  category labels, and selectable parameter rows from
  `WorkspaceState::filtered_parameters`. Render the central editor with a
  scalar one-cell grid or a scrollable table using the parameter's logical
  dimensions; each cell calls `selected_cell_view` and displays engineering or
  raw values based on `TableDisplay`. Clicking a cell calls `select_cell`.

  Render the inspector in a resizable right `SidePanel` using the selected
  `ParameterDefinition` fields: semantic ID, kind, title, category, address,
  byte range, dimensions, effective strides, signedness, endianness, numeric
  kind, raw flags, column-major state, conversion, and axis metadata. Render
  all `ValidationReport` issues and the recent status in a bottom diagnostics
  panel. When prerequisites are absent, explain exactly which document is
  needed.

- [x] **Step 4: Implement customization controls and the command palette.**

  Add a View menu with panel toggles, the three layout presets, theme choices,
  density choices, engineering/raw display, a keyboard settings window, and
  reset-to-defaults. Add `Open Debug Report` to the View/Tools workflow with a
  default `F12` shortcut; its report can be copied or exported to a new text
  file. `apply_layout_preset` sets:

  ```text
  Standard:    browser=true, editor=true, inspector=true, diagnostics=true
  DataEntry:   browser=true, editor=true, inspector=false, diagnostics=false
  Diagnostics: browser=true, editor=false, inspector=true, diagnostics=true
  ```

  `Ctrl+K` (or the user's configured command-palette shortcut) opens a modal
  palette. Its query filters command ID, label, category, and description;
  disabled entries stay visible with their reason. Selecting an enabled entry
  dispatches it and closes the palette.

  The keyboard settings view lists every descriptor, its current shortcut, and
  a text field for replacement. Empty strings disable a shortcut. Persist
  preferences after successful changes and restore defaults through one action.

- [x] **Step 5: Implement shortcut matching and settings persistence in the app.**

  Support the key grammar `Ctrl`, `Alt`, `Shift`, plus one letter or `F1`-`F12`
  for the first slice. Parse the stored string into modifiers and an egui key;
  invalid custom strings remain displayed but are ignored at runtime. Poll
  `egui::Context::input` once per frame, dispatch at most one matching enabled
  command, and do not intercept text entry when a text field has focus.

  Load preferences from `settings_path()` in `TunerApp::new`. Save them after
  a layout, panel, theme, density, display, shortcut, or reset change. A save
  failure remains a visible warning but does not undo the in-memory choice.

- [x] **Step 6: Run headless UI-state tests and compile the native UI.**

  Run:

  ```powershell
  cargo test -p tuner-app
  cargo check -p tuner-app
  ```

  Expected result: all app tests pass and the native UI compiles with the
  configured eframe/rfd dependencies.

### Task 4: Verify the application against the supplied fixture and finish docs

**Files:**

- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-09-13-desktop-app-shell-design.md`
- Modify: `docs/superpowers/plans/2026-09-13-desktop-app-shell.md`
- Test: `crates/tuner-app/src/lib.rs`

**Interfaces:**

- Consumes: the complete app shell and the supplied BIN/XDF pair.
- Produces: fixture smoke coverage, documented launch instructions, verified
  hashes, and completed implementation artifacts.

- [x] **Step 1: Add a fixture-loading regression test.**

  Load the paths relative to `CARGO_MANIFEST_DIR` and assert that the app can
  pair them, sees 2,915 parameters, 57 categories, 8,745 axes, and zero
  `unsupported-storage` diagnostics. Do not edit or save the fixture from the
  test. Keep the existing XDF crate fixture test as the source of exact axis
  counts and use this test to prove the app state can load the same pair.

- [x] **Step 2: Document desktop launch and customization.**

  Add to README:

  ```text
  cargo run -p tuner-app
  ```

  Explain that the first window opens BIN/XDF through the toolbar, stores UI
  preferences under the platform app-data directory, exposes Ctrl+K command
  search and F12 detailed debug reporting, and never overwrites an input BIN.
  Mention that compare, transfer, hex, and visualization panels are subsequent
  capability slices registered on the same app spine.

- [x] **Step 3: Run the complete verification suite.**

  Run:

  ```powershell
  cargo fmt --all -- --check
  cargo build --workspace
  cargo test --workspace
  cargo run --quiet -p tuner-cli -- xdf-validate "Test bin and xdf/SCGa05_cal.xdf" "Test bin and xdf/SCGa05_cal.bin"
  Get-FileHash -Algorithm SHA256 "Test bin and xdf\SCGa05_cal.xdf"
  Get-FileHash -Algorithm SHA256 "Test bin and xdf\SCGa05_cal.bin"
  ```

  Confirm validation remains `valid:true`, `issue_count:0`,
  `parameter_count:2915`, and `bin_size_bytes:654336`. Confirm the local fixture
  XDF/BIN hashes match the recorded baseline (values omitted from this public
  plan).

- [ ] **Step 4: Perform a Windows GUI smoke test.**

  Run `cargo run -p tuner-app`, open the supplied BIN and XDF, filter for a
  parameter, select a table cell, verify the inspector shows column-major
  storage, edit a supported value, observe dirty state, undo and redo it, and
  invoke Save As to a new explicit output path. Confirm the fixture files keep
  their original hashes and remove only the newly created smoke-test output.

  Environment note: the current session has no native Windows app surface
  available to the computer-use tool, so this visual check remains pending for
  a local/native-enabled run.

- [ ] **Step 5: Self-review and mark the written artifacts complete.**

  Check all plan boxes, verify every public field used by the UI exists in the
  app model, confirm no UI handler writes bytes directly, and scan the plan for
  unfinished-work wording. Change the spec status to `Implemented` only after all
  verification commands and the GUI smoke test succeed.

## Plan self-review checklist

- Scope: the first slice is a usable desktop shell plus customization spine;
  compare, transfer, hex, visualization, checksum, and recovery remain named
  follow-on capabilities rather than hidden unfinished requirements.
- Boundary: `WorkspaceState` owns every document operation; egui only renders
  views and dispatches command IDs.
- Evolution: preferences use stable IDs and versioned JSON; command and
  extension descriptors are independent of panel placement.
- Safety: rejected edits happen before transaction commit, Save As uses the
  no-overwrite core method, and fixture files are never written.
