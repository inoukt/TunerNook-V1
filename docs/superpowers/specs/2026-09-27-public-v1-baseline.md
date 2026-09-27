# Public TunerNook-V1 baseline

Date: 2026-09-27  
Status: Approved publication scope

## Goal

Publish the current TunerNook v0.1.5 source as a public backup in
`inoukt/TunerNook-V1`, using the existing local folder and excluding local
calibration data and personal/build artifacts.

## Requirements

- The new public repository is `TunerNook-V1`; the prior `TunerNook` repository
  remains untouched as `upstream`.
- Apply MIT to project-owned source, with copyright holder `inoukt`, and mark
  `tuner-app` as MIT. The existing MIT declarations in the other crates remain.
- Exclude the SCGa05 calibration BIN/XDF pair, extracted VBF definitions,
  personal photos, logs, `target/`, `.freebuff`, and local `.superpowers`
  snapshots. Keep authored synthetic source fixtures only.
- The eight tests that require the private SCGa05 fixture are ignored by default
  with a clear reason, and can still be run locally with `--include-ignored`.
  Document that the smoke script also requires those local-only files.
- Update the root agent notes and historical plan snippets so public docs do
  not encode workstation-specific drive paths or claim there is no Git repo.
- Keep app version `0.1.5`; this is a packaging/publication change only, with no
  runtime behavior changes.
- Publish a reviewed initial commit to `origin/main`, tag it `v0.1.5`, and push
  a `codex/four-area-app-split` branch at the same baseline. Do not push to
  `upstream` or include the unapproved four-area design proposal.

## Safety

- Stage an explicit allowlist; never run `git add .` for the initial public
  commit.
- Review all staged names and content for calibration data, generated output,
  local paths, credentials, and personal files before committing.
- No GitHub release or binaries are created in this slice.
