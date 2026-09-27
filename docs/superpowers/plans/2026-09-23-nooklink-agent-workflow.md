# NookLink Agent-Assisted Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a user-led NookLink task, evidence, proposal-review, and approval workflow while keeping TunerNook useful without an agent.

**Architecture:** Keep external agents outside TunerNook; a user asks their configured agent, which sends short task/progress/proposal messages through the existing authenticated `tuner-ui/v1` bridge. The app stores a bounded task state, shows setup guidance and progress, validates proposed XDF-cell edits against captured BIN/XDF hashes, and applies approved edits through one existing core transaction. This first slice does not embed an agent, perform universal ROM reverse engineering, load runtime add-ons, or implement Live Helper.

**Tech Stack:** Rust 2021, `eframe`/`egui`, existing `serde`/`serde_json`, `tuner-core`, `tuner-xdf`; no new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-23-nooklink-agent-workflow-design.md`

## Global Constraints

- “TunerNook works normally when no agent is configured.”
- “User setup and explicit per-task invocation are required for task-oriented agent work.”
- “A user-marked long/complex task, direct raw byte-range read, proposal apply, or NookLink-requested save requires a small in-app puzzle. Quick tasks and ordinary state/table/map reads do not.” Challenge answers and approvals are not exposed over IPC; the puzzle is friction, not proof-of-human, and independent filesystem grants or full UI automation can bypass or solve it.
- “Agent inspection is scoped to the selected project and requested task; no general filesystem or network access is implied by NookLink.”
- “Evidence refers to stable document identity (BIN/XDF hashes where available), byte ranges, parameter IDs, and operation IDs so the user can inspect the basis of a finding.”
- “Before applying a proposal, TunerNook rechecks the source identities and validates every operation. A stale or invalid proposal is rejected for refresh/review, never silently rebased.”
- “Agent-proposed edits use the same app/core validation and undo transaction paths as native edits. Existing explicit-output and no-overwrite rules remain in force.”
- “Long operations are cancellable where the underlying agent/API supports it; cancellation must not partially apply a proposal.”
- “NookLink stays local and authenticated using the existing local-control boundaries. Agent credentials are not stored in BIN-specific project files.”
- “The future Live Helper is a separate project slice after the task and approval workflow proves useful.”
- “Runtime add-on loading and Live Helper behavior are excluded from the first implementation; both require separate opt-in designs before implementation.”
- “During development, an agent may help implement permanent built-in functions through ordinary source changes, tests, and review.” This remains the developer workflow, not runtime code generation.

## Review Focus

- BIN, XDF, or in-memory conversion-override changes after task start: approval must be disabled/rejected as stale; cover in Task 4.
- Malformed task envelopes, unknown capability names, or out-of-range progress: reject cleanly without crashing or freezing the UI; cover in Task 2.
- User cancellation racing a late progress/proposal message: cancellation is terminal and no proposal can apply; cover in Tasks 1–3.
- A proposal with one invalid/non-finite/out-of-range cell among valid cells: apply none and add no undo entry; cover in Task 4.
- No configured agent, unsupported capability, or no open project: setup/help remains usable and no task starts automatically; cover in Tasks 2–3.
- Raw byte reads without a task ID, before raw-read approval, after cancellation, or against a different BIN identity: reject; cover in Tasks 2–3. Ordinary table/map/state inspection remains unchanged.
- A quick task starts without a puzzle; a user-marked long/complex task requires the puzzle; the agent cannot self-mark or self-approve it; cover in Task 3.
- Proposal apply and NookLink-requested BIN/XDF Save As cannot occur until a fresh, correct in-app challenge; cover in Tasks 3–4. Generic dispatch of native Save As commands must not bypass these task-bound actions.
- Agent-supplied evidence strings contain credentials/challenge values, project restoration is in flight, or an inactive-project history write fails: persist no unvalidated text, misattribute no task, and retain failed history for retry; cover in Task 5.

## File Map

- Create `crates/tuner-app/src/agent_tasks.rs` for typed task, evidence, proposal, bounded-history data, and task-state transitions.
- Modify `crates/tuner-app/src/lib.rs` to own the task registry, route messages in `handle_ui_ipc_request`, expose task state/capabilities, add bounded read-only BIN-range inspection, render setup/review windows, and apply approved edits through `WorkspaceState`.
- Modify `crates/tuner-app/src/ui_ipc.rs` to add a typed, optional NookLink task message to the existing authenticated JSONL request. Keep `tuner-ui/v1` additive and backward-compatible.
- Create `docs/nooklink-agent-setup.md` with provider-neutral steps for configuring an external agent to use the existing file API or live desktop bridge; the app does not install, launch, or choose an agent.
- Update `docs/tuner-ui-ipc.md` and `README.md` with the setup and task-message contract.
- Tests live with `agent_tasks.rs` for state transitions and in `lib.rs`/`ui_ipc.rs` for app and wire integration, following existing test patterns.

---

### Task 1: Add the NookLink task and proposal state model

**Files:**
- Create: `crates/tuner-app/src/agent_tasks.rs`
- Modify: `crates/tuner-app/src/lib.rs` to declare `mod agent_tasks;` and import the task types.
- Test: unit tests in `agent_tasks.rs`.

**Interfaces:**
- Produces `AgentCapability::{Diagnostics, TuningHelp, AutomationEditing, ReverseEngineeringSearch}`.
- Produces `AgentTaskCommand`, a serde-tagged message with `Start`, `Progress`, `Submit`, `Poll`, and `Fail` variants. User cancellation and review are app-side actions; agent messages never include `Cancel`, `Approve`, or `Reject`.
- Produces `AgentTaskStatus::{AwaitingUserApproval, Running, AwaitingReview, Accepted, Rejected, Cancelled, Failed, Stale, Applied}`.
- Produces `DocumentIdentity { bin_sha256: Option<String>, xdf_sha256: Option<String>, workspace_data_revision: u64 }` and `AgentTaskStore::{start, update_progress, submit, poll, fail, cancel, decide, mark_applied, is_empty}`. Task IDs are strings formed from process ID, start timestamp, and a per-process counter so they remain distinguishable in project history across restarts.
- `AgentTaskCommand` fields are: `Start { capability, goal }`, `Progress { task_id, phase, progress_percent, message }`, `Submit { task_id, report }`, `Poll { task_id }`, and `Fail { task_id, message }`.
- Produces `AgentReport { summary: String, findings: Vec<String>, confidence: Option<f32>, evidence: Vec<AgentEvidence>, operations: Vec<ProposedOperation> }`, `AgentEvidence { summary: String, semantic_id: Option<String>, byte_range: Option<[usize; 2]> }`, and `ProposedOperation::SetEngineeringCell { semantic_id: String, row: usize, column: usize, engineering_value: f64 }`. Confidence is optional and, when present, finite in `[0,1]`. There is no arbitrary byte-write, path, shell-command, or executable-code operation in this slice.
- Produces `AgentTaskLevel::{Quick, LongComplex}` and `AgentTaskRecord { task_id: String, capability: AgentCapability, goal: String, document_identity: DocumentIdentity, status: AgentTaskStatus, level: Option<AgentTaskLevel>, raw_read_authorization_pending: bool, raw_read_authorized: bool, phase: String, progress_percent: Option<u8>, message: String, report: Option<AgentReport> }` plus the existing `AgentTaskHistoryItem` for bounded project-scoped history; full prompts/conversations are not persisted. Challenge prompts/answers remain in non-serialized app UI state.
- Produces `AgentTaskError { message: String }` for invalid IDs, limits, or state transitions.
- `AgentTaskStore::start(capability, goal, identity) -> Result<String, AgentTaskError>` creates `AwaitingUserApproval`; app-only `approve_start(task_id, level)` transitions it to `Running` as `Quick` (no puzzle) or `LongComplex` (only called after the UI challenge succeeds). `request_raw_read_authorization(task_id)` marks that task's raw-read challenge pending; `allow_raw_reads(task_id, current_bin_sha256)` grants task-scoped reads only when the captured BIN identity still matches. `update_progress`, `submit`, and `fail` accept only `Running` tasks. `poll`, `cancel`, `decide`, `mark_applied`, and `is_empty` keep their existing contracts.

- [x] **Step 1: Write the task-state regression test**

```rust
#[test]
fn cancelled_task_rejects_late_progress_and_proposals() {
    let mut tasks = AgentTaskStore::default();
    let id = tasks.start(
        AgentCapability::Diagnostics,
        "inspect mappings".into(),
        DocumentIdentity::default(),
    ).unwrap();
    tasks.cancel(&id).unwrap();

    assert!(tasks.update_progress(&id, "late result".into(), Some(50), String::new()).is_err());
    assert!(tasks.submit(&id, empty_report()).is_err());
    assert_eq!(tasks.poll(&id).unwrap().status, AgentTaskStatus::Cancelled);
}

fn empty_report() -> AgentReport {
    AgentReport {
        summary: String::new(),
        findings: Vec::new(),
        confidence: None,
        evidence: Vec::new(),
        operations: Vec::new(),
    }
}
```

- [x] **Step 2: Run the focused test to confirm the model is missing**

Run: `cargo test -p tuner-app cancelled_task_rejects_late_progress_and_proposals`

Expected: FAIL because `agent_tasks` and its state types do not exist.

- [x] **Step 3: Implement the smallest typed state machine**

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentTaskStatus {
    Running,
    AwaitingReview,
    Accepted,
    Rejected,
    Cancelled,
    Failed,
    Stale,
    Applied,
}

pub(crate) struct AgentTaskStore {
    session_prefix: String,
    next_id: u64,
    tasks: std::collections::BTreeMap<String, AgentTaskRecord>,
}
```

Implement `Default` with `session_prefix = "{process_id}-{unix_timestamp_nanos}"`; append and increment `next_id` for each new task. Implement only valid transitions: `start → Running → AwaitingReview → Accepted/Rejected/Applied`, with `Running → Cancelled/Failed`; terminal states reject later updates. Validate progress is at most 100 and retain a bounded recent history, without storing credentials or full conversations.

- [x] **Step 4: Add transition and bounds tests, then rerun the focused tests**

Run: `cargo test -p tuner-app agent_tasks::tests`

Expected: PASS for cancellation, terminal-state rejection, valid progress, and the `Running → AwaitingReview` transition.

- [x] **Step 5: Review the new module boundary**

Confirm proposal operations are typed and contain no arbitrary paths, raw byte offsets, shell commands, or code payloads. Keep agent state separate from `OperationCoordinator`, which tracks one app-owned background operation.

### Task 2: Add typed agent messages to NookLink

**Files:**
- Modify: `crates/tuner-app/src/ui_ipc.rs`
- Modify: `crates/tuner-app/src/lib.rs`, especially `TunerApp`, `handle_ui_ipc_request`, `ui_ipc_capabilities`, and `ui_ipc_state`.
- Test: `ui_ipc.rs` parsing tests and `lib.rs` app-level tests.

**Interfaces:**
- `UiIpcRequest` gains `#[serde(default)] agent_task: Option<AgentTaskCommand>` and optional `task_id`; existing requests remain valid. The tagged `kind` payload is `start`, `progress`, `submit`, `poll`, or `fail`.
- Adds the read-only `read_bin_bytes` action using `task_id`, `byte_offset`, and `byte_length`. It requires an approved `Running` task with raw-read authorization whose captured BIN hash still matches the active BIN; it uses a zero-based half-open range and caps one response at 65,536 bytes. Missing, pending, unauthorized, terminal, or cross-BIN task IDs are denied. The first unauthorized read stages an in-app challenge and returns an `approval_required` error; retry succeeds after the user solves it.
- The top-level action is `agent_task`. The agent sends `Start`, `Progress`, `Submit`, `Poll`, or `Fail`; `Start` creates a pending request. The user chooses quick start (no puzzle) or long/complex start (challenge required) in TunerNook. A raw-byte request needs a separate task-bound challenge. No IPC command can solve or approve a challenge.
- `TunerApp` stores `AgentTaskStore`; `ui_ipc_state` includes task IDs, capability, status, phase, progress, report, document hashes, and review decision, but never the endpoint token.
- Task request captures the current BIN SHA-256, exact XDF SHA-256 when present, and `WorkspaceState::data_revision()` so in-memory conversion overrides and BIN edits make pending proposals stale. The agent is not launched by TunerNook; the user first asks through their configured agent client.

- [x] **Step 1: Add a serde test for the nested task envelope**

```rust
#[test]
fn parses_agent_task_start_without_affecting_legacy_requests() {
    let request: UiIpcRequest = serde_json::from_str(
        r#"{"action":"agent_task","agent_task":{"kind":"start","capability":"diagnostics","goal":"check mappings"}}"#,
    ).unwrap();
    assert!(matches!(request.agent_task, Some(AgentTaskCommand::Start { .. })));
    assert!(serde_json::from_str::<UiIpcRequest>(r#"{"action":"ping"}"#).is_ok());
}
```

- [x] **Step 2: Run the focused parser test and confirm it fails**

Run: `cargo test -p tuner-app parses_agent_task_start_without_affecting_legacy_requests`

Expected: FAIL because `UiIpcRequest` has no `agent_task` field or typed message.

- [x] **Step 3: Add the optional typed field and dispatch**

Add the serde-defaulted task field and `task_id`, advertise `"agent_task"` and `"read_bin_bytes"`, and route task messages to `AgentTaskStore`. A `Start` request creates a pending task; reject malformed transitions and progress above 100 with a normal `UiIpcResponse::error`. Implement `read_bin_bytes` from only `workspace.bin`: require an approved running task with raw-read authorization, require its captured BIN SHA-256 to match the active BIN, then require `byte_offset` and `byte_length`, reject zero length, reject lengths above 65,536, use `checked_add`, and reject any end past `bin.len()`. Return only `{ offset, length, hex, bin_sha256, dirty }`; do not accept a file path or expose an approval command. On a valid range request without raw authorization, stage a UI-only challenge and return an error telling the agent to wait and retry. Every request must complete promptly; long analysis stays in the external agent, which reports progress through subsequent requests.

- [x] **Step 4: Test IPC state, cancellation polling, and capability visibility**

Add app-level tests that begin a task and verify it remains pending, deny reads with missing/pending/unauthorized/terminal/cross-BIN task IDs, grant raw-read authorization through the app-only store transition, then verify a valid read returns exact bytes and current BIN hash. Also test progress/cancellation and late-update rejection, unknown capabilities, progress above 100, missing BIN, overflowed/out-of-range reads, and the 65,536-byte limit. Assert state includes task status but no challenge answer/approval operation, and no task action exposes `UiIpcEndpoint.token`.

Run: `cargo test -p tuner-app ui_ipc_agent_task`

Expected: PASS; existing `ui_ipc` authentication and JSONL tests remain unchanged and pass.

### Task 3: Add setup guidance and a task monitor

**Files:**
- Modify: `crates/tuner-app/src/lib.rs` Tools menu, `TunerApp` initialization, and `eframe::App::ui` window rendering.
- Create: `docs/nooklink-agent-setup.md`
- Modify: `README.md` and `docs/tuner-ui-ipc.md` to link to setup guidance.
- Test: headless app tests in `lib.rs`.

**Interfaces:**
- Tools → NookLink opens a setup/task window; it does not install, launch, or contact an agent.
- The setup view distinguishes the file-oriented `tuner-api/v1` process from the live `tuner-ui/v1` bridge, explains how the user configures their chosen agent, and shows current app actions from `ui_ipc_capabilities`.
- A pending task shows its capability, goal, and captured project identity. “Start quick task” proceeds without a puzzle; “Mark long/complex” requires solving a small in-app arithmetic challenge. The agent cannot set the long flag or approve itself. A first raw-byte request stages a separate challenge; ordinary state/table/map reads do not. NookLink-requested BIN and XDF Save As actions require a task ID, matching active document identity, and a fresh challenge. Generic dispatch of `file.save-as` or `file.save-xdf-as` is rejected in favor of the task-bound actions; native user dialogs remain unchanged. Challenge material and answer are held only in app UI state and are not serialized into IPC state. The challenge is friction, not proof-of-human.
- The app owns private `NookLinkChallengeAction::{LongStart { task_id }, RawRead { task_id }, SaveAs { task_id, path, bin_sha256 }}` and `NookLinkChallenge { action, prompt, expected_answer }`; one UI-only completion handler executes the associated action after a correct answer. Task 4 extends the enum with `ApplyProposal { task_id }`.
- The task view displays agent-reported progress as such, the task's project identity, and its current status. User cancellation is visible to the agent on its next poll.

- [x] **Step 1: Add a headless test for the unconfigured state**

```rust
#[test]
fn nooklink_setup_is_available_without_starting_or_configuring_an_agent() {
    let app = TunerApp::headless();
    assert!(app.agent_tasks.is_empty());
    assert!(app.nooklink_setup_open == false);
    assert!(app.ui_ipc_capabilities()["actions"].as_array().unwrap()
        .iter().any(|action| action == "agent_task"));
}
```

- [x] **Step 2: Run the focused setup test to confirm it fails**

Run: `cargo test -p tuner-app nooklink_setup_is_available_without_starting_or_configuring_an_agent`

Expected: FAIL because setup/task window state has not been added.

- [x] **Step 3: Add the setup/task window and provider-neutral instructions**

Add a Tools menu entry and window state, display the manifest path and local protocol names without displaying the token, and explain: configure the chosen agent with the NookLink bridge, inspect its advertised capabilities, then ask that agent explicitly. Add a task list with phase, agent-reported progress, goal, and Cancel for running tasks. For pending requests, offer quick start without a puzzle or a user-marked long/complex start that requires a correct challenge. Show a separate UI-only challenge for raw-byte access. Proposal application and NookLink-requested BIN/XDF Save As must also pass a fresh challenge in addition to existing user review/output rules. Generic `dispatch` cannot invoke native Save As dialogs directly. No challenge answer is accepted over IPC. Do not claim the puzzle defeats UI-controlling agents or grants filesystem isolation. Do not add provider-specific auto-installation.

- [x] **Step 4: Test setup and task rendering/state through the existing headless UI test pattern**

Verify that opening the setup view does not create a task or perform an external call, and that a running task appears with its progress and cancel control. Run: `cargo test -p tuner-app nooklink_`

Expected: PASS for setup and monitor tests; no configured agent is a normal supported state.

- [x] **Step 5: Write concise setup instructions**

In `docs/nooklink-agent-setup.md`, document the user-owned sequence: open TunerNook, read setup, configure the chosen agent, ask it explicitly, choose quick or long/complex task level, and review its result. Explain that ordinary state/table/map reads do not require a puzzle; raw byte reads, proposal apply, and NookLink-requested BIN/XDF Save As do. State that the local token must remain private and independent filesystem access/full UI automation is outside the puzzle's protection.

### Task 4: Review and atomically apply approved proposals

**Files:**
- Modify: `crates/tuner-app/src/agent_tasks.rs`
- Modify: `crates/tuner-app/src/lib.rs`, adding a proposal preview/apply path to `WorkspaceState` and review controls to the NookLink task window.
- Test: task-model and app-level tests in `agent_tasks.rs` and `lib.rs`.

**Interfaces:**
- An agent submits an `AgentReport` with findings, evidence references, optional confidence, and zero or more typed `SetEngineeringCell` operations.
- `WorkspaceState::preview_agent_operations(&[ProposedOperation])` validates the entire proposal without changing the active BIN.
- `WorkspaceState::apply_agent_operations(&[ProposedOperation])` revalidates, writes every cell in one `BinDocument::transaction`, and commits only if all writes are valid.
- `AgentTaskStore::mark_stale(task_id)` is app-only; it transitions `AwaitingReview` or `Accepted` to `Stale` and clears raw-read authorization when document identity or proposal validation fails.
- The UI offers Approve/Reject only after a proposal is in `AwaitingReview`. Applying operations requires the user to solve a fresh in-app challenge after reviewing the exact preview. NookLink agent messages cannot approve their own proposals.

- [x] **Step 1: Write stale-document and atomicity tests**

Create proposal tests using the existing BIN/XDF fixture helpers: changing either the BIN hash or exact XDF hash makes approval fail without changing bytes; a proposal with a valid cell followed by an out-of-range cell leaves bytes and `undo_depth` unchanged.

- [x] **Step 2: Run the tests to confirm they fail before implementation**

Run: `cargo test -p tuner-app agent_proposal_`

Expected: FAIL because there is no NookLink proposal apply path.

- [x] **Step 3: Implement document-bound validation and preview**

At task start, retain the active BIN hash, exact XDF hash, and `WorkspaceState::data_revision()`. On proposal submission, review, and after the Apply challenge, compare them with current state; the revision catches in-memory conversion-override changes that do not modify the XDF file. Resolve every `semantic_id`, verify each row/column, require finite engineering values, and dry-run writes on a cloned BIN transaction. Mark a changed/missing document or revision as `Stale` and disable Apply; do not silently rebase.

- [x] **Step 4: Implement one atomic approval transaction**

Use `ParameterDefinition::write_engineering_cell` for every proposed cell in one `BinDocument::transaction("approved NookLink proposal")`. Abort on the first error; commit once after all writes pass. Then update workspace revision/validation, invalidate search caches, mark the task `Applied`, and leave Save As explicit.

- [x] **Step 5: Add approve/reject UI and rerun the proposal tests**

Show summary, findings, evidence references, confidence/uncertainty, and exact proposed cell/value changes. Reject records the decision without modifying BIN or undo history. Apply is enabled only when preview and document-identity checks pass and the user solves a fresh in-app challenge. NookLink-requested Save As also requires a fresh challenge and keeps explicit-output/no-overwrite rules.

Run: `cargo test -p tuner-app agent_proposal_`

Expected: PASS for preview, stale rejection, atomic rollback, approved single-transaction undo, and reject-without-write cases.

### Task 5: Record decisions, finish protocol docs, and run regressions

**Files:**
- Modify: `crates/tuner-app/src/agent_tasks.rs` for bounded review history.
- Modify: `crates/tuner-app/src/lib.rs` for project-scoped history and debug-report summary.
- Modify: `crates/tuner-app/src/ui_ipc.rs` only if final response fields need serialization changes.
- Modify: `docs/nooklink-agent-setup.md`, `docs/tuner-ui-ipc.md`, and `README.md`.
- Test: `crates/tuner-app/src/agent_tasks.rs` and `crates/tuner-app/src/lib.rs`.

**Interfaces:**
- Keep the most recent 50 decision summaries per BIN project; retain capability, task/status, BIN/XDF hashes plus `workspace_data_revision`, validated evidence references, decision, and applied-operation summary. Do not persist full conversations, endpoint tokens, challenge material, unvalidated evidence strings, or agent credentials.
- Expose the same decision summary in the debug report so a user can share the outcome without losing provenance.

- [x] **Step 1: Test serialization and bounded history**

Add tests that round-trip task decision summaries through `ProjectPreferences`, retain no more than 50 entries, and verify that serialized history contains no token/credential fields.

- [x] **Step 2: Implement project-scoped history**

Add a serde-defaulted `Vec<AgentTaskHistoryItem>` to `ProjectPreferences`, bump `PROJECT_SETTINGS_VERSION`, sanitize the list and text lengths, and append a summary on accept, reject, cancel, stale, failure, or apply. Store capability, BIN/XDF identity, evidence references, decision, and operation summary; do not save agent setup credentials or conversations.

- [x] **Step 3: Add history to diagnostics and finish the protocol examples**

Add a short NookLink task summary to `debug_report_text`; document task start/progress/submit/poll, user cancellation, proposal review, limits, and the fact that the external agent must be configured by the user. Link setup docs from the README and UI IPC guide.

- [x] **Step 4: Run focused serialization and diagnostic tests**

Run: `cargo test -p tuner-app agent_task_history`

Expected: PASS; loading old project-preference JSON defaults history to empty.

- [x] **Step 5: Run final verification once**

Run: `cargo fmt --all -- --check`  
Run: `cargo test -p tuner-app`  
Run: `cargo test --workspace`  
Run: `cargo build --release -p tuner-app`

Expected: formatting clean, all tests pass, and the release executable links. Do not close or replace an already-running TunerNook process; if Windows locks the release executable, leave it running and report that the user must close it before rebuilding that exact path.

## Deliberately deferred

- Agent-specific PPC/ARM/other-ISA reverse-engineering engines; agents may choose methods, while later capability slices define specific evidence tools.
- Runtime plugin/add-on authoring, loading, permissions, and isolation.
- The separately activated Live Helper/trainer.
- Automatic agent installation, launch, selection, or vendor-specific setup flows.
- Arbitrary agent code execution, shell access, direct byte-write proposal operations, and model/vendor dependencies.

## Execution note

This plan assumes the external agent is invoked by the user and then calls NookLink. TunerNook does not run or select the agent. This workspace currently has no Git metadata, so the plan uses reviewable task boundaries but does not prescribe Git commits.
