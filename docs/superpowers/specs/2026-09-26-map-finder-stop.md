# Map Finder cancellation

## Goal

Let a user (or a UI-control agent) stop a long 2D Map Finder scan without
blocking the interface or exposing a misleading partial result.

## Behavior

- While a scan is active, show Stop in both the Map Finder window and the amber
  background-operation notice. Both controls use the same cancellation state;
  clicking either changes the controls/status to “Stopping…” until the worker
  exits. Other background operations do not show a Stop control.
- The worker checks a shared atomic cancellation flag periodically in the
  offset loop. Cancellation returns an explicit result state, discards partial
  candidates, and never changes BIN/XDF data.
- Completion clears the active operation and flag, reports “Scan canceled,” and
  does not force progress to 100%.
- Add `cancel_map_search` to the existing local UI IPC action set so an agent
  that can start a scan can stop it. Idle cancellation returns a clear error.
- Keep the existing single-worker scan architecture. Benchmark a throwaway CPU
  shape-splitting prototype separately; do not add GPU dependencies/backends or
  production parallelism in this slice.

## Safety and tests

Cancellation is UI/control state only. Tests cover cancellation during work,
discarding partial results, Stop input from both UI locations, hiding the popup
control for non-scan work, IPC success/idle behavior, and status cleanup.
Existing scan ranking and progress behavior remain unchanged when cancellation
is not requested.
