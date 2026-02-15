use crate::error::{AgentreeError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Metadata stored for each worktree to track backend and creation info
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorktreeMetadata {
    /// Backend used to create this worktree
    pub backend: String,
    /// ISO 8601 timestamp of creation
    pub created_at: String,
    /// Version of agentree that created this worktree
    pub version: String,
}

impl WorktreeMetadata {
    /// Create new metadata with current timestamp
    pub fn new(backend: String) -> Self {
        Self {
            backend,
            created_at: chrono::Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Get the path where metadata should be stored for a worktree
    ///
    /// For worktrees, `.git` is a file pointing to the actual gitdir.
    /// We parse that file and store metadata in the gitdir as `agentree-meta.json`.
    /// For main repos (where `.git` is a directory), store in `.git/agentree-meta.json`.
    pub fn metadata_path(worktree_path: &Path) -> Result<PathBuf> {
        let git_path = worktree_path.join(".git");

        if git_path.is_file() {
            // Worktree case: .git is a file containing "gitdir: /path/to/gitdir"
            let content = fs::read_to_string(&git_path).map_err(|e| {
                AgentreeError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to read .git file: {}", e),
                ))
            })?;

            // Parse gitdir path from file
            let gitdir_line = content
                .lines()
                .find(|line| line.starts_with("gitdir: "))
                .ok_or_else(|| {
                    AgentreeError::Worktree(format!(
                        "Invalid .git file format at {}",
                        git_path.display()
                    ))
                })?;

            let gitdir_path = gitdir_line.trim_start_matches("gitdir: ").trim();
            let gitdir = PathBuf::from(gitdir_path);

            Ok(gitdir.join("agentree-meta.json"))
        } else if git_path.is_dir() {
            // Main repo case: .git is a directory
            Ok(git_path.join("agentree-meta.json"))
        } else {
            Err(AgentreeError::Worktree(format!(
                "No .git found at {}",
                worktree_path.display()
            )))
        }
    }

    /// Load metadata from a worktree directory
    ///
    /// Returns:
    /// - Ok(Some(metadata)) if file exists and is valid
    /// - Ok(None) if file doesn't exist (not an error - may be created externally)
    /// - Err only for actual IO/parse errors on existing files
    pub fn load(worktree_path: &Path) -> Result<Option<Self>> {
        let meta_path = Self::metadata_path(worktree_path)?;

        if !meta_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&meta_path)?;
        let metadata: WorktreeMetadata = serde_json::from_str(&content)?;

        Ok(Some(metadata))
    }

    /// Save metadata to a worktree directory
    pub fn save(&self, worktree_path: &Path) -> Result<()> {
        let meta_path = Self::metadata_path(worktree_path)?;

        // Ensure parent directory exists
        if let Some(parent) = meta_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(&meta_path, content)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_metadata_new() {
        let metadata = WorktreeMetadata::new("claude-vm".to_string());

        assert_eq!(metadata.backend, "claude-vm");
        assert!(!metadata.created_at.is_empty());
        assert_eq!(metadata.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_metadata_serialize_deserialize() {
        let original = WorktreeMetadata {
            backend: "local".to_string(),
            created_at: "2024-01-15T12:34:56Z".to_string(),
            version: "0.1.0".to_string(),
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: WorktreeMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_load_nonexistent_returns_none() {
        let temp_dir = TempDir::new().unwrap();
        let worktree_dir = temp_dir.path().join("worktree");
        fs::create_dir_all(&worktree_dir).unwrap();

        // Create .git directory to make it look like a main repo
        fs::create_dir(worktree_dir.join(".git")).unwrap();

        let result = WorktreeMetadata::load(&worktree_dir).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_save_and_load_main_repo() {
        let temp_dir = TempDir::new().unwrap();
        let worktree_dir = temp_dir.path().join("main-repo");
        fs::create_dir_all(&worktree_dir).unwrap();

        // Create .git directory (main repo case)
        fs::create_dir(worktree_dir.join(".git")).unwrap();

        let metadata = WorktreeMetadata {
            backend: "claude".to_string(),
            created_at: "2024-01-15T12:34:56Z".to_string(),
            version: "0.1.0".to_string(),
        };

        // Save
        metadata.save(&worktree_dir).unwrap();

        // Load and verify
        let loaded = WorktreeMetadata::load(&worktree_dir).unwrap();
        assert_eq!(loaded, Some(metadata));
    }

    #[test]
    fn test_save_and_load_worktree() {
        let temp_dir = TempDir::new().unwrap();
        let gitdir = temp_dir.path().join("main").join(".git").join("worktrees").join("branch");
        fs::create_dir_all(&gitdir).unwrap();

        let worktree_dir = temp_dir.path().join("worktree");
        fs::create_dir_all(&worktree_dir).unwrap();

        // Create .git file pointing to gitdir (worktree case)
        let git_file_content = format!("gitdir: {}\n", gitdir.display());
        fs::write(worktree_dir.join(".git"), git_file_content).unwrap();

        let metadata = WorktreeMetadata {
            backend: "claude-vm".to_string(),
            created_at: "2024-01-15T12:34:56Z".to_string(),
            version: "0.1.0".to_string(),
        };

        // Save
        metadata.save(&worktree_dir).unwrap();

        // Load and verify
        let loaded = WorktreeMetadata::load(&worktree_dir).unwrap();
        assert_eq!(loaded, Some(metadata));

        // Verify it was saved in the gitdir
        let meta_path = gitdir.join("agentree-meta.json");
        assert!(meta_path.exists());
    }

    #[test]
    fn test_metadata_path_resolves_correctly() {
        let temp_dir = TempDir::new().unwrap();
        let gitdir = temp_dir.path().join(".git").join("worktrees").join("feature");
        fs::create_dir_all(&gitdir).unwrap();

        let worktree_dir = temp_dir.path().join("feature-worktree");
        fs::create_dir_all(&worktree_dir).unwrap();

        // Create .git file
        let git_file_content = format!("gitdir: {}\n", gitdir.display());
        fs::write(worktree_dir.join(".git"), git_file_content).unwrap();

        let meta_path = WorktreeMetadata::metadata_path(&worktree_dir).unwrap();
        let expected = gitdir.join("agentree-meta.json");

        assert_eq!(meta_path, expected);
    }

    #[test]
    fn test_load_missing_git_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let worktree_dir = temp_dir.path().join("no-git");
        fs::create_dir_all(&worktree_dir).unwrap();

        let result = WorktreeMetadata::load(&worktree_dir);
        assert!(result.is_err());
    }
}
