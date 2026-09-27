# Workspace Table Size Lifecycle — Plan

**Spec:** `docs/superpowers/specs/2026-09-25-workspace-size-lifecycle.md`

## Scope

Trace and test the actual user resize → workspace switch → restore and close → reopen paths. Change production code only if a new test reproduces stale or altered geometry.

## Steps

- [x] Add headless egui regressions for real edge resizing while canvas-clamped, plus workspace switching and title-bar close/reopen.
- [x] Observe RED: a 1000 px saved width stayed 1000 after a user drag made the visible table 760 px; the full app also rendered a saved 900 px window at 758 px because it was constrained to the central editor pane.
- [x] Use the shared shell-safe bounds for table windows and record explicit manual resizes even when the old rectangle is clamped. Passive clamping still does not rewrite manual memory.
- [x] Run formatting, the app suite, workspace suite, and optimized release check.
- [x] After the user closed the running GUI, build the release executable and run `scripts/smoke-test.sh --app` through Git Bash; all 21 checks passed and the smoke GUI closed gracefully.
- [x] Update `AGENTS.md`, this ledger, the previous geometry ledger, and final snapshots with the confirmed root cause and verification results.
