# Formula Editor and XDF Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add project-scoped conversion editing with preview, safe XDF Save As export, and authenticated AI-agent control without changing the source XDF or BIN undo history.

**Architecture:** Keep the loaded source XDF as an immutable reference and expose a separate effective XDF model whose parameter/axis conversion strings may be overridden. Persist overrides in the existing BIN-identity project file using the source XDF normalized fingerprint, serialize the effective model through the parser's retained XML tree, and queue file work through the existing `OperationCoordinator`. The Inspector, command registry, and `tuner-ui/v1` all call the same conversion-target helpers.

**Tech Stack:** Rust 2021, egui/eframe 0.36, serde/serde_json, existing `tuner-cal` conversion parser, existing `tuner-xdf` XML model, existing `tuner-core` file safety primitives, and standard-library filesystem/threading only.

**Spec:** `docs/superpowers/specs/2026-09-14-formula-editor-xdf-export-design.md`

## Global Constraints

- Formula syntax remains the existing `tuner-cal` language: `X`, finite numbers, `+`, `-`, `*`, `/`, parentheses, and unary signs.
- Project overrides are keyed by source XDF normalized fingerprint plus semantic parameter/axis identity; never match by title or list position.
- Applying a formula changes only the effective mapping and project preferences; it never writes BIN bytes or adds a BIN undo entry.
- Save XDF As never overwrites an existing target and never modifies the source XDF.
- The UI thread remains the only place that mutates workspace, effective XDF, project preferences, and operation state.
- Use existing egui, operation, persistence, command, and IPC patterns; add no third-party dependency.
- Every production behavior change starts with a failing regression test and ends with a focused test pass.
- This workspace has no `.git` directory; use checked source/test checkpoints instead of commit steps.

---

### Task 1: Add conversion-target identity, project persistence, and effective XDF state

**Files:**
- Modify: `crates/tuner-app/src/lib.rs:40-850` for preference schema, target keys, workspace source/effective XDF state, and override application.
- Test: `crates/tuner-app/src/lib.rs` in the existing `#[cfg(test)]` module.

**Interfaces:**
- Produces `ConversionTarget { semantic_id: String, axis_index: Option<usize> }` with `key(xdf_fingerprint: &str) -> String`.
- Produces `WorkspaceState::source_xdf` as the unmodified XDF reference and keeps `WorkspaceState::xdf` as the effective model.
- Produces `TunerApp::apply_conversion_override`, `TunerApp::reset_conversion_override`, and `TunerApp::apply_project_conversion_overrides`.

- [x] **Step 1: Write failing persistence and identity tests**

Add tests with these exact behaviors:

```rust
#[test]
fn conversion_override_key_includes_xdf_identity_and_target_kind() {
    let parameter = ConversionTarget::parameter("constant:uid:1");
    let axis = ConversionTarget::axis("table:uid:2", 1);

    assert_eq!(
        parameter.key("xdf-a"),
        "xdf-a:parameter:constant:uid:1"
    );
    assert_eq!(axis.key("xdf-a"), "xdf-a:axis:table:uid:2:1");
    assert_ne!(parameter.key("xdf-a"), parameter.key("xdf-b"));
}

#[test]
fn project_preferences_round_trip_conversion_overrides_and_old_json_defaults_empty() {
    let mut preferences = ProjectPreferences::for_identity("bin");
    preferences
        .conversion_overrides
        .insert("xdf-a:parameter:constant:uid:1".to_string(), "X * 2".to_string());

    let encoded = serde_json::to_string(&preferences).unwrap();
    let decoded: ProjectPreferences = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.conversion_overrides, preferences.conversion_overrides);

    let old: ProjectPreferences = serde_json::from_str(
        r#"{"version":2,"bin_identity":"bin","layout":"Standard","show_browser":true,"show_editor":true,"show_inspector":true,"show_diagnostics":true,"browser_organization":"Categories","catalog_sort":"Title","catalog_sort_direction":"Ascending","browser_filter":"","category_state_initialized":false,"known_categories":[],"collapsed_categories":[],"favorite_tables":[],"recent_tables":[],"browser_collapsed":false,"inspector_collapsed":false,"browser_width":300,"inspector_width":320,"diagnostics_height":150,"table_windows":{},"tab_orders":{},"open_table_keys":[],"active_table_key":null,"search_state":{"open":false,"query":"","match_mode":"Contains","field_scope":"Metadata","sort_key":"Title","sort_direction":"Ascending","result_limit":100,"window":{"x":0,"y":0,"width":900,"height":600,"zoom_percent":100}}}"#,
    ).unwrap();
    assert!(old.conversion_overrides.is_empty());
}
```

- [x] **Step 2: Run the focused tests and verify the expected RED failure**

Run:

```powershell
cargo test -p tuner-app --lib conversion_override_key_includes_xdf_identity_and_target_kind project_preferences_round_trip_conversion_overrides_and_old_json_defaults_empty
```

Expected: compilation/test failure because `ConversionTarget` and `conversion_overrides` do not yet exist.

- [x] **Step 3: Implement the minimal persisted model**

Add `conversion_overrides: BTreeMap<String, String>` with `#[serde(default)]` to `ProjectPreferences`, initialize it empty, and sanitize keys/formulas by removing empty keys and limiting formula strings to 4096 Unicode scalar values. Bump `PROJECT_SETTINGS_VERSION` to `3`; the existing `version <= PROJECT_SETTINGS_VERSION` loader accepts older project files and serde defaults the missing map.

Add:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
struct ConversionTarget {
    semantic_id: String,
    axis_index: Option<usize>,
}

impl ConversionTarget {
    fn parameter(semantic_id: impl Into<String>) -> Self {
        Self { semantic_id: semantic_id.into(), axis_index: None }
    }

    fn axis(semantic_id: impl Into<String>, axis_index: usize) -> Self {
        Self { semantic_id: semantic_id.into(), axis_index: Some(axis_index) }
    }

    fn key(&self, xdf_fingerprint: &str) -> String {
        match self.axis_index {
            Some(index) => format!("{xdf_fingerprint}:axis:{}:{index}", self.semantic_id),
            None => format!("{xdf_fingerprint}:parameter:{}", self.semantic_id),
        }
    }
}
```

Store `source_xdf: Option<XdfDocument>` in `WorkspaceState`. `set_documents` clones the incoming XDF into `source_xdf` before assigning the effective `xdf`; document-installation code does the same. Add a helper that returns the source fingerprint and the current target's source/effective formula.

- [x] **Step 4: Add failing effective-override and reset tests**

Add a fixture-based test that loads a parameter with conversion `X`, applies `X * 2`, and asserts:

```rust
assert_eq!(app.workspace.source_xdf.as_ref().unwrap().parameter(&id).unwrap().conversion.as_deref(), Some("X"));
assert_eq!(app.workspace.xdf.as_ref().unwrap().parameter(&id).unwrap().conversion.as_deref(), Some("X * 2"));
assert!(app.workspace.bin.as_ref().unwrap().undo_depth() == 0);
app.reset_conversion_override(ConversionTarget::parameter(id.clone())).unwrap();
assert_eq!(app.workspace.xdf.as_ref().unwrap().parameter(&id).unwrap().conversion.as_deref(), Some("X"));
```

Also test that an override under a different XDF fingerprint is ignored and leaves the source formula in effect.

- [x] **Step 5: Run the new tests to verify RED, then implement effective application**

Run the new test by name and confirm it fails because the target methods are missing. Then implement:

```rust
fn apply_project_conversion_overrides(&mut self) -> usize;
fn apply_conversion_override(&mut self, target: ConversionTarget, source: &str) -> Result<(), WorkspaceError>;
fn reset_conversion_override(&mut self, target: ConversionTarget) -> Result<(), WorkspaceError>;
```

`apply_conversion_override` must parse the candidate before mutating either model or preferences, find the parameter/axis by exact semantic ID, set the effective conversion, store the key/value, mark project preferences dirty, and recompute validation. `reset_conversion_override` restores the matching field from `source_xdf`, removes only that override key, and recomputes validation. `apply_project_conversion_overrides` iterates exact model IDs and reports unmatched keys through the existing status/diagnostic path.

Call `apply_project_conversion_overrides` after XDF installation and again after restored project preferences arrive, so opening the XDF before the BIN still applies the correct project map.

- [x] **Step 6: Run focused and existing persistence tests**

Run:

```powershell
cargo test -p tuner-app --lib conversion_override project_preferences project_state
```

Expected: all selected tests pass with no BIN undo changes.

---

### Task 2: Add semantic-preserving XDF serialization for effective formulas

**Files:**
- Modify: `crates/tuner-xdf/src/lib.rs:25-1305` for export errors, retained source tree, effective conversion updates, and XML serialization.
- Test: `crates/tuner-xdf/src/lib.rs` in the existing test module.

**Interfaces:**
- Produces `XdfDocument::to_xdf_text(&self) -> Result<String, XdfError>`.
- The method serializes the document's retained XML tree while applying each current parameter and axis conversion in parser order.

- [x] **Step 1: Write the failing export round-trip test**

Add a small XDF fixture containing a parameter conversion, an axis conversion, and an unknown vendor element. Parse it, change the effective parameter/axis formulas, export, parse the exported text, and assert the formulas round-trip while the original source bytes remain unchanged:

```rust
#[test]
fn export_materializes_effective_parameter_and_axis_formulas_without_touching_source() {
    let source = br#"<XDFFORMAT><XDFHEADER/><XDFVENDOR value="keep"/><XDFTABLE uniqueid="map" title="Map"><XDFCONVERT><MATH equation="X"/></XDFCONVERT><XDFAXIS id="x" indexcount="2"><XDFCONVERT><MATH equation="X + 1"/></XDFCONVERT><EMBEDDEDDATA mmedaddress="0" mmedelementsizebits="8" mmedtypeflags="0x06"/></XDFAXIS><EMBEDDEDDATA mmedaddress="0" mmedelementsizebits="8" mmedrowcount="1" mmedcolcount="2" mmedmajorstridebits="16" mmedminorstridebits="8" mmedtypeflags="0x06"/></XDFTABLE></XDFFORMAT>"#;
    let mut document = XdfDocument::parse(source).unwrap();
    document.parameters[0].conversion = Some("X * 2".to_string());
    document.parameters[0].axes[0].conversion = Some("X + 3".to_string());

    let exported = document.to_xdf_text().unwrap();
    let reparsed = XdfDocument::parse(exported.as_bytes()).unwrap();
    assert_eq!(reparsed.parameters[0].conversion.as_deref(), Some("X * 2"));
    assert_eq!(reparsed.parameters[0].axes[0].conversion.as_deref(), Some("X + 3"));
    assert!(exported.contains("XDFVENDOR"));
    assert_eq!(document.exact_sha256, sha256_hex(source));
}
```

- [x] **Step 2: Run the export test and verify the expected RED failure**

Run:

```powershell
cargo test -p tuner-xdf --lib export_materializes_effective_parameter_and_axis_formulas_without_touching_source
```

Expected: compilation failure because `to_xdf_text` and the retained source tree do not yet exist.

- [x] **Step 3: Retain the parsed XML tree**

Add a private `source_root: XmlNode` to `XdfDocument` and assign `root.clone()` in `parse_with_source` after computing diagnostics. Add `XdfError::Export { message: String }` with a display branch. Keep the existing normalized public fields unchanged.

- [x] **Step 4: Implement conversion-node updates and XML escaping**

Implement the smallest recursive helpers needed by `to_xdf_text`:

```rust
fn update_parameter_nodes(root: &mut XmlNode, parameters: &[ParameterDefinition]) -> Result<(), XdfError>;
fn set_conversion_node(node: &mut XmlNode, source: Option<&str>);
fn escape_xml(value: &str, attribute: bool) -> String;
fn write_xml_node(node: &XmlNode, output: &mut String, depth: usize);
```

Walk `collect_parameter_nodes` order, update each parameter's conversion, then walk each parameter's `XDFAXIS` descendants in order and update the corresponding `AxisDefinition` conversion. Reuse an existing `XDFCONVERT/MATH` node when present; otherwise append `<XDFCONVERT><MATH equation="…"/></XDFCONVERT>`. Return `XdfError::Export` if source and normalized parameter/axis counts disagree. Serialize the retained unknown nodes, attributes, and child order with escaped text/attributes and a UTF-8 XML declaration.

- [x] **Step 5: Run export tests and the XDF suite**

Run:

```powershell
cargo test -p tuner-xdf --lib export_materializes_effective_parameter_and_axis_formulas_without_touching_source
cargo test -p tuner-xdf --lib
```

Expected: the new round-trip test and all existing XDF tests pass.

---

### Task 3: Add formula preview, Inspector editing, and command entry points

**Files:**
- Modify: `crates/tuner-app/src/lib.rs:1028-1650` for stable commands and availability, `1818-2560` for preview/edit helpers, and `6800-7900` for Inspector UI.
- Test: `crates/tuner-app/src/lib.rs` in the existing test module.

**Interfaces:**
- Produces `ConversionTarget` selection from the selected parameter or selected axis.
- Produces `TunerApp::conversion_preview(target, candidate) -> FormulaPreview` and `TunerApp::apply_conversion_override(...)` UI behavior.
- Adds stable command IDs `view.formula-editor` and `file.save-xdf-as`.

- [x] **Step 1: Write failing preview and invalid-apply tests**

Add tests that use an existing fixture conversion:

```rust
#[test]
fn conversion_preview_reports_evaluation_and_inversion_for_candidate_formula() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(Some(bin), Some(xdf), None, None);
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
    app.workspace.set_documents(Some(bin), Some(xdf), None, None);
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();
    let before = app.workspace.xdf.as_ref().unwrap().parameter(&id).unwrap().conversion.clone();
    assert!(app.apply_conversion_override(ConversionTarget::parameter(id.clone()), "X +").is_err());
    assert_eq!(app.workspace.xdf.as_ref().unwrap().parameter(&id).unwrap().conversion, before);
    assert!(app.project_preferences.conversion_overrides.is_empty());
}
```

- [x] **Step 2: Run the tests and verify the expected RED failure**

Run:

```powershell
cargo test -p tuner-app --lib conversion_preview_reports_evaluation_and_inversion_for_candidate_formula invalid_conversion_override_does_not_replace_effective_formula_or_project_state
```

Expected: compilation failure because preview state and the Inspector-facing methods do not yet exist.

- [x] **Step 3: Implement preview and draft state**

Add a transient `FormulaPreview` containing `parse_error`, `invertible`, `inverse_error`, and sample rows with raw/engineering values. Add `formula_editor_draft: String` and a target identity field to `TunerApp`; synchronize the draft whenever the selected parameter/axis target changes. Use the existing `tuner-cal` parser through the direct workspace dependency already used by `tuner-xdf`, or the existing `ParameterDefinition::compile_conversion` path, and do not add another expression engine.

Use samples `0.0`, `1.0`, and the current selected raw value when available. Evaluate with `Conversion::evaluate`; use `Conversion::invert` for inverse status. Do not mutate the workspace during preview.

- [x] **Step 4: Implement Inspector controls and stable commands**

Add a `Conversion / Formula` section to the Inspector showing target identity, source/effective status, multiline draft editor, live parse/inversion messages, samples, and buttons for Apply override, Reset to XDF, Revert, and Save XDF As. Applying calls the shared override method; Revert reloads the effective formula into the draft only. Make `view.formula-editor` reveal the Inspector and focus the formula section through the existing egui memory pattern. Make `file.save-xdf-as` use the existing command button/availability path.

Add command availability reasons for missing XDF/target and background operations. Keep engineering cell/axis editing enabled only when the effective conversion is invertible; raw display/edit paths remain available.

- [x] **Step 5: Run focused app tests and headless render checks**

Run:

```powershell
cargo test -p tuner-app --lib conversion_preview invalid_conversion_override
cargo test -p tuner-app --lib formula
```

Expected: preview, invalid atomicity, command registration, and Inspector rendering tests pass.

---

### Task 4: Add asynchronous Save XDF As and re-parse verification

**Files:**
- Modify: `crates/tuner-app/src/operations.rs:20-390` for SavingXdf operation messages and worker.
- Modify: `crates/tuner-app/src/lib.rs:1550-1660, 4190-4220, 5200-5390, 7000-7145` for command dispatch, operation handling, file dialog, and status text.
- Test: `crates/tuner-app/src/operations.rs` tests if needed and `crates/tuner-app/src/lib.rs` tests.

**Interfaces:**
- Produces `OperationKind::SavingXdf` and `OperationCoordinator::spawn_save_xdf(id, document, path)`.
- Produces `TunerApp::start_xdf_save(path) -> Result<(), WorkspaceError>`.

- [x] **Step 1: Write failing operation and safety tests**

Add tests for a new target and an existing target:

```rust
#[test]
fn xdf_save_as_refuses_existing_target_and_reparses_new_output() {
    let source_path = unique_temp_path("source.xdf");
    let target_path = unique_temp_path("output.xdf");
    std::fs::write(&source_path, FIXTURE_XDF).unwrap();
    let document = XdfDocument::load(&source_path).unwrap();
    let original = std::fs::read(&source_path).unwrap();

    let output = document.to_xdf_text().unwrap();
    write_new_xdf(&target_path, output.as_bytes()).unwrap();
    assert!(XdfDocument::load(&target_path).is_ok());
    assert_eq!(std::fs::read(&source_path).unwrap(), original);
    assert!(write_new_xdf(&target_path, output.as_bytes()).is_err());
}
```

Use the existing unique temporary-path helper style in the test module; do not delete arbitrary directories.

- [x] **Step 2: Run the safety test and verify RED**

Run:

```powershell
cargo test -p tuner-app --lib xdf_save_as_refuses_existing_target_and_reparses_new_output
```

Expected: compilation failure because the XDF save worker/helper does not exist.

- [x] **Step 3: Implement the background worker and atomic new-file write**

Add `SavingXdf` messages and a worker that clones the effective XDF, calls `to_xdf_text`, refuses an existing path, creates the target with `create_new`, writes the UTF-8 bytes, and reparses the written file with `XdfDocument::load`. Return `OperationResult::written` only after the re-parse succeeds; report errors through the existing operation channel. Reuse the existing `spawn_write_text` safety shape where possible, extracting one small `write_new_file` helper if both workers need it.

Add `start_xdf_save` to reject missing XDF, busy operations, and empty paths, then begin `OperationKind::SavingXdf`. Add the File menu/command dialog filter for `*.xdf`, and handle `OperationPayload::Written` with “Saved a new XDF at …”. Add `SavingXdf` to `operation_block_reason` so conflicting document/edit actions are disabled.

- [x] **Step 4: Run operation tests and full app tests**

Run:

```powershell
cargo test -p tuner-app --lib xdf_save_as
cargo test -p tuner-app --lib
```

Expected: new safety tests and all existing app tests pass.

---

### Task 5: Extend `tuner-ui/v1` for formula inspection, overrides, and Save As

**Files:**
- Modify: `crates/tuner-app/src/ui_ipc.rs:15-75` to add `axis_index` and `formula` request fields.
- Modify: `crates/tuner-app/src/lib.rs:4217-4610` for direct request handling, capabilities, state, and debug report.
- Modify: `docs/tuner-ui-ipc.md` for request examples and safety behavior.
- Test: `crates/tuner-app/src/ui_ipc.rs` and `crates/tuner-app/src/lib.rs` existing tests.

**Interfaces:**
- `get_conversion` accepts `semantic_id` and optional `axis_index`.
- `set_conversion_override` accepts `semantic_id`, optional `axis_index`, and `formula`.
- `reset_conversion_override` accepts `semantic_id` and optional `axis_index`.
- `save_xdf_as` accepts `path` and returns the queued operation state.

- [x] **Step 1: Write failing IPC behavior tests**

Add a headless request test that proves agent actions use the same model methods:

```rust
#[test]
fn ui_ipc_conversion_actions_inspect_apply_reset_and_queue_xdf_save() {
    let (xdf, bin) = column_major_fixture();
    let mut app = TunerApp::headless();
    app.workspace.set_documents(Some(bin), Some(xdf), None, None);
    let id = app.workspace.filtered_parameters("")[0].semantic_id.clone();

    let get = app.handle_ui_ipc_request(&UiIpcRequest::for_test("get_conversion", &id, None, None));
    assert!(get.ok);
    let set = app.handle_ui_ipc_request(&UiIpcRequest::for_test("set_conversion_override", &id, None, Some("X * 2")));
    assert!(set.ok);
    assert_eq!(app.workspace.xdf.as_ref().unwrap().parameter(&id).unwrap().conversion.as_deref(), Some("X * 2"));
    let reset = app.handle_ui_ipc_request(&UiIpcRequest::for_test("reset_conversion_override", &id, None, None));
    assert!(reset.ok);
}
```

Add a transport-level assertion that the capabilities response lists all four actions.

- [x] **Step 2: Run the focused tests and verify RED**

Run:

```powershell
cargo test -p tuner-app --lib ui_ipc_conversion_actions_inspect_apply_reset_and_queue_xdf_save
```

Expected: compilation/test failure because the request fields/actions are not implemented.

- [x] **Step 3: Implement the direct actions through shared helpers**

Deserialize `axis_index` and `formula`. Resolve a target only from an explicit semantic ID plus optional axis index. Implement `get_conversion` using `conversion_preview`, `set_conversion_override` using `apply_conversion_override`, and `reset_conversion_override` using `reset_conversion_override`. For `save_xdf_as`, call `start_xdf_save` and return the existing operation snapshot. Add `view.formula-editor` to the `dispatch` path so the agent can reveal the same Inspector section.

Return structured data containing target key, source/effective formulas, override state, parse/inversion status, and samples. Add all actions to `ui_ipc_capabilities` and include active formula target/override count in `ui_ipc_state` and `debug_report_text`.

- [x] **Step 4: Update the IPC documentation and run tests**

Document authenticated JSONL examples for inspect, apply, reset, and Save As, including the rule that `save_xdf_as` refuses an existing path and that formula changes do not edit BIN bytes. Run:

```powershell
cargo test -p tuner-app --lib ui_ipc
cargo test -p tuner-app --lib
```

Expected: all IPC and app tests pass.

---

### Task 6: Review, documentation, and completion verification

**Files:**
- Modify: `docs/superpowers/sdd/2026-09-14-table-interaction/progress.md` if the roadmap ledger needs the completed formula/IPC slice.
- Modify: `docs/superpowers/sdd/2026-09-14-table-interaction/final-review-package.md` only if the existing review package references changed test counts or scope.
- Modify: `docs/tuner-ui-ipc.md` if final action names/examples differ from the implementation.

- [x] **Step 1: Self-review the spec and plan against implemented behavior**

Check every included item in `docs/superpowers/specs/2026-09-14-formula-editor-xdf-export-design.md`: project override identity, source/effective separation, validation/inversion, Inspector controls, Save XDF As safety/re-parse, IPC actions, and tests. Search the plan and implementation for `TBD`, `TODO`, debug logging, stale action names, and accidental XDF overwrite paths.

- [x] **Step 2: Run the complete verification gate**

Run:

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo check --workspace
cargo build --workspace
```

Expected: every command exits with code 0; report the exact test count from the fresh workspace run.

- [x] **Step 3: Run one native executable smoke test**

Start `target/debug/tuner-app.exe`, read `%APPDATA%\TunerNook\ui-ipc-v1.json`, then use a UTF-8-no-BOM JSONL client to call `ping`, `capabilities`, `state`, and `get_conversion` after loading a BIN/XDF. Call `set_conversion_override` only on a test fixture, verify the response and state, and stop the exact smoke-test process afterward.

- [x] **Step 4: Update the ledger and hand off**

Record the fresh test count, build/check results, and native IPC smoke result. Note the remaining roadmap items: top-level XDF `<XDFFUNCTION>` editing, scalar/flag/raw-hex editors, and 2D/3D graphs/surfaces.
