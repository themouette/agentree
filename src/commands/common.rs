//! Common types and utilities shared across command implementations

use clap::Args;

/// Common workspace arguments shared by all workspace-related commands
///
/// This struct contains the standard set of arguments that control workspace
/// creation and configuration. It can be flattened into command-specific
/// argument structs using `#[command(flatten)]`.
#[derive(Args, Debug, Clone)]
pub struct WorkspaceArgs {
    /// Branch name for the workspace
    pub branch: String,

    /// Git ref to create branch from (if workspace doesn't exist)
    #[arg(short = 'b', long)]
    pub base: Option<String>,

    /// Backend to use for this worktree (overrides config)
    #[arg(long)]
    pub backend: Option<String>,

    /// Worktree location (overrides config)
    #[arg(long = "worktree-location")]
    pub worktree_location: Option<String>,
}
