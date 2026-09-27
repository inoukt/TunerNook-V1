# TunerNook First Beta Release

## Goal

Publish the first Windows x64 beta executable for the next app version while
keeping the existing `v0.1.5` source backup and MIT licensing intact.

## Release identity

- App version and native title: `0.1.6-beta.1`.
- Git tag and GitHub prerelease: `v0.1.6-beta.1`.
- Target: Windows x64 MSVC, optimized Cargo release profile.
- Asset: only `tuner-app.exe`, renamed for the release. Do not attach calibration
  BIN/XDF data or local test fixtures.
- MIT remains the project license. The existing `v0.1.5` tag is unchanged.

## Verification

- Keep the native title test tied to `CARGO_PKG_VERSION` so future version
  increments do not require a hard-coded title-test edit.
- Run formatting and workspace tests.
- Build the app release target and all binaries required by the existing smoke
  harness; run `scripts/smoke-test.sh --app` from Git for Windows Bash.
- Verify the executable is present, is Windows x64, has the beta version title,
  and contains no bundled user BIN/XDF files.
- Publish as a GitHub prerelease, then verify tag, prerelease flag, asset name,
  and uploaded asset size.
