use crate::agent::{Agent, AgentType};
use crate::backend::{Backend, BackendKind, BackendType};
use crate::commands::common::{WorkspaceArgs, WorkspaceContext};
use crate::error::Result;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct AgentArgs {
    #[command(flatten)]
    pub workspace: WorkspaceArgs,

    /// Flags to pass through to the AI agent (after --)
    #[arg(last = true)]
    pub flags: Vec<String>,

    /// Agent to use (overrides config). Required for 'local' and 'docker-sandbox' backends, optional for 'claude-vm'
    #[arg(long)]
    pub agent: Option<String>,
}

pub fn execute(args: AgentArgs) -> Result<()> {
    // Initialize workspace context (validation, git root discovery, config loading)
    let ctx = WorkspaceContext::init(
        args.workspace.backend.as_deref(),
        args.workspace.worktree_location.as_deref(),
        args.agent.as_deref(),
        None,
    )?;

    // Ensure workspace exists (create or resume) and set up metadata
    let workspace = ctx.ensure_workspace(&args.workspace.branch, args.workspace.base.as_deref())?;

    // Determine which backend we're using.
    // Priority: CLI --backend flag > workspace metadata (creation-time backend) > config default.
    // Reading metadata ensures that a workspace created with e.g. `--backend local` continues
    // to use the local backend even when the global/project config defaults to another backend.
    let backend_kind = ctx.effective_backend_for(&workspace, args.workspace.backend.is_some());

    // Determine the logical agent name for trait dispatch.
    // ClaudeVm with no explicit agent still defaults to "claude" so that
    // CLAUDE.md injection and settings.json allowedTools are set up even
    // when claude-vm manages agent selection on its own.
    let agent_logical_name: &str = match backend_kind {
        BackendKind::ClaudeVm if args.agent.is_none() => "claude",
        _ => args
            .agent
            .as_deref()
            .or(ctx.config.agent.default.as_deref())
            .unwrap_or("claude"),
    };

    // Resolve agent binary path and default flags for backend invocation.
    // - local / docker-sandbox: agent is required
    // - claude-vm: agent is optional (claude-vm can handle agent selection)
    let (agent_bin, default_args) = match backend_kind {
        BackendKind::Local | BackendKind::DockerSandbox => {
            let (bin, flags) = ctx.config.resolve_agent(args.agent.as_deref())?;
            (Some(bin), flags)
        }
        BackendKind::ClaudeVm => {
            if args.agent.is_some() {
                let (bin, flags) = ctx.config.resolve_agent(args.agent.as_deref())?;
                (Some(bin), flags)
            } else {
                (None, vec![])
            }
        }
    };

    // Combine default_args from config with user-provided flags
    let mut all_flags = default_args;
    all_flags.extend(args.flags);

    // Prepare agent workspace files (e.g. CLAUDE.md injection, settings.json allowedTools)
    let agent_impl = AgentType::from_name(agent_logical_name);
    let token = agent_impl
        .prepare(&workspace.path)
        .map_err(|e| eprintln!("Warning: could not set up agent session files: {e}"))
        .ok();

    // Agent respects backend isolation (BACK-09)
    let backend = BackendType::from_kind(backend_kind);
    let result = backend.agent(&workspace.path, agent_bin.as_deref(), &all_flags);

    // Revert agent workspace files after the agent exits
    if let Some(ref t) = token {
        agent_impl.cleanup(&workspace.path, t);
    }

    result
}
