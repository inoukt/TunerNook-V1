# Shifted digit shortcut matching

## Goal

Make digit shortcuts such as `Shift+1` work when a keyboard layout maps the
shifted number-row key to punctuation (for example, physical `1` becoming
logical `!`).

## Behavior

- Preserve existing logical-key matching.
- For configured digit keys (`0`–`9`), also match the physical digit key on a
  non-repeating key-press event. Use that same physical digit when recording a
  shortcut, so `Shift+1` can be captured even when its logical key is `!`.
- Keep exact modifier matching and the existing text-input focus guard.
- Do not use physical-key fallback for letters or punctuation.
- Increment the app version to `0.1.3`; preserve `APP_NAME`.

## Verification

Test logical/physical mismatch for Shift+1, recorder capture from that event,
and unchanged modifier/focus behavior; then run formatting, app/workspace tests,
release build, and GUI smoke test.
