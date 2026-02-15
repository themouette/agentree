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

    fn agent(&self, workspace_path: &Path, flags: &[String]) -> Result<()> {
        // Build args: ["agent"] + flags
        let mut args = vec!["agent"];
        let flag_refs: Vec<&str> = flags.iter().map(|s| s.as_str()).collect();
        args.extend(flag_refs);

        // Delegate to claude-vm agent command which starts agent in the VM
        run_interactive(&self.binary, &args, workspace_path)
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
}
