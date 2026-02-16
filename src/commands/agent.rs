use crate::backend::{Backend, BackendKind, BackendType};
use crate::commands::common::{WorkspaceArgs, WorkspaceContext};
use crate::error::Result;
use crate::utils::git::validate_workspace_path;
use crate::worktree::{metadata::WorktreeMetadata, operations};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct AgentArgs {
    #[command(flatten)]
    pub workspace: WorkspaceArgs,

    /// Flags to pass through to the AI agent (after --)
    #[arg(last = true)]
    pub flags: Vec<String>,

    /// Agent to use (overrides config). Required for 'local' backend, optional for 'claude-vm'
    #[arg(long)]
    pub agent: Option<String>,
}

pub fn execute(args: AgentArgs) -> Result<()> {
    // Initialize workspace context (validation, git root discovery, config loading)
    let ctx = WorkspaceContext::init(
        args.workspace.backend.as_deref(),
        args.workspace.worktree_location.as_deref(),
        args.agent.as_deref(),
        None, // editor not used in agent command
    )?;

    // Determine which backend we're using
    let backend_kind = ctx.config.effective_backend();

    // Resolve agent based on backend requirements:
    // - local backend: agent is required
    // - claude-vm backend: agent is optional (claude-vm can handle agent selection)
    let (agent_bin, default_args) = match backend_kind {
        BackendKind::Local => {
            // Local backend requires an agent
            let (bin, args) = ctx.config.resolve_agent(args.agent.as_deref())?;
            (Some(bin), args)
        }
        BackendKind::ClaudeVm => {
            // Claude-vm backend: agent is optional
            if args.agent.is_some() {
                // User explicitly specified an agent, resolve it
                let (bin, args) = ctx.config.resolve_agent(args.agent.as_deref())?;
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

    // Agent respects backend isolation (BACK-09)
    let backend = BackendType::from_kind(ctx.config.effective_backend());
    backend.agent(result.path(), agent_bin.as_deref(), &all_flags)?;

    Ok(())
}
