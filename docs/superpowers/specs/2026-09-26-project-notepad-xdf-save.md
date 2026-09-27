# Project Notepad and XDF Save Confirmation

## Goal

Add project-scoped notes in a reusable floating utility window, and let users
replace an existing XDF destination only after the save flow's existing user
confirmation. A successful XDF save leaves the editor clean, so closing it does
not ask again.

## Behavior

- The Project Notepad stores plain text and its in-app Stay on top preference in
  the active BIN project's preferences. Old project settings deserialize to an
  empty note with Stay on top disabled. It never edits BIN/XDF contents.
- Open the notepad from Tools and the window dock. Reuse the standard movable,
  resizable utility window, saved geometry, focus, minimize, close, and restore
  behavior. Stay on top means above other TunerNook windows, not other desktop
  applications. The action is available only while a BIN project is loaded.
- Saving XDFs to a new destination remains create-new. If the native Save dialog
  returns an already-existing path, its overwrite confirmation authorizes the
  replacement; do not add a second app confirmation. NookLink's existing
  in-app challenge is the single approval for its save, and must explicitly
  identify when the destination already exists.
- Before replacing an existing XDF, serialize and reparse a staged file. Keep
  the existing XDF intact if staging or verification fails. On successful save,
  reload the saved XDF and mark the editor clean; closing it must not prompt.
- Unconfirmed/raced-in destinations still fail closed. BIN Save As behavior is
  unchanged.

## Tests

- New XDF destinations are created; overwrite is rejected without the explicit
  confirmation flag; confirmed overwrite replaces a valid XDF only after the
  replacement parses; staging failures preserve the prior file.
- Successful editor save marks the draft clean and a subsequent close needs no
  save prompt.
- Project notes and Stay on top round-trip; legacy/default settings yield
  empty/false; the Tools entry, dock registration, editable note, geometry,
  and in-app topmost behavior work without changing BIN bytes.
