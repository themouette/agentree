use crate::error::Result;
use crate::utils::git::get_git_root;
use crate::worktree::{recovery, validation};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct CleanArgs {
    // No arguments needed for now
}

pub fn execute(_args: CleanArgs) -> Result<()> {
    // Check git version
    validation::check_git_version()?;

    // Find repository root
    let _repo_root = get_git_root()?.ok_or_else(|| {
        crate::error::AgentreeError::Worktree(
            "Not in a git repository. Run this command from inside a git repository.".to_string(),
        )
    })?;

    // Run full cleanup: repair broken links first, then prune stale metadata
    recovery::try_repair()?;
    recovery::prune()?;

    println!("Cleanup complete.");

    Ok(())
}
