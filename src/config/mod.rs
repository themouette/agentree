use crate::backend::BackendKind;
use crate::constants;
use crate::worktree::config::WorktreeConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

mod loader;
pub mod validation;

pub use loader::{home_config_path, load, project_config_path, xdg_config_path};

/// Backend configuration section
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BackendConfig {
    /// Default backend to use (None means use hardcoded default: Local)
    pub default: Option<BackendKind>,
}

/// Information about a specific AI agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    /// Path to the agent binary
    pub bin: String,
    /// Default arguments to pass to the agent
    #[serde(default)]
    pub default_args: Vec<String>,
}

/// Agent configuration section
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    /// Default agent to use (e.g., "claude")
    pub default: Option<String>,
    /// Map of agent name to agent configuration
    #[serde(flatten)]
    pub agents: HashMap<String, AgentInfo>,
}

/// Root configuration structure combining all config sections
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// Worktree configuration
    #[serde(default)]
    pub worktree: WorktreeConfig,

    /// Backend configuration
    #[serde(default)]
    pub backend: BackendConfig,

    /// Agent configuration
    #[serde(default)]
    pub agent: AgentConfig,
}

impl Config {
    /// Merge this config with another config, where `other` takes precedence on a key-by-key basis.
    ///
    /// This is not a simple struct replacement - each field is merged independently:
    /// - If `other` has a field set, it overrides `self`'s field
    /// - If `other` doesn't have a field set, `self`'s field is preserved
    ///
    /// # Example
    /// ```
    /// # use agentree::config::Config;
    /// # use agentree::backend::BackendKind;
    /// let mut global = Config::default();
    /// global.backend.default = Some(BackendKind::ClaudeVm);
    ///
    /// let mut project = Config::default();
    /// project.worktree.location = Some("/custom".to_string());
    ///
    /// let merged = global.merge(project);
    /// // Both backend and location are preserved
    /// assert_eq!(merged.backend.default, Some(BackendKind::ClaudeVm));
    /// assert_eq!(merged.worktree.location, Some("/custom".to_string()));
    /// ```
    pub fn merge(self, other: Config) -> Config {
        let default_worktree = WorktreeConfig::default();

        // Merge agent configs - other's agents override self's agents with same name
        let mut agents = self.agent.agents;
        agents.extend(other.agent.agents);

        Config {
            worktree: WorktreeConfig {
                location: other.worktree.location.or(self.worktree.location),
                template: if other.worktree.template != default_worktree.template {
                    other.worktree.template
                } else {
                    self.worktree.template
                },
            },
            backend: BackendConfig {
                default: other.backend.default.or(self.backend.default),
            },
            agent: AgentConfig {
                default: other.agent.default.or(self.agent.default),
                agents,
            },
        }
    }

    /// Apply CLI overrides to this config.
    ///
    /// CLI arguments have the highest precedence in the config chain:
    /// default -> global -> project -> CLI
    ///
    /// # Errors
    /// Returns an error if the backend name is invalid. The error includes
    /// the list of available backends.
    pub fn with_cli_overrides(
        mut self,
        backend: Option<&str>,
        worktree_location: Option<&str>,
        agent: Option<&str>,
    ) -> Result<Self, crate::error::AgentreeError> {
        if let Some(backend_str) = backend {
            let kind = BackendKind::from_str(backend_str)?;
            self.backend.default = Some(kind);
        }

        if let Some(location) = worktree_location {
            self.worktree.location = Some(location.to_string());
        }

        // CLI agent overrides config default
        if let Some(agent_name) = agent {
            self.agent.default = Some(agent_name.to_string());
        }

        Ok(self)
    }

    /// Get the effective backend, falling back to hardcoded default if not configured.
    ///
    /// The fallback is `BackendKind::Local` when no backend is configured anywhere
    /// (default config, global config, project config, or CLI).
    pub fn effective_backend(&self) -> BackendKind {
        self.backend.default.unwrap_or(BackendKind::Local)
    }

    /// Resolve an agent name to its binary path and default arguments.
    ///
    /// # Arguments
    /// * `agent_name` - Optional agent name. If None, uses the default agent from config.
    ///
    /// # Returns
    /// * `Ok((binary_path, default_args))` - The agent binary path and its default arguments
    /// * `Err` - If no agent is specified/configured or the agent is unknown
    ///
    /// # Errors
    /// * No agent specified and no default configured
    /// * Unknown agent name (not in config or default agents)
    ///
    /// # Fallback behavior
    /// If an agent is not found in the config, falls back to hardcoded defaults
    /// for known agents (claude, opencode). This allows using default agents
    /// without requiring configuration.
    pub fn resolve_agent(
        &self,
        agent_name: Option<&str>,
    ) -> Result<(String, Vec<String>), crate::error::AgentreeError> {
        let name = agent_name
            .or(self.agent.default.as_deref())
            .ok_or_else(|| {
                crate::error::AgentreeError::ConfigError(
                    "No agent specified and no default configured".to_string(),
                )
            })?;

        // First try to get from config
        if let Some(info) = self.agent.agents.get(name) {
            return Ok((info.bin.clone(), info.default_args.clone()));
        }

        // Fall back to hardcoded defaults for known agents
        if let Some(binary) = constants::default_agent_binary(name) {
            return Ok((binary.to_string(), vec![]));
        }

        // Unknown agent - show both configured and default agents
        let mut available: Vec<String> = constants::DEFAULT_AGENTS
            .iter()
            .map(|a| format!("{} (default)", a))
            .collect();
        available.extend(
            self.agent
                .agents
                .keys()
                .map(|k| format!("{} (configured)", k)),
        );

        Err(crate::error::AgentreeError::ConfigError(format!(
            "Unknown agent '{}'. Available: {}",
            name,
            available.join(", ")
        )))
    }

    /// Validate configuration and return warnings and errors.
    ///
    /// Returns (warnings, errors) tuple where:
    /// - Warnings are non-fatal issues (e.g., path doesn't exist yet)
    /// - Errors are fatal issues that prevent using the config
    pub fn validate(&self) -> (Vec<validation::ConfigWarning>, Vec<String>) {
        validation::validate(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.backend.default, None);
        assert_eq!(config.worktree.location, None);
        assert_eq!(config.worktree.template, "{repo}/{branch}");
    }

    #[test]
    fn test_deserialize_empty_toml() {
        let toml = r#""#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.backend.default, None);
        assert_eq!(config.worktree.template, "{repo}/{branch}");
    }

    #[test]
    fn test_deserialize_partial_worktree() {
        let toml = r#"
        [worktree]
        location = "/tmp/worktrees"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.worktree.location, Some("/tmp/worktrees".to_string()));
        assert_eq!(config.worktree.template, "{repo}/{branch}");
        assert_eq!(config.backend.default, None);
    }

    #[test]
    fn test_deserialize_partial_backend() {
        let toml = r#"
        [backend]
        default = "claude-vm"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.backend.default, Some(BackendKind::ClaudeVm));
        assert_eq!(config.worktree.template, "{repo}/{branch}");
    }

    #[test]
    fn test_deserialize_full_config() {
        let toml = r#"
        [worktree]
        location = "/custom/path"
        template = "{feature}/{branch}"

        [backend]
        default = "local"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.worktree.location, Some("/custom/path".to_string()));
        assert_eq!(config.worktree.template, "{feature}/{branch}");
        assert_eq!(config.backend.default, Some(BackendKind::Local));
    }

    #[test]
    fn test_backend_kind_serde_local() {
        let toml = r#"
        [backend]
        default = "local"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.backend.default, Some(BackendKind::Local));
    }

    #[test]
    fn test_backend_kind_serde_claude_vm() {
        let toml = r#"
        [backend]
        default = "claude-vm"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.backend.default, Some(BackendKind::ClaudeVm));
    }

    #[test]
    fn test_merge_preserves_both_configs() {
        // Global sets backend, project sets location - both should be preserved
        let mut global = Config::default();
        global.backend.default = Some(BackendKind::ClaudeVm);

        let mut project = Config::default();
        project.worktree.location = Some("/custom".to_string());

        let merged = global.merge(project);

        assert_eq!(merged.backend.default, Some(BackendKind::ClaudeVm));
        assert_eq!(merged.worktree.location, Some("/custom".to_string()));
    }

    #[test]
    fn test_merge_project_overrides_global() {
        // Both set location - project should win
        let mut global = Config::default();
        global.worktree.location = Some("/global".to_string());

        let mut project = Config::default();
        project.worktree.location = Some("/project".to_string());

        let merged = global.merge(project);

        assert_eq!(merged.worktree.location, Some("/project".to_string()));
    }

    #[test]
    fn test_merge_preserves_global_template() {
        // Global sets template, project doesn't - global template should be preserved
        let mut global = Config::default();
        global.worktree.template = "{branch}".to_string();

        let project = Config::default(); // Uses default template

        let merged = global.merge(project);

        assert_eq!(merged.worktree.template, "{branch}");
    }

    #[test]
    fn test_merge_neither_sets_backend() {
        // Neither sets backend - should remain None
        let global = Config::default();
        let project = Config::default();

        let merged = global.merge(project);

        assert_eq!(merged.backend.default, None);
    }

    #[test]
    fn test_effective_backend_returns_local_when_none() {
        let config = Config::default();
        assert_eq!(config.effective_backend(), BackendKind::Local);
    }

    #[test]
    fn test_effective_backend_returns_configured_value() {
        let mut config = Config::default();
        config.backend.default = Some(BackendKind::ClaudeVm);
        assert_eq!(config.effective_backend(), BackendKind::ClaudeVm);
    }

    #[test]
    fn test_with_cli_overrides_backend() {
        let config = Config::default();
        let result = config.with_cli_overrides(Some("claude-vm"), None, None);

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.backend.default, Some(BackendKind::ClaudeVm));
    }

    #[test]
    fn test_with_cli_overrides_invalid_backend() {
        let config = Config::default();
        let result = config.with_cli_overrides(Some("invalid"), None, None);

        assert!(result.is_err());
        match result {
            Err(crate::error::AgentreeError::BackendNotFound { name, available }) => {
                assert_eq!(name, "invalid");
                assert!(available.contains(&"local".to_string()));
                assert!(available.contains(&"claude-vm".to_string()));
                // Claude backend removed
                assert_eq!(available.len(), 2);
            }
            _ => panic!("Expected BackendNotFound error"),
        }
    }

    #[test]
    fn test_with_cli_overrides_location() {
        let config = Config::default();
        let result = config.with_cli_overrides(None, Some("/cli/location"), None);

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.worktree.location, Some("/cli/location".to_string()));
    }

    #[test]
    fn test_full_precedence_chain() {
        // Test 4-level precedence: default -> global -> project -> CLI

        // Start with default
        let mut default = Config::default();
        assert_eq!(default.backend.default, None);
        assert_eq!(default.worktree.location, None);
        assert_eq!(default.worktree.template, "{repo}/{branch}");

        // Global sets backend and template
        let mut global = Config::default();
        global.backend.default = Some(BackendKind::ClaudeVm);
        global.worktree.template = "{branch}".to_string();
        default = default.merge(global);

        assert_eq!(default.backend.default, Some(BackendKind::ClaudeVm));
        assert_eq!(default.worktree.template, "{branch}");
        assert_eq!(default.worktree.location, None);

        // Project sets location and overrides backend
        let mut project = Config::default();
        project.worktree.location = Some("/project/worktrees".to_string());
        project.backend.default = Some(BackendKind::Local);
        default = default.merge(project);

        assert_eq!(default.backend.default, Some(BackendKind::Local));
        assert_eq!(
            default.worktree.location,
            Some("/project/worktrees".to_string())
        );
        assert_eq!(default.worktree.template, "{branch}"); // Still from global

        // CLI overrides backend and location
        let result = default.with_cli_overrides(Some("claude-vm"), Some("/cli/worktrees"), None);
        assert!(result.is_ok());
        let final_config = result.unwrap();

        assert_eq!(final_config.backend.default, Some(BackendKind::ClaudeVm));
        assert_eq!(
            final_config.worktree.location,
            Some("/cli/worktrees".to_string())
        );
        assert_eq!(final_config.worktree.template, "{branch}"); // Still from global
    }

    #[test]
    fn test_resolve_agent_from_config() {
        // Agent defined in config
        let mut config = Config::default();
        config.agent.agents.insert(
            "custom".to_string(),
            AgentInfo {
                bin: "/usr/local/bin/custom-agent".to_string(),
                default_args: vec!["--quiet".to_string()],
            },
        );

        let result = config.resolve_agent(Some("custom"));
        assert!(result.is_ok());
        let (bin, args) = result.unwrap();
        assert_eq!(bin, "/usr/local/bin/custom-agent");
        assert_eq!(args, vec!["--quiet"]);
    }

    #[test]
    fn test_resolve_agent_fallback_claude() {
        // Agent not in config, but is a default agent
        let config = Config::default();

        let result = config.resolve_agent(Some("claude"));
        assert!(result.is_ok());
        let (bin, args) = result.unwrap();
        assert_eq!(bin, "claude");
        assert_eq!(args, Vec::<String>::new());
    }

    #[test]
    fn test_resolve_agent_fallback_opencode() {
        // Agent not in config, but is a default agent
        let config = Config::default();

        let result = config.resolve_agent(Some("opencode"));
        assert!(result.is_ok());
        let (bin, args) = result.unwrap();
        assert_eq!(bin, "opencode");
        assert_eq!(args, Vec::<String>::new());
    }

    #[test]
    fn test_resolve_agent_config_overrides_default() {
        // Agent defined in config should override default
        let mut config = Config::default();
        config.agent.agents.insert(
            "claude".to_string(),
            AgentInfo {
                bin: "/custom/claude".to_string(),
                default_args: vec!["--verbose".to_string()],
            },
        );

        let result = config.resolve_agent(Some("claude"));
        assert!(result.is_ok());
        let (bin, args) = result.unwrap();
        assert_eq!(bin, "/custom/claude");
        assert_eq!(args, vec!["--verbose"]);
    }

    #[test]
    fn test_resolve_agent_unknown() {
        // Agent not in config and not a default
        let config = Config::default();

        let result = config.resolve_agent(Some("unknown-agent"));
        assert!(result.is_err());
        match result {
            Err(crate::error::AgentreeError::ConfigError(msg)) => {
                assert!(msg.contains("Unknown agent 'unknown-agent'"));
            }
            _ => panic!("Expected ConfigError"),
        }
    }

    #[test]
    fn test_resolve_agent_uses_config_default() {
        // No agent specified, should use config default
        let mut config = Config::default();
        config.agent.default = Some("claude".to_string());

        let result = config.resolve_agent(None);
        assert!(result.is_ok());
        let (bin, _) = result.unwrap();
        assert_eq!(bin, "claude");
    }

    #[test]
    fn test_resolve_agent_no_agent_no_default() {
        // No agent specified and no default configured
        let config = Config::default();

        let result = config.resolve_agent(None);
        assert!(result.is_err());
        match result {
            Err(crate::error::AgentreeError::ConfigError(msg)) => {
                assert!(msg.contains("No agent specified"));
            }
            _ => panic!("Expected ConfigError"),
        }
    }
}
