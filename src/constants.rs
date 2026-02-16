//! Centralized constants for agentree

/// Default agents available without configuration
/// These are used for:
/// - Convenience shortcuts (e.g., `agentree claude <branch>`)
/// - Shell completion for --agent flag
/// - Fallback agents when no config exists
///
/// # Examples
/// ```
/// use agentree::constants::DEFAULT_AGENTS;
/// assert!(DEFAULT_AGENTS.contains(&"claude"));
/// assert!(DEFAULT_AGENTS.contains(&"opencode"));
/// ```
pub const DEFAULT_AGENTS: &[&str] = &["claude", "opencode"];

/// Backend names for completion and validation
pub const BACKEND_NAMES: &[&str] = &["local", "claude-vm"];

/// Validate that an agent name is safe for use in shell scripts
///
/// Agent names must contain only lowercase letters, digits, and hyphens
/// to prevent shell injection when used in completion scripts.
///
/// # Examples
/// ```
/// use agentree::constants::validate_agent_name;
/// assert!(validate_agent_name("claude"));
/// assert!(validate_agent_name("opencode"));
/// assert!(validate_agent_name("my-agent-2"));
/// assert!(!validate_agent_name("bad name")); // spaces not allowed
/// assert!(!validate_agent_name("bad;name")); // semicolons not allowed
/// ```
pub fn validate_agent_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Default agent configurations
/// Used as fallback when no agent config is found
///
/// # Security
/// This function validates agent names before returning them to prevent
/// potential shell injection in completion scripts.
pub fn default_agent_binary(agent: &str) -> Option<&'static str> {
    // Validate agent name for security
    if !validate_agent_name(agent) {
        return None;
    }

    match agent {
        "claude" => Some("claude"),
        "opencode" => Some("opencode"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_agent_name_valid() {
        assert!(validate_agent_name("claude"));
        assert!(validate_agent_name("opencode"));
        assert!(validate_agent_name("my-agent"));
        assert!(validate_agent_name("agent2"));
        assert!(validate_agent_name("a"));
    }

    #[test]
    fn test_validate_agent_name_invalid() {
        assert!(!validate_agent_name("")); // empty
        assert!(!validate_agent_name("bad name")); // space
        assert!(!validate_agent_name("bad;name")); // semicolon
        assert!(!validate_agent_name("bad$name")); // dollar sign
        assert!(!validate_agent_name("bad`name")); // backtick
        assert!(!validate_agent_name("bad\nname")); // newline
        assert!(!validate_agent_name("Bad-Agent")); // uppercase
        assert!(!validate_agent_name("bad_name")); // underscore
    }

    #[test]
    fn test_default_agents_are_valid() {
        for agent in DEFAULT_AGENTS {
            assert!(
                validate_agent_name(agent),
                "Default agent '{}' should be valid",
                agent
            );
        }
    }

    #[test]
    fn test_default_agent_binary_validates() {
        // Valid agents
        assert_eq!(default_agent_binary("claude"), Some("claude"));
        assert_eq!(default_agent_binary("opencode"), Some("opencode"));

        // Invalid names return None
        assert_eq!(default_agent_binary("bad;name"), None);
        assert_eq!(default_agent_binary("bad name"), None);
    }
}
