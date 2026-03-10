//! Agent abstraction — mirrors the `Backend` / `BackendType` pattern in `crate::backend`.
//!
//! Each agent implementation encapsulates the workspace-level setup and teardown
//! hooks specific to that agent tool (e.g., injecting `CLAUDE.md` blocks,
//! updating `.claude/settings.json`). The [`AgentType`] enum dispatcher
//! provides a single concrete type that the call site can use without knowing
//! which agent is active.

use crate::error::{AgentreeError, Result};
use std::path::Path;

mod claude;
mod generic;
mod opencode;

pub use claude::ClaudeAgent;
pub use claude::ClaudeToken;
pub use generic::GenericAgent;
pub use opencode::OpencodeAgent;
pub use opencode::OpencodeToken;

// ─── Shared constants used by multiple agent implementations ─────────────────

/// Opening marker for an agentree-injected block in workspace context files
/// (e.g. `CLAUDE.md`, `AGENTS.md`).
pub(crate) const AGENTREE_START: &str = "<!-- agentree:start -->";

/// Closing marker for an agentree-injected block.
pub(crate) const AGENTREE_END: &str = "<!-- agentree:end -->";

/// Marker embedded in every agentree-owned hook command so that `cleanup` can
/// identify and remove exactly these entries without touching user-defined hooks.
pub(crate) const AGENTREE_HOOK_MARKER: &str = "# agentree-hook";

// ─── Shared helpers used by multiple agent implementations ───────────────────

/// Inject an agentree block into the workspace context file at `path`.
///
/// The content is wrapped in XML markers so it can be cleanly extracted later:
/// ```text
/// <!-- agentree:start -->
/// <template content>
/// <!-- agentree:end -->
/// ```
///
/// Idempotent: if `<!-- agentree:start -->` is already present, does nothing.
/// Appends to an existing file; creates the file if it does not exist.
///
/// The caller supplies the `template` content string (typically via
/// `include_str!` from the appropriate template file).
pub(crate) fn inject_agentree_block(path: &Path, template: &str) -> Result<()> {
    let block = format!("{}\n{}{}\n", AGENTREE_START, template, AGENTREE_END);

    if path.exists() {
        let content = std::fs::read_to_string(path).map_err(AgentreeError::Io)?;
        if content.contains(AGENTREE_START) {
            return Ok(()); // already injected
        }
        let separator = if content.ends_with('\n') {
            "\n"
        } else {
            "\n\n"
        };
        std::fs::write(path, format!("{}{}{}", content, separator, block))
            .map_err(AgentreeError::Io)?;
    } else {
        std::fs::write(path, &block).map_err(AgentreeError::Io)?;
    }
    Ok(())
}

/// Remove the `<!-- agentree:start -->…<!-- agentree:end -->` block from `path`.
///
/// Non-fatal: all errors are silently ignored.
/// Deletes the file if it becomes empty (only whitespace) after removal.
pub(crate) fn remove_agentree_block(path: &Path) {
    if !path.exists() {
        return;
    }
    let content = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return,
    };

    let start_pos = match content.find(AGENTREE_START) {
        Some(p) => p,
        None => return,
    };
    let end_pos = match content.find(AGENTREE_END) {
        Some(p) => p,
        None => return,
    };

    // Guard against malformed files where the end marker precedes the start.
    if end_pos < start_pos + AGENTREE_START.len() {
        return;
    }

    // Byte offset just past `<!-- agentree:end -->`, skipping one trailing newline
    let end_byte = end_pos + AGENTREE_END.len();
    let end_byte = if content.as_bytes().get(end_byte) == Some(&b'\n') {
        end_byte + 1
    } else {
        end_byte
    };

    let before = &content[..start_pos];
    let after = &content[end_byte..];

    let remaining = match (before.trim().is_empty(), after.trim().is_empty()) {
        (true, true) => String::new(),
        (true, false) => after.trim_start_matches('\n').to_string(),
        (false, true) => format!("{}\n", before.trim_end_matches('\n')),
        (false, false) => format!(
            "{}\n{}",
            before.trim_end_matches('\n'),
            after.trim_start_matches('\n')
        ),
    };

    if remaining.trim().is_empty() {
        let _ = std::fs::remove_file(path);
    } else {
        let final_content = if remaining.ends_with('\n') {
            remaining
        } else {
            remaining + "\n"
        };
        let _ = std::fs::write(path, final_content);
    }
}

/// Per-agent workspace setup/teardown contract.
///
/// # Lifecycle
///
/// 1. Call [`Agent::prepare`] before starting the agent process. The returned
///    [`Agent::PrepareToken`] records what was done.
/// 2. Run the agent process.
/// 3. Call [`Agent::cleanup`] with the token after the process exits to undo
///    any changes made in `prepare`.
pub trait Agent {
    /// Opaque token returned by `prepare`, consumed by `cleanup`.
    type PrepareToken;

    /// Set up workspace files required by this agent.
    ///
    /// Returns a token that `cleanup` uses to undo exactly what was done.
    fn prepare(&self, workspace_path: &Path) -> Result<Self::PrepareToken>;

    /// Undo changes made by `prepare`.
    ///
    /// Errors are silently ignored — cleanup is best-effort.
    fn cleanup(&self, workspace_path: &Path, token: &Self::PrepareToken);

    /// Human-readable agent name (e.g. `"claude"`, `"generic"`).
    fn name(&self) -> &str;
}

/// Opaque token produced by [`AgentType::prepare`].
///
/// Wraps the inner agent's token so that [`AgentType::cleanup`] can pass
/// the correct variant back to the inner implementation.
pub enum AgentToken {
    Claude(ClaudeToken),
    Opencode(OpencodeToken),
    Generic(()),
}

/// Concrete agent implementation dispatcher.
///
/// Mirrors [`crate::backend::BackendType`]: a single enum whose variants hold
/// the concrete agent structs.  The [`Agent`] impl delegates every call to the
/// inner variant.
pub enum AgentType {
    Claude(ClaudeAgent),
    Opencode(OpencodeAgent),
    Generic(GenericAgent),
}

impl AgentType {
    /// Resolve a logical agent name to its concrete implementation.
    ///
    /// * `"claude"` → [`AgentType::Claude`]
    /// * `"opencode"` → [`AgentType::Opencode`]
    /// * any other name → [`AgentType::Generic`]
    pub fn from_name(name: &str) -> Self {
        match name {
            "claude" => AgentType::Claude(ClaudeAgent::new()),
            "opencode" => AgentType::Opencode(OpencodeAgent::new()),
            _ => AgentType::Generic(GenericAgent::new()),
        }
    }
}

impl Agent for AgentType {
    type PrepareToken = AgentToken;

    fn prepare(&self, workspace_path: &Path) -> Result<AgentToken> {
        match self {
            AgentType::Claude(a) => a.prepare(workspace_path).map(AgentToken::Claude),
            AgentType::Opencode(a) => a.prepare(workspace_path).map(AgentToken::Opencode),
            AgentType::Generic(a) => a.prepare(workspace_path).map(AgentToken::Generic),
        }
    }

    fn cleanup(&self, workspace_path: &Path, token: &AgentToken) {
        match (self, token) {
            (AgentType::Claude(a), AgentToken::Claude(t)) => a.cleanup(workspace_path, t),
            (AgentType::Opencode(a), AgentToken::Opencode(t)) => a.cleanup(workspace_path, t),
            (AgentType::Generic(a), AgentToken::Generic(t)) => a.cleanup(workspace_path, t),
            // Safety valve for mismatched variants (should never occur in normal usage).
            _ => {}
        }
    }

    fn name(&self) -> &str {
        match self {
            AgentType::Claude(a) => a.name(),
            AgentType::Opencode(a) => a.name(),
            AgentType::Generic(a) => a.name(),
        }
    }
}
