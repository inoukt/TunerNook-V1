# Formula Editor and XDF Export Design

**Date:** 2026-09-14  
**Status:** Approved in conversation

## Goal

Add a safe formula/conversion editor for mapped parameters and axes, while allowing the
resulting mapping to be materialized into a new XDF without overwriting the source XDF.

This is the first editor slice of the larger editor roadmap. Scalar, flag/bitmask, raw-hex,
and graph/surface work remain separate follow-up slices.

## Scope

### Included

1. Edit the conversion formula for the selected parameter or selected stored axis.
2. Validate formulas with the existing `tuner-cal` parser and language.
3. Preview raw-to-engineering evaluation and engineering-to-raw inversion.
4. Apply formulas as project-scoped overrides tied to the BIN identity, XDF fingerprint, and
   parameter/axis identity.
5. Reset an override to the source XDF formula.
6. Save the effective mapping as a new XDF through an explicit Save As action.
7. Expose inspection, override, reset, and Save As operations through `tuner-ui/v1`.
8. Keep all BIN edits and formula changes separate: applying a formula does not modify BIN bytes
   or create a BIN undo entry.

### Deferred

- Editing top-level XDF `<XDFFUNCTION>` metadata. The parser currently retains these objects as
  auxiliary metadata rather than executable mappings.
- Adding new formula functions or a second expression language. The first editor uses the
  existing `X`, arithmetic, parentheses, unary signs, finite-value, and linear-inversion rules.
- Scalar/flag/bitmask/raw-hex editor surfaces and 2D/3D graph rendering. They will consume the
  same selection, conversion, command, persistence, and window patterns later.

## Data and persistence model

`ProjectPreferences` gains a conversion-override map. An override key contains:

```
<xdf-normalized-fingerprint>:parameter:<semantic-id>
<xdf-normalized-fingerprint>:axis:<semantic-id>:<axis-index>
```

The project is already selected by BIN identity, so including the XDF fingerprint prevents a
formula from silently applying to the same BIN when a different XDF is opened. Only explicit
overrides are persisted; the source formula remains available from the loaded XDF and remains
the reset target.

On XDF installation, matching overrides are applied to the in-memory normalized model before
the first validation/read. Overrides that do not match a parameter or axis are ignored and
reported in diagnostics rather than applied by title or position.

Applying an override follows this order:

1. Parse the candidate with `tuner_cal::Conversion::parse`.
2. Keep the old formula and preferences unchanged if parsing fails.
3. Update the selected in-memory parameter/axis conversion and the project override map.
4. Recompute mapped values/validation and mark project settings dirty, not the BIN dirty.

A formula may be valid for reading but not invertible. The UI and API report that condition;
engineering writes are disabled with the parser's reason, while raw writes remain available.

## Formula editor UI

The Inspector gets a `Conversion / Formula` section for the current parameter or selected axis.
It shows:

- target identity and whether the formula comes from the XDF or a project override;
- the effective formula in a multiline editor;
- live parse status and an actionable error position/message;
- a compact preview using the selected raw value plus representative samples (`0`, `1`, and
  the current raw value when available);
- inverse status and a sample engineering-to-raw result when inversion is possible;
- `Apply override`, `Reset to XDF`, and `Revert` actions;
- a `Save XDF As…` action that exports the current effective mapping.

The editor does not write on every keystroke. Typing updates only draft UI state; Apply is the
transaction boundary for the mapping override. Invalid drafts never replace a working formula.

The Save As action uses the existing asynchronous waiting/diagnostics pattern, refuses an
existing output path, and reports the output path and effective override count when complete.

## XDF export

The exporter creates a new UTF-8 XDF from the loaded document's effective normalized mapping.
It updates parameter and axis conversion math nodes and retains the parsed supported structure,
categories, axes, flags, unknown elements, and auxiliary source nodes where the parser retained
them. Formatting, comments, processing instructions, and source-only lexical details may be
normalized or omitted; the original XDF is never changed.

If a target has no existing conversion node, the exporter writes the canonical form:

```xml
<XDFCONVERT><MATH equation="..." /></XDFCONVERT>
```

The output is written atomically and is re-parsed before the operation is reported successful.
The re-parse check must confirm the exported target formulas and document fingerprint are
available; otherwise the output is treated as failed and the source remains untouched.

## Agent control

The existing authenticated loopback `tuner-ui/v1` bridge gains stable direct actions:

- `get_conversion`: return target, source/effective formula, override state, parse status,
  invertibility, and preview samples;
- `set_conversion_override`: validate and apply a parameter/axis formula;
- `reset_conversion_override`: remove the selected target override;
- `save_xdf_as`: queue Save XDF As and return the operation state.

Requests use semantic IDs and explicit axis indices. They never target an object by visible title
alone. Responses reuse the existing `ok`, `message`, `request_id`, and structured `data`
contract. The UI thread remains the only place where project state, normalized mappings, and
operations are mutated.

The command registry also exposes the formula editor and Save XDF As so keyboard, menu, and AI
paths share the same availability checks and side effects.

## Error handling and compatibility

- Invalid formulas leave the effective formula and project preferences unchanged.
- Missing BIN/XDF/target reports a normal workspace error; no partial operation is queued.
- Non-invertible formulas remain readable and explicitly read-only for engineering writes.
- Save As never overwrites an existing path and never mutates the source XDF.
- New serialized preference fields use defaults so older settings continue to load.
- Existing BIN undo/redo, table editing, axis editing, and validation behavior remain unchanged
  except for using the effective conversion formula.

## Testing

Tests will cover:

- project override key identity and persistence/migration;
- parameter and axis override application;
- invalid formula atomicity;
- read-only behavior for non-invertible formulas;
- preview evaluation and inversion status;
- exported XDF re-parse, formula materialization, source preservation, and existing-target
  refusal;
- formula editor Apply/Reset behavior in headless egui;
- authenticated IPC inspection, override, reset, and Save As requests.

The existing full workspace formatting, test, check, and build commands remain the completion
gate.
