# Public TunerNook-V1 Baseline Implementation Plan

> **For agentic workers:** Execute inline, task by task. Do not use subagents or push to `upstream`.

**Goal:** Publish a curated v0.1.5 source backup to the new public GitHub repository without local calibration data or build artifacts.

**Architecture:** Add source-repository metadata and fixture guidance, make private-fixture tests opt-in, then create and publish an audited baseline commit on `main`. Preserve the old repository as `upstream`; publish a separate development branch at the same base for the later app split.

**Tech Stack:** Git/GitHub CLI and the existing Rust/Cargo workspace. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-27-public-v1-baseline.md`

## Global Constraints

- Do not publish the SCGa05 BIN/XDF pair, extracted VBF definitions, local photos/logs, build output, `.freebuff`, `.superpowers`, or the unapproved four-area design spec.
- Use MIT with copyright holder `inoukt`; retain app version `0.1.5`.
- Keep the same local folder. `origin` is `TunerNook-V1`; `upstream` remains `TunerNook`.
- Stage an explicit file allowlist, inspect it, and only push the reviewed baseline to `origin`.
- Do not upload binaries or create a GitHub Release.

## Review Focus

1. No private BIN/XDF or extracted-definition file is staged, including the spaced fixture directory.
2. Root photos, logs, `target/`, local caches, and SDD snapshots stay out of the public commit.
3. A public clone's default workspace test run does not fail because private fixtures are absent; fixture tests remain explicit opt-ins.
4. The pushed `main`, `v0.1.5` tag, and development branch all point to the reviewed baseline, and `upstream` is untouched.
5. MIT applies only to project-authored code; the README clearly distinguishes local-only fixtures.

---

### Task 1: Prepare safe public-source metadata

**Files:**
- Create: `.gitignore`, `LICENSE`
- Modify: `crates/tuner-app/Cargo.toml`, `README.md`, `AGENTS.md`
- Modify: `AGENTS.md` and `docs/superpowers/plans/2026-09-22-compare-transfer-graph.md` to remove local drive paths.

- [x] Add narrow ignore rules for build output, private fixture/extracted-definition paths, logs, root photos, `.freebuff`, `.superpowers`, and local-only design drafts.
- [x] Add the standard MIT license (`Copyright (c) 2026 inoukt`) and set the app crate license metadata to MIT.
- [x] Document excluded SCGa05 fixtures, ignored fixture tests, smoke-test requirements, and license scope in README.
- [x] Update current agent notes to describe the GitHub remotes and fixture policy; replace workstation-specific path examples.
- [x] Verify `git check-ignore` covers every private/build path and scan publishable text for credentials/absolute workstation paths.

### Task 2: Make private-fixture tests opt-in

**Files:**
- Modify: `crates/tuner-app/src/tests.rs`
- Modify: `crates/tuner-xdf/tests/real_fixture.rs`
- Modify: `crates/tuner-xdf/src/lib.rs`

- [x] Mark the 4 app tests, 3 XDF integration tests, and 1 XDF unit test that require SCGa05 as ignored with a local-fixture reason.
- [x] Run `cargo test --workspace --quiet`; expect all non-fixture tests to pass and exactly 8 fixture tests ignored.
- [x] Run `cargo test --workspace -- --include-ignored` with the local fixtures present; expect all 539 tests to pass.
- [x] Run `cargo fmt --all -- --check`.

### Task 3: Audit, commit, and publish the v0.1.5 backup

**Files:**
- Git index/history only; stage the reviewed allowlist.

- [x] Stage `.gitignore`, `LICENSE`, `AGENTS.md`, Cargo files, README, `crates/`, `docs/`, and `scripts/`; exclude local artifacts and the unapproved architecture spec.
- [x] Review `git diff --cached --name-only` and the complete staged diff; confirm no calibration data, photos, logs, target artifacts, credentials, or private local paths.
- [ ] Create the initial baseline commit on local `main` and push only to `origin/main`.
- [ ] Create/push tag `v0.1.5` and branch `codex/four-area-app-split` at the baseline commit.
- [ ] Verify remote URLs, branch/tag SHAs, default branch, and that `upstream` remains unchanged.
- [ ] Run `cargo clean` after verification so build artifacts do not rebuild the 17.4 GiB cache.

## Deferred

- The four-area `lib.rs` design remains unapproved and is excluded from this baseline.
- Fixture-dependent smoke checks remain local-only until a safe distributable fixture exists.
