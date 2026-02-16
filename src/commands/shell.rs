use crate::backend::{Backend, BackendType};
use crate::commands::common::{WorkspaceArgs, WorkspaceContext};
use crate::error::Result;
use crate::utils::git::validate_workspace_path;
use crate::worktree::{metadata::WorktreeMetadata, operations};
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
        None, // editor not used in shell command
    )?;

    // Auto-create workspace (idempotent - returns Resumed if exists)
    let result = operations::ensure_workspace(
        &ctx.config.worktree,
        &ctx.repo_root,
        &args.workspace.branch,
        args.workspace.base.as_deref(),
    )?;

    // Save metadata for newly created workspaces (not for resumed ones)
    if let operations::CreateResult::Created(_) = result {
        let metadata = WorktreeMetadata::new(ctx.config.effective_backend().to_string());
        metadata.save(result.path())?;
    }

    // Print auto-create message only for Created (not Resumed, which should be silent for shell)
    if let operations::CreateResult::Created(_) = result {
        println!("{}", result.message(&args.workspace.branch));
    }

    // Validate path accessibility for backend
    validate_workspace_path(result.path(), &ctx.config.effective_backend())?;

    // Create backend and open shell (respects backend isolation per BACK-07)
    let backend = BackendType::from_kind(ctx.config.effective_backend());
    backend.shell(result.path())?;

    Ok(())
}
