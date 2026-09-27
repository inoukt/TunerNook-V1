# Shifted Digit Shortcut Fix

> Inline execution; no Git metadata is available.

**Goal:** Make `Shift+1` workspace switching robust to shifted-symbol logical
keys by matching the physical digit key as a fallback.

**Architecture:** Extend the shared shortcut matcher only for digit bindings.
Keep modifier equality, logical-key matching, and focus gating unchanged.

## Steps

- [x] Add failing regressions for logical `Exclamationmark` / physical `Num1`
  matching and recording; keep physical fallback limited to digits.
- [x] Implement digit-only physical-key matching and capture; verify RED →
  GREEN.
- [x] Bump to `0.1.3`, update project notes, run the full verification battery,
  and capture before/final SDD snapshots.
