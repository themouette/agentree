use crate::error::{AgentreeError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeConfig {
    /// Custom worktree base directory (default: None, meaning `../worktrees/`)
    #[serde(default)]
    pub location: Option<String>,

    /// Path template for worktree naming (default: "{repo}/{branch}")
    #[serde(default = "default_template")]
    pub template: String,
}

fn default_template() -> String {
    "{repo}/{branch}".to_string()
}

impl Default for WorktreeConfig {
    fn default() -> Self {
        Self {
            location: None,
            template: default_template(),
        }
    }
}

impl WorktreeConfig {
    /// Resolve the worktree location directory
    ///
    /// Returns the configured location if set, otherwise defaults to ../worktrees
    /// relative to the repository root.
    pub fn resolve_location(&self, repo_root: &Path) -> Result<PathBuf> {
        if let Some(location) = &self.location {
            Ok(PathBuf::from(location))
        } else {
            // Default location: ../worktrees/
            repo_root
                .parent()
                .ok_or_else(|| {
                    AgentreeError::Worktree("Cannot determine parent directory".to_string())
                })
                .map(|p| p.join("worktrees"))
        }
    }

    /// Validate configuration and return warnings (not errors - config is still usable)
    /// Following NetworkIsolationConfig::validate() pattern
    pub fn validate(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        // Check if location path exists (if specified)
        if let Some(location) = &self.location {
            let path = std::path::Path::new(location);
            if !path.exists() {
                warnings.push(format!(
                    "Worktree location '{}' does not exist. It will be created when first worktree is added.",
                    location
                ));
            }
        }

        warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WorktreeConfig::default();
        assert_eq!(config.location, None);
        assert_eq!(config.template, "{repo}/{branch}");
    }

    #[test]
    fn test_deserialize_empty_table() {
        // Empty TOML (no fields set) should use defaults
        let toml = r#""#;

        let config: WorktreeConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.location, None);
        assert_eq!(config.template, "{repo}/{branch}");
    }

    #[test]
    fn test_deserialize_with_location() {
        let toml = r#"
        location = "/tmp/worktrees"
        "#;

        let config: WorktreeConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.location, Some("/tmp/worktrees".to_string()));
        assert_eq!(config.template, "{repo}/{branch}");
    }

    #[test]
    fn test_deserialize_with_template() {
        let toml = r#"
        template = "{date}-{branch}"
        "#;

        let config: WorktreeConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.location, None);
        assert_eq!(config.template, "{date}-{branch}");
    }

    #[test]
    fn test_deserialize_full() {
        let toml = r#"
        location = "/custom/path"
        template = "{feature}/{branch}"
        "#;

        let config: WorktreeConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.location, Some("/custom/path".to_string()));
        assert_eq!(config.template, "{feature}/{branch}");
    }

    #[test]
    fn test_validate_nonexistent_location_warns() {
        // Use a path that definitely doesn't exist
        let nonexistent = "/tmp/nonexistent-worktree-path-12345";
        let config = WorktreeConfig {
            location: Some(nonexistent.to_string()),
            template: "{repo}/{branch}".to_string(),
        };

        let warnings = config.validate();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("does not exist"));
        assert!(warnings[0].contains(nonexistent));
    }

    #[test]
    fn test_validate_no_warnings_default() {
        let config = WorktreeConfig::default();
        let warnings = config.validate();
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_validate_existing_location_no_warning() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let config = WorktreeConfig {
            location: Some(temp_dir.path().to_string_lossy().to_string()),
            template: "{repo}/{branch}".to_string(),
        };

        let warnings = config.validate();
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_resolve_location_default() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let repo_root = temp_dir.path().join("repo");
        std::fs::create_dir(&repo_root).unwrap();

        let config = WorktreeConfig::default();
        let location = config.resolve_location(&repo_root).unwrap();

        assert_eq!(location, temp_dir.path().join("worktrees"));
    }

    #[test]
    fn test_resolve_location_custom() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let repo_root = temp_dir.path().join("repo");
        let custom_location = temp_dir.path().join("custom");

        let config = WorktreeConfig {
            location: Some(custom_location.to_string_lossy().to_string()),
            template: "{repo}/{branch}".to_string(),
        };

        let location = config.resolve_location(&repo_root).unwrap();
        assert_eq!(location, custom_location);
    }
}
