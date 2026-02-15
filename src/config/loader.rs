use crate::error::Result;
use std::fs;
use std::path::{Path, PathBuf};

use super::Config;

/// Get the global config file path
///
/// Returns None if the platform doesn't have a standard config directory
pub fn global_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("agentree").join("agentree.toml"))
}

/// Get the project config file path
pub fn project_config_path(repo_root: &Path) -> PathBuf {
    repo_root.join("agentree.toml")
}

/// Load config from a specific file
fn from_file(path: &Path) -> Result<Config> {
    let contents =
        fs::read_to_string(path).map_err(|e| crate::error::AgentreeError::ConfigLoad {
            path: path.display().to_string(),
            error: e.to_string(),
        })?;

    toml::from_str(&contents).map_err(|e| crate::error::AgentreeError::ConfigLoad {
        path: path.display().to_string(),
        error: e.to_string(),
    })
}

/// Load and merge configuration from global and project config files
///
/// Missing config files are silently skipped. The merge strategy is key-by-key:
/// - Start with default config
/// - If global config exists, load and merge it (global overrides defaults)
/// - If project config exists, load and merge it (project overrides global)
///
/// Each config field is merged independently, so setting one field in project config
/// doesn't clobber other fields from global config.
///
/// After loading, the config is validated. Warnings are emitted to stderr, and errors
/// cause the function to return an error.
pub fn load(repo_root: &Path) -> Result<Config> {
    let mut config = Config::default();

    // Try to load global config and merge
    if let Some(global_path) = global_config_path() {
        if global_path.exists() {
            let global_config = from_file(&global_path)?;
            config = config.merge(global_config);
        }
    }

    // Try to load project config and merge (overrides global on a key-by-key basis)
    let project_path = project_config_path(repo_root);
    if project_path.exists() {
        let project_config = from_file(&project_path)?;
        config = config.merge(project_config);
    }

    // Validate the merged config
    let (warnings, errors) = config.validate();

    // Emit warnings to stderr
    for warning in warnings {
        eprintln!("Warning: {}", warning.message);
    }

    // If there are errors, return the first one
    if let Some(error) = errors.first() {
        return Err(crate::error::AgentreeError::ConfigLoad {
            path: "config".to_string(),
            error: error.clone(),
        });
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_global_config_path_ends_with_agentree_toml() {
        let path = global_config_path();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.ends_with("agentree/agentree.toml"));
    }

    #[test]
    fn test_project_config_path() {
        let repo_root = Path::new("/tmp/test-repo");
        let config_path = project_config_path(repo_root);
        assert_eq!(config_path, Path::new("/tmp/test-repo/agentree.toml"));
    }

    #[test]
    fn test_from_file_valid_toml() {
        let temp_dir = tempdir().unwrap();
        let config_file = temp_dir.path().join("config.toml");

        let toml_content = r#"
        [worktree]
        location = "/tmp/worktrees"
        template = "{branch}"

        [backend]
        default = "claude-vm"
        "#;

        fs::write(&config_file, toml_content).unwrap();

        let config = from_file(&config_file).unwrap();
        assert_eq!(config.worktree.location, Some("/tmp/worktrees".to_string()));
        assert_eq!(config.worktree.template, "{branch}");
        assert_eq!(
            config.backend.default,
            Some(crate::backend::BackendKind::ClaudeVm)
        );
    }

    #[test]
    fn test_from_file_invalid_toml_returns_config_load_error() {
        let temp_dir = tempdir().unwrap();
        let config_file = temp_dir.path().join("config.toml");

        let invalid_toml = r#"
        [worktree
        location = "invalid
        "#;

        fs::write(&config_file, invalid_toml).unwrap();

        let result = from_file(&config_file);
        assert!(result.is_err());

        match result {
            Err(crate::error::AgentreeError::ConfigLoad { path, error }) => {
                assert!(path.contains("config.toml"));
                assert!(!error.is_empty());
            }
            _ => panic!("Expected ConfigLoad error"),
        }
    }

    #[test]
    fn test_from_file_missing_file_returns_config_load_error() {
        let temp_dir = tempdir().unwrap();
        let config_file = temp_dir.path().join("nonexistent.toml");

        let result = from_file(&config_file);
        assert!(result.is_err());

        match result {
            Err(crate::error::AgentreeError::ConfigLoad { path, error }) => {
                assert!(path.contains("nonexistent.toml"));
                assert!(error.contains("No such file") || error.contains("not found"));
            }
            _ => panic!("Expected ConfigLoad error"),
        }
    }

    #[test]
    fn test_load_with_no_config_files_returns_defaults() {
        let temp_dir = tempdir().unwrap();
        let repo_root = temp_dir.path();

        let config = load(repo_root).unwrap();

        // Should be default values
        assert_eq!(config.worktree.location, None);
        assert_eq!(config.worktree.template, "{repo}/{branch}");
        assert_eq!(config.backend.default, None);
    }

    #[test]
    fn test_load_with_project_config_only() {
        let temp_dir = tempdir().unwrap();
        let repo_root = temp_dir.path();
        let config_file = project_config_path(repo_root);

        let toml_content = r#"
        [worktree]
        location = "/project/worktrees"

        [backend]
        default = "local"
        "#;

        fs::write(&config_file, toml_content).unwrap();

        let config = load(repo_root).unwrap();
        assert_eq!(
            config.worktree.location,
            Some("/project/worktrees".to_string())
        );
        assert_eq!(
            config.backend.default,
            Some(crate::backend::BackendKind::Local)
        );
    }

    #[test]
    fn test_load_invalid_project_config_returns_error() {
        let temp_dir = tempdir().unwrap();
        let repo_root = temp_dir.path();
        let config_file = project_config_path(repo_root);

        let invalid_toml = r#"[worktree invalid"#;
        fs::write(&config_file, invalid_toml).unwrap();

        let result = load(repo_root);
        assert!(result.is_err());

        match result {
            Err(crate::error::AgentreeError::ConfigLoad { path, .. }) => {
                assert!(path.contains("agentree.toml"));
            }
            _ => panic!("Expected ConfigLoad error"),
        }
    }

    #[test]
    fn test_load_with_invalid_toml_produces_config_load_error_with_file_path() {
        let temp_dir = tempdir().unwrap();
        let repo_root = temp_dir.path();
        let config_file = project_config_path(repo_root);

        // Write invalid TOML
        let invalid_toml = r#"
        [worktree]
        location = "unclosed string
        "#;
        fs::write(&config_file, invalid_toml).unwrap();

        let result = load(repo_root);
        assert!(result.is_err());

        match result {
            Err(crate::error::AgentreeError::ConfigLoad { path, error }) => {
                assert!(path.contains("agentree.toml"));
                assert!(!error.is_empty());
            }
            _ => panic!("Expected ConfigLoad error"),
        }
    }

    #[test]
    fn test_load_with_valid_config_nonexistent_location_succeeds_with_warning() {
        let temp_dir = tempdir().unwrap();
        let repo_root = temp_dir.path();
        let config_file = project_config_path(repo_root);

        // Write valid config with nonexistent location
        let toml_content = r#"
        [worktree]
        location = "/tmp/nonexistent-worktree-path-54321"
        "#;
        fs::write(&config_file, toml_content).unwrap();

        // Should succeed (warnings are emitted to stderr, not errors)
        let result = load(repo_root);
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(
            config.worktree.location,
            Some("/tmp/nonexistent-worktree-path-54321".to_string())
        );
    }
}
