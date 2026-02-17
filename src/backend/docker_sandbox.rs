use crate::backend::exec::{run_captured, run_host_command, run_interactive, ExecOutput};
use crate::backend::Backend;
use crate::error::{AgentreeError, Result};
use crate::utils::git;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

/// Backend that uses Docker Sandboxes for microVM-based isolation
///
/// Docker Sandboxes provide hypervisor-level isolation stronger than containers,
/// suitable for running AI agents on untrusted code.
///
/// # Lifecycle
/// - Sandboxes are created lazily on first shell/agent call
/// - Sandboxes persist across commands (per-workspace, not per-command)
/// - Sandboxes are removed when the workspace is removed
///
/// # Platform Support
/// - macOS: ✅ Supported (Docker Desktop 4.58+)
/// - Windows: ✅ Supported (Docker Desktop 4.58+)
/// - Linux: ❌ Not supported (microVMs require macOS/Windows)
#[derive(Debug, Clone)]
pub struct DockerSandboxBackend {
    binary: String,
    config: Option<crate::config::DockerSandboxConfig>,
}

impl DockerSandboxBackend {
    /// Create a new DockerSandboxBackend with the default binary name
    pub fn new() -> Self {
        Self {
            binary: "docker".to_string(),
            config: None,
        }
    }

    /// Set a custom binary path (builder pattern)
    pub fn with_binary(mut self, binary: String) -> Self {
        self.binary = binary;
        self
    }

    /// Set the configuration for this backend
    pub fn with_config(mut self, config: crate::config::DockerSandboxConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Check if git mounting is enabled (default: true)
    fn should_mount_git(&self) -> bool {
        self.config
            .as_ref()
            .and_then(|c| c.mount_main_git)
            .unwrap_or(true)
    }

    /// Generate a deterministic sandbox name from the workspace path
    ///
    /// Format: agentree-{branch}-{hash}
    /// Example: agentree-feature-a1b2c3d4
    fn sandbox_name(&self, workspace_path: &Path) -> String {
        // Extract branch name from path
        let branch = self.extract_branch_from_path(workspace_path);

        // Generate hash of workspace path for uniqueness
        let mut hasher = DefaultHasher::new();
        workspace_path.hash(&mut hasher);
        let hash = hasher.finish();

        // Use lower 32 bits to keep name reasonable length
        format!("agentree-{}-{:08x}", branch, (hash & 0xFFFFFFFF) as u32)
    }

    /// Extract branch name from workspace path
    ///
    /// Tries to extract from "agentree-{branch}" pattern.
    /// For other patterns, uses the whole directory name as a fallback.
    fn extract_branch_from_path(&self, path: &Path) -> String {
        path.file_name()
            .and_then(|n| n.to_str())
            .and_then(|name| {
                // Try "agentree-{branch}" pattern
                name.strip_prefix("agentree-").map(|s| s.to_string())
            })
            .unwrap_or_else(|| {
                // Fallback: use whole directory name
                // This handles custom templates and non-standard naming
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            })
    }

    /// Check if a sandbox exists
    fn sandbox_exists(&self, workspace_path: &Path) -> Result<bool> {
        let name = self.sandbox_name(workspace_path);
        let output = run_captured(&self.binary, &["sandbox", "ls", "--format", "{{.Name}}"], workspace_path)?;

        if !output.success() {
            return Ok(false);
        }

        // Check if our sandbox name appears in the list
        Ok(output.stdout.lines().any(|line| line.trim() == name))
    }

    /// Remove a sandbox
    pub fn remove_sandbox(&self, workspace_path: &Path) -> Result<()> {
        let name = self.sandbox_name(workspace_path);

        // Check if sandbox exists first
        if !self.sandbox_exists(workspace_path)? {
            return Ok(()); // Already removed
        }

        let output = run_captured(&self.binary, &["sandbox", "rm", "-f", &name], workspace_path)?;

        if !output.success() {
            return Err(AgentreeError::BackendExecution {
                backend: "docker-sandbox".to_string(),
                error: format!("Failed to remove sandbox: {}", output.stderr.trim()),
            });
        }

        Ok(())
    }

    /// Get additional mount arguments for git worktrees
    ///
    /// If the workspace is a worktree and mount_main_git is enabled (default: true),
    /// returns mount args for the main repo's .git directory so that git commands
    /// work properly inside the sandbox.
    fn get_git_mount_args(&self, workspace_path: &Path) -> Result<Vec<String>> {
        let mut args = Vec::new();

        // Check if git mounting is enabled
        if !self.should_mount_git() {
            return Ok(args);
        }

        // Save original directory and ensure it's restored in all code paths
        let original_dir = std::env::current_dir()?;

        // Change to workspace directory to run git commands
        std::env::set_current_dir(workspace_path).map_err(|e| {
            AgentreeError::BackendExecution {
                backend: "docker-sandbox".to_string(),
                error: format!("Failed to change to workspace directory: {}", e),
            }
        })?;

        // Get git common directory, ensuring we restore original dir in all paths
        let git_common_dir = match git::get_git_common_dir() {
            Ok(dir) => {
                // Restore directory before processing result
                std::env::set_current_dir(&original_dir).map_err(|e| {
                    AgentreeError::BackendExecution {
                        backend: "docker-sandbox".to_string(),
                        error: format!("Failed to restore original directory: {}", e),
                    }
                })?;
                dir
            }
            Err(e) => {
                // Restore directory even on error (ignore restoration errors to preserve original error)
                let _ = std::env::set_current_dir(&original_dir);
                return Err(e);
            }
        };

        // If we have a git common dir and it's different from workspace/.git, mount it
        if let Some(git_dir) = git_common_dir {
            let workspace_git = workspace_path.join(".git");

            // Only add mount if the git common dir is not inside the workspace
            // (i.e., this is actually a worktree pointing to external .git)
            if git_dir != workspace_git && !git_dir.starts_with(workspace_path) {
                if let Some(git_dir_str) = git_dir.to_str() {
                    // Mount the main repo's .git directory at the same path (read-only)
                    args.push("-v".to_string());
                    args.push(format!("{}:{}:ro", git_dir_str, git_dir_str));
                }
            }
        }

        Ok(args)
    }
}

impl Default for DockerSandboxBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for DockerSandboxBackend {
    fn shell(&self, workspace_path: &Path) -> Result<()> {
        let workspace_str = workspace_path
            .to_str()
            .ok_or_else(|| AgentreeError::BackendExecution {
                backend: "docker-sandbox".to_string(),
                error: format!("Invalid workspace path: {}", workspace_path.display()),
            })?;

        // Get git mount arguments if this is a worktree
        let git_mounts = self.get_git_mount_args(workspace_path)?;

        // Build command: docker sandbox run [git-mounts] bash workspace
        let mut args = vec!["sandbox".to_string(), "run".to_string()];
        args.extend(git_mounts);
        args.push("bash".to_string());
        args.push(workspace_str.to_string());

        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        run_interactive(&self.binary, &args_refs, workspace_path)
    }

    fn exec(&self, workspace_path: &Path, command: &[String]) -> Result<ExecOutput> {
        // Per BACK-08: exec runs on host for speed (like claude-vm backend)
        run_host_command(workspace_path, command, self.name())
    }

    fn agent(&self, workspace_path: &Path, agent: Option<&str>, flags: &[String]) -> Result<()> {
        let workspace_str = workspace_path
            .to_str()
            .ok_or_else(|| AgentreeError::BackendExecution {
                backend: "docker-sandbox".to_string(),
                error: format!("Invalid workspace path: {}", workspace_path.display()),
            })?;

        match agent {
            Some(agent_name) => {
                // Get git mount arguments if this is a worktree
                let git_mounts = self.get_git_mount_args(workspace_path)?;

                // Docker Sandbox syntax: docker sandbox run [OPTIONS] AGENT WORKSPACE [-- AGENT_ARGS...]
                let mut args = vec!["sandbox".to_string(), "run".to_string()];

                // Add git mounts for worktrees
                args.extend(git_mounts);

                // Add agent and workspace
                args.push(agent_name.to_string());
                args.push(workspace_str.to_string());

                // Add agent flags after -- separator if provided
                if !flags.is_empty() {
                    args.push("--".to_string());
                    args.extend(flags.iter().cloned());
                }

                let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                run_interactive(&self.binary, &args_refs, workspace_path)
            }
            None => {
                // No agent specified - same error as LocalBackend
                Err(AgentreeError::ConfigError(
                    "No agent specified. Use --agent <name> or configure a default agent."
                        .to_string(),
                ))
            }
        }
    }

    fn name(&self) -> &str {
        "docker-sandbox"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_new_creates_with_default_binary() {
        let backend = DockerSandboxBackend::new();
        assert_eq!(backend.binary, "docker");
    }

    #[test]
    fn test_with_binary_stores_custom_binary() {
        let backend = DockerSandboxBackend::new()
            .with_binary("/custom/docker".to_string());
        assert_eq!(backend.binary, "/custom/docker");
    }

    #[test]
    fn test_name_returns_docker_sandbox() {
        let backend = DockerSandboxBackend::new();
        assert_eq!(backend.name(), "docker-sandbox");
    }

    #[test]
    fn test_default_trait() {
        let backend = DockerSandboxBackend::default();
        assert_eq!(backend.binary, "docker");
    }

    #[test]
    fn test_sandbox_name_format() {
        let backend = DockerSandboxBackend::new();
        let path = PathBuf::from("/worktrees/agentree-feature-branch");
        let name = backend.sandbox_name(&path);

        // Should start with "agentree-"
        assert!(name.starts_with("agentree-"));

        // Should contain the branch name
        assert!(name.contains("feature-branch"));

        // Should end with an 8-character hex hash
        let parts: Vec<&str> = name.split('-').collect();
        assert!(parts.len() >= 3);
        let hash_part = parts.last().unwrap();
        assert_eq!(hash_part.len(), 8);
        assert!(hash_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_sandbox_name_deterministic() {
        let backend = DockerSandboxBackend::new();
        let path = PathBuf::from("/worktrees/agentree-test");

        let name1 = backend.sandbox_name(&path);
        let name2 = backend.sandbox_name(&path);

        // Same path should always produce same name
        assert_eq!(name1, name2);
    }

    #[test]
    fn test_extract_branch_from_path_agentree_prefix() {
        let backend = DockerSandboxBackend::new();
        let path = PathBuf::from("/worktrees/agentree-my-feature");
        let branch = backend.extract_branch_from_path(&path);
        assert_eq!(branch, "my-feature");
    }

    #[test]
    fn test_extract_branch_from_path_custom_pattern() {
        let backend = DockerSandboxBackend::new();
        // For non-standard patterns, use the whole directory name
        let path = PathBuf::from("/worktrees/myrepo-bugfix-123");
        let branch = backend.extract_branch_from_path(&path);
        assert_eq!(branch, "myrepo-bugfix-123");
    }

    #[test]
    fn test_extract_branch_from_path_fallback() {
        let backend = DockerSandboxBackend::new();
        let path = PathBuf::from("/worktrees/simple");
        let branch = backend.extract_branch_from_path(&path);
        assert_eq!(branch, "simple");
    }

    #[test]
    fn test_with_config() {
        let config = crate::config::DockerSandboxConfig {
            binary: Some("/custom/docker".to_string()),
            network_policy: Some("restricted".to_string()),
            persistent: Some(false),
            mount_main_git: Some(false),
        };

        let backend = DockerSandboxBackend::new().with_config(config.clone());
        assert_eq!(backend.config, Some(config));
    }

    #[test]
    fn test_should_mount_git_default() {
        let backend = DockerSandboxBackend::new();
        assert_eq!(backend.should_mount_git(), true);
    }

    #[test]
    fn test_should_mount_git_enabled() {
        let config = crate::config::DockerSandboxConfig {
            binary: None,
            network_policy: None,
            persistent: Some(true),
            mount_main_git: Some(true),
        };

        let backend = DockerSandboxBackend::new().with_config(config);
        assert_eq!(backend.should_mount_git(), true);
    }

    #[test]
    fn test_should_mount_git_disabled() {
        let config = crate::config::DockerSandboxConfig {
            binary: None,
            network_policy: None,
            persistent: Some(true),
            mount_main_git: Some(false),
        };

        let backend = DockerSandboxBackend::new().with_config(config);
        assert_eq!(backend.should_mount_git(), false);
    }

}
