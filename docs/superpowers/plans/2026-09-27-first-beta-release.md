# First Beta Release Implementation Plan

> **For agentic workers:** Execute inline, task by task. Do not use subagents or force-push.

**Goal:** Build and publish TunerNook's first optimized Windows beta release without calibration data.

**Architecture:** Bump only `tuner-app` to the next prerelease version and make its title test derive the expected version from Cargo metadata. Build and smoke-test the optimized workspace, then tag the verified version and attach only the app executable to a GitHub prerelease.

**Tech Stack:** Rust/Cargo, Git for Windows Bash, GitHub CLI.

**Spec:** `docs/superpowers/specs/2026-09-27-first-beta-release.md`

## Global Constraints

- Keep `v0.1.5` unchanged; publish `0.1.6-beta.1` as `v0.1.6-beta.1`.
- Keep MIT licensing and existing `origin`/`upstream` routing.
- Do not upload BIN/XDF files, local fixtures, or build caches.
- Publish the GitHub release as a prerelease and attach only the Windows app executable.

## Review Focus

1. Version mismatch between Cargo package, native title, tag, and release name.
2. Binary built for a non-Windows or non-x64 target.
3. Release asset accidentally containing local calibration data.
4. Smoke-test failure or fixture mutation during verification.
5. Tag collision with an existing release.

---

### Task 1: Set beta version and keep title verification version-aware

**Files:**
- Modify: `crates/tuner-app/Cargo.toml`
- Modify: `crates/tuner-app/src/tests.rs`
- Update: `Cargo.lock`

- [x] Set the app version to `0.1.6-beta.1`.
- [x] Make `native_window_title_displays_app_version` compare with `concat!("TunerNook v", env!("CARGO_PKG_VERSION"))`.
- [x] Run the focused title test; expect it to pass.

### Task 2: Build and verify the release candidate

**Files:**
- Build: `target/release/tuner-app.exe`, `tuner-cli.exe`, `tuner-api.exe` (ignored output)

- [x] Run `cargo fmt --all -- --check` and `cargo test -p tuner-app native_window_title_displays_app_version`.
- [x] Run `cargo test --workspace`; expect 531 passing and 8 local-fixture tests ignored.
- [x] Run `cargo build --release --workspace`; expect the optimized Windows x64 binaries to build.
- [x] Run `scripts/smoke-test.sh --app` from Git for Windows Bash; expect 21/21 and an unchanged fixture BIN.
- [x] Record executable SHA-256 and size for release-asset verification.

### Task 3: Publish and verify the prerelease

**Files:**
- Create: Git commit, tag `v0.1.6-beta.1`, GitHub prerelease and executable asset.
- Update: `AGENTS.md`, this plan's completion boxes.

- [ ] Commit the app-version update and release notes on `main`; fast-forward `codex/four-area-app-split` to the same commit.
- [ ] Create/push `v0.1.6-beta.1` without moving `v0.1.5`.
- [ ] Create the GitHub prerelease and attach only `TunerNook-v0.1.6-beta.1-windows-x64.exe`.
- [ ] Verify the remote release flag, tag SHA, asset size, and repository tree contains no BIN/XDF payloads.
- [ ] Mark the release task complete in the plan and execution ledger.
