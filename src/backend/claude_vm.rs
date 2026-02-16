use crate::backend::exec::{run_host_command, run_interactive, ExecOutput};
use crate::backend::Backend;
use crate::error::Result;
use std::path::Path;

/// Backend that delegates to the claude-vm CLI for VM-isolated execution
#[derive(Debug, Clone)]
pub struct ClaudeVmBackend {
    binary: String,
}

impl ClaudeVmBackend {
    /// Create a new ClaudeVmBackend with the default binary name
    pub fn new() -> Self {
        Self {
            binary: "claude-vm".to_string(),
        }
    }

    /// Create a new ClaudeVmBackend with a custom binary path
    pub fn with_binary(binary: String) -> Self {
        Self { binary }
    }
}

impl Default for ClaudeVmBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeVmBackend {
    /// Build command-line arguments for the claude-vm agent command
    ///
    /// # Arguments
    /// * `agent` - Optional agent binary name
    /// * `flags` - Additional flags to pass to the agent
    ///
    /// # Returns
    /// Vector of arguments to pass to claude-vm
    ///
    /// # Format
    /// - No args:       ["agent"]
    /// - With agent:    ["agent", "--", "--agent", "claude"]
    /// - With flags:    ["agent", "--", "--verbose"]
    /// - With both:     ["agent", "--", "--agent", "opencode", "--quiet"]
    pub(crate) fn build_agent_args(&self, agent: Option<&str>, flags: &[String]) -> Vec<String> {
        let mut args = vec!["agent".to_string()];

        // Add -- separator before agent-specific arguments if we have any
        if agent.is_some() || !flags.is_empty() {
            args.push("--".to_string());
        }

        // Add --agent flag if agent binary is specified
        if let Some(agent_name) = agent {
            args.push("--agent".to_string());
            args.push(agent_name.to_string());
        }

        // Append remaining flags
        args.extend_from_slice(flags);

        args
    }
}

impl Backend for ClaudeVmBackend {
    fn shell(&self, workspace_path: &Path) -> Result<()> {
        // Delegate to claude-vm shell command which opens a shell inside the VM
        run_interactive(&self.binary, &["shell"], workspace_path)
    }

    fn exec(&self, workspace_path: &Path, command: &[String]) -> Result<ExecOutput> {
        // Per BACK-08: exec always runs on host, not in VM
        run_host_command(workspace_path, command, self.name())
    }

    fn agent(&self, workspace_path: &Path, agent: Option<&str>, flags: &[String]) -> Result<()> {
        let args = self.build_agent_args(agent, flags);
        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        run_interactive(&self.binary, &args_refs, workspace_path)
    }

    fn name(&self) -> &str {
        "claude-vm"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_with_default_binary() {
        let backend = ClaudeVmBackend::new();
        assert_eq!(backend.binary, "claude-vm");
    }

    #[test]
    fn test_with_binary_stores_custom_binary() {
        let backend = ClaudeVmBackend::with_binary("/custom/path/to/claude-vm".to_string());
        assert_eq!(backend.binary, "/custom/path/to/claude-vm");
    }

    #[test]
    fn test_name_returns_claude_vm() {
        let backend = ClaudeVmBackend::new();
        assert_eq!(backend.name(), "claude-vm");
    }

    #[test]
    fn test_default_trait() {
        let backend = ClaudeVmBackend::default();
        assert_eq!(backend.binary, "claude-vm");
    }

    #[test]
    fn test_build_agent_args_no_agent_no_flags() {
        // When no agent and no flags: just "agent" subcommand, no "--" separator
        let backend = ClaudeVmBackend::new();
        let args = backend.build_agent_args(None, &[]);
        assert_eq!(args, vec!["agent"]);
    }

    #[test]
    fn test_build_agent_args_with_agent() {
        // When agent specified: "agent", "--", "--agent", "claude"
        let backend = ClaudeVmBackend::new();
        let args = backend.build_agent_args(Some("claude"), &[]);
        assert_eq!(args, vec!["agent", "--", "--agent", "claude"]);
    }

    #[test]
    fn test_build_agent_args_with_flags() {
        // When flags specified: "agent", "--", flags...
        let backend = ClaudeVmBackend::new();
        let flags = vec!["--verbose".to_string()];
        let args = backend.build_agent_args(None, &flags);
        assert_eq!(args, vec!["agent", "--", "--verbose"]);
    }

    #[test]
    fn test_build_agent_args_with_agent_and_flags() {
        // When both specified: "agent", "--", "--agent", "opencode", flags...
        let backend = ClaudeVmBackend::new();
        let flags = vec!["--quiet".to_string()];
        let args = backend.build_agent_args(Some("opencode"), &flags);
        assert_eq!(args, vec!["agent", "--", "--agent", "opencode", "--quiet"]);
    }
}
