# Workflow Preferences, Favorites, and Shortcuts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist the requested Sweep default, make favorite maps easy to reopen, and provide discoverable Settings and reliable command/workspace shortcuts.

**Architecture:** Extend the existing global `AppPreferences`, Settings utility window, parameter browser, command registry, and named-workspace switch path. Keep favorites as stable references and workspace-slot resolution as a small pure helper; do not add a second store, window, or shortcut subsystem.

**Tech Stack:** Rust workspace, `eframe`/`egui`, `serde`, existing `tuner-app` headless UI tests.

**Spec:** `docs/superpowers/specs/2026-09-25-workflow-preferences-favorites-shortcuts-design.md`

## Global Constraints

- BIN/XDF bytes, identities, and document revisions remain unchanged; no BIN/XDF writes are part of this feature.
- Sweep and keybindings are app-global `AppPreferences`; favorites remain in their existing XDF/project-aware stable-key store.
- Workspace shortcut slots resolve at invocation time from existing workspace IDs sorted numerically; they do not depend on the active-first menu order or workspace names.
- Settings changes use the existing preference persistence path; empty shortcut overrides remain explicit unbound values.
- Preserve every existing command ID and default chord; new slot IDs are `workspace.slot.1` through `workspace.slot.4`.
- Increment app `SETTINGS_VERSION` from 3 to 4 for the Sweep preference and shortcut-default migration; do not change `PROJECT_SETTINGS_VERSION` (12).
- No new dependencies, global OS hotkeys, or NookLink protocol/agent-write changes.
- This checkout has no Git metadata; capture before/final snapshots and update the SDD ledger instead of creating commits.
- Do not run any identical focused test workflow more than 10 times in a session.
- At each task boundary run `cargo fmt --all -- --check`, `cargo test -p tuner-app`, `cargo test --workspace`, and `cargo build --release -p tuner-app`; do not terminate a running user app to replace the release executable.

## Files and Responsibilities

- Modify `crates/tuner-app/src/lib.rs`: `AppPreferences`, Sweep rendering state, command descriptors/dispatch, Settings entry points and shortcut editor, Favorites rendering.
- Modify `crates/tuner-app/src/workspace_layouts.rs`: pure ID-sorted workspace-slot lookup and focused unit tests.
- Modify `crates/tuner-app/src/tests.rs`: preference migration, browser interactions, settings routing, and shortcut capture/dispatch regressions.
- Modify `AGENTS.md`: record delivered behavior and final verification counts after implementation.
- Add no dependency and no parallel state store; current modules remain authoritative.

## Review Focus

- A legacy settings file has a partial shortcut map: add only missing new defaults; preserve custom values and explicit empty/unbound overrides. Test: `legacy_shortcut_overrides_gain_workspace_defaults_without_overwriting_custom_or_empty` (Task 3).
- Active-first workspace display order changes while switching: shortcut slots still use sorted persistent IDs, including non-contiguous IDs. Test: `workspace_shortcut_slots_use_stable_numeric_id_order` (Task 3).
- A favorite is removed while its table is open: remove only the browser link and update the pin state, not the table window. Test: `unfavoriting_removes_link_but_keeps_open_table` (Task 2).
- A key recorder sees typing in another field or an invalid/duplicate chord: do not consume unrelated text or silently reassign the binding. Tests: `shortcut_capture_requires_armed_recorder` and `shortcut_capture_rejects_invalid_or_conflicting_chords` (Task 4).
- A slot is absent or workspace switching is blocked by an operation: no panic, stale target, or partial workspace change. Test: `workspace_slot_shortcut_respects_slot_availability_and_switch_guard` (Task 3).

---

### Task 1: Persist Sweep and make Settings reachable

**Files:**
- Modify: `crates/tuner-app/src/lib.rs`
- Test: `crates/tuner-app/src/tests.rs`

**Interfaces:**
- Consumes: existing `AppPreferences`, `view.keyboard-settings` command, `WindowId::Settings`, and selected-cell animation render paths.
- Produces: `AppPreferences.sweep_enabled: bool`, default `true`; toolbar Sweep reads/writes this field. File → Settings and the gear button route to the existing Settings window; Tools → Keyboard Shortcuts opens/focuses that same window and requests its shortcut section.

- [x] **Step 1: Write failing tests**

  Add `legacy_preferences_default_sweep_is_enabled_and_sweep_setting_round_trips` to assert settings version 3 JSON with no `sweep_enabled` loads enabled and upgrades to version 4, and that false/true values survive serialization. Add `sweep_toolbar_toggles_and_saves_app_preference` to click Sweep in a headless toolbar frame and verify the existing preference file receives the disabled value. Add `settings_entry_points_focus_one_existing_window` to click File → Settings, the gear button, and Tools → Keyboard Shortcuts in headless frames; assert all target the same `WindowId::Settings`, with the last action selecting the shortcut section.

- [x] **Step 2: Run the focused tests and observe expected failures**

  Run: `cargo test -p tuner-app legacy_preferences_default_sweep_is_enabled_and_sweep_setting_round_trips -- --nocapture`, `cargo test -p tuner-app sweep_toolbar_toggles_and_saves_app_preference -- --nocapture`, and `cargo test -p tuner-app settings_entry_points_focus_one_existing_window -- --nocapture`.

  Expected: first fails because the preference and persistent toggle do not exist; second fails because the requested entry points/section targeting are not wired.

- [x] **Step 3: Implement persisted Sweep and shared Settings routes**

  Add `sweep_enabled` with default `true` to `AppPreferences`; increment `SETTINGS_VERSION` from 3 to 4 and leave `PROJECT_SETTINGS_VERSION` at 12. Replace `TunerApp.animate_selected_cell_colors` with the preference as the single source of truth, and persist the toolbar toggle. Route File and gear actions through the existing Settings command/window. Add Tools → Keyboard Shortcuts as a focus/section request on that same utility; make the section request scroll the existing settings content into view without creating another window.

- [x] **Step 4: Verify this task**

  Rerun both focused tests, then run the full verification battery from Global Constraints. Update this task's evidence in `.superpowers/sdd/2026-09-25-workflow-preferences-favorites-shortcuts/progress.md`.

### Task 2: Add linked Favorites to the Parameters browser

**Files:**
- Modify: `crates/tuner-app/src/lib.rs`
- Test: `crates/tuner-app/src/tests.rs`

**Interfaces:**
- Consumes: existing favorite keys, current-XDF catalog summaries, list filter, `toggle_favorite`, and `open_table`.
- Produces: a collapsible Favorites section above the category tree whose entries are references resolved from the current XDF; no cloned table state or copied parameter data.

- [x] **Step 1: Write failing browser tests**

  Add `favorites_section_renders_filtered_links_from_current_xdf` to assert only resolvable current-XDF favorites render and the existing quick filter applies. Add `unfavoriting_removes_link_but_keeps_open_table` to favorite/open a parameter, remove its favorite through the table pin path, and assert its browser favorite entry disappears while its open table remains and becomes unpinned.

- [x] **Step 2: Run the focused tests and observe expected failures**

  Run: `cargo test -p tuner-app favorites_section_renders_filtered_links_from_current_xdf -- --nocapture` and `cargo test -p tuner-app unfavoriting_removes_link_but_keeps_open_table -- --nocapture`.

  Expected: both fail because the browser currently has no dedicated Favorites section.

- [x] **Step 3: Render linked favorite entries**

  Resolve favorite keys against the current catalog/XDF fingerprint, apply the current browser filter, and render a collapsible section before categories. Reuse existing star actions, stable table keys, selection state, and `open_table`; do not remove stale persisted favorites or close a window when it is unfavorited.

- [x] **Step 4: Verify this task**

  Rerun both focused tests, then run the full verification battery from Global Constraints. Record that adding/removing a favorite does not change BIN bytes, XDF identity, or document revision in the SDD ledger.

### Task 3: Add stable workspace slots and shortcut binding rules

**Files:**
- Modify: `crates/tuner-app/src/lib.rs`
- Modify: `crates/tuner-app/src/workspace_layouts.rs`
- Test: `crates/tuner-app/src/tests.rs`

**Interfaces:**
- Consumes: `ProjectPreferences.active_workspace_id`, `saved_workspace_snapshots`, existing guarded `switch_workspace`, `CommandDescriptor`, and `AppPreferences.shortcuts`.
- Produces: `workspace_id_for_slot(project: &ProjectPreferences, slot: u8) -> Option<u64>` resolving existing IDs in ascending numeric order; four command IDs `workspace.slot.1`…`.4`; digit-aware parsing for supported chords. Task 4 adds conflict comparison when the recorder consumes it.

- [x] **Step 1: Write failing model/command tests**

  Add `workspace_shortcut_slots_use_stable_numeric_id_order` with a non-contiguous ID set and different active IDs; assert the same slot mapping each time. Add `workspace_slot_shortcut_respects_slot_availability_and_switch_guard`; assert missing slots are safely disabled/no-op and a busy operation leaves the active layout unchanged. Add `legacy_shortcut_overrides_gain_workspace_defaults_without_overwriting_custom_or_empty`; preserve a custom binding and an empty override while populating missing slot defaults. Add `workspace_shortcut_chords_parse_digit_keys` to cover Shift+1…Shift+4, existing letter/function-key chords, invalid input, equivalent chord spellings, and the four registered slot defaults.

- [x] **Step 2: Run the focused tests and observe expected failures**

  Run: `cargo test -p tuner-app workspace_shortcut_slots_use_stable_numeric_id_order -- --nocapture`, `cargo test -p tuner-app workspace_slot_shortcut_respects_slot_availability_and_switch_guard -- --nocapture`, `cargo test -p tuner-app legacy_shortcut_overrides_gain_workspace_defaults_without_overwriting_custom_or_empty -- --nocapture`, and `cargo test -p tuner-app workspace_shortcut_chords_parse_digit_keys -- --nocapture`.

  Expected: missing workspace-slot lookup/commands/default migration fail to compile or assert; parser cases for number keys fail.

- [x] **Step 3: Implement slot lookup, command descriptors, and binding validation**

  Implement the pure ID-sorted lookup in `workspace_layouts.rs`; add stable workspace-slot descriptors and a `SwitchWorkspaceSlot(u8)` command routed through the existing guarded switch method. Add Shift+1…Shift+4 defaults. During preference load, insert defaults only for absent workspace-slot override IDs, preserving present custom and empty values. Extend `parse_key` for digit keys and derive equality for parsed chords. Defer the conflict comparator until Task 4, where the Settings recorder consumes it.

- [x] **Step 4: Verify this task**

  Rerun the focused slot/migration/parser tests, then run the full verification battery from Global Constraints. Verify command descriptors are visible to the command palette and continue to dispatch through the normal command registry.

### Task 4: Build the shortcut recorder and finish documentation

**Files:**
- Modify: `crates/tuner-app/src/lib.rs`
- Test: `crates/tuner-app/src/tests.rs`
- Modify: `AGENTS.md`

**Interfaces:**
- Consumes: slot commands/defaults and chord-conflict rules from Task 3; existing Settings utility, command registry, and preference persistence.
- Produces: explicit armed-recorder state, command search, record/clear/reset actions, conflict feedback, and the new user-facing Settings/shortcut workflow.

- [x] **Step 1: Write failing interaction tests**

  Add `shortcut_capture_requires_armed_recorder` to show ordinary typing is untouched until Record is clicked and an armed chord is captured. Add `shortcut_capture_rejects_invalid_or_conflicting_chords` to assert conflicting bindings remain unchanged and the UI identifies the owner. Add `shortcut_clear_sets_explicit_unbound_override`, `shortcut_reset_clears_override_to_default`, and `shortcut_editor_lists_every_command_palette_action`; assert all settings persist using the existing preferences path.

- [x] **Step 2: Run the focused interaction tests and observe expected failures**

  Run each new test by exact name with `cargo test -p tuner-app <test_name> -- --nocapture`, including `shortcut_clear_sets_explicit_unbound_override`.

  Expected: the current text-only shortcut fields do not implement explicit recording, conflict feedback, reset, or armed-input routing.

- [x] **Step 3: Implement the recorder in the existing Settings utility**

  Add a searchable view over every registered command descriptor. Capture key events only while one command's Record action is armed; serialize supported chords consistently, reject invalid or conflicting chords without mutating the binding map, and keep ordinary text-input focus behavior unchanged. Provide Clear (store an explicit empty override) and Reset (remove the override so the descriptor/default map applies). Show the stable ID-to-name mapping for workspace slots 1–4. Persist successful changes immediately through the existing settings path.

- [x] **Step 4: Update project notes and finish the verification battery**

  Add `global_preference_edits_preserve_saved_workspace_snapshots_and_document_identity` to prove direct Sweep/shortcut preference edits do not change BIN/XDF bytes, identity, revision, or saved inactive workspace-view snapshots. Update `AGENTS.md` with the delivered feature and test counts. Run focused shortcut tests, `cargo fmt --all -- --check`, `cargo test -p tuner-app`, `cargo test --workspace`, and `cargo build --release -p tuner-app`. Once the app is closed, run `bash scripts/smoke-test.sh --app` and confirm the fixture BIN checksum is unchanged. The Settings window itself may update its own current dock/open state through existing lifecycle behavior.

- [x] **Step 5: Capture final snapshots and self-review**

  Save final snapshots for every changed source/config/doc file under the SDD `final/` directory, compare their SHA-256 hashes to current files, and review spec coverage, settings migration, favorite link ownership, shortcut conflict behavior, and workspace switching. Record any deferred minor and exact verification output in the ledger. No commit step: `.git` is absent.
