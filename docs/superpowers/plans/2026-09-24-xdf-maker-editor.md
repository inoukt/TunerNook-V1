# XDF Maker and Editor — implementation plan

Spec: `docs/superpowers/specs/2026-09-24-xdf-maker-editor.md`
Ledger: `.superpowers/sdd/2026-09-24-xdf-maker-editor/progress.md`

## Constraints

- Preserve source XDF XML not explicitly edited; keep unsupported/vendor fields intact.
- Never overwrite an input BIN or XDF. The authoring draft is in-memory and Save As is explicit.
- Keep XDF changes separate from BIN cell edits and route them through the app's existing review/history conventions.
- Do not emit an unverified Float64 XDF storage encoding.

## Tasks

- [x] 1. Add a safe typed XDF authoring model and source-preserving serializer operations.
- [x] 2. Add a draft editor with New, Map Finder, Hex selection, and existing-definition entry points, validation, undo/redo, and Save As.
- [x] 3. Add NookLink's dedicated XDF authoring capability, typed proposal review/apply, and revision-bound Save As challenge.
- [x] 4. Add regression/integration coverage, protocol/setup documentation, and final verification.

## Verification

- Run focused XDF tests after Task 1, then focused app tests after Tasks 2–3.
- Final battery: `cargo fmt --all -- --check`, `cargo test -p tuner-app`, `cargo test --workspace`, `cargo build --release -p tuner-app`, and `scripts/smoke-test.sh --app`.
- Do not repeat an identical focused test workflow more than ten times.
