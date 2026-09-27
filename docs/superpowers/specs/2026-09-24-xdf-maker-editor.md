# XDF Maker and Editor — design spec

## Goal

Create and modify XDF definitions safely from a single in-memory draft editor. Drafts can begin empty, from a 2D Map Finder candidate, from a Hex Editor selection, or from an existing parsed definition. NookLink may submit typed proposals for the same authoring operations; it cannot mutate/save without the app's human review and challenge gates.

## Authoring model

- Retain the existing normalized XDF model as the source of truth for supported metadata and preserve untouched XML nodes/attributes.
- Support header metadata, categories, tables, constants, flags, bitfields, conversions, and attached axis definitions. Include add/edit/duplicate/reorder/delete operations.
- Reorder definitions only among siblings in the same source XML container; reject cross-container moves so unknown/vendor wrapper semantics are not silently changed.
- Expose XDF address separately from the resolved BIN byte offset. Support element width, numeric kind, signedness, endianness, dimensions, bit strides/byte skips, bit ranges, and axis address/count/stride/metadata.
- Numeric storage: signed and unsigned integer widths up to 64 bits and IEEE binary32. Binary64 is authorable only if a canonical XDF marker is verified against a known-good fixture; otherwise retain as unsupported and do not emit it.
- Validate positive dimensions and widths, valid bit ranges, checked address translation, overflow, identity uniqueness, and loaded-BIN range. Overlap is a warning, not a blocker.
- Serialize then reparse drafts before applying/saving. Preserve unknown XML; never offer arbitrary XML authoring.

## Draft/editor behavior

- One movable/resizable editor, sections General, Layout/Storage, Axes, Conversion, and Validation/Preview.
- Map Finder prefill includes candidate address, dimensions, numeric display format and byte order plus selected axis suggestions; every inferred field remains visibly reviewable/unverified.
- Hex-selection prefill uses the selected BIN range as a starting address only; user must define dimensions and storage before apply.
- Existing item entry points edit the definition, not its BIN cells. Keep the loaded source document unchanged until an explicit draft operation is accepted.
- Draft mutations are undoable/redoable. Closing a dirty draft prompts to save, discard, or continue editing. Save XDF As requires a new destination and rejects overwrites.

## NookLink behavior

- Add `xdf_authoring` capability and typed definition operations (no XML payloads).
- Require a task-bound source identity; reject stale BIN/XDF/draft revisions and unauthorized operation kinds.
- Show proposed definition diffs in the app. Applying is one atomic draft-history operation and requires the existing in-app challenge/approval flow.
- Saving an agent-authored draft is a second user action with a fresh challenge tied to the exact draft revision and explicit new path.

## Acceptance

- Round-trip supported authoring changes while retaining unrelated/unknown source XML.
- UI entry points create usable drafts and do not silently trust heuristic axes or selections.
- Invalid/out-of-range drafts cannot be applied or saved; source files remain byte-identical.
- Agent proposals are typed, reviewable, revision-bound, and cannot self-approve or self-save.
