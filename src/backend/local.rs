use crate::backend::exec::{run_captured, run_interactive, ExecOutput};
use crate::backend::Backend;
use crate::error::{AgentreeError, Result};
use std::env;
use std::path::Path;

/// Local backend that runs commands directly without isolation
#[derive(Debug)]
pub struct LocalBackend {
    shell_binary: String,
}

impl LocalBackend {
    /// Create a new LocalBackend
    pub fn new() -> Self {
        let shell_binary = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        Self { shell_binary }
    }
}

impl Default for LocalBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for LocalBackend {
    fn shell(&self, workspace_path: &Path) -> Result<()> {
        run_interactive(&self.shell_binary, &[], workspace_path)
    }

    fn exec(&self, workspace_path: &Path, command: &[String]) -> Result<ExecOutput> {
        if command.is_empty() {
            return Err(AgentreeError::BackendExecution {
                backend: "local".to_string(),
                error: "No command provided".to_string(),
            });
        }

        let binary = &command[0];
        let args: Vec<&str> = command[1..].iter().map(|s| s.as_str()).collect();
        run_captured(binary, &args, workspace_path)
    }

    fn agent(&self, workspace_path: &Path, _flags: &[String]) -> Result<()> {
        // Local backend doesn't support agent mode
        // Users should use 'claude' or 'claude-vm' backend for AI agent sessions
        let _ = workspace_path; // Suppress unused warning
        Err(AgentreeError::BackendExecution {
            backend: "local".to_string(),
            error: "Local backend does not support agent command. Use 'claude' or 'claude-vm' backend.".to_string(),
        })
    }

    fn name(&self) -> &str {
        "local"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_local_backend_new() {
        let backend = LocalBackend::new();
        assert!(!backend.shell_binary.is_empty());
    }

    #[test]
    fn test_local_backend_name() {
        let backend = LocalBackend::new();
        assert_eq!(backend.name(), "local");
    }

    #[test]
    fn test_local_backend_exec_echo() {
        let backend = LocalBackend::new();
        let temp_dir = TempDir::new().unwrap();

        let command = vec!["echo".to_string(), "hello".to_string()];
        let result = backend.exec(temp_dir.path(), &command);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.success());
        assert!(output.stdout.contains("hello"));
    }

    #[test]
    fn test_local_backend_exec_empty_command() {
        let backend = LocalBackend::new();
        let temp_dir = TempDir::new().unwrap();

        let command = vec![];
        let result = backend.exec(temp_dir.path(), &command);

        assert!(result.is_err());
        match result {
            Err(AgentreeError::BackendExecution { backend, error }) => {
                assert_eq!(backend, "local");
                assert!(error.contains("No command provided"));
            }
            _ => panic!("Expected BackendExecution error"),
        }
    }

    #[test]
    fn test_local_backend_agent_not_supported() {
        let backend = LocalBackend::new();
        let temp_dir = TempDir::new().unwrap();

        let result = backend.agent(temp_dir.path(), &[]);

        assert!(result.is_err());
        match result {
            Err(AgentreeError::BackendExecution { backend, error }) => {
                assert_eq!(backend, "local");
                assert!(error.contains("does not support agent"));
            }
            _ => panic!("Expected BackendExecution error"),
        }
    }
}
