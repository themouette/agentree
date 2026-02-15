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
    let status = Command::new(binary)
        .args(args)
        .current_dir(working_dir)
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
