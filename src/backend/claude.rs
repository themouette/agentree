use crate::backend::exec::{run_host_command, run_interactive, ExecOutput};
use crate::backend::Backend;
use crate::error::Result;
use std::path::Path;

/// Backend that delegates to the claude CLI for local execution
#[derive(Debug, Clone)]
pub struct ClaudeBackend {
    binary: String,
}

impl ClaudeBackend {
    /// Create a new ClaudeBackend with the default binary name
    pub fn new() -> Self {
        Self {
            binary: "claude".to_string(),
        }
    }

    /// Create a new ClaudeBackend with a custom binary path
    pub fn with_binary(binary: String) -> Self {
        Self { binary }
    }
}

impl Default for ClaudeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for ClaudeBackend {
    fn shell(&self, workspace_path: &Path) -> Result<()> {
        // Claude backend runs locally (no VM), open shell directly
        // Detect shell from SHELL env var, default to sh
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".to_string());
        run_interactive(&shell, &[], workspace_path)
    }

    fn exec(&self, workspace_path: &Path, command: &[String]) -> Result<ExecOutput> {
        // Run directly on host - no isolation for exec
        run_host_command(workspace_path, command, self.name())
    }

    fn agent(&self, workspace_path: &Path, flags: &[String]) -> Result<()> {
        // Start Claude Code CLI in the worktree directory with passed flags
        let flag_refs: Vec<&str> = flags.iter().map(|s| s.as_str()).collect();
        run_interactive(&self.binary, &flag_refs, workspace_path)
    }

    fn name(&self) -> &str {
        "claude"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_with_default_binary() {
        let backend = ClaudeBackend::new();
        assert_eq!(backend.binary, "claude");
    }

    #[test]
    fn test_with_binary_stores_custom_binary() {
        let backend = ClaudeBackend::with_binary("/custom/path/to/claude".to_string());
        assert_eq!(backend.binary, "/custom/path/to/claude");
    }

    #[test]
    fn test_name_returns_claude() {
        let backend = ClaudeBackend::new();
        assert_eq!(backend.name(), "claude");
    }

    #[test]
    fn test_default_trait() {
        let backend = ClaudeBackend::default();
        assert_eq!(backend.binary, "claude");
    }
}
