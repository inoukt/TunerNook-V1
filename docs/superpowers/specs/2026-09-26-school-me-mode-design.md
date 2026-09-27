# School-Me Mode — Design

**Status:** Conversational design approved 2026-09-26; awaiting written-spec review.

## Goal

Add an optional, app-wide teaching mode that explains TunerNook's actionable
controls in clear, respectful, elementary-school-level language through large,
immediate hover and keyboard-focus bubbles. The mode teaches the UI without
changing any calibration, document, or command behavior.

## User experience

- `School-Me Mode` is a global app preference, **off by default**. Legacy app
  settings also deserialize it as off.
- Users can toggle it in Settings, View, and the command palette through the
  registered `view.toggle-school-me-mode` command. The current setting is
  included in existing read-only `tuner-ui/v1` state; no new IPC action or
  protocol is introduced.
- When enabled, hovering an actionable control or keyboard-focusing it opens
  one large bubble immediately, without an intentional dwell delay. It closes
  after the pointer leaves both the control and its bubble and keyboard focus
  leaves the control.
- Bubbles stay within the app viewport, anchor near their control, and scroll
  internally when their explanation is too long. They use a colorful, friendly
  card style with readable contrast; there is no sound, flashing, or tutorial
  overlay.
- Each explanation uses the relevant subset of these headings: **What it
  does**, **When to use it**, and **Keep in mind**. Wording is simple and
  encouraging, not baby talk. Technical words are briefly defined. Tuning and
  raw-data help must not promise that a tune or edit is safe merely because the
  app accepted it.
- Disabled controls explain both their purpose and why they are unavailable.
  When the mode is off, existing app behavior and ordinary tooltips remain
  unchanged.

## Coverage and content

Cover every actionable control and distinct interaction in the current app:
menus and commands, toolbar and dock, settings, browser and inspector, table
and axis editing, cell selection/copy/paste, 3D graph interaction, Compare,
Search, Map Finder, Hex Editor, XDF Maker / Editor, workspace dialogs, and
NookLink. Every table cell shares context-aware help for cell actions rather
than maintaining duplicate prose for each value. Static labels and
display-only numbers are not separate help targets.

Use a shared School-Me help component and stable help identifiers. Registered
commands may reuse their existing descriptions as technical detail, but must
also resolve to plain-language School-Me content. Every custom interactive
widget uses the shared helper with an explicit help identifier; do not invent
explanations from button labels. Tests and review must make missing command and
custom-control help discoverable.

Example voice:

> **What it does:** Opens the BIN calibration file so you can inspect a working
> copy. **Keep in mind:** Use Save As to create an output file; the original
> file is not silently overwritten.

## Settings and agent control

Store `school_me_mode: bool` in global `AppPreferences`, defaulting to `false`,
and bump the app-preference version. Register `view.toggle-school-me-mode` so
the setting is discoverable in the UI command catalog and can be toggled using
existing command dispatch. Add a read-only `school_me_mode` field to UI IPC
state. This preserves the existing agent-control pattern without adding
provider-specific behavior or granting data-write authority.

## Accessibility and safety

- Keyboard focus receives the same help as pointer hover.
- Bubble placement is clamped to the viewport; long copy remains readable via
  scrolling.
- Hover/focus help is display-only and cannot trigger commands or change BIN,
  XDF, undo history, workspace state, or agent approval state.
- The copy should distinguish viewing/editing the in-memory document from
  saving an output and should be especially careful around raw values, axes,
  map detection, and unverified XDF definitions.

## Tests and acceptance

- New/default and legacy settings leave the mode off; enabling it round-trips
  through app settings.
- Settings, View, command palette/dispatch, and UI IPC state agree on the
  selected mode.
- With the mode off, School-Me bubbles do not appear. With it on, pointer entry
  and keyboard focus show help immediately for representative controls from
  each major tool area; disabled controls show their reason.
- Moving the pointer from a control into its help bubble keeps it open.
- Bubble placement remains inside a small viewport and long help remains
  scrollable.
- A coverage audit verifies every registered command and every registered
  custom help identifier resolves to non-empty educational copy.
- Help-only interaction leaves BIN/XDF bytes and document revisions unchanged.
- Run `cargo fmt --all -- --check`, `cargo test -p tuner-app`,
  `cargo test --workspace`, `cargo build --release -p tuner-app`, and
  `bash scripts/smoke-test.sh --app` following the workspace verification
  policy. Because the repository has no Git metadata, record execution with its
  SDD plan/ledger and source snapshots instead of a commit.

## Non-goals

- An onboarding wizard, guided tour, narrated lessons, or persistent help
  sidebar.
- Changing command availability, BIN/XDF processing, calibration semantics,
  external agent protocols, or approval rules.
- Automatically generated or AI-authored explanations at runtime.
