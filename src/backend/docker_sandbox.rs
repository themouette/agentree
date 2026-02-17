use crate::backend::exec::{run_captured, run_host_command, run_interactive, ExecOutput};
use crate::backend::Backend;
use crate::error::{AgentreeError, Result};
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
///
/// # Worktree Limitations
/// Docker Sandboxes do not support custom volume mounts (`-v` flag).
/// The workspace is automatically mounted at the same absolute path,
/// but the main repository's `.git` directory cannot be mounted separately.
///
/// This means git operations in worktrees may have limited functionality
/// inside the sandbox. For full git worktree support, consider using
/// the `claude-vm` backend instead.
///
/// The `mount_main_git` config option has no effect on this backend.
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
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        // Try to strip "agentree-" prefix, otherwise use the full name
        file_name
            .strip_prefix("agentree-")
            .unwrap_or(file_name)
            .to_string()
    }

    /// Check if a sandbox exists
    fn sandbox_exists(&self, workspace_path: &Path) -> Result<bool> {
        let name = self.sandbox_name(workspace_path);
        let output = run_captured(
            &self.binary,
            &["sandbox", "ls", "--format", "{{.Name}}"],
            workspace_path,
        )?;

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

        let output = run_captured(
            &self.binary,
            &["sandbox", "rm", "-f", &name],
            workspace_path,
        )?;

        if !output.success() {
            return Err(AgentreeError::BackendExecution {
                backend: "docker-sandbox".to_string(),
                error: format!("Failed to remove sandbox: {}", output.stderr.trim()),
            });
        }

        Ok(())
    }
}

impl Default for DockerSandboxBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for DockerSandboxBackend {
    fn shell(&self, workspace_path: &Path) -> Result<()> {
        let workspace_str =
            workspace_path
                .to_str()
                .ok_or_else(|| AgentreeError::BackendExecution {
                    backend: "docker-sandbox".to_string(),
                    error: format!("Invalid workspace path: {}", workspace_path.display()),
                })?;

        let sandbox_name = self.sandbox_name(workspace_path);

        // Ensure sandbox exists (create if needed)
        if !self.sandbox_exists(workspace_path)? {
            // Create sandbox with a default agent (claude)
            // This just creates the sandbox infrastructure, doesn't run the agent
            let create_args = vec![
                "sandbox",
                "create",
                "--name",
                &sandbox_name,
                "claude",
                workspace_str,
            ];

            let output = run_captured(&self.binary, &create_args, workspace_path)?;

            if !output.success() {
                return Err(AgentreeError::BackendExecution {
                    backend: "docker-sandbox".to_string(),
                    error: format!("Failed to create sandbox: {}", output.stderr.trim()),
                });
            }
        }

        // Execute interactive bash shell in the sandbox
        let args = vec!["sandbox", "exec", "-it", &sandbox_name, "/bin/bash"];

        run_interactive(&self.binary, &args, workspace_path)
    }

    fn exec(&self, workspace_path: &Path, command: &[String]) -> Result<ExecOutput> {
        // Per BACK-08: exec runs on host for speed (like claude-vm backend)
        run_host_command(workspace_path, command, self.name())
    }

    fn agent(&self, workspace_path: &Path, agent: Option<&str>, flags: &[String]) -> Result<()> {
        let workspace_str =
            workspace_path
                .to_str()
                .ok_or_else(|| AgentreeError::BackendExecution {
                    backend: "docker-sandbox".to_string(),
                    error: format!("Invalid workspace path: {}", workspace_path.display()),
                })?;

        match agent {
            Some(agent_name) => {
                let sandbox_name = self.sandbox_name(workspace_path);

                // Ensure sandbox exists (create if needed)
                if !self.sandbox_exists(workspace_path)? {
                    // Create sandbox with the specified agent
                    let create_args = vec![
                        "sandbox",
                        "create",
                        "--name",
                        &sandbox_name,
                        agent_name,
                        workspace_str,
                    ];

                    let output = run_captured(&self.binary, &create_args, workspace_path)?;

                    if !output.success() {
                        return Err(AgentreeError::BackendExecution {
                            backend: "docker-sandbox".to_string(),
                            error: format!("Failed to create sandbox: {}", output.stderr.trim()),
                        });
                    }
                }

                // Run agent in the existing sandbox
                let mut args = vec!["sandbox", "run", &sandbox_name];

                // Add agent flags after -- separator if provided
                if !flags.is_empty() {
                    args.push("--");
                    args.extend(flags.iter().map(|s| s.as_str()));
                }

                run_interactive(&self.binary, &args, workspace_path)
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
        let backend = DockerSandboxBackend::new().with_binary("/custom/docker".to_string());
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
}
