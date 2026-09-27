# NookLink agent setup

NookLink is optional. TunerNook works normally without a configured agent.

## Quick start

1. Choose and configure your external agent to use the local NookLink connection
   shown under **Tools → NookLink**. TunerNook does not launch or configure it.
2. In that window, choose **Save context file…** and attach the saved
   `NookLink-Agent-Context.md` to your agent conversation.
3. Choose **Copy starter prompt**, paste it into the conversation, and replace
   the goal placeholder with what you want done.
4. Ask your agent to work. Approve task starts and review its findings in
   TunerNook.

The context file travels with the app and explains its current capabilities and
approval boundaries. Keep the local endpoint token private.

## Technical setup and safety

1. Open TunerNook and choose **Tools → NookLink**. The window shows the local endpoint manifest path, supported desktop actions, and task progress.
2. Configure your chosen external agent to use the local protocol. `tuner-api/v1` is a file-oriented JSONL process; `tuner-ui/v1` controls the running desktop app through its local authenticated endpoint. See [desktop protocol details](tuner-ui-ipc.md). Keep the manifest token private and out of BIN-specific project files.
3. Explicitly ask that agent to perform a task for the selected project. TunerNook does not select, launch, or contact an agent. Each task starts pending in the NookLink window. Choose **Start quick task** or **Long/complex task**; the latter requires a fresh in-app arithmetic challenge.
4. Watch the agent-reported phase and progress in TunerNook. You can cancel a pending or running task; the agent sees cancellation on its next Poll. Review the returned findings and any proposed changes in TunerNook before applying them.

Quick tasks and ordinary state, table, and map inspection are puzzle-free. User-marked Long/complex task starts, direct `read_bin_bytes` ranges, proposal Apply, and NookLink-requested BIN or XDF Save As each require a fresh in-app challenge. A raw read also requires a running task, matching active BIN identity, and a nonempty range of at most 65,536 bytes within the loaded in-memory BIN. The task's raw-read authorization is cleared on submit, failure, or cancellation. BIN Save As requires a running task; XDF Save As requires a running task for the active source XDF or an applied `xdf_authoring` task for its unchanged draft revision. Both display the exact destination for confirmation and recheck identity before saving. Generic `dispatch` cannot invoke native Save As dialogs. Use the task-bound `save_as` or `save_xdf_as` action with an explicit destination path; native user-initiated Save As remains available.

For `tuner-ui/v1`, the external agent sends `agent_task` Start with a capability and goal, then receives a task ID with status `awaiting_user_approval`. After you approve the start in the NookLink window, it sends Progress updates, Submit with a report, Poll to observe status, or Fail with an error message. Submit moves the task to `awaiting_review`; only you can accept or reject the report in the app. Applying proposed edits has its own challenge and uses the validated undoable edit path.

For XDF authoring, the agent can use the dedicated `xdf_authoring` capability and submit typed definition changes—never raw XML. TunerNook previews the proposed changes in the XDF Maker / Editor; your approval applies them to its in-memory draft. Saving is a separate `save_xdf_as` request with a fresh challenge, bound to the exact approved draft revision and an explicit new path. The original XDF is never overwritten. The task protocol and sample JSON are in [desktop protocol details](tuner-ui-ipc.md).

Each BIN project's settings retain at most 50 decision summaries: task ID, capability, status, captured BIN/XDF hashes and workspace revision, bounded evidence references, decision, and applied-operation summary. Old project settings load with an empty history. Full prompts, conversations, report text, manifest tokens, credentials, and challenge prompts/answers are not stored in that history or the debug report. A saved revision is provenance only; reopening history never authorizes a write.

The arithmetic challenge is a friction and consent checkpoint, not proof of human presence. It is not exposed through IPC. An agent with full UI automation may solve it, and separate filesystem permissions can grant independent access. NookLink stays within the existing local authenticated control boundary. Agent credentials are not stored in BIN-specific project files. During development, agents may help implement permanent built-in functions through ordinary source changes, tests, and review; runtime code generation is not part of this workflow. Runtime add-on loading and Live Helper require separate opt-in designs before implementation.
