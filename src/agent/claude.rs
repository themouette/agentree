use crate::error::{AgentreeError, Result};
use std::path::Path;

use super::Agent;

/// Token returned by `ClaudeAgent::prepare`.
///
/// Tracks whether `.claude/` was created by `prepare` so that
/// `cleanup` can remove it if it is now empty.
pub struct ClaudeToken {
    // Used by ClaudeAgent::cleanup (implemented in Plan 02).
    #[allow(dead_code)]
    pub(crate) claude_dir_created: bool,
}

/// Agent implementation for Claude.
///
/// `prepare` injects an `<!-- agentree:start -->` block into `CLAUDE.md`
/// and merges `allowedTools` entries into `.claude/settings.json`.
/// `cleanup` reverts those changes.
///
/// This is a stub — the full implementation is provided in Plan 02.
pub struct ClaudeAgent;

impl ClaudeAgent {
    pub fn new() -> Self {
        ClaudeAgent
    }
}

impl Default for ClaudeAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl Agent for ClaudeAgent {
    type PrepareToken = ClaudeToken;

    fn prepare(&self, _workspace_path: &Path) -> Result<Self::PrepareToken> {
        // Stub — full implementation replaces this in Plan 02.
        Err(AgentreeError::ConfigError(
            "ClaudeAgent::prepare not yet implemented".into(),
        ))
    }

    fn cleanup(&self, _workspace_path: &Path, _token: &Self::PrepareToken) {
        // Stub — no-op until Plan 02.
    }

    fn name(&self) -> &str {
        "claude"
    }
}
