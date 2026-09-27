# NookLink Agent-Assisted Workflow Design

Date: 2026-09-23  
Status: Approved design; human-checkpoint detail clarified 2026-09-23

## Goal

Make TunerNook an excellent native calibration editor that can delegate long,
ambiguous, or specialized work to an agent when the user asks. The agent returns
a supported recommendation; the user remains the decision-maker and TunerNook
remains responsible for validating and applying approved changes safely.

## Product principle

TunerNook should not require an agent for ordinary editing, browsing,
visualization, validation, or other deterministic workflows. Native app code
owns those fast and repeatable operations. The agent is an optional specialist
for work whose reasoning may depend on the ECU family, processor, ROM layout,
or the agent's own capabilities.

NookLink is the neutral communication and control layer—not a model, agent
runtime, or processor-specific reverse-engineering engine. It builds on the
existing local `tuner-api/v1` file/workspace API and `tuner-ui/v1` live desktop
control bridge. Capabilities are discovered rather than assumed, and the user
may use TunerNook without connecting an agent.

## Approaches considered

1. **Implement complex analysis inside TunerNook.** Predictable and testable,
   but every ECU family and processor-specific technique would become app code
   to maintain.
2. **Let an agent control files and the app directly.** Flexible, but makes
   safety, reproducibility, and user review too dependent on agent behavior.
3. **Hybrid, evidence-first workflow (selected).** TunerNook supplies reliable
   read-only context and constrained actions; the user chooses when to ask an
   agent; the agent analyzes and proposes; TunerNook validates and the user
   approves before the app applies changes. This supports different agents and
   platforms without surrendering the app's safety guarantees.

## User flow

1. **Set up an agent.** Before the first request, TunerNook explains how to
   connect/configure an available agent, what access it receives, and which
   capabilities are supported. Setup is explicit; no agent is silently selected
   or contacted.
2. **Ask for help.** The user asks their configured agent and supplies or
   confirms the scope and relevant open project. An agent's task-start request
   only creates a pending request; it does not authorize BIN-byte access.
3. **Choose task level.** TunerNook shows the requested capability and goal.
   A quick/read-only task can start without the puzzle; the user marks a task
   long/complex in the app and solves the puzzle before it starts. The agent
   cannot set this choice on the user's behalf.
4. **Confirm sensitive steps.** Ordinary state, table, and map inspection does
   not require the puzzle. Direct raw byte-range reads, applying a proposed
   modification, and a NookLink-requested BIN or XDF Save As do. The answer and approval
   action are not available through NookLink IPC. The puzzle is a
   human-friction checkpoint, not proof that a person is present: an agent with
   independent file access or full UI automation may bypass or solve it.
5. **Work visibly.** Tasks expose their current phase,
   progress when available, cancellation, and a link to the evidence/context
   being examined. The UI remains responsive.
6. **Review the result.** The agent returns findings, supporting evidence,
   uncertainty/limitations, and proposed app operations. The result is a
   proposal, not an applied edit.
7. **Approve or reject.** TunerNook verifies the proposal against current BIN
   and XDF identities, previews its effects, and asks the user. Rejecting or
   cancelling leaves project data unchanged.
8. **Apply safely.** After the user reviews the proposal and solves the
   checkpoint, TunerNook performs supported changes using
   existing validation, transaction, undo/history, and explicit Save As rules.
   The operation and its agent/evidence provenance are recorded for review.

## Capability areas

NookLink is intended to support distinct capabilities over time:

- **Diagnostics:** explain app/BIN/XDF state and investigate reported problems.
- **Tuning help:** interpret tables and explain possible adjustments without
  treating a suggestion as verified calibration advice.
- **Automation editor:** help create or revise user-requested workflows/scripts;
  execution and any file changes remain reviewable and approval-gated.
- **Reverse-engineering search:** investigate unknown maps/axes using evidence
  from the selected ROM. The agent, not a PPC-only app heuristic, chooses the
  analysis method appropriate to the target. Results must distinguish observed
  bytes/references from inferred labels and confidence.
- **Developer-mode feature building (now):** while developing TunerNook, an
  agent may help implement permanent built-in functions through ordinary source
  changes, tests, and review. The user decides whether the changes are accepted
  into the product and released. This is a development workflow, not runtime
  code generation or loading inside the user's running app.
- **Future Live Helper:** a separately activated mode for contextual tuning and
  workflow recommendations, UI help, and training. It is advisory and cannot
  silently edit BIN/XDF data. While enabled, it may surface contextual advice;
  deeper investigations or changes still require the user to start a task.
- **Future add-on authoring:** optional, separately installable features may be
  explored later. They are not part of the initial NookLink workflow; any
  runtime extension mechanism needs its own permission, trust, compatibility,
  and isolation design before implementation.

These are capability families, not promises that every connected agent can do
each task. NookLink must report the available agent's capabilities and avoid
presenting unsupported actions as ready.

## Boundaries and safety

- User setup and explicit per-task invocation are required for task-oriented
  agent work. Long/complex starts, direct raw byte-range reads, proposal
  application, and NookLink-requested BIN/XDF saves require an in-app checkpoint.
  Ordinary state/table/map inspection does not. Raw reads are task-bound and
  limited to the same active BIN identity. Contextual advice may appear only
  while the user has separately activated the future Live Helper; it cannot
  apply changes.
- The checkpoint's answer and approval action are never exposed as NookLink
  IPC commands. The small puzzle is an added consent/friction step, not a
  cryptographic or reliable proof-of-human measure. It cannot prevent access
  that the user separately grants through filesystem permissions, nor can it
  guarantee resistance to an agent that can operate the app UI.
- Agent inspection is scoped to the selected project and requested task; no
  general filesystem or network access is implied by NookLink.
- Evidence refers to stable document identity (BIN/XDF hashes where available),
  byte ranges, parameter IDs, and operation IDs so the user can inspect the
  basis of a finding.
- Before applying a proposal, TunerNook rechecks the source identities and
  workspace data revision, including in-memory conversion overrides, and
  validates every operation. A stale or invalid proposal is rejected for
  refresh/review, never silently rebased.
- Agent-proposed edits use the same app/core validation and undo transaction
  paths as native edits. Existing explicit-output and no-overwrite rules remain
  in force.
- TunerNook does not claim an agent's confidence is proof. Unverified maps,
  axes, formulas, and automation remain visibly marked as proposals.
- Long operations are cancellable where the underlying agent/API supports it;
  cancellation must not partially apply a proposal.
- NookLink stays local and authenticated using the existing local-control
  boundaries. Agent credentials are not stored in BIN-specific project files.

## Initial implementation scope

The first implementation should establish the reusable user-led task and review
workflow, not attempt to build a universal reverse-engineering engine or the
Live Helper or a runtime add-on loader. Permanent product features continue to
be built in developer mode through source changes, tests, and user review. The
NookLink workflow should cover:

1. Agent setup guidance and capability/status discovery.
2. A user-invoked task lifecycle with progress, cancellation, and operation IDs.
3. A structured proposal/evidence result tied to the current BIN/XDF identity.
4. Review, approve, reject, stale-result handling, and safe application through
   existing transactions.
5. Agent-control documentation and tests for invocation, cancellation,
   approval, rejection, and stale project state.

Diagnostics, tuning assistance, automation editing, and reverse-engineering
search can then be added as independently reviewable capabilities using this
contract. The future Live Helper and runtime add-on authoring are separate
project slices after the task and approval workflow proves useful.

## Acceptance criteria

1. TunerNook works normally when no agent is configured.
2. The user sees setup guidance before the first agent request and explicitly
   asks for every task. Quick/read-only tasks can start without the puzzle;
   user-marked long/complex tasks require it. Ordinary state/table/map reads
   remain available, while direct raw reads, proposal application, and
   NookLink-requested BIN/XDF saves require the in-app checkpoint. No challenge answer
   or approval is accepted by IPC.
3. Agent capability discovery is visible and unsupported capabilities are not
   offered as executable actions.
4. A long task reports status and can be cancelled without applying partial
   edits.
5. A completed task presents evidence, uncertainty, and proposed changes for
   user review; rejecting it leaves BIN/XDF data and undo history unchanged.
6. Approval is bound to the BIN/XDF identity and effective workspace data
   revision used to create the proposal. If either document or its conversion
   override state changes, TunerNook requires a refreshed proposal.
7. Approved operations use existing validation and undoable transactions and
   never overwrite an original file implicitly.
8. Task and decision history identifies the agent capability, project
   identity, evidence references, approval decision, and applied operation.
9. The NookLink contract remains agent-neutral and can support multiple
   processor families without putting ISA assumptions in the shared workflow.
10. Agent-assisted built-in feature development uses source changes and tests;
    accepting/releasing those changes remains a developer/user decision.
11. Runtime add-on loading and Live Helper behavior are excluded from the first
    implementation; both require separate opt-in designs before implementation.
