# TunerNook — Agent Working Notes

Clean-room, Windows-first BIN/XDF calibration editor. Cargo workspace: `tuner-core`,
`tuner-xdf`, `tuner-cal`, `tuner-transfer`, `tuner-cli`, `tuner-api`, `tuner-app`
(egui/eframe desktop shell). Backend crates are dependency-free; see `README.md`
for architecture, safety rules, and build commands.

## Workspace facts (updated 2026-09-27)

- The local folder is a Git checkout for the public `TunerNook-V1` project.
  `origin` points to `https://github.com/inoukt/TunerNook-V1`; `upstream` points
  to the previous `TunerNook` repository and must not receive pushes.
- The public repository excludes the local SCGa05 calibration BIN/XDF pair,
  extracted VBF definitions, photos, logs, build output, `.freebuff`, and local
  `.superpowers` snapshots. Do not stage everything blindly; review staged files.
- Eight tests use the local-only SCGa05 fixture pair and are ignored by default.
  With authorized local fixture files present under `Test bin and xdf/`, run
  `cargo test --workspace -- --include-ignored` to include them. The smoke script
  also requires that fixture pair; it is not distributed.
- Durable source history is in Git. SDD plans/specs live under
  `docs/superpowers/`; detailed snapshots and per-run ledgers under
  `.superpowers/sdd/` are local-only.
- Implementation follows **superpowers SDD**: plan in
  `docs/superpowers/plans/`, design spec in `docs/superpowers/specs/`, and a
  per-plan execution ledger in `.superpowers/sdd/<plan>/progress.md`. Always read
  the relevant ledger before resuming a plan.
- Process rules referenced by ledgers: identical focused test commands may not be
  rerun more than 10 times in one session (see Task 5 process violation in the
  NookLink ledger), and completed plans get their `- [ ]` checkboxes ticked.
- Verification battery used by every task: `cargo fmt --all -- --check`,
  `cargo test -p tuner-app`, `cargo test --workspace`,
  `cargo build --release -p tuner-app`.
- Runtime smoke test: `bash scripts/smoke-test.sh` (CLI + JSONL API against the
  local-only `Test bin and xdf/SCGa05_cal.*` fixtures; add `--app` to also launch,
  verify, and close the release GUI). 21 checks, exits nonzero on failure, and
  asserts the input BIN is never modified. Requires `python` on PATH.

## Current status: NookLink agent workflow — COMPLETE

Plan: `docs/superpowers/plans/2026-09-23-nooklink-agent-workflow.md` (all 24
checkboxes ticked; ledger:
`.superpowers/sdd/2026-09-23-nooklink-agent-workflow/progress.md`).

All five tasks are implemented and reviewed, including both fix rounds:

1. `crates/tuner-app/src/agent_tasks.rs` — typed task/proposal state model
   (`AgentTaskStore`, statuses, quick vs. long/complex levels, task-scoped
   raw-read authorization, stale marking, bounded decision history).
2. `crates/tuner-app/src/ui_ipc.rs` — additive `tuner-ui/v1` messages:
   `agent_task` (start/progress/submit/poll/fail) and bounded `read_bin_bytes`
   (64 KiB cap, task + BIN-identity gated).
3. Tools → NookLink setup/task window with in-app arithmetic challenges for
   long/complex starts, raw reads, proposal apply, and NookLink-requested Save
   As; quick tasks and ordinary reads need no challenge. Setup docs:
   `docs/nooklink-agent-setup.md`; protocol docs: `docs/tuner-ui-ipc.md`.
4. Proposal review/apply: document identity (BIN hash + XDF hash +
   `workspace_data_revision`) is rechecked before review and apply; a changed or
   missing identity marks the proposal `Stale` and never silently rebases.
   Approved edits apply through exactly one `BinDocument::transaction`.
5. Bounded project-scoped decision history (max 50 per BIN, no tokens, no
   conversations, unvalidated agent evidence prose excluded) persisted in
   project preferences and summarized in the debug report
   (`[NOOKLINK DECISIONS]` section).

Post-completion follow-up (by Buffy, 2026-09-23): closed the deferred Task 1
minor by adding terminal-transition tests `failed_task_is_terminal_and_rejects_late_updates`
and `accepted_proposal_rejects_late_agent_updates_before_and_after_apply` to
`agent_tasks.rs`; delta recorded in the `review-final-followup.diff` section of
`review-final.diff` (pre-change snapshot in
`snapshots/final-followup-before/`). Final whole-branch review recorded in the
ledger with no open findings.

Last full verification: app tests 265/265, workspace 340/340, fmt clean,
release build links. Runtime smoke (2026-09-23): all `tuner-cli` commands pass
on the real `Test bin and xdf/SCGa05_cal.*` fixtures, a scripted `tuner-api`
JSONL session (capabilities/inspect/inspect_xdf/xdf_validate/plan_edit) succeeds
and rejects unknown actions, and the release `tuner-app` launches, stays live,
and closes on WM_CLOSE with empty stdout/stderr.

Hex editor polish (Buffy, 2026-09-23): selected hex cells now draw a prominent
2.0 px white contour stroke so selections are visible at a glance (mapped-but-
unselected cells keep their 1.5 px green stroke). Recorded in the
`2026-09-22-raw-hex-editor` ledger with snapshot and diff. Note: a concurrent
`2026-09-23-nooklink-agent-quick-start` session ran in parallel; its test
rewrite landed mid-verification and is included in the counts above.

## Current status: XDF Maker / Editor — COMPLETE

Plan: `docs/superpowers/plans/2026-09-24-xdf-maker-editor.md`; design:
`docs/superpowers/specs/2026-09-24-xdf-maker-editor.md`; execution ledger and
before/after snapshots: `.superpowers/sdd/2026-09-24-xdf-maker-editor/`.

- `tuner-xdf` exposes validated typed draft operations for add/edit/delete/reorder,
  header/categories, parameter storage/layout, conversions, and axes. Export
  reparses and retains unknown XML, category extensions, and multiple category
  memberships. Reordering is limited to siblings in the same XML container;
  cross-container moves fail closed.
- **Tools → XDF Maker / Editor** opens the movable/resizable in-memory draft.
  New, Map Finder, Hex selection, existing table, table Cells menu, and UI IPC
  actions converge on the same editor. Edits have undo/redo, raw BIN preview,
  loaded-BIN range validation, dirty-close protection, and explicit non-overwriting
  Save XDF As. Finder/Hex-prefilled definitions remain reviewable suggestions.
- NookLink adds the `xdf_authoring` capability and typed operations. The user
  reviews and challenge-applies proposals to the draft; saving requires a second
  challenge bound to source identity and the exact draft revision. No arbitrary
  XML or BIN writes are accepted in that capability.
- Authoring supports signed/unsigned integers through 64 bits and IEEE binary32.
  Float64 XDF storage is not emitted without a verified format marker. Checksum
  and auxiliary-object editing remains deferred.

Final verification (2026-09-24): app 283/283, workspace 365/365, XDF crate
41 unit + 3 real-fixture tests, formatting clean, optimized app build succeeds,
and release GUI smoke passes 21/21 including graceful close.

## What is deliberately NOT here (future slices, per plan)

- Checksum-provider panels (next natural capability slice on the app spine).
- Runtime add-on/plugin loading; separately activated Live Helper — both require
  separate opt-in designs before implementation.
- Agent installation/launch/selection; arbitrary code execution; raw byte-write
  proposal operations.

## Safety invariants to preserve in any change

- Never overwrite an input BIN/XDF; output paths must be explicit and new.
- All BIN mutations go through `tuner-core` transactions (undo/redo stays
  consistent); the UI never touches raw buffers directly.
- Agent-facing surfaces can read state freely but can never approve themselves:
  challenges, approvals, and review decisions are app-side only, and challenge
  answers never appear in IPC state or persisted history.

## Current: staged tuner-app production module split

The test module is already extracted to `crates/tuner-app/src/tests.rs` by the
completed `2026-09-25-render-efficiency-and-lib-split` plan. Do not repeat that
move. The old line-based proposal at
`docs/superpowers/specs/2026-09-23-tuner-app-module-split.md` is historical; its
line ranges and first phase are stale. Current work is tracked in
`docs/superpowers/plans/2026-09-27-dirty-state-and-app-module-split.md` with a
separate design spec and SDD ledger. Production code is being split in small,
cohesive, behavior-preserving slices, starting with table coloring/Sweep.

Hex/Finder navigation follow-up (2026-09-24): corrected virtual-row scroll pitch
so Map Finder inspection and exact-value Hex search display the selected high-
offset cell instead of an earlier region. The `6010.00` big-endian float32
regression resolves to `0x1EFA58`. See
`.superpowers/sdd/2026-09-22-raw-hex-editor/progress.md`. Latest verification:
app 285/285, workspace 367/367, formatting clean, optimized app build and
release GUI smoke 21/21.

XDF Editor menu follow-up (2026-09-24): “XDF Maker / Editor” is available in
the Tools dropdown through its existing command. A headless menu interaction
test opens the item and confirms it launches the editor. Latest verification:
app 286/286, workspace 368/368, formatting clean, optimized app build and
release GUI smoke 21/21. See
`.superpowers/sdd/2026-09-24-xdf-maker-editor/progress.md`.

Table toolbar wrapping follow-up (2026-09-24): the active-cell edit controls
and quick actions now occupy separate wrapping rows, keeping them accessible
in narrower table windows. Pointer interaction tests target cells using the
rendered headers rather than fixed offsets. Latest verification: app 287/287,
workspace 369/369, formatting clean, optimized app build and release GUI smoke
21/21. See `.superpowers/sdd/2026-09-14-table-interaction/progress.md`.

Hex Editor robust auto-color follow-up (2026-09-24): Auto range now uses
visible finite values with 2nd–98th percentile clipping by default; Whole BIN
and Fixed range remain available. NookLink can inspect/change the scope through
the existing hex state/coloring actions. Latest verification: app 292/292,
workspace 374/374, formatting clean, optimized app build, and release GUI smoke
21/21. See `.superpowers/sdd/2026-09-22-raw-hex-editor/progress.md`.

Recent XDF follow-up (2026-09-24): File → Recent XDFs mirrors Recent BINs with
up to 16 newest-first paths, success-only recording, disabled missing entries,
and a clear action. The list is visible in read-only NookLink UI state. Latest
verification: app 295/295, workspace 377/377, formatting clean, optimized app
build, and release GUI smoke 21/21. See
`.superpowers/sdd/2026-09-24-recent-xdf/progress.md`.

## Current status: screen-safe dock/workspace snapshots — COMPLETE

Plan: `docs/superpowers/plans/2026-09-24-window-management-dock.md`; design:
`docs/superpowers/specs/2026-09-24-window-management-dock-design.md`; execution
ledger and before/final snapshots: `.superpowers/sdd/2026-09-24-window-management-dock/`.

- The slim bottom dock now manages tables, surfaces, Search, Map Finder, Hex,
  Compare/Compare 3D, XDF Editor, Action History, Debug, Settings, and NookLink.
  Dock focus determines foreground priority; minimizing keeps a window's state,
  and dirty XDF drafts retain the save/discard close confirmation.
- Named layouts reuse the existing per-BIN project preference store as the
  active state and keep only inactive snapshots. Utility geometry is saved only
  for windows without existing typed memories. Startup restores after the
  existing BIN/XDF consent flow; session-only XDF draft and Action History
  windows are skipped and reported.
- NookLink exposes stable window IDs and UI-only focus/minimize/restore/close
  plus workspace create/switch/rename/delete actions. `state` reports open
  windows and active/saved layouts. These actions do not write BIN/XDF data.

Final verification (2026-09-24): formatting clean; `cargo test -p tuner-app`
330/330; `cargo test --workspace` 412/412 including supplied-XDF fixture tests;
optimized app build succeeds; release GUI smoke 21/21 with graceful close. A
visible release-app walkthrough through the NookLink bridge opened the supplied
BIN/XDF fixture, created and switched layouts with table/surface/Map Finder
windows, restarted into the saved `Table Focus` layout, and verified the fixture
BIN/XDF bytes were unchanged. Their hash values are omitted from this public
project note.
The smoke profile used a temporary APPDATA directory; the normal user profile
and fixture bytes were not changed. The app-content geometry clamp reduced the
requested Map Finder rect to the usable shell bounds and that actual rect was
retained with the snapshot.

## Focus/z-order follow-up (2026-09-24)

Workspace dialogs, the command palette, startup/XDF restore prompts, and the
unsaved-XDF confirmation now retain priority over docked windows. Debug's
foreground override and the loading overlay yield to these priority surfaces;
normal focused-window behavior remains unchanged when they are closed. Verified
with layout and command-palette/table overlap regressions plus the full app
suite (333/333) and formatting check; `cargo check --release -p tuner-app` also
passes. After the running app was closed, `cargo build --release -p tuner-app`
completed and refreshed `target/release/tuner-app.exe`. See
`.superpowers/sdd/2026-09-24-window-management-dock/progress.md`.

## Table resize persistence follow-up (2026-09-24)

Manual table resizing now reads egui's actual edge/corner drag responses, so a
user-sized border disables content auto-fit and survives reopening. The
regression simulates a corner drag, checks the saved size/fit mode, then closes
and reopens the table. Verified: fmt clean; app tests 334/334; workspace tests
416/416; optimized release build succeeds. Existing settings that already have
`fit_to_content=true` cannot recover a previously lost larger border; resize
once to the desired size after updating. See the window-management SDD ledger.

## Table window workspace-size follow-up (2026-09-25)

Table windows now use the existing shell-safe bounds, not the central editor
pane, so changing browser/inspector layout across workspaces no longer silently
shrinks a floating table. Tables can overlap those side panes, while staying
below the toolbar and above the dock/diagnostics. If a previously clamped table
is explicitly resized, that new actual rectangle is persisted; passive clamp
still does not overwrite the saved manual geometry. Per-workspace memories
remain independent.

Verified: formatting clean; app 348/348; workspace 430/430; optimized release
build succeeds; Git Bash `scripts/smoke-test.sh --app` passes 21/21 and closes
gracefully. See `.superpowers/sdd/2026-09-25-workspace-size-lifecycle/progress.md`.

## Current status: project/workspace and status UX — COMPLETE

Plan: `docs/superpowers/plans/2026-09-25-project-workspace-ux.md`; design:
`docs/superpowers/specs/2026-09-25-project-workspace-ux-design.md`; ledger and
before/final snapshots: `.superpowers/sdd/2026-09-25-project-workspace-ux/`.

- `.tnproj` manifests reference existing BIN/XDF files and preserve the active
  workspace plus named snapshots; they never contain or write BIN bytes. Open
  Project validates the path identity before replacing the current documents,
  restores through the existing async loaders, and opens BIN-only with a clear
  warning when its optional XDF is missing. Save Project/Save Project As keep
  the manifest association in project preferences and retain the existing BIN
  Save As no-overwrite path. The current loaded BIN/XDF cannot be replaced by a
  `.tnproj` save. Generic NookLink dispatch cannot save project manifests until
  a typed, user-approved agent save flow exists.
- Reset Workspace Windows is available from View → Workspace and Arrange. It
  resets active-workspace geometry for open and remembered-closed tables,
  surfaces, search, hex, Map Finder, compare windows, and utility windows, while
  preserving other layouts, zoom/view settings, selections, focus/minimize
  state, and BIN edit history.
- Toolbar action/status groups wrap independently; the empty canvas explains
  the next BIN/XDF action. Browser filtering is labeled List filter, and
  Advanced Search explains `*`/`?` wildcards and the BIN requirement for value
  searches. Edited BINs explicitly say “Unsaved BIN edits” and point to BIN Save
  As; healthy diagnostics are compact, while mapping/XDF issues are emphasized
  with counts and readable summaries.

Final verification (2026-09-25): `cargo fmt --all -- --check` clean;
`cargo test -p tuner-app` 366/366; `cargo test --workspace` 448/448;
`cargo build --release -p tuner-app` succeeds; Git Bash
`scripts/smoke-test.sh --app` passes 21/21, including graceful release-GUI
close and unchanged fixture BIN checksum. The smoke app used a unique temporary
APPDATA profile; the normal user profile and fixture bytes were unchanged.

## Current status: workspace-number canvas backgrounds — COMPLETE

Plan: `docs/superpowers/plans/2026-09-25-workspace-background-image.md`; design:
`docs/superpowers/specs/2026-09-25-workspace-background-image-design.md`; ledger
and before/final snapshots: `.superpowers/sdd/2026-09-25-workspace-background-image/`.

- Global `AppPreferences.workspace_backgrounds` is keyed only by numeric
  workspace ID, so the same workspace number shares its background across BIN
  projects. It is not part of BIN-scoped preferences or `.tnproj` manifests;
  legacy main settings default to no background. Backgrounds can be assigned
  without a BIN loaded.
- Right-clicking blank central canvas opens Set, Fit/Fill, Opacity, and
  conditional Remove options. Existing table/cell context menus retain their
  routing. Images are copied byte-for-byte into the app-settings sibling
  `workspace-backgrounds/` directory with content-addressed names; removing a
  reference does not delete a possibly shared asset. Supported still formats
  are PNG, JPEG, and BMP. If a managed content-addressed path is unreadable or
  contains unexpected bytes, replacement uses a collision-safe suffix and
  preserves the damaged file for any other reference.
- One active texture is cached by workspace ID and relative reference. No-image
  layouts do no image I/O/decode/upload; selection and table-position changes do
  not invalidate it. Missing/corrupt references remain replaceable/removable.
  Texture uploads respect egui's renderer-reported side limit, downscaling only
  the temporary GPU image with aspect ratio preserved; original managed bytes
  remain intact for a higher-resolution renderer.

- NookLink IPC state/actions were not extended for background configuration in
  this slice; agent-side image selection needs a separate file-access/approval
  design. Decode/resize remains synchronous on the one-time image/workspace
  cache miss; move it to the operation coordinator if users observe stalls.

Final verification (2026-09-25): formatting clean; app 385/385; workspace
467/467; optimized release build succeeds; `scripts/smoke-test.sh --app` passes
21/21, including graceful GUI close and unchanged fixture BIN. The smoke used a
temporary APPDATA profile; normal user settings were untouched. Optimized
headless CPU/texture-queue measurements at a 2048-side limit: no-image 2.33 ms
with 0 image work, 1080p first preparation 9.75 ms (repeat 0.024 ms), and 4K
downscaled to 2048×1152 first preparation 273.46 ms (repeat 0.029 ms). These
are not physical GPU timings: adapter discovery found only a dedicated GTX
1070 Ti (8 GiB), with no integrated GPU available for the requested modest/iGPU
renderer check.

## Current status: workflow preferences, Favorites, and shortcuts — COMPLETE

Plan: `docs/superpowers/plans/2026-09-25-workflow-preferences-favorites-shortcuts.md`;
design: `docs/superpowers/specs/2026-09-25-workflow-preferences-favorites-shortcuts-design.md`;
execution ledger and before/final snapshots:
`.superpowers/sdd/2026-09-25-workflow-preferences-favorites-shortcuts/`.

- Sweep is enabled by default and persisted in app preferences. File → Settings,
  the main-screen gear, and Tools → Keyboard Shortcuts route to the same Settings
  window.
- The Parameters browser has a filtered Favorites section containing links to
  definitions in the active XDF. Removing a favorite removes the link/pin but
  leaves any open table window intact.
- Settings includes searchable bindings for every registered command, Record,
  Clear (explicitly unbound), and Reset (restore the command default). The
  recorder accepts supported key chords only while armed and reports conflicts
  without changing either command's binding. Shift+1 through Shift+4 switch to
  workspace slots selected by ascending persistent workspace ID; unavailable
  slots are safe no-ops. App preferences migrate from settings version 3 to 4;
  project settings remain unchanged.
- This phase does not extend NookLink or alter BIN/XDF contents. Maps Search and
  the notepad remain deferred to the separately planned Phase 2.

Final verification (2026-09-25): formatting clean; `cargo test -p tuner-app`
400/400; `cargo test --workspace` 482/482; optimized app build succeeds; Git
Bash `scripts/smoke-test.sh --app` passes 21/21, including graceful GUI close
and unchanged fixture BIN checksum. See the SDD ledger for detailed test
evidence and review rulings.

## Map Finder score calibration follow-up (2026-09-26)

Map Finder directional consistency now measures the difference between rising
and falling edges, rather than awarding half-credit to balanced/random
directions. This corrects the false-high score shown at `0x18F470` without
changing raw values, endian interpretation, scan dimensions, threshold, or axis
assignment. The screenshot candidate recalculates from 88 to 80/100; the prior
coherent 10×10 map at `0x1EF8F0` remains 93/100. Regression:
`extreme_mixed_prefix_does_not_score_like_a_coherent_map`.

Verification (2026-09-26): fmt clean; `cargo test -p tuner-app` 401/401;
`cargo test --workspace` 483/483; optimized release build succeeds;
`scripts/smoke-test.sh --app` passes 21/21 with graceful GUI close and unchanged
fixture BIN checksum. See
`.superpowers/sdd/2026-09-26-map-finder-score-calibration/progress.md`.

## Map Finder Stop and worker feasibility (2026-09-26)

- Map Finder scans can be stopped from the window or through the NookLink
  `cancel_map_search` action. Cancellation is checked in batches of 64 candidate
  offsets; partial candidate results are discarded, progress is not forced to
  100%, and BIN/XDF data is untouched. State reports `cancel_requested` while
  cancellation is pending.
- A throwaway CPU shape-splitting benchmark on a local extracted VBF BIN,
  bytes `0x180000..0x1C0000` (256 KiB, Float32 Big, 12–16 × 12–16, 25 shapes),
  found 31 candidates in each mode. Median serial time was 6.35 s, two workers
  3.72 s (1.71×), and four workers 2.69 s (2.36×), on a 4-worker runtime.
  This is a feasibility result, not a production parallel implementation; GPU
  compute remains deferred because the app has no compute backend.

Verification (2026-09-26): formatting clean; `cargo test -p tuner-app` 406/406;
`cargo test --workspace` 488/488; optimized release build succeeds;
`scripts/smoke-test.sh --app` passes 21/21 with graceful GUI close and unchanged
fixture BIN checksum. See
`.superpowers/sdd/2026-09-26-map-finder-stop/progress.md`.

Map Finder popup Stop follow-up (2026-09-26): the amber background-operation
notice now shows the same Stop/Stopping control beside scan progress as Map
Finder. It reuses `request_map_search_cancel`; the control is scan-only and
disappears for other background work. Regression clicks the popup button,
checks cancellation/status, and verifies XDF loading has no Stop button.
Verified: fmt clean; app 407/407; workspace 489/489; release build succeeds;
Git Bash `scripts/smoke-test.sh --app` passes 21/21. Details and snapshots:
`.superpowers/sdd/2026-09-26-map-finder-stop/progress.md`.

## Project Notepad and XDF overwrite confirmation (2026-09-26)

- XDF Save As still creates new files by default. The Windows native Save
  dialog's single `FOS_OVERWRITEPROMPT` confirmation authorizes replacing an
  existing destination; NookLink's existing approval challenge does the same
  and explicitly warns that the named file will be replaced. Replacements are
  staged and reparsed before the old file is moved; invalid data and
  non-regular destinations are refused, and an unconfirmed overwrite leaves
  the old file unchanged. A successful editor save clears draft dirtiness, so
  closing after save does not ask again.
- Project Notepad is a movable/resizable project utility window, reachable in
  Tools and the dock. Plain-text notes and its Stay on top setting persist in
  BIN-project preferences; legacy settings default to empty/false. Its
  in-app topmost toggle does not affect other desktop applications or BIN/XDF
  contents.
- `scripts/smoke-test.sh --app` now gives the release GUI a temporary `APPDATA`
  profile. This prevents smoke runs from loading or rewriting the user's normal
  settings.

Verification (2026-09-26): formatting clean; app 412/412; workspace 494/494;
optimized release build succeeds. The latest GUI smoke run passed CLI/API and
launch/stdout/stderr checks (20/21 overall) but the app did not exit on
`CloseMainWindow` within the harness's 4-second grace period, even with the
isolated profile; the harness force-stopped only its own child process. An
earlier run passed 21/21, so graceful close remains an intermittent/unresolved
smoke result, not a claimed pass. See
`.superpowers/sdd/2026-09-26-project-notepad-xdf-save/progress.md`.

## Current status: School-Me Mode — COMPLETE

Plan: `docs/superpowers/plans/2026-09-26-school-me-mode.md`; design:
`docs/superpowers/specs/2026-09-26-school-me-mode-design.md`; execution ledger
and before/final snapshots:
`.superpowers/sdd/2026-09-26-school-me-mode/`.

- School-Me Mode is an app-wide preference, off by default, saved at settings
  version 5, exposed in Settings and View → School-Me Mode, and reported in the
  existing read-only NookLink UI state. No IPC actions, protocol versions, or
  dependencies were added.
- `crates/tuner-app/src/school_help.rs` holds 106 typed custom-control
  explanations plus generated help for every registered command. School-Me
  uses one movable, resizable, scrollable help window, drawn once after the
  app's other windows and refreshed by the hovered control. It accepts only
  an unobscured response on the topmost layer (including disabled controls),
  ignores keyboard focus on covered controls, and keeps the last explanation
  readable over blank canvas or the help window itself. Closing it turns the
  mode off. Ordinary tooltip timing is unchanged.
- Help is wired across the app shell, browser/categories/favorites, workspace
  layouts and dock, tables/axes/editors, Search, Map Finder, Hex, Compare/3D,
  XDF Maker / Editor, inspector, debug, notepad, startup prompts, and NookLink.
  Tool-specific copy distinguishes heuristic candidates, raw bytes,
  engineering conversions, in-memory BIN edits, and explicit Save As. Hovering
  help is display-only and never changes BIN/XDF content or approves an agent
  action.

Final verification (2026-09-26): formatting clean; app 437/437; workspace
519/519; optimized release build succeeds; Git Bash
`scripts/smoke-test.sh --app` passes 21/21, including graceful release-GUI
close and unchanged fixture BIN checksum. See the School-Me SDD ledger for
focused interaction/non-mutation tests and source/document snapshot hashes.

## Current status: angle-stable 3D surface drawing — COMPLETE (2026-09-27)

- Surface cells are split into two triangle primitives and painter-sorted by
  average camera-space depth from the same yaw/pitch transform used for point
  projection. This avoids screen-Y ordering errors and invalid convex-quad
  fills at rotated angles. Cell borders remain visible; the diagonal is hidden.
- Both normal and compare 3D views use the shared corrected geometry path.
- App package version is now `0.1.1`; native title shows `TunerNook v0.1.1`.
  Keep `APP_NAME` as `TunerNook` for stable settings paths and increment the app
  patch version on subsequent changes.

Final verification (2026-09-27): formatting clean; app 443/443; workspace
526/526; optimized release build succeeds; Git Bash
`scripts/smoke-test.sh --app` passes 21/21, including graceful versioned-GUI
close and unchanged fixture BIN checksum. See
`.superpowers/sdd/2026-09-27-surface-angle-depth/` for test-first evidence and
source snapshots.

## Current status: first-open zoom and window placement — COMPLETE (2026-09-27)

Plan/spec: `docs/superpowers/plans/2026-09-27-window-open-defaults.md` and
`docs/superpowers/specs/2026-09-27-window-open-defaults.md`; execution ledger
and snapshots: `.superpowers/sdd/2026-09-27-window-open-defaults/`.

- The toolbar's **New table zoom** preference is app-wide, defaults to 100%,
  moves in 10% increments, and clamps to 50–200%. It applies only to a table
  without saved memory; that table's own saved zoom remains authoritative.
- Fresh table, surface, Hex, Map Finder, Search, Compare, and utility windows
  cascade from the central editor canvas. Their initial size is constrained to
  the available canvas, while existing saved/manual geometry stays intact.
  Utility windows continue to use the existing workspace geometry map.
- Typed memories mark whether a position has been captured. Older serialized
  non-default rectangles remain saved; legacy default tool rectangles migrate
  as fresh placement. Compare applies its workspace rectangle once when
  opening/restoring, then returns to ordinary move/resize behavior.
- The native title is now `TunerNook v0.1.2`; `APP_NAME` remains stable. App
  settings schema is version 6 and project settings schema is version 13. No
  BIN/XDF data or dependencies changed.

Final verification (2026-09-27): formatting clean; app 451/451; workspace
534/534; optimized release build succeeds; Git Bash
`scripts/smoke-test.sh --app` passes 21/21, including graceful `v0.1.2` GUI
close and unchanged fixture BIN checksum. An earlier parallel app-suite run
had one transient save-worker timeout; its isolated test and the subsequent
complete app/workspace runs passed. See the SDD ledger for evidence and hashes.

## Current status: shifted digit shortcuts — COMPLETE (2026-09-27)

Plan/spec: `docs/superpowers/plans/2026-09-27-shifted-digit-shortcuts.md` and
`docs/superpowers/specs/2026-09-27-shifted-digit-shortcuts.md`; SDD ledger and
before/final snapshots: `.superpowers/sdd/2026-09-27-shifted-digit-shortcuts/`.

- egui-winit prefers a mapped logical key over the physical key. Shift+1 can
  therefore arrive as logical `Exclamationmark` plus physical `Num1`; TunerNook
  previously matched only the logical key. Digit shortcuts now match either
  logical digits or a non-repeating physical digit event with the requested
  modifiers. The shortcut recorder likewise records the physical digit for
  number keys. Letters retain their prior logical-key behavior.
- The app-side text-focus guard and workspace slot ordering are unchanged. No
  settings schema, BIN/XDF data, or dependencies changed.
- The native title is now `TunerNook v0.1.3`; `APP_NAME` remains stable.

Final verification (2026-09-27): formatting clean; app 453/453; workspace
536/536; optimized release build succeeds; Git Bash
`scripts/smoke-test.sh --app` passes 21/21, including graceful `v0.1.3` GUI
close and unchanged fixture BIN checksum. The red/green regression reproduces
the logical-symbol/physical-digit event and verifies Shift+1 switches back to
the lower-ID workspace; shortcut-recorder capture is covered separately.

## Current status: 3D surface viewport clipping — COMPLETE (2026-09-27)

Plan/spec: `docs/superpowers/plans/2026-09-27-surface-viewport-clipping.md` and
`docs/superpowers/specs/2026-09-27-surface-viewport-clipping.md`; SDD ledger and
before/final snapshots: `.superpowers/sdd/2026-09-27-surface-viewport-clipping/`.

- The reported saved view was yaw 61.47992°, pitch 11.319976°, zoom 199%, with
  wireframe enabled. At that view, projected triangles extend past the inner
  chart rectangle, while the painter previously clipped to the wider plot
  rectangle. This allowed colored faces/edges to streak through the axis-label
  margins.
- Normal and Compare 3D renderers now clip graph geometry to a one-pixel-inset
  chart painter; axis labels remain on the wider plot painter. Large zoom still
  crops the surface at the chart edge, but no longer paints outside the frame.
- Native title is `TunerNook v0.1.4`; `APP_NAME` remains unchanged. No BIN/XDF
  data, settings schemas, or dependencies changed.

Final verification (2026-09-27): formatting clean; app 454/454; workspace
537/537; optimized release build succeeds; Git Bash
`scripts/smoke-test.sh --app` passes 21/21, including graceful `v0.1.4` GUI
close and unchanged fixture BIN checksum. A headless render regression confirms
the exact saved high-zoom view overflows geometrically but its painted faces are
clipped to the chart viewport.

## Current status: constant-time BIN dirty state and staged app split — COMPLETE (2026-09-27)

Plan/spec: `docs/superpowers/plans/2026-09-27-dirty-state-and-app-module-split.md`
and `docs/superpowers/specs/2026-09-27-dirty-state-and-app-module-split.md`;
SDD ledger and snapshots:
`.superpowers/sdd/2026-09-27-dirty-state-and-app-module-split/`.

- `BinDocument` keeps an exact changed-byte count updated on transaction commit,
  undo, and redo. `is_dirty()` and `changed_byte_count()` are now O(1). Net-zero
  transactions and dropped edits preserve the count; editing bytes back to
  their original values correctly returns the BIN to clean.
- Table coloring, palette-menu, range, and Sweep helpers now live in
  `crates/tuner-app/src/table_colors.rs`. Existing public crate-root helpers
  remain available; the moved function bodies match the before snapshot apart
  from visibility and formatting.
- The old 2026-09-23 line-based split proposal is marked historical. Tests are
  already in `tests.rs`; future production extraction should use small,
  cohesive slices tracked by their own plan and ledger.
- App version/title is now `0.1.5`. No dependencies, BIN/XDF bytes, or UI/IPC
  behavior changed.

Final verification (2026-09-27): formatting clean; core tests 12/12; app tests
455/455; workspace tests 539/539; optimized release build succeeds; runtime
smoke passes 21/21 (including graceful release GUI close and unchanged fixture
BIN checksum).

## Current status: first beta release — IN PROGRESS (2026-09-27)

Plan/spec: `docs/superpowers/plans/2026-09-27-first-beta-release.md` and
`docs/superpowers/specs/2026-09-27-first-beta-release.md`.

- Release candidate is `0.1.6-beta.1`, following the saved `v0.1.5` baseline.
- Keep MIT. The GitHub prerelease must contain only the Windows x64 app
  executable; never attach BIN/XDF fixtures.
