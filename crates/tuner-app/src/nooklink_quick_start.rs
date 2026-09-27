use std::fs;
use std::io;
use std::path::Path;

pub(super) const AGENT_CONTEXT: &str = include_str!("../../../docs/nooklink-agent-context.md");

pub(super) const STARTER_PROMPT: &str = concat!(
    "I’m using TunerNook with NookLink. Attach the NookLink Agent Context file to this conversation and read it before acting. ",
    "Help me with this goal: [describe your task]. First check which NookLink actions are available in the running app. ",
    "Ask me if the goal or required access is unclear. Use only supported NookLink actions, explain your findings and evidence, ",
    "and wait for in-app approval or review before proceeding when requested. Never try to bypass an in-app challenge. ",
    "TunerNook does not launch or configure the external agent."
);

pub(super) fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
    clipboard
        .set_text(text.to_string())
        .map_err(|error| error.to_string())
}

pub(super) fn write_context_file(path: &Path) -> io::Result<()> {
    fs::write(path, AGENT_CONTEXT)
}
