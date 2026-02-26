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

    // Validate base ref early so we get helpful "Did you mean X?" suggestions
    // instead of a raw git error buried inside the spinner.
    if let Some(ref_name) = base {
        crate::utils::git::validate_start_ref(ref_name)?;
    }

    let status = detect_branch_status(branch)?;

    // Error if -b is provided but the branch already exists; -b only applies
    // when creating a new branch from scratch.
    if let Some(ref_name) = base {
        match &status {
            BranchStatus::ExistsNotCheckedOut => {
                return Err(AgentreeError::Worktree(format!(
                    "Branch '{}' already exists locally.\n\
                     Remove '-b {}' or use a different branch name.",
                    branch, ref_name
                )));
            }
            BranchStatus::ExistsOnRemote(remote_ref) => {
                return Err(AgentreeError::Worktree(format!(
                    "Branch '{}' already exists on remote ('{}').\n\
                     Remove '-b {}' or use a different branch name.",
                    branch, remote_ref, ref_name
                )));
            }
            _ => {} // InWorktree errors below; DoesNotExist is the normal case
        }
    }

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
