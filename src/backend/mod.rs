use crate::error::{AgentreeError, Result};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

mod exec;
mod local;

pub use exec::ExecOutput;
pub use local::LocalBackend;

/// Backend selection enum used for config and CLI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Local,
    ClaudeVm,
    Claude,
}

impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendKind::Local => write!(f, "local"),
            BackendKind::ClaudeVm => write!(f, "claude-vm"),
            BackendKind::Claude => write!(f, "claude"),
        }
    }
}

impl FromStr for BackendKind {
    type Err = AgentreeError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "local" => Ok(BackendKind::Local),
            "claude-vm" | "claudevm" => Ok(BackendKind::ClaudeVm),
            "claude" => Ok(BackendKind::Claude),
            _ => Err(AgentreeError::BackendNotFound {
                name: s.to_string(),
                available: vec!["local".to_string(), "claude-vm".to_string(), "claude".to_string()],
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
    fn agent(&self, workspace_path: &Path, flags: &[String]) -> Result<()>;

    /// Get the backend name
    fn name(&self) -> &str;
}

/// Concrete backend implementation dispatcher
pub enum BackendType {
    Local(LocalBackend),
    // TODO: Plan 02 - Add ClaudeVm(ClaudeVmBackend)
    // TODO: Plan 02 - Add Claude(ClaudeBackend)
}

impl BackendType {
    /// Create a new local backend
    pub fn local() -> Self {
        BackendType::Local(LocalBackend::new())
    }
}

impl Backend for BackendType {
    fn shell(&self, workspace_path: &Path) -> Result<()> {
        match self {
            BackendType::Local(backend) => backend.shell(workspace_path),
        }
    }

    fn exec(&self, workspace_path: &Path, command: &[String]) -> Result<ExecOutput> {
        match self {
            BackendType::Local(backend) => backend.exec(workspace_path, command),
        }
    }

    fn agent(&self, workspace_path: &Path, flags: &[String]) -> Result<()> {
        match self {
            BackendType::Local(backend) => backend.agent(workspace_path, flags),
        }
    }

    fn name(&self) -> &str {
        match self {
            BackendType::Local(backend) => backend.name(),
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
        assert_eq!(BackendKind::Claude.to_string(), "claude");
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
        assert_eq!(BackendKind::from_str("claude").unwrap(), BackendKind::Claude);
    }

    #[test]
    fn test_backend_kind_from_str_invalid() {
        let result = BackendKind::from_str("invalid");
        assert!(result.is_err());
        match result {
            Err(AgentreeError::BackendNotFound { name, available }) => {
                assert_eq!(name, "invalid");
                assert_eq!(available.len(), 3);
            }
            _ => panic!("Expected BackendNotFound error"),
        }
    }
}
