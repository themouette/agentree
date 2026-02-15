use crate::config;
use crate::error::Result;
use crate::utils::git::get_git_root;
use crate::worktree::{metadata::WorktreeMetadata, operations, validation};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct CreateArgs {
    /// Branch name to create worktree for
    pub branch: String,

    /// Base branch or commit to create from (also accepts positional START_REF)
    #[arg(value_name = "START_REF")]
    pub start_ref: Option<String>,

    /// Base branch or commit (deprecated: use positional START_REF instead)
    #[arg(long = "base", hide = true)]
    pub base_alias: Option<String>,

    /// Backend to use for this worktree (overrides config)
    #[arg(long)]
    pub backend: Option<String>,

    /// Worktree location (overrides config)
    #[arg(long = "worktree-location")]
    pub worktree_location: Option<String>,
}

pub fn execute(args: CreateArgs) -> Result<()> {
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

    // Load config from files and apply CLI overrides
    let config = config::load(&repo_root)?
        .with_cli_overrides(args.backend.as_deref(), args.worktree_location.as_deref())?;

    // Resolve effective base reference (prefer positional arg, fall back to --base flag)
    let effective_base = args.start_ref.or(args.base_alias);

    // Create the worktree
    let result = operations::create_worktree(
        &config.worktree,
        &repo_root,
        &args.branch,
        effective_base.as_deref(),
    )?;

    // Save metadata for newly created worktrees (not for resumed ones)
    if let operations::CreateResult::Created(_) = result {
        let metadata = WorktreeMetadata::new(config.effective_backend().to_string());
        metadata.save(result.path())?;
    }

    // Print success message
    println!("{}", result.message(&args.branch));

    Ok(())
}
