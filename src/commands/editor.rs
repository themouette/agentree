use crate::backend::run_interactive;
use crate::commands::common::WorkspaceContext;
use crate::error::Result;
use clap::Parser;

/// Open an editor in a workspace
#[derive(Parser, Debug)]
pub struct EditorArgs {
    /// Branch name for the workspace
    pub branch: String,

    /// Base branch to create from (if workspace doesn't exist)
    #[arg(value_name = "START_REF")]
    pub start_ref: Option<String>,

    /// Additional arguments to pass to the editor
    #[arg(last = true)]
    pub args: Vec<String>,

    /// Override backend (for workspace creation)
    #[arg(long)]
    pub backend: Option<String>,

    /// Override editor binary
    #[arg(long)]
    pub editor: Option<String>,

    /// Override worktree location
    #[arg(long = "worktree-location")]
    pub worktree_location: Option<String>,
}

pub fn execute(args: EditorArgs) -> Result<()> {
    let ctx = WorkspaceContext::init(
        args.backend.as_deref(),
        args.worktree_location.as_deref(),
        None,
        args.editor.as_deref(),
    )?;

    let workspace = ctx.ensure_workspace(&args.branch, args.start_ref.as_deref())?;

    let editor_bin = ctx.config.effective_editor();
    let mut all_args = ctx.config.editor.default_args.clone();
    all_args.extend_from_slice(&args.args);
    let args_refs: Vec<&str> = all_args.iter().map(|s| s.as_str()).collect();

    run_interactive(&editor_bin, &args_refs, &workspace.path)?;

    Ok(())
}
