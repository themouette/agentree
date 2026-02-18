use crate::backend::{Backend, BackendType};
use crate::commands::common::{WorkspaceArgs, WorkspaceContext};
use crate::error::Result;
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
        None,
        None,
    )?;

    // Ensure workspace exists (create or resume) and set up metadata
    let workspace = ctx.ensure_workspace(&args.workspace.branch, args.workspace.base.as_deref())?;

    // EXEC ALWAYS RUNS ON HOST (BACK-08 decision): Create local backend directly
    let backend = BackendType::local();
    let output = backend.exec(&workspace.path, &args.command)?;

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
