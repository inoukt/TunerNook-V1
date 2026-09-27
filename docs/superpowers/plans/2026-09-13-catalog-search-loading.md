# Catalog, Search, and Responsive Loading Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Correct XDF category interpretation and deliver a responsive, sortable, searchable desktop-like TunerNook workspace with saved state and test-first background operations.

**Architecture:** Add a document-level category-reference dialect to `tuner-xdf`, pure catalog/search/operation models to `tuner-app`, and integrate them into the existing `WorkspaceState`/`TunerApp` shell. BIN/XDF parsing, validation, report export, and search run in generation-tagged worker operations; only completed, current results are installed on the egui thread.

**Tech Stack:** Rust 2021, eframe/egui 0.36.2 with glow, std threads/channels, serde JSON, existing tuner-core and tuner-xdf APIs, PowerShell verification.

**Spec:** `docs/superpowers/specs/2026-09-13-catalog-search-loading-design.md`

## Global Constraints

- Keep `WorkspaceState` as the only UI boundary that mutates BIN bytes.
- Preserve Save As and debug-export no-overwrite behavior.
- Use the supplied XDF's source-truth MPI count of 88.
- Do not add a native second process/window in this slice; retain a persistent in-process search window with a future companion boundary.
- No production code may be written before its focused failing test is run and observed failing.
- Do not alter the supplied fixture files.
- This workspace is not a Git repository, so verification replaces commit steps.

---

### Task 1: Add XDF category-reference dialect detection and resolved paths

**Files:**
- Modify: `crates/tuner-xdf/src/lib.rs:158-176, 930-1010, 1309-1360, 1810-1825`
- Test: `crates/tuner-xdf/src/lib.rs` module tests near `normalizes_header_and_category_metadata`

**Interfaces:**
- Produces `CategoryReferenceMode`, `CategoryMembership::resolved_category_index`, and `XdfDocument::category_reference_mode`.
- Consumes the existing `XdfCategory`, `XmlNode`, and parser diagnostic structures.

- [x] **Step 1: Write the failing fixture dialect test.**

Add a test that loads `Test bin and xdf/SCGa05_cal.xdf` and asserts the document selects one-based mode, that raw category `57` resolves to the final declared category, and that the first parameter's resolved membership has a name. Also assert that the fixture no longer emits `missing-category` diagnostics caused by raw category `57`.

```rust
#[test]
fn detects_one_based_category_positions_in_the_supplied_fixture() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("Test bin and xdf")
        .join("SCGa05_cal.xdf");
    let document = XdfDocument::load(path).unwrap();

    assert_eq!(
        document.category_reference_mode,
        CategoryReferenceMode::OneBasedPosition
    );
    assert!(document
        .parameters
        .iter()
        .flat_map(|parameter| parameter.category_memberships.iter())
        .any(|membership| {
            membership.category_index == 57
                && membership.resolved_category_index == Some(56)
                && membership.category_name.is_some()
        }));
    assert!(!document
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "missing-category"));
}
```

- [x] **Step 2: Run the focused test and verify the expected red failure.**

Run:

```powershell
cargo test -p tuner-xdf detects_one_based_category_positions_in_the_supplied_fixture -- --exact
```

Expected: compilation/test failure because `CategoryReferenceMode`, the resolved field, and the document field do not exist yet, or because the current parser reports missing category references.

- [x] **Step 3: Write the exact-index fallback test before implementation.**

Extend the existing synthetic category test with a second category and an exact reference to declared index `2`; assert that the mode remains `DeclaredIndex` and the category name remains the index-2 declaration. Keep the existing missing-category test and assert it still reports an unknown `99` reference.

```rust
#[test]
fn keeps_declared_indices_when_one_based_mode_has_no_decisive_evidence() {
    let document = XdfDocument::parse(
        br#"<XDFFORMAT><XDFHEADER>
          <CATEGORY index="0" name="Axis" />
          <CATEGORY index="1" name="Airflow" />
          <CATEGORY index="2" name="Fuel" />
        </XDFHEADER><XDFCONSTANT>
          <CATEGORYMEM index="0" category="2" />
          <EMBEDDEDDATA mmedaddress="0x00" />
        </XDFCONSTANT></XDFFORMAT>"#,
    )
    .unwrap();

    assert_eq!(document.category_reference_mode, CategoryReferenceMode::DeclaredIndex);
    assert_eq!(document.parameters[0].category.as_deref(), Some("Fuel"));
    assert_eq!(
        document.parameters[0].category_memberships[0].resolved_category_index,
        Some(2)
    );
}
```

- [x] **Step 4: Implement minimal dialect detection and resolution.**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryReferenceMode {
    DeclaredIndex,
    OneBasedPosition,
}
```

Add `resolved_category_index: Option<u64>` to `CategoryMembership` and `category_reference_mode: CategoryReferenceMode` to `XdfDocument`. Implement `detect_category_reference_mode(categories, parameter_nodes) -> Result<CategoryReferenceMode, XdfError>` by checking for a contiguous declared index sequence starting at zero and a raw membership value greater than the maximum declared index whose predecessor is a valid zero-based position. Default to declared-index mode. Pass the selected mode into `parse_parameter` and `parse_category_memberships`; resolve one-based values with `categories.get(raw as usize - 1)` and declared values with `category.index == raw`. Keep the raw value unchanged and emit the existing `missing-category` diagnostic when resolution returns `None`. Add one warning diagnostic with code `category-reference-one-based` and message explaining that header positions were used when one-based mode is selected.

- [x] **Step 5: Run the focused parser tests and the existing XDF suite.**

Run:

```powershell
cargo test -p tuner-xdf detects_one_based_category_positions_in_the_supplied_fixture keeps_declared_indices_when_one_based_mode_has_no_decisive_evidence -- --nocapture
cargo test -p tuner-xdf
```

Expected: both new tests and all existing parser/conversion tests pass, with the fixture's missing-category regression removed.

- [x] **Step 6: Add a pure category-path helper test.**

Add a public `ParameterDefinition::category_path()` method that filters resolved names, sorts memberships by `slot` while retaining source order for ties, and returns `Vec<String>`. Test that memberships at slots `0`, `1`, and `2` return the same ordered path and that an unresolved membership is omitted rather than represented as a fake category.

- [x] **Step 7: Implement and verify the category-path helper.**

Implement the method without changing the raw membership order or semantic ID format. Run:

```powershell
cargo test -p tuner-xdf category_path
```

Expected: the path test passes and all existing tests remain green.

---

### Task 2: Create pure catalog metadata, tree, and sorting models

**Files:**
- Create: `crates/tuner-app/src/catalog.rs`
- Modify: `crates/tuner-app/src/lib.rs:1-20, 942-1110, 2860-3005, 3210-3235`
- Test: `crates/tuner-app/src/catalog.rs` module tests and `crates/tuner-app/src/lib.rs` integration tests

**Interfaces:**
- Produces `CatalogSortKey`, `SortDirection`, `CategoryNode`, `ParameterSummary`, `ParameterGroup`, `summary_from_parameter`, `build_category_tree`, and `sort_summaries`.
- Consumes `tuner_xdf::ParameterDefinition`, `ParameterKind`, and `ByteRange`.

- [x] **Step 1: Write a failing summary-metadata test.**

Create a small XDF table fixture in the catalog tests, call `summary_from_parameter`, and assert title, type, `rows`, `columns`, `element_count`, `byte_size`, address, and category path.

```rust
#[test]
fn summary_exposes_shape_and_mapped_size() {
    let xdf = XdfDocument::parse(TABLE_XDF).unwrap();
    let summary = summary_from_parameter(&xdf.parameters[0]);
    assert_eq!(summary.title, "Map");
    assert_eq!(summary.kind, ParameterKind::Table);
    assert_eq!((summary.rows, summary.columns), (2, 3));
    assert_eq!(summary.element_count, Some(6));
    assert_eq!(summary.byte_size, 6);
    assert_eq!(summary.address, 0);
    assert_eq!(summary.category_path, vec!["Fuel"]);
}
```

- [x] **Step 2: Run the focused catalog test and verify red.**

Run:

```powershell
cargo test -p tuner-app summary_exposes_shape_and_mapped_size -- --exact
```

Expected: failure because the catalog module and metadata fields do not exist.

- [x] **Step 3: Add failing tree and sort tests.**

Add tests that build summaries for `Limiter > RPM` and `Limiter > Speed`, assert one root with two children and subtree counts, and assert title ascending, bytes descending, type ordering, and address ordering use semantic ID as a stable tie-breaker.

```rust
#[test]
fn category_tree_preserves_nested_paths_and_subtree_counts() {
    let tree = build_category_tree(vec![
        summary("A", &["Limiter", "RPM"]),
        summary("B", &["Limiter", "RPM"]),
        summary("C", &["Limiter", "Speed"]),
    ]);
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].name, "Limiter");
    assert_eq!(tree[0].total_count, 3);
    assert_eq!(tree[0].children[0].name, "RPM");
    assert_eq!(tree[0].children[0].total_count, 2);
}

#[test]
fn catalog_sort_is_stable_and_supports_size_and_direction() {
    let mut rows = vec![summary_with_size("same", 4, "b"), summary_with_size("same", 8, "a")];
    sort_summaries(&mut rows, CatalogSortKey::Bytes, SortDirection::Descending, &[], &[]);
    assert_eq!(rows.iter().map(|row| row.byte_size).collect::<Vec<_>>(), vec![8, 4]);
}
```

- [x] **Step 4: Run those tests and verify the intended failures.**

Run:

```powershell
cargo test -p tuner-app category_tree_preserves_nested_paths_and_subtree_counts catalog_sort_is_stable_and_supports_size_and_direction -- --exact
```

Expected: compilation/test failure due to missing catalog interfaces.

- [x] **Step 5: Implement the catalog module.**

Define `ParameterSummary` with `semantic_id`, `unique_id`, `title`, `category`, `category_path`, `category_path_key`, `kind`, `rows`, `columns`, `element_count`, `byte_size`, and `address`. Define `CategoryNode` with `key`, `name`, `path`, `total_count`, `parameters`, and `children`. Build a tree by inserting each summary into path components and incrementing every ancestor's total count. Give an empty path the key/name `Uncategorized`.

Define `CatalogSortKey` values `Category`, `Title`, `Type`, `Dimensions`, `Elements`, `Bytes`, `Address`, `Favorite`, and `Recent`, plus `SortDirection`. Implement a stable comparator with semantic ID as the final tie-breaker. Favorite sorting uses the supplied favorite key list; recent sorting uses the supplied recent key list. Export display-name helpers for the UI.

- [x] **Step 6: Replace the app-local summary/group types with catalog exports.**

Remove the duplicate definitions from `lib.rs`, add `mod catalog; pub use catalog::{...};`, and update `WorkspaceState::filtered_parameters`/`grouped_parameters` to call `summary_from_parameter` and match the complete category path text as well as title/IDs. Keep the existing method signatures where possible so existing tests and extensions remain source-compatible.

- [x] **Step 7: Run the catalog and existing app tests.**

Run:

```powershell
cargo test -p tuner-app catalog_
cargo test -p tuner-app
```

Expected: metadata/tree/sort tests pass and existing workspace/window tests still pass.

---

### Task 3: Add the pure search query and result engine

**Files:**
- Create: `crates/tuner-app/src/search.rs`
- Modify: `crates/tuner-app/src/lib.rs:1-20, 1050-1110, 1570-1625`
- Test: `crates/tuner-app/src/search.rs` module tests

**Interfaces:**
- Produces `SearchMatchMode`, `SearchFieldScope`, `SearchSortKey`, `SearchState`, `SearchWindowMemory`, `SearchResult`, `SearchMatch`, `compile_query`, and `search_snapshot`.
- Consumes `ParameterSummary`, `XdfDocument`, immutable BIN bytes, and `ParameterDefinition` cell readers.

- [x] **Step 1: Write failing metadata matcher tests.**

Test case-insensitive contains, whole-field exact matching, `*` multi-character wildcard, `?` one-character wildcard, and wildcard literals that do not panic on empty strings.

```rust
#[test]
fn search_matches_contains_exact_and_wildcard_modes() {
    let row = summary("Driver Pedal Torque Request", &["Torque", "Request"]);
    assert!(search_summary(&row, "pedal", SearchMatchMode::Contains, SearchFieldScope::Title).is_match());
    assert!(search_summary(&row, "driver pedal torque request", SearchMatchMode::Exact, SearchFieldScope::Title).is_match());
    assert!(search_summary(&row, "Driver*Request", SearchMatchMode::Wildcard, SearchFieldScope::Title).is_match());
    assert!(!search_summary(&row, "Driver?Pedal", SearchMatchMode::Wildcard, SearchFieldScope::Title).is_match());
}
```

- [x] **Step 2: Run the matcher test and verify red.**

Run:

```powershell
cargo test -p tuner-app search_matches_contains_exact_and_wildcard_modes -- --exact
```

Expected: failure because the search module and match modes do not exist.

- [x] **Step 3: Write failing value-search tests.**

Use a two-cell integer XDF and BIN bytes `[10, 20]`. Assert raw exact query `0x0A` finds row 0/column 0, engineering query `10` finds the same cell, metadata-only search does not inspect values, and an invalid numeric value produces a query error with no value matches.

```rust
#[test]
fn search_reads_raw_and_engineering_values_without_matching_errors() {
    let xdf = XdfDocument::parse(VALUE_XDF).unwrap();
    let results = search_snapshot(&xdf, Some(&[10, 20]), &SearchState::for_query("0x0A", SearchFieldScope::RawValues));
    assert_eq!(results[0].matches[0].row, 0);
    assert_eq!(results[0].matches[0].column, 0);

    let bad = search_snapshot(&xdf, Some(&[10, 20]), &SearchState::for_query("not-a-number", SearchFieldScope::RawValues));
    assert!(bad.is_empty());
}
```

- [x] **Step 4: Run the value test and verify red.**

Run:

```powershell
cargo test -p tuner-app search_reads_raw_and_engineering_values_without_matching_errors -- --exact
```

Expected: failure because value scanning and result models are missing.

- [x] **Step 5: Implement the query compiler and search snapshot.**

Define serializable enums with safe defaults. `SearchState` stores `open`, `query`, match mode, field scope, result sort/direction, selected result, result limit, and `SearchWindowMemory`. Implement a lower-case linear glob matcher where `*` matches zero or more characters and `?` matches exactly one character. Exact mode compares the normalized complete field; contains uses substring; empty query returns all metadata rows but never scans values unless the user entered a non-empty numeric query.

Parse raw numeric values as decimal or `0x` hexadecimal integers and engineering values as finite decimal floats. Use `1e-9 * max(abs(query), 1.0)` for finite engineering comparisons. For each matching parameter, return its summary plus all matching cell locations up to the result limit, raw/engineering display strings, and per-cell diagnostics for unreadable or non-convertible values. Never turn an error string into a match. Sort results by the selected search key and semantic ID.

- [x] **Step 6: Add persistence and search-action tests.**

Test serde round-trip/default migration for `SearchState`, clamping search window geometry and result limit, and a search-result action helper that toggles a favorite key without changing BIN bytes.

- [x] **Step 7: Run search tests and all app tests.**

Run:

```powershell
cargo test -p tuner-app search
cargo test -p tuner-app
```

Expected: all search and existing app tests pass.

---

### Task 4: Add the deterministic operation coordinator and workers

**Files:**
- Create: `crates/tuner-app/src/operations.rs`
- Modify: `crates/tuner-app/src/lib.rs:1-20, 126-230, 1570-1645, 2340-2405, 2525-2645`
- Test: `crates/tuner-app/src/operations.rs` module tests

**Interfaces:**
- Produces `OperationKind`, `OperationPhase`, `OperationState`, `OperationMessage`, `OperationResult`, and `OperationCoordinator`.
- Consumes immutable path/BIN/XDF snapshots and returns installable payloads tagged with operation IDs and generation IDs.

- [x] **Step 1: Write failing state-transition tests.**

Test begin/update/complete/error transitions, monotonically increasing IDs, contextual message rotation every 1.4 seconds using an injected elapsed duration, and stale result rejection after a newer operation begins.

```rust
#[test]
fn coordinator_rejects_stale_results_and_reports_current_phase() {
    let mut coordinator = OperationCoordinator::default();
    let old = coordinator.begin(OperationKind::LoadingXdf, "old.xdf");
    let new = coordinator.begin(OperationKind::LoadingBin, "new.bin");
    assert!(!coordinator.accepts(old));
    assert!(coordinator.accepts(new));
    assert_eq!(coordinator.message_at(std::time::Duration::from_secs_f32(1.5)), "Reading BIN…");
    coordinator.complete(new, "Loaded BIN");
    assert!(coordinator.active().is_none());
}
```

- [x] **Step 2: Run the operation test and verify red.**

Run:

```powershell
cargo test -p tuner-app coordinator_rejects_stale_results_and_reports_current_phase -- --exact
```

Expected: failure because the coordinator does not exist.

- [x] **Step 3: Implement the pure coordinator state machine.**

Implement operation IDs, active operation state, last result/error, stale count, phase labels, and `message_at`. `OperationKind` labels must be exactly:

```text
Reading BIN…, Interpreting XDF…, Validating mappings…, Restoring workspace…, Saving BIN…, Exporting debug report…, Searching…
```

Use `Instant` only in the runtime coordinator; let the pure message selector accept a supplied `Duration` for deterministic tests.

- [x] **Step 4: Add real worker spawn helpers and test result tagging.**

Implement `spawn_load_bin`, `spawn_load_xdf`, `spawn_validate`, and `spawn_search` with `std::thread::spawn` and `std::sync::mpsc::channel`. Worker payloads must own their `BinDocument`, `XdfDocument`, validation report, or search results. Add a test that starts two operations and verifies the receiver can identify the newer operation ID even when the older result arrives later.

- [x] **Step 5: Run operation tests and compile the app.**

Run:

```powershell
cargo test -p tuner-app operation
cargo check -p tuner-app
```

Expected: coordinator/worker tests pass and the native app compiles before UI integration.

---

### Task 5: Integrate catalog tree, sorting, metadata labels, and persistence

**Files:**
- Modify: `crates/tuner-app/src/lib.rs:20-235, 1660-1905, 2815-3010, 3085-3235, 3440-3505, 4000-4380`
- Test: `crates/tuner-app/src/lib.rs` headless tests

**Interfaces:**
- Consumes catalog types and category paths from Tasks 1–2.
- Produces project-persisted browser sort state, nested tree rendering, metadata-rich titles/tabs/rows, and migration-safe defaults.

- [x] **Step 1: Write failing persistence and UI-model tests.**

Add tests that a new project records every category path as collapsed, that a nested path key survives project preference round-trip, that sort key/direction survive JSON migration, and that a table title label contains `rows×columns` and byte count.

```rust
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
```

- [x] **Step 2: Run the new persistence/title tests and verify red.**

Run:

```powershell
cargo test -p tuner-app project_defaults_collapse_nested_category_paths_and_persist_catalog_sort -- --exact
```

Expected: failure because project sort fields and metadata label helpers do not exist.

- [x] **Step 3: Extend versioned preferences and project synchronization.**

Bump `PROJECT_SETTINGS_VERSION`, add `catalog_sort`, `catalog_sort_direction`, and `search_state` to `ProjectPreferences`, and add tolerant defaults/sanitization. Mirror the fields through `apply_project_preferences_to_legacy`, `sync_legacy_preferences_to_project`, reset, load, and save. Keep existing v1 JSON readable. Store search geometry/query/filter state with the project; do not store result snapshots or BIN bytes.

- [x] **Step 4: Implement nested category tree rendering.**

Replace the flat `grouped_parameters` browser path with `build_category_tree`, preserving the existing browser scroll frame and collapse controls. Use stable category path keys for `collapsed_categories`. `Collapse all` records every visible node key; `Expand all` removes visible node keys. Display subtree counts and indent child nodes. Apply favorites/recent ranking and the selected catalog sort within each node. Do not make the panel's minimum width depend on the title; retain horizontal scrolling.

- [x] **Step 5: Add metadata-rich labels and sort controls.**

Add `summary_label` and `table_window_title` helpers. Use the format `{title} · {kind} · {rows}×{columns} · {elements} el · {bytes} B` in rows/tabs/window titles, prefixing category/path where space allows. Add browser selectors for catalog sort and direction. Add tooltip text with the full title, path, address, dimensions, elements, byte size, and semantic ID. Include the same fields in the inspector and debug report.

- [x] **Step 6: Run app tests and fixture shape regression.**

Run:

```powershell
cargo test -p tuner-app
cargo run --quiet -p tuner-cli -- inspect-xdf "Test bin and xdf/SCGa05_cal.xdf"
```

Expected: headless state tests pass; fixture output shows nested resolved paths and source-truth MPI 88.

---

### Task 6: Integrate Ctrl+F persistent search and taskbar-like refocus behavior

**Files:**
- Modify: `crates/tuner-app/src/lib.rs:266-280, 465-750, 1570-1645, 2100-2310, 2610-2680, 3085-3185, 3720-3810`
- Test: `crates/tuner-app/src/lib.rs` headless tests

**Interfaces:**
- Consumes `SearchState`, `SearchResult`, `OperationCoordinator`, catalog metadata, and favorite/open-table methods.
- Produces `search_open`, a persistent internal window with saved geometry, Ctrl+F command dispatch, search result actions, and background generations.

- [x] **Step 1: Write failing command and action tests.**

Test that `view.search` is registered with `Ctrl+F`, dispatching it opens the search state without mutating document bytes, closing only hides the window, and a favorite action from a result updates the existing favorite key list.

```rust
#[test]
fn ctrl_f_search_command_opens_persistent_window_state() {
    let mut app = TunerApp::headless();
    let before = app.workspace.bin.as_ref().map(|bin| bin.bytes().to_vec());
    app.dispatch_command("view.search").unwrap();
    assert!(app.search_state.open);
    assert_eq!(app.workspace.bin.as_ref().map(|bin| bin.bytes().to_vec()), before);
}
```

- [x] **Step 2: Run the command test and verify red.**

Run:

```powershell
cargo test -p tuner-app ctrl_f_search_command_opens_persistent_window_state -- --exact
```

Expected: failure because the search command/state is not integrated.

- [x] **Step 3: Register and dispatch the search command.**

Add `BuiltinCommand::Search`, descriptor `view.search` with default shortcut `Ctrl+F`, default shortcut JSON entry, and dispatch behavior that sets `search_state.open = true`, requests query focus, and clears no existing query. Extend shortcut key parsing only as needed for `F` and avoid intercepting Ctrl+F inside an active text editor except to focus the search window.

- [x] **Step 4: Implement the persistent search window.**

Add an egui internal `Window` with stable project-scoped ID, movable/resizable geometry, zoom controls, query field, match mode selector, field scope selector, result sort/direction selector, result count, and a clear/close control. Store geometry after pointer interaction settles. Add a compact `Search` workspace tab/button beside the table tabs; clicking it reopens/focuses the window. Closing hides it while keeping state.

- [x] **Step 5: Connect debounced background search and result actions.**

When query or search settings change, increment a generation and start a search worker after a short debounce. Poll results each frame; accept only the current operation and generation. Render each result with category path, type, shape, bytes, address, first matching cell, favorite toggle, and `Open/Focus`. Double-clicking a result calls existing `open_table`. Show “BIN required for value search” when metadata is available but no BIN exists. Show parse errors inline.

- [x] **Step 6: Run search integration tests and compile.**

Run:

```powershell
cargo test -p tuner-app search ctrl_f_search
cargo check -p tuner-app
```

Expected: command, persistence, result action, and stale-generation tests pass and the UI compiles.

---

### Task 7: Integrate background document operations, validation, save/export, and busy overlay

**Files:**
- Modify: `crates/tuner-app/src/lib.rs:966-1050, 1570-1645, 2160-2415, 2525-2690, 3850-4020`
- Test: `crates/tuner-app/src/lib.rs` headless tests and `crates/tuner-app/src/operations.rs`

**Interfaces:**
- Consumes operation coordinator/worker payloads and existing document methods.
- Produces responsive BIN/XDF loading, validation, restoration, Save As, report export, action gating, overlay animation, and operation diagnostics.

- [x] **Step 1: Write failing operation-gating and transition tests.**

Test that active loading disables open/save/edit/search-start actions, that completing a current result installs the document, that an older result cannot replace it, and that a failed load leaves the previous document intact with an error status.

```rust
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
```

- [x] **Step 2: Run the gating test and verify red.**

Run:

```powershell
cargo test -p tuner-app stale_load_cannot_replace_newer_document_and_failure_preserves_previous_state -- --exact
```

Expected: failure because the app has no coordinator, operation result installer, or action gate.

- [x] **Step 3: Add coordinator/runtime fields and result polling.**

Add `operations: OperationCoordinator` and pending document/project state to `TunerApp`. Poll the receiver at the start of every frame, install only current results, recompute validation, restore project tables/categories after current XDF installation, and call `context.request_repaint_after(Duration::from_millis(50))` while active. Keep status messages in `WorkspaceState`.

- [x] **Step 4: Convert file operations to workers.**

After the native picker returns, start `spawn_load_bin` or `spawn_load_xdf` instead of calling `WorkspaceState::open_*` synchronously. Schedule validation after a paired document change. Schedule project restoration as a tagged phase. Snapshot BIN bytes for Save As and report text before spawning `save_as` or `fs::write`; preserve the no-overwrite checks. A picker cancellation does not start an operation.

- [x] **Step 5: Gate conflicting commands and edits.**

Make `command_availability` and edit buttons consult `operations.is_busy()`. Open BIN/XDF, Save As, Undo/Redo, Apply Cell, and starting another search are disabled while a conflicting document/save operation is active; table movement, category expansion, and report viewing remain safe. Disabled command entries show “busy: {phase}”.

- [x] **Step 6: Draw the gentle busy overlay.**

Add a stable egui `Area` over the usable workspace. Draw a rounded amber panel, a caution glyph such as `⚠`, smooth sine pulse based on `ctx.input(|input| input.time)`, the active operation message, and a small “TunerNook is still working” line. Rotate only the operation-specific strings from `OperationCoordinator::message_at`; use no flashing or sound. Keep the toolbar and bottom diagnostics panel available.

- [x] **Step 7: Extend debug reporting and verify operation tests.**

Include active/last operation kind, phase, ID, generation, elapsed milliseconds, and stale-result count in `debug_report_text`. Run:

```powershell
cargo test -p tuner-app operation stale_load_cannot_replace_newer_document_and_failure_preserves_previous_state -- --nocapture
cargo check -p tuner-app
```

Expected: state/worker/gating tests pass and the full app compiles.

---

### Task 8: Regression verification, documentation, and completion audit

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-09-13-catalog-search-loading-design.md`
- Modify: `docs/superpowers/plans/2026-09-13-catalog-search-loading.md`
- Test: `crates/tuner-xdf/src/lib.rs`, `crates/tuner-app/src/lib.rs`

**Interfaces:**
- Consumes all implementation tasks.
- Produces documented controls, fresh verification evidence, and an explicit native-smoke-test status.

- [x] **Step 1: Add fixture category/path regression assertions.**

Extend the existing app fixture test to assert the category-reference mode is one-based, the root category tree contains `MPI` with 88 tables, `Limiter` with 91, and `Torque Model` with 52, and no `missing-category` warning remains from the one-based references. Keep the existing 2,915 parameter, 8,745 axis, 654,336-byte BIN, and valid mapping assertions.

- [x] **Step 2: Document the new workflow.**

Add README instructions for category tree expansion, catalog sorting, metadata-rich table labels, Ctrl+F search modes/scopes/value search, favorite/open result actions, persistent Search workspace behavior, and the animated busy indicator. State clearly that the first version uses a persistent internal window and that a native companion window is a future extension.

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

Expected: format/build/tests succeed; validation reports `valid:true`, `issue_count:0`, `parameter_count:2915`, and `bin_size_bytes:654336`; local fixture XDF/BIN hashes match the recorded baseline (values omitted from this public plan).

- [ ] **Step 4: Run a native smoke test when a Windows GUI surface is available.**

Run `cargo run -p tuner-app`, open the fixture pair, observe the loading overlay, expand `Limiter > RPM`, change sort to byte size descending, press Ctrl+F, search `*torque*` and a raw/engineering value, favorite/open a result, move/resize Search and a table, close/reopen Search, and restart the app. Confirm saved category expansion, sort/search state, window geometry, and unchanged fixture hashes. If this session still has no native GUI surface, record that limitation in the final response and do not claim visual verification.

Verification note: this session exposed no native Windows GUI surface through the available
computer-use bridge, so visual smoke testing remains pending; build, headless integration,
and fixture validation checks were completed.

- [x] **Step 5: Self-review the written artifacts and implementation.**

Read the spec and plan again. Search the plan for `TBD`, `TODO`, `implement later`, and vague “handle edge cases” wording. Check every public type used by the UI exists, every new production behavior has a test that was observed failing before implementation, no worker can install a stale result, and no UI operation writes input BIN/XDF files. Mark the spec `Implemented` only if the full verification suite passes; otherwise record the exact failing command and remaining limitation.
