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
    // Use single quotes to handle spaces in paths
    println!("cd '{}'", worktree.path.display());

    Ok(())
}
