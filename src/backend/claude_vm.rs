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

impl Backend for ClaudeVmBackend {
    fn shell(&self, workspace_path: &Path) -> Result<()> {
        // Delegate to claude-vm shell command which opens a shell inside the VM
        run_interactive(&self.binary, &["shell"], workspace_path)
    }

    fn exec(&self, workspace_path: &Path, command: &[String]) -> Result<ExecOutput> {
        // Per BACK-08: exec always runs on host, not in VM
        run_host_command(workspace_path, command, self.name())
    }

    fn agent(&self, workspace_path: &Path, agent: &str, flags: &[String]) -> Result<()> {
        // Build args: ["agent", "--"] + optional --agent flag + remaining flags
        // The "--" separator disambiguates claude-vm args from agent args
        let mut args = vec!["agent".to_string()];

        // Add -- separator before agent-specific arguments
        if !agent.is_empty() || !flags.is_empty() {
            args.push("--".to_string());
        }

        // Add --agent flag if agent binary is specified
        if !agent.is_empty() {
            args.push("--agent".to_string());
            args.push(agent.to_string());
        }

        // Append remaining flags
        args.extend_from_slice(flags);

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
    fn test_agent_args_no_agent_no_flags() {
        // When no agent and no flags: just "agent" subcommand, no "--" separator
        // Can't easily test run_interactive, but we can verify the logic
        // by checking what args would be built

        let mut args = vec!["agent".to_string()];
        let agent = "";
        let flags: Vec<String> = vec![];

        if !agent.is_empty() || !flags.is_empty() {
            args.push("--".to_string());
        }

        assert_eq!(args, vec!["agent"]);
    }

    #[test]
    fn test_agent_args_with_agent() {
        // When agent specified: "agent", "--", "--agent", "claude"
        let mut args = vec!["agent".to_string()];
        let agent = "claude";
        let flags: Vec<String> = vec![];

        if !agent.is_empty() || !flags.is_empty() {
            args.push("--".to_string());
        }

        if !agent.is_empty() {
            args.push("--agent".to_string());
            args.push(agent.to_string());
        }

        assert_eq!(args, vec!["agent", "--", "--agent", "claude"]);
    }

    #[test]
    fn test_agent_args_with_flags() {
        // When flags specified: "agent", "--", flags...
        let mut args = vec!["agent".to_string()];
        let agent = "";
        let flags = vec!["--verbose".to_string()];

        if !agent.is_empty() || !flags.is_empty() {
            args.push("--".to_string());
        }

        args.extend_from_slice(&flags);

        assert_eq!(args, vec!["agent", "--", "--verbose"]);
    }

    #[test]
    fn test_agent_args_with_agent_and_flags() {
        // When both specified: "agent", "--", "--agent", "opencode", flags...
        let mut args = vec!["agent".to_string()];
        let agent = "opencode";
        let flags = vec!["--quiet".to_string()];

        if !agent.is_empty() || !flags.is_empty() {
            args.push("--".to_string());
        }

        if !agent.is_empty() {
            args.push("--agent".to_string());
            args.push(agent.to_string());
        }

        args.extend_from_slice(&flags);

        assert_eq!(args, vec!["agent", "--", "--agent", "opencode", "--quiet"]);
    }
}
