use crate::commands::common::{WorkspaceArgs, WorkspaceContext};
use crate::error::Result;
use crate::utils::progress::with_spinner;
use crate::worktree::operations::{BranchStatus, CreateResult};
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

    let branch = &args.workspace.branch;
    let base = args.workspace.base.as_deref();

    // Detect branch status to show intent before the slow git operation
    let status = operations::detect_branch_status(branch)?;

    let result: CreateResult = match &status {
        BranchStatus::InWorktree(_) => {
            // Already exists — instant, no spinner needed
            operations::create_worktree(&ctx.config.worktree, &ctx.repo_root, branch, base)?
        }
        BranchStatus::ExistsNotCheckedOut => {
            let msg = format!("Creating worktree for '{}'...", branch);
            with_spinner(&msg, || {
                operations::create_worktree(&ctx.config.worktree, &ctx.repo_root, branch, base)
            })?
        }
        BranchStatus::DoesNotExist => {
            let msg = format!("Creating branch '{}' and worktree...", branch);
            with_spinner(&msg, || {
                operations::create_worktree(&ctx.config.worktree, &ctx.repo_root, branch, base)
            })?
        }
    };

    // Save metadata for newly created worktrees
    if result.was_created() {
        let metadata = WorktreeMetadata::new(ctx.config.effective_backend().to_string());
        metadata.save(result.path())?;
    }

    // Print success message
    println!("{}", result.message(branch));

    Ok(())
}
