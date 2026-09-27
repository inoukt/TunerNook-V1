# TunerNook UI control (`tuner-ui/v1`)

For user-led agent setup, task approval, raw-read authorization, and Save As confirmation, see [NookLink agent setup](nooklink-agent-setup.md).

TunerNook exposes a local-only JSONL bridge for development tools and AI agents. It binds to
`127.0.0.1` on an ephemeral port and writes the port plus a per-launch token to
`ui-ipc-v1.json` beside the normal TunerNook settings file. The bridge is not reachable from
other machines.

Read the manifest, connect to its `host` and `port`, and send one JSON object per line. Every
request must include the manifest `protocol` and `token`:

```json
{"protocol":"tuner-ui/v1","token":"…","request_id":"1","action":"state"}
```

The first useful calls are:

```json
{"protocol":"tuner-ui/v1","token":"…","request_id":"2","action":"capabilities"}
{"protocol":"tuner-ui/v1","token":"…","request_id":"3","action":"debug_report"}
{"protocol":"tuner-ui/v1","token":"…","request_id":"4","action":"dispatch","command":"view.debug-report"}
```

Supported direct actions currently include `open_bin`, `open_xdf`, `save_as`, `save_xdf_as`,
`open_table`, `select_cells`, `focus_table`, `close_table`, `open_surface`, `focus_surface`,
`reset_surface_view`, `open_compare_bin`, `open_compare_xdf`, `close_compare`,
`set_compare_mode`, `set_compare_filter`, `build_transfer_plan`,
`approve_transfer_address`, `apply_transfer_plan`, `set_filter`, `set_search_query`,
`use_last_xdf`, `keep_current_xdf`, `get_conversion`, `set_conversion_override`, and
`reset_conversion_override`. Dock windows and workspace snapshots are available through
`focus_window`, `minimize_window`, `restore_window`, `close_window`, `workspace_create`,
`workspace_switch`, `workspace_rename`, and `workspace_delete`. XDF authoring is available through `open_xdf_editor`,
`xdf_editor_state`, `create_xdf_from_map_candidate`, and
`create_xdf_from_hex_selection`. Hex editing is available through `open_hex_editor`,
`focus_hex_editor`, `close_hex_editor`, `hex_state`, `set_hex_address`,
`set_hex_display`, `set_hex_coloring`, `search_hex`, `select_hex_range`, and
`write_hex`. The 2D Map Finder is controllable through `open_map_finder`,
`focus_map_finder`, `close_map_finder`, `map_search_state`, `search_maps`,
`cancel_map_search`,
`set_map_finder_view`, `set_map_finder_coloring`, `select_map_candidate`,
`inspect_map_candidate`, `assign_map_axis`, and `clear_map_axis`. `dispatch` is the
stable path for registered UI commands; its
optional `edit_value` and `clamp` fields populate the same edit controls used by the UI.

## Window and workspace control

`state` and `snapshot` expose currently open dock targets as `windows`, each with a stable `id`,
`kind`, `title`, `open`, `minimized`, and `focused` fields. They also expose the active workspace
ID/name and `workspaces`, including the active layout and saved snapshots. Stable window IDs are
`search`, `map_finder`, `hex_editor`, `compare`, `compare_surface`, `xdf_editor`, `debug_report`,
`settings`, `nooklink`, plus `table:<table-key>`, `surface:<surface-key>`, and
`history:<table-key>`.

Use `focus_window`, `minimize_window`, `restore_window`, or `close_window` with `window_id` to
control an open dock target. These actions change presentation only. Closing a dirty XDF editor
does not bypass its in-app save/discard confirmation. Workspace actions use `workspace_id` and/or
`workspace_name`: `workspace_create` returns the new ID/name, `workspace_switch` activates a saved
layout, `workspace_rename` changes its name, and `workspace_delete` removes it. Layout changes do
not reload or modify BIN/XDF documents; invalid IDs, duplicate names, last-layout deletion, and
changes during document operations are rejected. Workspace snapshots are scoped to the active BIN
project. On restart, the active layout is restored only after the existing BIN/XDF consent prompt
and document load; BIN-only restore waits for its matching XDF. Unsaved XDF drafts and Action
History are session-only, skipped on restart, and reported in Diagnostics.

```json
{"protocol":"tuner-ui/v1","token":"…","request_id":"w1","action":"focus_window","window_id":"table:<table-key>"}
{"protocol":"tuner-ui/v1","token":"…","request_id":"w2","action":"workspace_create","workspace_name":"Track"}
{"protocol":"tuner-ui/v1","token":"…","request_id":"w3","action":"workspace_switch","workspace_id":1}
```

Compare mode accepts `destination`, `source`, `delta`, and `percent_delta`. The compare BIN is
read-only. Transfer planning requires an exact source/destination XDF hash match, and applying a
ready plan changes the active destination only through one undoable transaction; Save As remains
required to publish a new BIN.

```json
{"protocol":"tuner-ui/v1","token":"…","request_id":"13","action":"open_compare_bin","path":"C:/tuning/source.bin"}
{"protocol":"tuner-ui/v1","token":"…","request_id":"14","action":"set_compare_mode","command":"delta"}
{"protocol":"tuner-ui/v1","token":"…","request_id":"15","action":"build_transfer_plan","selected_semantic_ids":["table:uid:map"]}
{"protocol":"tuner-ui/v1","token":"…","request_id":"16","action":"apply_transfer_plan"}
```

The `state`/`snapshot` response includes a `compare` object with source/destination paths and
hashes, the active graph mode, selected semantic/cell range, transfer selection, plan status,
changed-byte count, and blocking issue codes/messages.

The response also includes `xdf_editor`, the in-memory authoring draft, with its revision,
dirty state, XDF hash, categories, parameter definitions, storage fields, and axes. Use
`create_xdf_from_map_candidate` with a `candidate_index` or
`create_xdf_from_hex_selection` to open a prefilled draft; Finder and axis values are suggestions
and must be verified before applying them as definitions. These actions do not write an XDF.
Top-level state also exposes `recent_bins` and `recent_xdfs` as newest-first path lists (up to 16
entries each), matching the File menu so an agent can discover recent documents without opening
or changing them.

Conversion actions address a parameter by its exact `semantic_id`; add `axis_index` to address
one of that parameter's axes:

```json
{"protocol":"tuner-ui/v1","token":"…","request_id":"5","action":"get_conversion","semantic_id":"table:uid:map"}
{"protocol":"tuner-ui/v1","token":"…","request_id":"6","action":"get_conversion","semantic_id":"table:uid:map","axis_index":0}
{"protocol":"tuner-ui/v1","token":"…","request_id":"7","action":"set_conversion_override","semantic_id":"table:uid:map","formula":"X * 2"}
{"protocol":"tuner-ui/v1","token":"…","request_id":"8","action":"reset_conversion_override","semantic_id":"table:uid:map"}
```

Formula overrides are validated with TunerNook's existing conversion grammar, stored in the
active BIN project's settings, and do not modify BIN bytes or BIN undo history. Responses include
source/effective formulas plus parse, sample-evaluation, and inversion status. `save_xdf_as`
requires a `path` and task ID. A running task may save the unchanged active XDF; an applied
`xdf_authoring` task may save only the exact draft revision it produced. It stages the destination
in the NookLink window and does not start saving until the user completes a fresh arithmetic
challenge; document identity and draft revision are checked again before saving. The worker refuses
to overwrite an existing path and reports success only after reparsing the written output:

```json
{"protocol":"tuner-ui/v1","token":"…","request_id":"9","action":"save_xdf_as","task_id":"<running-task-id>","path":"C:/tuning/edited.xdf"}
```

`dispatch` rejects `file.save-as` and `file.save-xdf-as`; use the task-bound `save_as` or
`save_xdf_as` action. Native user-initiated Save As dialogs remain available in the desktop UI.

## NookLink tasks and decisions

Users configure and invoke their own external agent. `agent_task` starts a task pending user
approval; the app does not launch an agent. The returned `task_id` is used for subsequent calls:

```json
{"protocol":"tuner-ui/v1","token":"…","request_id":"t1","action":"agent_task","agent_task":{"kind":"start","capability":"diagnostics","goal":"inspect the active map"}}
{"protocol":"tuner-ui/v1","token":"…","request_id":"t2","action":"agent_task","agent_task":{"kind":"progress","task_id":"<task-id>","phase":"scan","progress_percent":40,"message":"checking candidates"}}
{"protocol":"tuner-ui/v1","token":"…","request_id":"t3","action":"agent_task","agent_task":{"kind":"submit","task_id":"<task-id>","report":{"summary":"reviewed map","findings":[],"confidence":0.8,"evidence":[{"summary":"table location","semantic_id":"table:uid:map","byte_range":[288,320]}],"operations":[]}}}
{"protocol":"tuner-ui/v1","token":"…","request_id":"t4","action":"agent_task","agent_task":{"kind":"poll","task_id":"<task-id>"}}
{"protocol":"tuner-ui/v1","token":"…","request_id":"t5","action":"agent_task","agent_task":{"kind":"fail","task_id":"<task-id>","message":"inspection failed"}}
```

Use Fail instead of Submit when the agent cannot complete a running task. Progress accepts an
optional percentage from 0 to 100. Poll returns the current task record, including `status`;
the user starts or cancels the task and accepts or rejects submitted findings in the NookLink
window. A proposed cell edit is reviewed and applied only through the app's validated path.
Quick tasks and ordinary state/table/map inspection are puzzle-free. User-marked Long/complex
starts, direct raw byte-range reads, proposal Apply, and NookLink-requested BIN/XDF Save As
require a fresh in-app arithmetic challenge. It is a consent checkpoint, not proof of human
presence, and challenge prompts and answers never appear in IPC.

`xdf_authoring` is a dedicated capability. It accepts typed definition operations—not XML—for
adding/replacing/deleting/reordering definitions, changing header/categories, and setting axes,
storage, strides, and conversions. Reordering is supported within the same source XML container;
cross-container moves are rejected rather than moving definitions across unknown/vendor wrappers.
The user reviews the proposed changes in NookLink; Apply changes only the in-memory draft. A
second, revision-bound `save_xdf_as` challenge is required to write it. Supported authored storage
is signed/unsigned integer through 64 bits and IEEE binary32; Float64 XDF encoding is not emitted
unless a compatible encoding is verified.

Example `AgentReport.operations` entry for a new table:

```json
{
  "XdfAuthoring": {
    "operations": [{
      "action": "add_parameter",
      "definition": {
        "unique_id": "agent-load-map",
        "kind": "table",
        "title": "Load Map",
        "description": "",
        "category": "Fuel",
        "xdf_address": 288,
        "element_width_bits": 16,
        "rows": 8,
        "columns": 12,
        "signed": false,
        "endianness": "little",
        "numeric_kind": "integer",
        "column_major": false,
        "row_stride_bits": 192,
        "column_stride_bits": 16,
        "conversion": "X",
        "bit_offset": null,
        "bit_width": null,
        "bit_mask": null,
        "axes": []
      }
    }]
  }
}
```

`read_bin_bytes` takes `task_id`, `byte_offset`, and `byte_length`. It reads the active in-memory
BIN only, rejects paths, requires a running task with matching BIN identity and task-bound
in-app authorization, and returns hex for a nonempty range of at most 65,536 bytes within the
BIN. The first unauthorized request asks the user for the challenge; retry after authorization:

```json
{"protocol":"tuner-ui/v1","token":"…","request_id":"t6","action":"read_bin_bytes","task_id":"<task-id>","byte_offset":288,"byte_length":32}
{"protocol":"tuner-ui/v1","token":"…","request_id":"t7","action":"save_as","task_id":"<running-task-id>","path":"C:/tuning/candidate.bin"}
```

Project settings retain the latest 50 bounded decision summaries per BIN identity, including
the captured workspace data revision as provenance. They do not retain the task goal, full
report, conversation, endpoint token, credentials, or challenge. Historical revisions do not
authorize writes. The debug report exposes concise decision provenance.

Surface actions address a table by its exact `semantic_id`. `open_surface` opens or focuses the
table's interactive 3D surface, `focus_surface` brings an already-open surface to the front, and
`reset_surface_view` restores its default camera, range, and display settings:

```json
{"protocol":"tuner-ui/v1","token":"…","request_id":"10","action":"open_surface","semantic_id":"table:uid:map"}
{"protocol":"tuner-ui/v1","token":"…","request_id":"11","action":"focus_surface","semantic_id":"table:uid:map"}
{"protocol":"tuner-ui/v1","token":"…","request_id":"12","action":"reset_surface_view","semantic_id":"table:uid:map"}
```

Hex editor actions use zero-based BIN byte offsets. Hex writes accept complete
byte pairs separated by whitespace or commas, including optional `0x` prefixes.
Each accepted `write_hex` request is one undoable edit; invalid text or a range
past the BIN end leaves the in-memory BIN unchanged.

```json
{"protocol":"tuner-ui/v1","token":"…","request_id":"17","action":"open_hex_editor"}
{"protocol":"tuner-ui/v1","token":"…","request_id":"18","action":"set_hex_address","byte_offset":288}
{"protocol":"tuner-ui/v1","token":"…","request_id":"19","action":"set_hex_display","display_format":"float32","endianness":"little","decimal_places":3}
{"protocol":"tuner-ui/v1","token":"…","request_id":"20","action":"set_hex_coloring","color_mode":"auto_range"}
{"protocol":"tuner-ui/v1","token":"…","request_id":"21","action":"search_hex","query":"0.5","forward":true}
{"protocol":"tuner-ui/v1","token":"…","request_id":"22","action":"select_hex_range","byte_offset":288,"byte_length":4}
{"protocol":"tuner-ui/v1","token":"…","request_id":"23","action":"write_hex","byte_offset":288,"hex":"DE AD BE EF"}
```

`display_format` accepts `bytes`, `u8`, `i8`, `u16`, `i16`, `u32`, `i32`,
`u64`, `i64`, `float32`, or `float64`; `endianness` is `little` or `big`.
`search_hex` scans byte patterns in Bytes mode and exact typed values in numeric
modes, wrapping to the start/end of the BIN. Numeric scans use values aligned
from byte offset zero; byte patterns can match at any offset. `set_hex_coloring` accepts `off`,
`auto_range`, or `fixed_range`, plus optional `[min,max]` `color_range` and RGBA
`color_low`, `color_middle`, and `color_high` arrays. Auto range uses
`color_range_scope: "visible_percentile"` by default, based on finite values in
the visible Hex Editor rows and clipped to the 2nd–98th percentiles when at
least eight values are visible; smaller views use their exact finite min/max.
Outliers saturate at the palette ends. Set `color_range_scope: "whole_bin"`
for a stable range based on every finite value in the BIN. Fixed range
continues to use the explicit `color_range` thresholds. `hex_state` reports
`color_range_scope` and the current `observed_color_range`.

```json
{"protocol":"tuner-ui/v1","token":"…","request_id":"24","action":"set_hex_coloring","color_mode":"auto_range","color_range_scope":"whole_bin"}
```

`search_maps` runs a background scan for likely 2D grids outside byte ranges
already described by the active XDF. Use `map_rows_min`, `map_rows_max`,
`map_columns_min`, and `map_columns_max` (2–64) to scan a range such as 4×4
through 12×12; the four bounds may include at most 128 shape combinations.
Legacy `map_rows` and `map_columns` still request one exact shape. Other fields
are `display_format` (numeric formats only), `endianness`, `minimum_score`
(0–100), `skip_mapped`, and `map_every_byte`. `map_start_address` and
`map_end_address` are hexadecimal strings; a blank end means BIN end.
`scan_all: true` ignores the address fields and scans the entire BIN while
keeping the selected type, endian, and size bounds. By default, starts use
natural element-width alignment for speed; `map_every_byte` also checks
unaligned offsets and can take longer on large BINs. It opens the finder and
returns immediately; poll `map_search_state` or
`state` for completion and ranked candidate details. Candidate scores are a
heuristic, not an XDF mapping or a safety/validity assertion. Nearby axis
suggestions are unverified monotonic raw sequences; the monotonicity score
alone does not establish that a sequence is a real calibration axis. The
preview therefore keeps `C0`/`R0` labels by default. Review raw values, then use
the UI's **Use X**/**Use Y** controls or the agent-control actions below to opt
into displaying a suggestion as an axis. This changes preview labels only and
never writes BIN or XDF data. Use
`select_map_candidate` with a zero-based `candidate_index` to preview one, or
`inspect_map_candidate` to open its selected cell (or first element) in the Hex
Editor; optional `row` and `column` select a cell within that candidate.
While a scan is running, `cancel_map_search` requests cooperative cancellation;
the worker stops at bounded intervals, discards partial candidates, and reports
`cancel_requested` in `map_search_state` until completion. Calling it while idle
returns an error. Cancellation changes no BIN/XDF data.
`assign_map_axis` requires the current zero-based `candidate_index`,
`map_axis_index`, and `axis_role` (`x` or `y`). The selected suggestion must
match that map dimension. `clear_map_axis` requires `candidate_index` and
`axis_role`. Candidate state reports `selected_x_axis` and `selected_y_axis` as
suggestion indexes or `null`. This is a display-only, per-scan assignment;
axes stored farther from the table may not be detected.
`set_map_finder_view` accepts `map_zoom_percent` (50–200),
`candidate_pane_width` (220–900), `map_position` `[x,y]`, and `map_size`
`[width,height]`; geometry is clamped to the current workspace viewport.
`set_map_finder_coloring` accepts the same `color_mode`, `color_range`, and
RGBA palette fields as `set_hex_coloring`, plus `decimal_places`. The
`state`/`snapshot` response includes `map_finder` settings, window
geometry, operation status, selected candidate, score/range/type, and current
candidate results including axis suggestions and `progress_percent` while a
scan is running. The same determinate percentage is shown in the Map Finder and
the app's background-operation notice.

```json
{"protocol":"tuner-ui/v1","token":"…","request_id":"24","action":"search_maps","map_rows_min":4,"map_rows_max":12,"map_columns_min":4,"map_columns_max":12,"display_format":"u16","endianness":"little","minimum_score":55,"skip_mapped":true,"map_every_byte":false,"scan_all":true}
{"protocol":"tuner-ui/v1","token":"…","request_id":"25","action":"map_search_state"}
{"protocol":"tuner-ui/v1","token":"…","request_id":"25a","action":"cancel_map_search"}
{"protocol":"tuner-ui/v1","token":"…","request_id":"26","action":"select_map_candidate","candidate_index":0}
{"protocol":"tuner-ui/v1","token":"…","request_id":"27","action":"inspect_map_candidate","candidate_index":0,"row":3,"column":4}
{"protocol":"tuner-ui/v1","token":"…","request_id":"28","action":"set_map_finder_view","map_zoom_percent":80,"candidate_pane_width":320}
{"protocol":"tuner-ui/v1","token":"…","request_id":"29","action":"set_map_finder_coloring","color_mode":"auto_range","decimal_places":2}
{"protocol":"tuner-ui/v1","token":"…","request_id":"30","action":"assign_map_axis","candidate_index":0,"map_axis_index":1,"axis_role":"x"}
{"protocol":"tuner-ui/v1","token":"…","request_id":"31","action":"clear_map_axis","candidate_index":0,"axis_role":"x"}
```

The `state`/`snapshot` response includes a `surface` object with open surfaces, active surface,
window geometry, camera, zoom, pan, height exaggeration, wireframe/axis visibility, and Z-range
settings. Surface rendering uses finite engineering values when available, falls back to finite
raw values, and auto-ranges with a five-percent Z padding (including a useful minimum padding for
flat maps). Missing cells remain holes instead of being rendered as zeroes.

The `state`/`snapshot` response also includes `hex` with open state, window geometry,
current byte address, selected half-open byte range, display format, endianness, precision,
color mode/palette/range, search query/status, BIN size, and the selected parameter/cell identity
used for byte-to-cell synchronization.

Requests are queued onto the UI thread, so edits, table focus, persistence, and command side
effects have one source of truth. Responses include `ok`, `message`, and, when useful, a
structured `data` snapshot. The server accepts multiple requests on one connection and closes
its manifest when the app exits normally.
