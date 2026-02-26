use crate::commands::common::{WorkspaceArgs, WorkspaceContext};
use crate::error::{AgentreeError, Result};
use crate::utils::progress::with_spinner;
use crate::worktree::metadata::WorktreeMetadata;
use crate::worktree::operations::{create_worktree, detect_branch_status, BranchStatus};
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

    let status = detect_branch_status(branch)?;

    if let BranchStatus::InWorktree(path) = &status {
        return Err(AgentreeError::Worktree(format!(
            "Worktree for '{}' already exists at {}.\nUse 'agentree agent {}' to open it.",
            branch,
            path.display(),
            branch
        )));
    }

    let msg = match &status {
        BranchStatus::DoesNotExist => format!("Creating branch '{}' and worktree...", branch),
        BranchStatus::ExistsNotCheckedOut => format!("Creating worktree for '{}'...", branch),
        BranchStatus::ExistsOnRemote(remote_ref) => format!(
            "Checking out '{}' from '{}' and creating worktree...",
            branch, remote_ref
        ),
        BranchStatus::InWorktree(_) => unreachable!(),
    };

    let result = with_spinner(&msg, || {
        create_worktree(&ctx.config.worktree, &ctx.repo_root, branch, base)
    })?;

    if result.was_created() {
        let metadata = WorktreeMetadata::new(ctx.config.effective_backend().to_string());
        metadata.save(result.path())?;
    }

    println!("{}", result.message(branch));

    Ok(())
}
