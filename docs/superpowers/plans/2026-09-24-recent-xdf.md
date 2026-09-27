# Recent XDF menu — implementation plan

## Scope

Mirror the existing Recent BIN flow for XDFs: persist up to 16 newest-first paths, record a path
only after a successful XDF load, let the user reopen or clear entries from File, and expose the
list in read-only `tuner-ui/v1` state. Keep project-specific “last XDF for this BIN” behavior
unchanged.

## Tasks

- [x] Add persistence, successful-load, UI IPC, and menu interaction tests; verify the new behavior
      is absent before implementation.
- [x] Add XDF history to global preferences and successful-load handling.
- [x] Add File menu reopen/clear actions and expose recent XDF paths in UI state/docs.
- [x] Run formatting, app/workspace tests, release build, and GUI smoke test.
