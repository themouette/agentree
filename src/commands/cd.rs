use crate::commands::common::WorkspaceContext;
use crate::error::{AgentreeError, Result};
use crate::utils::git::get_git_common_dir;
use crate::utils::progress::ensure_workspace_with_progress;
use crate::worktree::metadata::WorktreeMetadata;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct CdArgs {
    /// Branch name for the workspace (omit to navigate to the main repository)
    pub branch: Option<String>,

    /// Git ref to create branch from (if workspace doesn't exist)
    #[arg(short = 'b', long)]
    pub base: Option<String>,

    /// Backend to use for this worktree (overrides config)
    #[arg(long)]
    pub backend: Option<String>,

    /// Worktree location (overrides config)
    #[arg(long = "worktree-location")]
    pub worktree_location: Option<String>,
}

pub fn execute(args: CdArgs) -> Result<()> {
    match args.branch.as_deref().filter(|b| !b.is_empty()) {
        None => {
            // No branch: navigate to the main repository root.
            // --git-common-dir always points to the main repo's .git dir,
            // whether we're in a worktree or in the main repo itself.
            let common_dir = get_git_common_dir()?.ok_or_else(|| {
                AgentreeError::Worktree(
                    "Not in a git repository. Run this command from inside a git repository."
                        .to_string(),
                )
            })?;
            let main_repo = common_dir.parent().ok_or_else(|| {
                AgentreeError::Worktree("Cannot determine main repository path.".to_string())
            })?;
            println!("cd {}", shell_escape(&main_repo.to_string_lossy()));
        }
        Some(branch) => {
            let ctx = WorkspaceContext::init(
                args.backend.as_deref(),
                args.worktree_location.as_deref(),
                None,
                None,
            )?;

            let result = ensure_workspace_with_progress(
                &ctx.config.worktree,
                &ctx.repo_root,
                branch,
                args.base.as_deref(),
            )?;

            if result.was_created() {
                let metadata = WorktreeMetadata::new(ctx.config.effective_backend().to_string());
                metadata.save(result.path())?;
                // Inform the user something was created; skip this on plain resume to
                // keep navigation noise-free.
                eprintln!("{}", result.message(branch));
            }

            // Print cd command to stdout for shell eval
            println!("cd {}", shell_escape(&result.path().to_string_lossy()));
        }
    }

    Ok(())
}

/// Escape a string for safe use in shell commands.
/// Uses single quotes and handles embedded single quotes.
fn shell_escape(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
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
