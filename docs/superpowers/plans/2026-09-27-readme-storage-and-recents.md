# README Storage and Recent Files Implementation Plan

> **For agentic workers:** Execute inline; this is a documentation-only follow-up.

**Goal:** Explain the locations and behavior of project/workspace persistence and Recent BIN/XDF menus in the public README.

**Architecture:** Document four separate storage concepts—global settings, per-BIN workspace JSON, user-selected `.tnproj` manifests, and workspace-background assets—using the actual Windows and XDG path rules from `settings_path()`.

**Tech Stack:** Markdown and read-only source inspection.

**Spec:** `docs/superpowers/specs/2026-09-27-readme-storage-and-recents.md`

## Global Constraints

- Do not imply TunerNook copies or bundles BIN/XDF files.
- Describe Recent BIN/XDF history as stored paths, not file data.
- Keep README links and formatting valid; no application code changes.

## Review Focus

1. Distinguish auto-saved per-BIN preferences from the optional `.tnproj` manifest.
2. Do not imply clearing Recent deletes BIN/XDF files.
3. Keep Windows, XDG, and fallback paths consistent with the code; warn that preference/manifests may contain local paths.

---

### Task 1: Document persistence and Recent file behavior

**Files:**
- Modify: `README.md`

- [x] Inspect `settings_path()`, `project_settings_path()`, manifest save/load, and Recent BIN/XDF behavior.
- [x] Add exact path, retention, non-copying, and path-privacy semantics to README.
- [x] Verify README relative links and key path descriptions against implementation.
- [x] Run `git diff --check` and `cargo fmt --all -- --check` (documentation-only).
- [x] Commit and push README and SDD docs to `origin/main`; synchronize `codex/four-area-app-split`.
