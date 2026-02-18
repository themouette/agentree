use crate::backend::run_interactive;
use crate::config;
use crate::error::Result;
use crate::utils::git::get_git_root;
use crate::worktree::{metadata::WorktreeMetadata, operations, validation};
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
    // Check git version
    validation::check_git_version()?;

    // Find repository root
    let repo_root = get_git_root()?.ok_or_else(|| {
        crate::error::AgentreeError::Worktree(
            "Not in a git repository. Run this command from inside a git repository.".to_string(),
        )
    })?;

    // Check for submodules and warn
    validation::check_submodules_and_warn(&repo_root);

    // Load config with CLI overrides
    let config = config::load(&repo_root)?.with_cli_overrides(
        args.backend.as_deref(),
        args.worktree_location.as_deref(),
        None, // agent not used
        args.editor.as_deref(),
    )?;

    // Ensure workspace exists (auto-create if needed)
    let result = operations::ensure_workspace(
        &config.worktree,
        &repo_root,
        &args.branch,
        args.start_ref.as_deref(),
    )?;

    // Save metadata only for newly created workspaces
    if result.was_created() {
        let metadata = WorktreeMetadata::new(config.effective_backend().to_string());
        metadata.save(result.path())?;
    }
    eprintln!("{}", result.message(&args.branch));

    // Validate workspace path is accessible
    let workspace_path = result.path();
    if !workspace_path.exists() {
        return Err(crate::error::AgentreeError::Worktree(format!(
            "Workspace path does not exist: {}",
            workspace_path.display()
        )));
    }

    // Get effective editor from config (respects precedence)
    let editor_bin = config.effective_editor();

    // Combine default args from config with CLI args
    let mut all_args = config.editor.default_args.clone();
    all_args.extend_from_slice(&args.args);

    // Convert to &str references for run_interactive
    let args_refs: Vec<&str> = all_args.iter().map(|s| s.as_str()).collect();

    // Run editor directly on local machine (always, regardless of backend)
    run_interactive(&editor_bin, &args_refs, workspace_path)?;

    Ok(())
}
