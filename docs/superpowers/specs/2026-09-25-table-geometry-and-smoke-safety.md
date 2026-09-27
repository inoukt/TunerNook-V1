# Table Geometry Persistence and Smoke-Test Process Safety

## Goal

Reopening a table must restore the dimensions the user saved, while auto-fit tables must account for the rendered table controls. The GUI smoke test must close only the GUI process it launched, never another TunerNook instance.

## Behavior

- When a BIN project reopens a table after its XDF definition fingerprint changes, an exact table key wins; otherwise the most recently used saved geometry for the same semantic table is reused within that BIN project's preferences.
- For auto-fit tables, changing zoom updates the live window dimensions to the same content-fit size that reopening would calculate. Manually sized windows do not change size when zoom changes.
- A completed manual table resize is committed before a subsequent close snapshots its memory; the reopened rectangle matches the clamped saved rectangle.
- Explicitly resized windows remain user-sized. Smaller displays retain scrolling rather than creating unreachable windows.
- Explicitly resized windows remain user-sized. Auto-fit changes must not override `fit_to_content = false`.
- The smoke script can select a build directory and closes only the exact GUI process object it launched (PID-scoped). It must not use image-name-wide termination.
- No BIN/XDF data or existing app process is changed by verification.

## Constraints

- Keep the current typed table-window memory and project-scoped recent-table order as the single source of saved geometry and recency.
- Do not add dependencies or a second geometry store.
- Preserve canvas clamping and the existing scroll behavior when a complete table cannot fit on screen.
- Follow the repository's no-Git SDD snapshot/ledger process and do not repeat one focused test workflow more than 10 times.
