use crate::backend::{Backend, BackendType};
use crate::commands::common::{WorkspaceArgs, WorkspaceContext};
use crate::error::Result;
use crate::utils::git::validate_workspace_path;
use crate::worktree::{metadata::WorktreeMetadata, operations};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct ExecArgs {
    #[command(flatten)]
    pub workspace: WorkspaceArgs,

    /// Command to execute (after --)
    #[arg(last = true, required = true)]
    pub command: Vec<String>,
}

pub fn execute(args: ExecArgs) -> Result<()> {
    // Initialize workspace context (validation, git root discovery, config loading)
    let ctx = WorkspaceContext::init(
        args.workspace.backend.as_deref(),
        args.workspace.worktree_location.as_deref(),
        None, // agent not used in exec command
        None, // editor not used in exec command
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

    // Print auto-create message only for Created (not Resumed)
    if let operations::CreateResult::Created(_) = result {
        println!("{}", result.message(&args.workspace.branch));
    }

    // Validate path accessibility for backend
    validate_workspace_path(result.path(), &ctx.config.effective_backend())?;

    // EXEC ALWAYS RUNS ON HOST (BACK-08 decision): Create local backend directly
    let backend = BackendType::local();
    let output = backend.exec(result.path(), &args.command)?;

    // Print exec output: stdout to stdout, stderr to stderr
    print!("{}", output.stdout);
    eprint!("{}", output.stderr);

    // Exit with the backend's exit code if non-zero
    if let Some(code) = output.exit_code() {
        if code != 0 {
            std::process::exit(code);
        }
    }

    Ok(())
}
