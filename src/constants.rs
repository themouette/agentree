//! Centralized constants for agentree

/// Default agents available without configuration
/// These are used for:
/// - Convenience shortcuts (e.g., `agentree claude <branch>`)
/// - Shell completion for --agent flag
/// - Fallback agents when no config exists
pub const DEFAULT_AGENTS: &[&str] = &["claude", "opencode"];

/// Default agent configurations
/// Used as fallback when no agent config is found
pub fn default_agent_binary(agent: &str) -> Option<&'static str> {
    match agent {
        "claude" => Some("claude"),
        "opencode" => Some("opencode"),
        _ => None,
    }
}
