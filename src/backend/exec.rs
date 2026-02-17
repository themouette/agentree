use crate::error::{AgentreeError, Result};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

/// Output captured from a process execution
#[derive(Debug)]
pub struct ExecOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl ExecOutput {
    /// Create a new ExecOutput
    pub fn new(status: ExitStatus, stdout: String, stderr: String) -> Self {
        Self {
            status,
            stdout,
            stderr,
        }
    }

    /// Check if the command succeeded
    pub fn success(&self) -> bool {
        self.status.success()
    }

    /// Get the exit code if available
    pub fn exit_code(&self) -> Option<i32> {
        self.status.code()
    }
}

/// Run a command interactively (inherits stdio)
pub fn run_interactive(binary: &str, args: &[&str], working_dir: &Path) -> Result<()> {
    // Extract branch name from workspace path for environment variable
    // Workspace paths typically follow pattern: .../agentree-{branch}
    let branch = extract_branch_from_path(working_dir);

    let status = Command::new(binary)
        .args(args)
        .current_dir(working_dir)
        // Set agentree-specific environment variables for prompt customization
        .env("AGENTREE_WORKSPACE", "1")
        .env("AGENTREE_BRANCH", branch)
        .env("AGENTREE_WORKSPACE_PATH", working_dir)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| AgentreeError::BackendExecution {
            backend: binary.to_string(),
            error: e.to_string(),
        })?;

    if !status.success() {
        return Err(AgentreeError::BackendFailed {
            backend: binary.to_string(),
            exit_code: status.code(),
        });
    }

    Ok(())
}

/// Extract branch name from workspace path
///
/// Attempts to extract the branch name from the workspace directory name.
/// Common patterns: "agentree-{branch}" or "{repo}-{branch}"
/// Falls back to the directory name if pattern doesn't match.
fn extract_branch_from_path(path: &Path) -> &str {
    path.file_name()
        .and_then(|n| n.to_str())
        .and_then(|name| {
            // Try to extract from "agentree-{branch}" pattern
            name.strip_prefix("agentree-")
                // Or from "{repo}-{branch}" pattern (take after last dash)
                .or_else(|| name.rsplit_once('-').map(|(_, branch)| branch))
        })
        .unwrap_or_else(|| {
            // Fallback: use the whole directory name
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
        })
}

/// Run a command and capture output
pub fn run_captured(binary: &str, args: &[&str], working_dir: &Path) -> Result<ExecOutput> {
    let output = Command::new(binary)
        .args(args)
        .current_dir(working_dir)
        .output()
        .map_err(|e| AgentreeError::BackendExecution {
            backend: binary.to_string(),
            error: e.to_string(),
        })?;

    Ok(ExecOutput::new(
        output.status,
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    ))
}

/// Run a command directly on the host (common implementation for backends that don't isolate exec)
///
/// This is used by backends that run exec commands directly on the host rather than in isolation.
/// For example, claude-vm runs shell/agent in the VM but exec on the host per BACK-08.
///
/// # Arguments
/// * `workspace_path` - The workspace directory to run the command in
/// * `command` - The command and arguments to execute (command[0] is the binary)
/// * `backend_name` - The name of the backend for error messages
pub fn run_host_command(
    workspace_path: &Path,
    command: &[String],
    backend_name: &str,
) -> Result<ExecOutput> {
    if command.is_empty() {
        return Err(AgentreeError::BackendExecution {
            backend: backend_name.to_string(),
            error: "No command provided".to_string(),
        });
    }

    // Convert &[String] to &[&str] for run_captured
    let args: Vec<&str> = command[1..].iter().map(|s| s.as_str()).collect();
    run_captured(&command[0], &args, workspace_path)
}

/// Get the version of a binary by running it with a version flag
pub fn get_binary_version(binary: &str, version_flag: &str) -> Result<String> {
    let output = Command::new(binary)
        .arg(version_flag)
        .output()
        .map_err(|e| AgentreeError::BackendExecution {
            backend: binary.to_string(),
            error: e.to_string(),
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Try to extract version with pattern: v?(\d+\.\d+\.\d+)
    for line in combined.lines() {
        // Look for semantic version pattern
        if let Some(version) = extract_version(line) {
            return Ok(version);
        }
    }

    Err(AgentreeError::VersionParse {
        version: combined.trim().to_string(),
        error: "Could not find version pattern in output".to_string(),
    })
}

/// Extract semantic version from a string
fn extract_version(s: &str) -> Option<String> {
    // Simple version extraction: look for X.Y.Z pattern
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Skip 'v' prefix if present
        let start = if chars[i] == 'v' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
            i + 1
        } else if chars[i].is_ascii_digit() {
            i
        } else {
            i += 1;
            continue;
        };

        // Try to parse version from this position
        let mut end = start;
        let mut dots = 0;

        while end < chars.len() {
            if chars[end].is_ascii_digit() {
                end += 1;
            } else if chars[end] == '.' && dots < 2 {
                dots += 1;
                end += 1;
            } else {
                break;
            }
        }

        // Check if we found a valid version (must have at least 2 dots for X.Y.Z)
        if dots >= 2 {
            let version: String = chars[start..end].iter().collect();
            return Some(version);
        }

        i = end + 1;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_exec_output_fields() {
        let current_dir = env::current_dir().unwrap();
        let result = run_captured("echo", &["test"], &current_dir);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.success());
        assert_eq!(output.stdout.trim(), "test");
        assert!(output.exit_code().is_some());
    }

    #[test]
    fn test_run_captured_git_version() {
        let current_dir = env::current_dir().unwrap();
        let result = run_captured("git", &["--version"], &current_dir);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.success());
        assert!(output.stdout.contains("git version"));
    }

    #[test]
    fn test_get_binary_version_git() {
        let result = get_binary_version("git", "--version");
        assert!(result.is_ok());
        let version = result.unwrap();
        // Version should be in format X.Y.Z
        assert!(version.contains('.'));
    }

    #[test]
    fn test_extract_version() {
        assert_eq!(
            extract_version("git version 2.39.1"),
            Some("2.39.1".to_string())
        );
        assert_eq!(extract_version("v1.2.3"), Some("1.2.3".to_string()));
        assert_eq!(
            extract_version("version 10.20.30"),
            Some("10.20.30".to_string())
        );
        assert_eq!(extract_version("1.2.3-beta"), Some("1.2.3".to_string()));
        assert_eq!(extract_version("no version here"), None);
        assert_eq!(extract_version("1.2"), None); // Not enough dots
    }

    #[test]
    fn test_run_captured_nonexistent_binary() {
        let current_dir = env::current_dir().unwrap();
        let result = run_captured("nonexistent_binary_xyz", &[], &current_dir);
        assert!(result.is_err());
        match result {
            Err(AgentreeError::BackendExecution { backend, .. }) => {
                assert_eq!(backend, "nonexistent_binary_xyz");
            }
            _ => panic!("Expected BackendExecution error"),
        }
    }
}
