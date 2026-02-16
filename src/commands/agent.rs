use crate::backend::{Backend, BackendKind, BackendType};
use crate::config;
use crate::error::Result;
use crate::utils::git::{get_git_root, validate_workspace_path};
use crate::worktree::{metadata::WorktreeMetadata, operations, validation};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct AgentArgs {
    /// Branch name (workspace auto-created if needed)
    pub branch: String,

    /// Git ref to create branch from
    #[arg(value_name = "START_REF")]
    pub start_ref: Option<String>,

    /// Flags to pass through to the AI agent (after --)
    #[arg(last = true)]
    pub flags: Vec<String>,

    /// Backend to use for this worktree (overrides config)
    #[arg(long)]
    pub backend: Option<String>,

    /// Agent to use (overrides config). Required for 'local' backend, optional for 'claude-vm'
    #[arg(long)]
    pub agent: Option<String>,

    /// Worktree location (overrides config)
    #[arg(long = "worktree-location")]
    pub worktree_location: Option<String>,
}

pub fn execute(args: AgentArgs) -> Result<()> {
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
    let config = config::load(&repo_root)?.with_cli_overrides(
        args.backend.as_deref(),
        args.worktree_location.as_deref(),
        args.agent.as_deref(),
        None, // editor not used in agent command
    )?;

    // Determine which backend we're using
    let backend_kind = config.effective_backend();

    // Resolve agent based on backend requirements:
    // - local backend: agent is required
    // - claude-vm backend: agent is optional (claude-vm can handle agent selection)
    let (agent_bin, default_args) = match backend_kind {
        BackendKind::Local => {
            // Local backend requires an agent
            let (bin, args) = config.resolve_agent(args.agent.as_deref())?;
            (Some(bin), args)
        }
        BackendKind::ClaudeVm => {
            // Claude-vm backend: agent is optional
            if args.agent.is_some() {
                // User explicitly specified an agent, resolve it
                let (bin, args) = config.resolve_agent(args.agent.as_deref())?;
                (Some(bin), args)
            } else {
                // No agent specified, let claude-vm handle it
                (None, vec![])
            }
        }
    };

    // Combine default_args from config with user-provided flags
    let mut all_flags = default_args;
    all_flags.extend(args.flags);

    // Auto-create workspace (idempotent - returns Resumed if exists)
    let result = operations::ensure_workspace(
        &config.worktree,
        &repo_root,
        &args.branch,
        args.start_ref.as_deref(),
    )?;

    // Save metadata for newly created workspaces (not for resumed ones)
    if let operations::CreateResult::Created(_) = result {
        let metadata = WorktreeMetadata::new(config.effective_backend().to_string());
        metadata.save(result.path())?;
    }

    // Print auto-create message only for Created (not Resumed)
    if let operations::CreateResult::Created(_) = result {
        println!("{}", result.message(&args.branch));
    }

    // Validate path accessibility for backend
    validate_workspace_path(result.path(), &config.effective_backend())?;

    // Agent respects backend isolation (BACK-09)
    let backend = BackendType::from_kind(config.effective_backend());
    backend.agent(result.path(), agent_bin.as_deref(), &all_flags)?;

    Ok(())
}
