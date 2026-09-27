# TunerNook — `tuner-app` `lib.rs` module split proposal

Date: 2026-09-23
Status: Historical proposal; superseded for execution on 2026-09-27
Target file: `crates/tuner-app/src/lib.rs` (historical line count and boundaries)

> **Do not execute this line-based proposal as written.** Its source line map
> and Phase 1 are stale: the test module has already been extracted to
> `crates/tuner-app/src/tests.rs` by the completed 2026-09-25 render-efficiency
> plan. Use the staged production-module plan at
> `docs/superpowers/plans/2026-09-27-dirty-state-and-app-module-split.md` and
> its design spec for current work. Future production moves should be scoped
> into small, reviewed slices rather than applying the old line ranges.

## Goal

Split the 23k-line `lib.rs` into focused modules **without any behavior change**.
Every step compiles, passes the full battery (`cargo fmt --check`, `cargo test
--workspace`, release build) and the runtime smoke test, and is independently
revertible. No signature changes, no renames of public items, no logic edits —
only `mod`/`use` wiring and code movement. Public API paths stay identical via
re-exports, so `main.rs`, the test suite, and any external expectations keep
working untouched.

## Why these boundaries (evidence from the file)

- Lines 92–1500: preference/types + persistence helpers (~1.4k lines) —
  no `egui::Ui` rendering, pure state + JSON I/O.
- Lines 1445–2200: command registry + availability rules (~750 lines) —
  depends only on `WorkspaceState`, not on `TunerApp`.
- Lines 2200–4095: `WorkspaceState` + cell/selection/conversion logic (~1.9k
  lines) — the document-level state machine; no `egui::Ui` parameters except
  small color helpers that belong with them.
- Lines 4529–4619: NookLink challenge types (~90 lines).
- Lines 4620–16592: the `TunerApp` impl (~12k lines) — the actual monolith:
  document/load coordination (~2.3k), IPC (~2.0k), editing/dispatch (~1.2k),
  shell rendering (~1.1k), table windows (~1.2k), inspector/formula (~0.4k),
  search/operations (~0.3k), floating windows (~2.6k), settings/UI text
  helpers (~0.5k).
- Lines 16593–23316: `#[cfg(test)] mod tests` (~6.7k lines) — moves out whole.

## Proposed module layout

```
crates/tuner-app/src/
├── lib.rs                    # crate root: mod decls, pub use re-exports,
│                             # TunerApp struct def + new/headless + eframe::App
│                             # (target ≤ ~1.5k lines)
├── preferences.rs            # AppPreferences/ProjectPreferences/table memory
│                             # types, JSON (de)serialization, settings paths,
│                             # atomic save, sanitize/migration (~1.9k)
├── commands.rs               # BuiltinCommand, CommandRegistry, availability
│                             # rules (~0.8k)
├── workspace.rs              # WorkspaceState, CellView/Selection, CellOperation,
│                             # conversion plumbing, clipboard parsing
│                             # (~2.4k incl. its unit tests)
├── catalog_view.rs           # browser/catalog tree, category tree caching +
│                             # rendering of browser contents (~0.7k)
├── table_window.rs           # show_table_window, table styling/scale, cell
│                             # fills/colors, geometry guard, drag state,
│                             # tabs/arrangement (~2.2k)
├── inspector.rs              # show_inspector(+contents), formula editor,
│                             # diagnostics pane (~0.9k)
├── compare_view.rs           # show_compare_window + compare surface (~0.7k)
├── hex_view.rs               # show_hex_editor + hex interactions (~0.7k)
├── map_finder_view.rs        # show_map_finder (~0.5k)
├── search_view.rs            # show_search_window + search orchestration (~0.6k)
├── windows.rs                # small floating windows: command palette, action
│                             # history, prompts/overlays, debug report,
│                             # settings (~1.5k)
├── nooklink_ui.rs            # show_nooklink_setup + challenge handling +
│                             # proposal review/apply (~0.8k)
├── ipc.rs                    # poll_ui_ipc, handle_ui_ipc_request,
│                             # capabilities/state builders (~2.0k)
├── operations_ui.rs          # poll_background_operations,
│                             # install_operation_result, search orchestration
│                             # (~0.7k)
├── app_shell.rs              # toolbar, menus, browser rail, inspector rail,
│                             # shortcuts, command button helpers (~1.3k)
├── debug_report.rs           # debug_report_text + export (~1.0k)
├── format.rs                 # format_f64, axis header text, name() helpers,
│                             # inspector_row, raw_value_text (~0.3k)
├── shortcuts.rs              # ParsedShortcut/parse_shortcut/parse_key,
│                             # shortcut matching (~0.2k)
├── agent_tasks.rs            # (exists) task state model
├── ui_ipc.rs                 # (exists) wire protocol
├── catalog.rs                # (exists) summary types
├── compare.rs                # (exists) compare data helpers
├── hex.rs                    # (exists) hex helpers
├── map_search.rs             # (exists) scanner engine
├── operations.rs             # (exists) coordinator
├── search.rs                 # (exists) search engine
└── surface.rs                # (exists) 3D projection helpers
```

`lib.rs` keeps: `mod` declarations, `pub use` re-exports (so
`crate::AppPreferences` etc. keep resolving), `TunerApp` struct definition,
`TunerApp::new`/`headless`, and `eframe::App for TunerApp`.

## Historical no-behavior-change extraction outline

Order matters: each phase compiles and tests green on its own, and earlier
phases reduce the surface later phases must read. Never move a `#[cfg(test)]`
test away from the code it exercises until Phase 8; move each module's tests
with it or into a sibling `#[cfg(test)]` block.

**Guardrails for every phase:**

1. Snapshot the current `lib.rs` under
   `.superpowers/sdd/2026-09-23-nooklink-agent-workflow/snapshots/split-<phase>-before/`.
2. Move code with exact line ranges only when the boundaries are exact;
   otherwise move whole items (function/impl/struct) with their doc comments
   and attributes.
3. After each phase: `cargo fmt --all`, `cargo test -p tuner-app`, then full
   battery + smoke test before proceeding. One snapshot diff per phase is the
   durable record (this workspace has no Git).

### Phase 0 — Baseline + snapshot (no moves)

Snapshot current state. Record the full battery results as the split baseline.
Measurement only.

### Phase 1 — Move the tests module (already complete on 2026-09-25)

Cut lines 16593–23316 (`#[cfg(test)] mod tests`) into
`crates/tuner-app/src/tests.rs` and declare `#[cfg(test)] mod tests;` in
`lib.rs`.

- Zero risk: tests reference crate items through the same paths; the only
  wiring is the `mod` declaration.
- `lib.rs` drops from 23.3k → ~16.6k lines in one step.
- Verify: full battery + smoke test.

### Phase 2 — preferences.rs

Move the types from lines ~92–450 (TableUiScale, LayoutPreset, ThemeMode,
UiDensity, TableDisplay, TableColorMode/Settings, TableAxisRole,
BrowserOrganization, TableWindowMemory) plus the preference persistence block
(~1237–1443: `preferences_to_json` … `save_json_atomically`, incl.
`default_shortcuts`, settings paths, and the `AppPreferences`/
`ProjectPreferences` impls at 279–1132) into `preferences.rs`.

- These items depend only on serde/egui primitives and each other.
- Re-export everything from `lib.rs`; fix intra-crate references via
  `use crate::preferences::…` where needed (or rely on the re-exports).
- Verify: full battery + smoke test.

### Phase 3 — commands.rs

Move lines 1445–2200 (BuiltinCommand, CommandTarget, CommandDescriptor,
CommandEntry, CommandRegistry impl, `command_availability`,
`cell_selection_availability`, `parameter_selection_availability`) into
`commands.rs`. Dependencies: `WorkspaceState` (via `crate::workspace`), no
`TunerApp`, no `egui::Ui`.

- Verify: full battery + smoke test.

### Phase 4 — workspace.rs (+ conversion if it stays coupled)

Move lines 2200–4095: `WorkspaceError`, `CellView`, `AxisSelection`,
`CellSelection`, `CellOperation`, `WorkspaceState` + impl, clipboard parsing,
`raw_editor_text`, `parse_raw_editor_value`, `raw_hex_value_for_parameter`,
`FormulaSample`/`FormulaPreview`, `ConversionTarget` +
`document_conversion`/`set_document_conversion`. If `ConversionTarget` and its
free functions pull heavy dependencies, keep them in `workspace.rs` and skip a
separate `conversion.rs`.

- This is the phase with the most intra-module references; do it after
  preferences/commands so `use crate::{preferences::…, commands::…}` resolves.
- `TunerApp` methods keep calling `self.workspace.…` unchanged.
- Verify: full battery + smoke test.

### Phase 5 — format.rs + shortcuts.rs (leaf helpers)

Move the small leaf helpers into two tiny modules: `format.rs` gets
`format_f64_with_precision`, `format_f64`, `table_axis_header_text`, the
`*_name` functions, `inspector_row`, `raw_value_text`, `table_ui_scale`,
`effective_table_compare_mode`, `compare_mode_label`,
`parse_hex_display_format`/`parse_hex_endianness`/`parse_hex_color_mode`,
`quick_action_button`, `scale_margin`, `scale_table_style`; `shortcuts.rs` gets
`ParsedShortcut`/`parse_shortcut`/`parse_key`/`shortcut_matches`.

- Tiny modules, no risk, but they unblock reading the big `TunerApp` impl
  because the file stops mixing leaf utilities with orchestration.
- Verify: full battery + smoke test.

### Phase 6 — TunerApp method groups (the big one, split into 6 sub-steps)

Rust allows multiple `impl TunerApp` blocks across files, so each module gains
its own `impl TunerApp { … }` block and `lib.rs` keeps only the struct +
constructor + `eframe::App`. Sub-steps, each independently verifiable:

- **6a. `ipc.rs`** — move `poll_ui_ipc`, `conversion_target_from_request`,
  `conversion_state_value`, `surface_state_value`, `hex_state_value`,
  `map_finder_state_value`, `handle_ui_ipc_request` (~2.0k lines, the most
  self-contained big chunk).
- **6b. `operations_ui.rs`** — `maybe_start_search`/`start_search`,
  `install_operation_result`, `poll_background_operations`,
  `invalidate_search_after_bin_change` (~0.4k).
- **6c. `debug_report.rs`** — `debug_report_text` + `export_debug_report` +
  `start_debug_report_export` (~0.6k).
- **6d. `app_shell.rs`** — `show_toolbar`, menus (`show_recent_bins_menu`,
  `show_cells_menu`), `show_browser(+contents)`, `show_workspace_canvas`,
  `show_surface_windows(+window)`, `handle_shortcuts`, `command_button`,
  `apply_visual_preferences`, `persist_*` family, `shortcut_for`,
  `show_operation_overlay`, `show_xdf_reuse_prompt`,
  `show_startup_restore_prompt` (~2.0k).
- **6e. `table_window.rs` + `inspector.rs`** — `show_parameter_editor`,
  `show_table_window`, `apply_table_arrangement`, `show_table_tabs`, plus the
  table color/geometry/drag helpers → `table_window.rs`;
  `show_inspector(+contents)`, `show_formula_editor`, `show_diagnostics` →
  `inspector.rs`.
- **6f. `windows.rs` + `nooklink_ui.rs` + remaining views** —
  `show_command_palette`, `show_action_history_window`,
  `show_debug_report`/`raise_debug_report`, `show_settings` → `windows.rs`;
  `show_nooklink_setup` + challenge/proposal methods → `nooklink_ui.rs`;
  `show_compare_window(+surface)` → `compare_view.rs`; `show_hex_editor` + hex
  interactions → `hex_view.rs`; `show_map_finder` → `map_finder_view.rs`;
  `show_search_window` → `search_view.rs`.

After 6f, `lib.rs` retains only: mod decls, re-exports, `TunerApp` struct +
`new`/`headless`, `eframe::App` impl. Target ≤ ~1.5k lines.

### Phase 7 — Verification sweep

- Full battery: `cargo fmt --all -- --check`, `cargo test -p tuner-app`,
  `cargo test --workspace`, `cargo build --release -p tuner-app`.
- `bash scripts/smoke-test.sh --app` (launch + close the GUI).
- `cargo clippy --workspace --all-targets` must not exceed the current 39
  warnings; no new warnings allowed.
- `cargo doc -p tuner-app --no-deps` must show no broken intra-doc links.

### Phase 8 — (optional, later) test distribution

Optionally move each module's unit tests next to the code (they currently all
live in one `tests.rs`). Only if a module's tests don't share private helpers;
otherwise keep the single `tests.rs`. Keep the 10-repeat focused-test process
rule in mind while re-running suites.

## Risk register

| Risk | Mitigation |
|---|---|
| Invisible behavior change during moves | Moves only; no signature/logic edits; diff review per phase against snapshots |
| Visibility errors (`pub(crate)` vs private) | Expect compile errors, fix by widening to `pub(crate)` only — never change runtime behavior |
| Circular references between modules | Resolve with `crate::` paths; same-crate modules may import each other freely |
| Test breakage from moved items | Re-exports keep every path stable; tests move wholesale in Phase 1 |
| Losing the review trail | Per-phase snapshots + `review-split-<phase>.diff` under `.superpowers/sdd/…` |
| egui id-stability changes | Keep `egui::Id`-producing helpers byte-identical — id changes can alter UI state recall |
| Focused-test repetition limits | The 10-repeat rule still applies; the battery is identical each phase |

## Explicit non-goals

- No renaming of public items, no API reshaping, no `pub(crate)` tightening
  beyond what the compiler forces.
- No logic changes, no "while we're here" refactors.
- No behavior change to IPC, persistence, or challenge gating.
- No splitting of `agent_tasks.rs`, `ui_ipc.rs`, or the existing helper modules.

## Estimated outcome

| Phase | lib.rs after phase (approx) |
|---|---|
| start | 23,316 |
| 1 (tests out) | ~16,600 |
| 2–5 (types/helpers out) | ~12,500 |
| 6a–6f (TunerApp impl out) | ~1,500 |

Total: 9–11 new files, each 300–2,400 lines, every phase green. The battery +
smoke test after each phase is what makes "no behavior change" verifiable
rather than aspirational.
