use crate::backend::BackendKind;
use crate::worktree::config::WorktreeConfig;
use serde::{Deserialize, Serialize};

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
}
