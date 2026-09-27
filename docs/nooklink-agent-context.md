# TunerNook and NookLink — agent context

This file is a compact reference for an external agent that the user has chosen
and configured. TunerNook is a Windows-first BIN/XDF calibration editor.
NookLink is TunerNook's local, authenticated, agent-neutral communication and
control layer. It is not an agent, does not choose or launch one, and does not
grant general computer, filesystem, shell, or network access.

TunerNook is free, open-source software licensed under MIT. The project
maintainer does not warrant or take responsibility for work performed by an
external agent or outcomes from using TunerNook. The user must independently
review agent findings and all proposed or saved calibration changes.

## How to get oriented

1. Read the live `capabilities` response and current `state`; they are the
   source of truth for the running app's available actions and open documents.
2. Use only the advertised actions on the configured local protocol. Keep the
   endpoint token private; never include it in reports, prompts, or logs.
3. If using the task workflow, send `agent_task` Start with a clear capability
   and goal. Wait for the user to approve starting it in TunerNook. Report
   progress with short phases, then Submit findings with evidence or Fail with
   a useful reason. The user reviews the result in the app.

## Current feature areas

The live desktop protocol (`tuner-ui/v1`) can expose document and app state,
diagnostics, parameter/table browsing and editing, conversions and XDF saving,
BIN comparison and transfer planning, 3D surfaces, the raw hex editor, the 2D
Map Finder, and an XDF Maker / Editor. The editor can start from a new draft,
an existing XDF definition, a Map Finder candidate, or a Hex Editor selection.
Exact commands vary by app version; query `capabilities` and `xdf_editor_state`
instead of assuming an action exists. The file-oriented `tuner-api/v1` exposes
its own capabilities for inspection, comparison, XDF validation, transfer, and
edits.

Hex Editor auto coloring defaults to a robust percentile range computed from
visible values; `set_hex_coloring` can switch to a whole-BIN range or use
explicit fixed thresholds.

For XDF work, use the dedicated `xdf_authoring` task capability and submit
typed definition operations: add/replace/delete/reorder parameters, update
header metadata/categories, or edit supported axes/storage/conversions. The
app displays each proposal for review and applies it only to the in-memory XDF
draft after the user completes the in-app approval step. A separate,
revision-bound `save_xdf_as` request requires another approval. Never send XML
fragments or imply a definition was saved before TunerNook reports success.
Supported authoring storage is signed/unsigned integer through 64 bits and
IEEE binary32; do not invent a Float64 XDF encoding.

Map Finder candidates and axis suggestions are heuristic evidence, not proof
that a candidate is a real calibration map or axis. Describe uncertainty and
show the evidence so the user can verify it.

## User approval and safety

- The user selects/configures the external agent and explicitly asks it to do
  the work. TunerNook never starts the agent on its own.
- A task starts pending. The user starts or cancels it, then accepts or rejects
  submitted findings in TunerNook. Never treat a submitted proposal as already
  approved or applied. XDF definition proposals change only the editor draft
  after approval; a separate user-approved Save As is needed to write an XDF.
- Long/complex task starts, direct BIN byte-range reads, proposal Apply, and
  NookLink-requested BIN/XDF Save As require an in-app challenge. Wait for the
  user; do not attempt to solve or bypass it. Quick tasks and ordinary
  state/table/map inspection do not require that challenge.
- Use only the app's validated operations. Do not claim NookLink grants
  arbitrary file access, shell execution, or network access. Do not infer that
  an edit was saved unless TunerNook reports success.
- Provide concise findings, confidence/uncertainty, and relevant semantic IDs
  or byte ranges. Ask before expanding the user's goal or access scope.

## Detailed references

When the TunerNook source tree is available, see `docs/nooklink-agent-setup.md`
for user setup and safety details, and `docs/tuner-ui-ipc.md` for the desktop
protocol, action schemas, and examples.
