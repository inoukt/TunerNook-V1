# NookLink Agent Quick Start

## Goal

Make it easy for a user to start their own configured external agent with enough
accurate TunerNook context to use NookLink, without making NookLink launch or
configure that agent.

## Approved behavior

- The NookLink setup window shows a short numbered quick start.
- **Copy starter prompt** puts a provider-neutral prompt on the system clipboard.
- **Save context file…** writes a bundled, concise Markdown context file to a
  user-selected path so it can be attached to the agent conversation.
- The context describes TunerNook, current NookLink capability areas, how the
  agent learns available actions at runtime, the user approval workflow, and
  safety limits. It points to the detailed setup and protocol references.
- Copy/export feedback uses the app status area; clipboard and file errors are
  surfaced as warnings. Reuse the clipboard backend already in the dependency
  graph; no new package, NookLink protocol, or public API changes.

## Boundaries

The prompt and context are agent/vendor-neutral. The user remains responsible
for selecting/configuring/starting the external agent, approving task starts,
and reviewing results. Broader agent capability/intelligence design is deferred.
