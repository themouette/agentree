use crate::backend::BackendKind;
use crate::worktree::config::WorktreeConfig;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

mod loader;

pub use loader::{global_config_path, load, project_config_path};

/// Backend configuration section
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BackendConfig {
    /// Default backend to use (None means use hardcoded default: Local)
    pub default: Option<BackendKind>,
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
    /// global.backend.default = Some(BackendKind::Claude);
    ///
    /// let mut project = Config::default();
    /// project.worktree.location = Some("/custom".to_string());
    ///
    /// let merged = global.merge(project);
    /// // Both backend and location are preserved
    /// assert_eq!(merged.backend.default, Some(BackendKind::Claude));
    /// assert_eq!(merged.worktree.location, Some("/custom".to_string()));
    /// ```
    pub fn merge(self, other: Config) -> Config {
        let default_worktree = WorktreeConfig::default();

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
    ) -> Result<Self, crate::error::AgentreeError> {
        if let Some(backend_str) = backend {
            let kind = BackendKind::from_str(backend_str)?;
            self.backend.default = Some(kind);
        }

        if let Some(location) = worktree_location {
            self.worktree.location = Some(location.to_string());
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
        assert_eq!(
            config.worktree.location,
            Some("/tmp/worktrees".to_string())
        );
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
        default = "claude"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.worktree.location, Some("/custom/path".to_string()));
        assert_eq!(config.worktree.template, "{feature}/{branch}");
        assert_eq!(config.backend.default, Some(BackendKind::Claude));
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
    fn test_backend_kind_serde_claude() {
        let toml = r#"
        [backend]
        default = "claude"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.backend.default, Some(BackendKind::Claude));
    }

    #[test]
    fn test_merge_preserves_both_configs() {
        // Global sets backend, project sets location - both should be preserved
        let mut global = Config::default();
        global.backend.default = Some(BackendKind::Claude);

        let mut project = Config::default();
        project.worktree.location = Some("/custom".to_string());

        let merged = global.merge(project);

        assert_eq!(merged.backend.default, Some(BackendKind::Claude));
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
        let result = config.with_cli_overrides(Some("claude-vm"), None);

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.backend.default, Some(BackendKind::ClaudeVm));
    }

    #[test]
    fn test_with_cli_overrides_invalid_backend() {
        let config = Config::default();
        let result = config.with_cli_overrides(Some("invalid"), None);

        assert!(result.is_err());
        match result {
            Err(crate::error::AgentreeError::BackendNotFound { name, available }) => {
                assert_eq!(name, "invalid");
                assert!(available.contains(&"local".to_string()));
                assert!(available.contains(&"claude-vm".to_string()));
                assert!(available.contains(&"claude".to_string()));
            }
            _ => panic!("Expected BackendNotFound error"),
        }
    }

    #[test]
    fn test_with_cli_overrides_location() {
        let config = Config::default();
        let result = config.with_cli_overrides(None, Some("/cli/location"));

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
        global.backend.default = Some(BackendKind::Claude);
        global.worktree.template = "{branch}".to_string();
        default = default.merge(global);

        assert_eq!(default.backend.default, Some(BackendKind::Claude));
        assert_eq!(default.worktree.template, "{branch}");
        assert_eq!(default.worktree.location, None);

        // Project sets location and overrides backend
        let mut project = Config::default();
        project.worktree.location = Some("/project/worktrees".to_string());
        project.backend.default = Some(BackendKind::ClaudeVm);
        default = default.merge(project);

        assert_eq!(default.backend.default, Some(BackendKind::ClaudeVm));
        assert_eq!(default.worktree.location, Some("/project/worktrees".to_string()));
        assert_eq!(default.worktree.template, "{branch}"); // Still from global

        // CLI overrides backend and location
        let result = default.with_cli_overrides(Some("local"), Some("/cli/worktrees"));
        assert!(result.is_ok());
        let final_config = result.unwrap();

        assert_eq!(final_config.backend.default, Some(BackendKind::Local));
        assert_eq!(final_config.worktree.location, Some("/cli/worktrees".to_string()));
        assert_eq!(final_config.worktree.template, "{branch}"); // Still from global
    }
}
