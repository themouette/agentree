use crate::error::Result;
use crate::utils::git::get_git_root;
use crate::worktree::{operations, validation, config::WorktreeConfig};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct CreateArgs {
    /// Branch name to create worktree for
    pub branch: String,

    /// Base branch or commit to create from
    #[arg(short, long)]
    pub base: Option<String>,
}

pub fn execute(args: CreateArgs) -> Result<()> {
    // Check git version
    validation::check_git_version()?;

    // Find repository root
    let repo_root = get_git_root()?.ok_or_else(|| {
        crate::error::AgentreeError::Worktree(
            "Not in a git repository. Run this command from inside a git repository.".to_string(),
        )
    })?;

    // Check for submodules and warn
    validation::check_submodules_and_warn(&repo_root);

    // Use default worktree config (Phase 3 will add config file support)
    let config = WorktreeConfig::default();

    // Create the worktree
    let result = operations::create_worktree(
        &config,
        &repo_root,
        &args.branch,
        args.base.as_deref(),
    )?;

    // Print success message
    println!("{}", result.message(&args.branch));

    Ok(())
}
