use crate::error::{AgentreeError, Result};
use crate::utils::git::get_git_root;
use crate::worktree::{recovery, validation};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct CdArgs {
    /// Branch name to navigate to
    pub branch: String,
}

pub fn execute(args: CdArgs) -> Result<()> {
    // Check git version
    validation::check_git_version()?;

    // Get repo root (verify we're in a git repository)
    let _repo_root =
        get_git_root()?.ok_or_else(|| AgentreeError::Git("Not in a git repository".to_string()))?;

    // Get worktrees with cleanup
    let worktrees = recovery::ensure_clean_state()?;

    // Find worktree matching the branch
    let worktree = worktrees
        .iter()
        .find(|e| e.branch.as_deref() == Some(&args.branch))
        .ok_or_else(|| AgentreeError::WorktreeNotFound {
            branch: args.branch.clone(),
        })?;

    // Print cd command to stdout (for shell eval)
    // Use shell_escape to safely escape the path
    let path_str = worktree.path.to_string_lossy();
    println!("cd {}", shell_escape(&path_str));

    Ok(())
}

/// Escape a string for safe use in shell commands
/// Uses single quotes and escapes any embedded single quotes
fn shell_escape(s: &str) -> String {
    // If the string is empty, return empty quotes
    if s.is_empty() {
        return "''".to_string();
    }

    // Replace single quotes with '\'' (end quote, escaped quote, start quote)
    let escaped = s.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_escape_simple() {
        assert_eq!(shell_escape("simple"), "'simple'");
    }

    #[test]
    fn test_shell_escape_with_spaces() {
        assert_eq!(shell_escape("path with spaces"), "'path with spaces'");
    }

    #[test]
    fn test_shell_escape_with_single_quote() {
        assert_eq!(shell_escape("path's test"), "'path'\\''s test'");
    }

    #[test]
    fn test_shell_escape_empty() {
        assert_eq!(shell_escape(""), "''");
    }

    #[test]
    fn test_shell_escape_special_chars() {
        assert_eq!(shell_escape("$PATH & stuff"), "'$PATH & stuff'");
    }
}
