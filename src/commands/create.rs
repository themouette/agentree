use crate::commands::common::{WorkspaceArgs, WorkspaceContext};
use crate::error::Result;
use crate::worktree::{metadata::WorktreeMetadata, operations};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct CreateArgs {
    #[command(flatten)]
    pub workspace: WorkspaceArgs,
}

pub fn execute(args: CreateArgs) -> Result<()> {
    // Initialize workspace context (validation, git root discovery, config loading)
    let ctx = WorkspaceContext::init(
        args.workspace.backend.as_deref(),
        args.workspace.worktree_location.as_deref(),
        None, // agent not used in create command
    )?;

    // Create the worktree; resumes silently if the branch already has one
    let result = operations::create_worktree(
        &ctx.config.worktree,
        &ctx.repo_root,
        &args.workspace.branch,
        args.workspace.base.as_deref(),
    )?;

    // Save metadata for newly created worktrees
    if result.was_created() {
        let metadata = WorktreeMetadata::new(ctx.config.effective_backend().to_string());
        metadata.save(result.path())?;
    }

    // Print success message
    println!("{}", result.message(&args.workspace.branch));

    Ok(())
}
