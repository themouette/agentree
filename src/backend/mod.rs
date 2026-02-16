use crate::error::{AgentreeError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

mod claude_vm;
mod exec;
mod local;
mod registry;

pub use claude_vm::ClaudeVmBackend;
pub use exec::{run_host_command, run_interactive, ExecOutput};
pub use local::LocalBackend;
pub use registry::BackendRegistry;

/// Backend selection enum used for config and CLI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendKind {
    Local,
    ClaudeVm,
}

impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendKind::Local => write!(f, "local"),
            BackendKind::ClaudeVm => write!(f, "claude-vm"),
        }
    }
}

impl FromStr for BackendKind {
    type Err = AgentreeError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "local" => Ok(BackendKind::Local),
            "claude-vm" | "claudevm" => Ok(BackendKind::ClaudeVm),
            _ => Err(AgentreeError::BackendNotFound {
                name: s.to_string(),
                available: vec!["local".to_string(), "claude-vm".to_string()],
            }),
        }
    }
}

/// Backend trait that all backends must implement
pub trait Backend {
    /// Open an interactive shell in the workspace
    fn shell(&self, workspace_path: &Path) -> Result<()>;

    /// Execute a command in the workspace and capture output
    fn exec(&self, workspace_path: &Path, command: &[String]) -> Result<ExecOutput>;

    /// Start an AI agent session in the workspace
    ///
    /// # Arguments
    /// * `workspace_path` - Path to the workspace directory
    /// * `agent` - Optional binary path of the agent to run (e.g., "claude", "/usr/local/bin/opencode").
    ///   None indicates the backend should use its default agent selection
    /// * `flags` - Additional flags to pass to the agent
    fn agent(&self, workspace_path: &Path, agent: Option<&str>, flags: &[String]) -> Result<()>;

    /// Get the backend name
    fn name(&self) -> &str;
}

/// Concrete backend implementation dispatcher
pub enum BackendType {
    Local(LocalBackend),
    ClaudeVm(ClaudeVmBackend),
}

impl BackendType {
    /// Create a new local backend
    pub fn local() -> Self {
        BackendType::Local(LocalBackend::new())
    }

    /// Create a new claude-vm backend
    pub fn claude_vm() -> Self {
        BackendType::ClaudeVm(ClaudeVmBackend::new())
    }

    /// Create a backend from a BackendKind
    pub fn from_kind(kind: BackendKind) -> Self {
        match kind {
            BackendKind::Local => Self::local(),
            BackendKind::ClaudeVm => Self::claude_vm(),
        }
    }
}

impl Backend for BackendType {
    fn shell(&self, workspace_path: &Path) -> Result<()> {
        match self {
            BackendType::Local(backend) => backend.shell(workspace_path),
            BackendType::ClaudeVm(backend) => backend.shell(workspace_path),
        }
    }

    fn exec(&self, workspace_path: &Path, command: &[String]) -> Result<ExecOutput> {
        match self {
            BackendType::Local(backend) => backend.exec(workspace_path, command),
            BackendType::ClaudeVm(backend) => backend.exec(workspace_path, command),
        }
    }

    fn agent(&self, workspace_path: &Path, agent: Option<&str>, flags: &[String]) -> Result<()> {
        match self {
            BackendType::Local(backend) => backend.agent(workspace_path, agent, flags),
            BackendType::ClaudeVm(backend) => backend.agent(workspace_path, agent, flags),
        }
    }

    fn name(&self) -> &str {
        match self {
            BackendType::Local(backend) => backend.name(),
            BackendType::ClaudeVm(backend) => backend.name(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_kind_display() {
        assert_eq!(BackendKind::Local.to_string(), "local");
        assert_eq!(BackendKind::ClaudeVm.to_string(), "claude-vm");
    }

    #[test]
    fn test_backend_kind_from_str() {
        assert_eq!(BackendKind::from_str("local").unwrap(), BackendKind::Local);
        assert_eq!(BackendKind::from_str("Local").unwrap(), BackendKind::Local);
        assert_eq!(BackendKind::from_str("LOCAL").unwrap(), BackendKind::Local);
        assert_eq!(
            BackendKind::from_str("claude-vm").unwrap(),
            BackendKind::ClaudeVm
        );
        assert_eq!(
            BackendKind::from_str("claudevm").unwrap(),
            BackendKind::ClaudeVm
        );
        assert_eq!(
            BackendKind::from_str("CLAUDE-VM").unwrap(),
            BackendKind::ClaudeVm
        );
    }

    #[test]
    fn test_backend_kind_from_str_invalid() {
        let result = BackendKind::from_str("invalid");
        assert!(result.is_err());
        match result {
            Err(AgentreeError::BackendNotFound { name, available }) => {
                assert_eq!(name, "invalid");
                assert_eq!(available.len(), 2);
            }
            _ => panic!("Expected BackendNotFound error"),
        }
    }

    #[test]
    fn test_backend_type_from_kind_local() {
        let backend = BackendType::from_kind(BackendKind::Local);
        assert_eq!(backend.name(), "local");
    }

    #[test]
    fn test_backend_type_from_kind_claude_vm() {
        let backend = BackendType::from_kind(BackendKind::ClaudeVm);
        assert_eq!(backend.name(), "claude-vm");
    }
}
