# README Refresh Implementation Plan

> **For agentic workers:** Execute inline, task by task. No subagents needed.

**Goal:** Make the public README personal, candid, safety-conscious, and welcoming while showcasing two user-selected screenshots.

**Architecture:** Replace the technical-first README with a concise project story, differentiators, verified test results, safety notice, screenshot gallery, getting-started steps, and contribution invitation. Preserve deeper technical material in linked documentation. Keep the two provided screenshots byte-identical in `docs/screenshots/`.

**Tech Stack:** Markdown, Git, PowerShell file copy.

**Spec:** `docs/superpowers/specs/2026-09-27-readme-refresh-design.md`

## Global Constraints

- Keep the MIT license and existing beta/release links accurate.
- Do not claim tests prove the application or a calibration safe.
- Include exactly the two approved screenshots; do not add BIN/XDF files.
- State the author-built-for-self and open-to-feedback intent plainly.

## Review Focus

1. The screenshots render from repository-relative links and remain byte-identical to the approved originals.
2. Test counts match the latest verified Beta 1 run, including fixture tests skipped by default.
3. Safety language is prominent without promising legal enforceability or guaranteed correctness.
4. Feature descriptions distinguish heuristic map guesses and agent suggestions from verified definitions or user-approved edits.
5. The README retains enough build and documentation links for a new developer to get started.

---

### Task 1: Publish the approved screenshots in the repository

**Files:**
- Create: `docs/screenshots/tunernook-workspace-table-3d.png`
- Create: `docs/screenshots/tunernook-multi-window-workspace.png`

- [x] Copy only the two approved root screenshots without alteration.
- [x] Compare SHA-256 hashes of each source and destination; expect exact matches.

### Task 2: Rewrite the public README

**Files:**
- Modify: `README.md`

- [x] Lead with the personal story and an explicit “Yes, it’s vibe-coded” statement.
- [x] Describe differentiators with concrete behaviors, not superiority claims.
- [x] Embed both screenshots with useful alt text.
- [x] Include verified Beta 1 test counts and explain their limits.
- [x] Add a prominent no-warranty, no-responsibility and calibration-risk notice.
- [x] Invite use, improvement, sharing, comments, issues, and contributions; preserve MIT and getting-started links.

### Task 3: Verify and publish

- [x] Check every README relative link/image path exists, test-count copy, screenshot hashes, and absence of calibration payloads.
- [x] Run Markdown-oriented sanity checks and `cargo fmt --all -- --check` (no Rust changes expected).
- [ ] Commit and push the README and screenshots to `origin/main` and sync `codex/four-area-app-split`.
