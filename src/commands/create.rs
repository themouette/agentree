use crate::commands::common::{WorkspaceArgs, WorkspaceContext};
use crate::error::Result;
use crate::utils::progress::ensure_workspace_with_progress;
use crate::worktree::metadata::WorktreeMetadata;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct CreateArgs {
    #[command(flatten)]
    pub workspace: WorkspaceArgs,
}

pub fn execute(args: CreateArgs) -> Result<()> {
    let ctx = WorkspaceContext::init(
        args.workspace.backend.as_deref(),
        args.workspace.worktree_location.as_deref(),
        None,
        None,
    )?;

    let branch = &args.workspace.branch;
    let base = args.workspace.base.as_deref();

    let result =
        ensure_workspace_with_progress(&ctx.config.worktree, &ctx.repo_root, branch, base)?;

    if result.was_created() {
        let metadata = WorktreeMetadata::new(ctx.config.effective_backend().to_string());
        metadata.save(result.path())?;
    }

    println!("{}", result.message(branch));

    Ok(())
}
