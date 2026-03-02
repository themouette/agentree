//! Agent abstraction — mirrors the `Backend` / `BackendType` pattern in `crate::backend`.
//!
//! Each agent implementation encapsulates the workspace-level setup and teardown
//! hooks specific to that agent tool (e.g., injecting `CLAUDE.md` blocks,
//! updating `.claude/settings.json`). The [`AgentType`] enum dispatcher
//! provides a single concrete type that the call site can use without knowing
//! which agent is active.

use crate::error::Result;
use std::path::Path;

mod claude;
mod generic;

pub use claude::ClaudeAgent;
pub use claude::ClaudeToken;
pub use generic::GenericAgent;

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
    Generic(()),
}

/// Concrete agent implementation dispatcher.
///
/// Mirrors [`crate::backend::BackendType`]: a single enum whose variants hold
/// the concrete agent structs.  The [`Agent`] impl delegates every call to the
/// inner variant.
pub enum AgentType {
    Claude(ClaudeAgent),
    Generic(GenericAgent),
}

impl AgentType {
    /// Resolve a logical agent name to its concrete implementation.
    ///
    /// * `"claude"` → [`AgentType::Claude`]
    /// * any other name → [`AgentType::Generic`]
    pub fn from_name(name: &str) -> Self {
        match name {
            "claude" => AgentType::Claude(ClaudeAgent::new()),
            _ => AgentType::Generic(GenericAgent::new()),
        }
    }
}

impl Agent for AgentType {
    type PrepareToken = AgentToken;

    fn prepare(&self, workspace_path: &Path) -> Result<AgentToken> {
        match self {
            AgentType::Claude(a) => a.prepare(workspace_path).map(AgentToken::Claude),
            AgentType::Generic(a) => a.prepare(workspace_path).map(AgentToken::Generic),
        }
    }

    fn cleanup(&self, workspace_path: &Path, token: &AgentToken) {
        match (self, token) {
            (AgentType::Claude(a), AgentToken::Claude(t)) => a.cleanup(workspace_path, t),
            (AgentType::Generic(a), AgentToken::Generic(t)) => a.cleanup(workspace_path, t),
            // Safety valve for mismatched variants (should never occur in normal usage).
            _ => {}
        }
    }

    fn name(&self) -> &str {
        match self {
            AgentType::Claude(a) => a.name(),
            AgentType::Generic(a) => a.name(),
        }
    }
}
