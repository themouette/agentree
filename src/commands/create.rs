use crate::commands::common::WorkspaceArgs;
use crate::config;
use crate::error::Result;
use crate::utils::git::get_git_root;
use crate::worktree::{metadata::WorktreeMetadata, operations, validation};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct CreateArgs {
    #[command(flatten)]
    pub workspace: WorkspaceArgs,
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

    // Load config from files and apply CLI overrides
    let config = config::load(&repo_root)?.with_cli_overrides(
        args.workspace.backend.as_deref(),
        args.workspace.worktree_location.as_deref(),
        None, // agent not used in create command
        None, // editor not used in create command
    )?;

    // Create the worktree
    let result = operations::create_worktree(
        &config.worktree,
        &repo_root,
        &args.workspace.branch,
        args.workspace.base.as_deref(),
    )?;

    // Save metadata for newly created worktrees (not for resumed ones)
    if let operations::CreateResult::Created(_) = result {
        let metadata = WorktreeMetadata::new(config.effective_backend().to_string());
        metadata.save(result.path())?;
    }

    // Print success message
    println!("{}", result.message(&args.workspace.branch));

    Ok(())
}
