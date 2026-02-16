use crate::backend::{Backend, BackendType};
use crate::commands::common::{WorkspaceArgs, WorkspaceContext};
use crate::error::Result;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct ShellArgs {
    #[command(flatten)]
    pub workspace: WorkspaceArgs,
}

pub fn execute(args: ShellArgs) -> Result<()> {
    // Initialize workspace context (validation, git root discovery, config loading)
    let ctx = WorkspaceContext::init(
        args.workspace.backend.as_deref(),
        args.workspace.worktree_location.as_deref(),
        None, // agent not used in shell command
    )?;

    // Ensure workspace exists (create or resume) and set up metadata
    let workspace = ctx.ensure_workspace(&args.workspace.branch, args.workspace.base.as_deref())?;

    // Create backend and open shell (respects backend isolation per BACK-07)
    let backend = BackendType::from_kind(ctx.config.effective_backend());
    backend.shell(&workspace.path)?;

    Ok(())
}
