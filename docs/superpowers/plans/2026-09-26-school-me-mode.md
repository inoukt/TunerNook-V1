# School-Me Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an optional, off-by-default School-Me Mode that gives every actionable TunerNook control immediate, plain-language hover and keyboard-focus help.

**Architecture:** Store one global boolean in `AppPreferences` and expose a registered toggle command plus read-only UI state. Keep educational copy and a reusable egui tooltip renderer in a small `school_help.rs` module; annotate all command and custom-control surfaces in `lib.rs`, with headless interaction and coverage tests in `tests.rs`.

**Tech Stack:** Rust, egui 0.36, serde, existing TunerNook command registry and UI IPC.

**Spec:** `docs/superpowers/specs/2026-09-26-school-me-mode-design.md`

## Global Constraints

- School-Me Mode is a global app preference, off by default; legacy app settings also deserialize it as off.
- An enabled mode shows one large bubble at a time immediately on pointer hover or keyboard focus, without an intentional dwell delay; it remains open while the pointer is over the control or bubble or the control has keyboard focus.
- Bubbles stay within the app viewport, long explanations scroll internally, and the style uses readable contrast without sound or flashing.
- Hover/focus help is display-only and cannot trigger commands or change BIN, XDF, undo history, workspace state, or agent approval state.
- Use the existing `tuner-ui/v1` command/state paths; add no new IPC action, protocol, provider-specific behavior, or dependency.
- Cover actionable controls and distinct interactions in the current UI; static labels and display-only numbers are not separate help targets.
- Never claim that an accepted tune or edit is safe merely because TunerNook accepts it.
- With mode off, current UI behavior and ordinary tooltips remain unchanged.
- No `.git` directory exists. Use the plan's SDD ledger and before/final source snapshots instead of commits.

## Review Focus

1. Old settings and default preferences leave School-Me Mode off; switching it through each UI route must stay synchronized. Task 1 tests this.
2. Only the hovered/focused control owns the single bubble; it stays open while the pointer enters the bubble and has no dwell delay. Tasks 2–4 test this.
3. A large bubble near any screen edge remains usable in small viewports, and long copy scrolls. Task 2 tests this.
4. Disabled controls still explain both their function and the disabling reason. Tasks 3–4 test this.
5. Help must never mutate BIN/XDF content or the data revision. Task 4 tests this on representative editing surfaces.

---

### Task 1: Persist and expose the mode

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` (`AppPreferences`, command registry/dispatch, Settings, View menu, `ui_ipc_state`)
- Test: `crates/tuner-app/src/tests.rs`

**Interfaces:**
- Produces `AppPreferences.school_me_mode: bool`, default `false`.
- Produces the registered command ID `view.toggle-school-me-mode`.
- Adds read-only `school_me_mode` to the existing UI IPC state response.

- [x] **Step 1: Write failing preference and control-path tests.**

Add `school_me_mode_defaults_off_for_new_and_legacy_settings`, verifying a
legacy settings object without the field deserializes to `false` and new
preferences also start `false`. Add `school_me_mode_round_trips_and_is_exposed_to_nooklink`, toggling through the command and Settings UI, then checking the saved value and `ui_ipc_state`.

- [x] **Step 2: Run tests and verify the expected failures.**

Run: `cargo test -p tuner-app school_me_mode_`
Expected: FAIL because the preference, command, Settings control, and state field do not exist.

- [x] **Step 3: Implement the preference and toggle routes.**

Add `school_me_mode` to `AppPreferences`, set its default to `false`, and bump
`SETTINGS_VERSION` from 4 to 5. Register `view.toggle-school-me-mode`, add it
to View and the appearance section of Settings, and include the current value
in UI IPC state. Use existing dispatch; do not add an IPC action.

- [x] **Step 4: Run the focused tests and verify they pass.**

Run: `cargo test -p tuner-app school_me_mode_`
Expected: all new tests pass, including old-settings migration and state agreement.

### Task 2: Add the typed help catalog and bubble renderer

**Files:**
- Create: `crates/tuner-app/src/school_help.rs`
- Modify: `crates/tuner-app/src/lib.rs` (module declaration and tooltip style selection)
- Test: `crates/tuner-app/src/school_help.rs` and `crates/tuner-app/src/tests.rs`

**Interfaces:**
- Produces borrowed `SchoolHelpCopy<'a> { title: &'a str, what_it_does: &'a str, when_to_use: Option<&'a str>, keep_in_mind: Option<&'a str> }`, `ALL_CUSTOM_HELP_IDS`, and exhaustive `custom_help(SchoolHelpId) -> &'static SchoolHelpCopy<'static>` content for custom widgets.
- Produces `command_help(descriptor: &CommandDescriptor) -> SchoolHelpCopy<'_>` for every registered command, reusing its human-readable label/description plus a category-based usage hint and curated caution text for high-risk commands.
- Produces `show_for_response(ctx: &egui::Context, response: &egui::Response, enabled: bool, help: &SchoolHelpCopy<'_>)`, called only when School-Me Mode is enabled and the widget is hovered or keyboard-focused. It uses `egui::Tooltip::for_widget(response)` so help opens as soon as the hovered/focused response is rendered.
- Produces `configure_tooltip_style(ctx: &egui::Context, enabled: bool)`, setting egui's tooltip delay to zero only while the mode is on and restoring the library default when it is off.

- [x] **Step 1: Write failing catalog and tooltip behavior tests.**

Add `school_help_command_help_covers_all_registered_commands` and
`school_help_custom_catalog_covers_every_id_with_complete_copy`. Add
`school_help_bubble_shows_immediately_and_remains_hoverable` and
`school_help_shows_only_one_bubble_at_a_time`, and
`school_help_bubble_clamps_and_scrolls`, plus
`school_help_tooltip_delay_is_zero_only_when_mode_is_on`, verifying pointer
hover, keyboard focus, leaving the control, pointer travel into the bubble,
viewport edges, and long copy.

- [x] **Step 2: Run the tests and verify they fail for missing help behavior.**

Run: `cargo test -p tuner-app school_help_`
Expected: FAIL because the catalog and School-Me renderer are missing.

- [x] **Step 3: Implement the catalog and renderer.**

Define stable help IDs, the `ALL_CUSTOM_HELP_IDS` slice, and the three optional
copy sections in `school_help.rs`. For commands, use their registered
description as the specific explanation, a short category-based usage hint,
and curated cautions for save/edit/raw-value actions; this avoids duplicating
the command catalog. Use the already-installed
`egui::Tooltip::for_widget` path for the shared bubble; call it only for a
hovered or focused response while the preference is on. Configure egui's
tooltip delay to zero for School-Me Mode and restore its default while mode is
off so pointer travel and scroll remain reliable without changing normal use.
Give the bubble a large preferred width (about 420 points), clamp it to the
available viewport, keep long copy scrollable, and use a theme-aware readable
card without sound or flashing. Do not add a dependency.

- [x] **Step 4: Run the focused tests and verify they pass.**

Run: `cargo test -p tuner-app school_help`
Expected: help catalog coverage and immediate hover/focus behavior pass.

### Task 3: Cover the app shell, browser, and tables

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` (command buttons, menus, toolbar, dock, Settings, browser, inspector, workspace dialogs, tables and axes)
- Test: `crates/tuner-app/src/tests.rs`

**Interfaces:**
- Consumes Task 2's `SchoolHelpId`, `command_help`, and `show_for_response`.
- Every custom interactive control in these surfaces supplies a typed help ID; every command button resolves help from its stable command ID.

- [x] **Step 1: Add failing shell/browser/table interaction tests.**

Add `school_me_command_button_explains_hovered_command`,
`school_me_custom_browser_control_explains_hover`,
`school_me_table_cell_explains_selection_copy_and_edit`,
`school_me_disabled_command_explains_block_reason`, and
`school_me_mode_off_preserves_existing_tooltips`. Cover browser
filter/sort/search controls, table cell selection and multi-cell copy/paste,
axis-header actions, Settings, workspace, and dock.

- [x] **Step 2: Run the tests and verify the missing annotations fail.**

Run: `cargo test -p tuner-app school_me_`
Expected: FAIL on controls not yet routed through the shared help renderer.

- [x] **Step 3: Attach School-Me help to shell, browser, and table controls.**

Use `command_help` from the common command-button path. Attach explicit custom
help IDs to menu items, toggles, selectors, text fields, cell/axis widgets,
dock actions, and workspace dialogs. All table cells may reuse the same
selection/edit/copy explanation; do not create per-cell copy entries. Preserve
normal-mode tooltips and every widget's existing interaction.

- [x] **Step 4: Run the focused tests and verify they pass.**

Run: `cargo test -p tuner-app school_me_`
Expected: the shell/browser/table help tests pass with mode on and off.

### Task 4: Cover specialist tools and safe explanations

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` (Search, Map Finder, Hex Editor, Compare, 3D surfaces, XDF Maker / Editor, NookLink, and remaining dialogs)
- Modify: `crates/tuner-app/src/school_help.rs` (tool-specific educational copy)
- Test: `crates/tuner-app/src/tests.rs`

**Interfaces:**
- Reuses Task 2's typed help catalog and renderer and Task 3's command integration.
- Adds shared help IDs for tool-specific gestures and high-risk controls; no command semantics change.

- [x] **Step 1: Add failing tool-surface and non-mutation tests.**

Add `school_me_specialist_windows_explain_actions`,
`school_me_disabled_tool_actions_explain_reason`, and
`school_me_help_is_display_only_for_bin_and_xdf`. Cover Search, Map Finder,
Hex, Compare, 3D graphs, XDF authoring, and NookLink. Verify tooltips about
edits and Save As accurately distinguish in-memory edits from writing output.

- [x] **Step 2: Run the tests and verify missing tool annotations fail.**

Run: `cargo test -p tuner-app school_me_`
Expected: FAIL for tool controls that are not annotated yet.

- [x] **Step 3: Add curated tool help and annotate specialist controls.**

Cover each documented action and distinct gesture in Search, Map Finder, Hex,
Compare/3D, XDF Maker / Editor, and NookLink. Define raw-value, axis,
heuristic map, challenge, and Save As terms accurately; warnings must never
claim a tune or edit is safe merely because an operation succeeds.

- [x] **Step 4: Run tool tests and verify help is display-only.**

Run: `cargo test -p tuner-app school_me_`
Expected: all tool-surface tests pass and the tested document bytes/revisions
remain unchanged.

### Task 5: Coverage audit, documentation, and full verification

**Files:**
- Modify: `AGENTS.md` (completed feature status and verification results)
- Create: `.superpowers/sdd/2026-09-26-school-me-mode/progress.md`
- Snapshot: changed source and project-document files under the SDD directory

- [x] **Step 1: Audit all registered commands and custom controls.**

Run the help-catalog coverage test and review every interactive surface listed
in the spec. Confirm no registered command or custom interactive control lacks
help, all mode routes agree, and School-Me content introduces no writes or
agent approvals.

- [x] **Step 2: Run formatting and all project verification.**

Run: `cargo fmt --all -- --check`, `cargo test -p tuner-app`,
`cargo test --workspace`, and `cargo build --release -p tuner-app`. Confirm no
TunerNook process is running before replacing the release executable. Run
`bash scripts/smoke-test.sh --app` using the installed Git Bash executable if
the Windows `bash` shim resolves to WSL.

Expected: formatting clean, all tests pass, release build succeeds, and GUI
smoke passes all 21 checks with graceful close and unchanged fixture BIN.

- [x] **Step 3: Record the result and snapshots.**

Record test counts, build/smoke output, review findings, and any limitation in
`.superpowers/sdd/2026-09-26-school-me-mode/progress.md`. Save before/final
snapshots of each changed file and verify final SHA-256 values match the live
files. Do not add a commit step; this workspace has no Git metadata.
