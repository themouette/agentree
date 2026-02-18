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

            // Detect if the branch resolved to the main repository rather than a
            // dedicated worktree (happens when the branch is currently checked out
            // in the main repo, which git forbids from being in two places at once).
            let main_repo_path =
                get_git_common_dir()?.and_then(|d| d.parent().map(|p| p.to_path_buf()));
            let result_canonical = result.path().canonicalize().ok();
            let resolved_to_main_repo = match (&result_canonical, &main_repo_path) {
                (Some(rp), Some(mp)) => rp == mp,
                _ => false,
            };

            if resolved_to_main_repo {
                let main_display = main_repo_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| result.path().display().to_string());

                // Check if the caller is already sitting in the main repo.
                let already_there = std::env::current_dir()
                    .ok()
                    .and_then(|cwd| cwd.canonicalize().ok())
                    .zip(result_canonical)
                    .map(|(cwd, rp)| cwd == rp)
                    .unwrap_or(false);

                if already_there {
                    eprintln!(
                        "Warning: '{}' is the current branch of the main repository — you're already here.",
                        branch
                    );
                    eprintln!(
                        "         Tip: Use 'agentree cd' (no argument) to navigate here intentionally."
                    );
                } else {
                    eprintln!(
                        "Warning: '{}' is checked out in the main repository, not in a separate worktree.",
                        branch
                    );
                    eprintln!(
                        "         Navigating to the main repository at {}.",
                        main_display
                    );
                    eprintln!(
                        "         Tip: Switch the main repo to another branch first if you want an isolated worktree."
                    );
                }
            } else if result.was_created() {
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
