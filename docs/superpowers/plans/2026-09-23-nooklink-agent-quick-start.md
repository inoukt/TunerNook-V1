# NookLink Agent Quick Start Implementation

**Goal:** Add an in-app, provider-neutral way to start an external agent with
accurate TunerNook context.

**Spec:** `docs/superpowers/specs/2026-09-23-nooklink-agent-quick-start-design.md`

**Constraints:** Do not launch/configure agents or alter either NookLink protocol.
Bundle the context using the existing Rust build. Reuse the already-locked
clipboard backend; do not add a new package.

## Task 1: Quick-start prompt and context export

- [x] Add a failing headless UI test for the Quick Start controls and prompt-copy action.
- [x] Add an agent-readable context Markdown file and embed it in `tuner-app`.
- [x] Add **Copy starter prompt** and **Save context file…** actions with clear app status feedback.
- [x] Add concise Quick Start instructions to the setup guide; retain the detailed protocol guide.
- [x] Verify exported bytes match the embedded context and copy/export errors are surfaced.
- [x] Run focused tests, app/workspace suites, formatting, release build, and smoke test.

**Review focus:** The app must not imply it launches/configures an agent; exported context must
be complete without the source tree; save failure must not be reported as success; the prompt
must direct the user to attach the context file and replace its goal placeholder.
