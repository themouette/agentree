//! Common types and utilities shared across command implementations

use crate::config::Config;
use crate::error::Result;
use clap::Args;
use std::path::PathBuf;

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

/// Workspace context initialized from CLI arguments and environment
///
/// This struct encapsulates the common initialization logic shared across
/// all workspace commands. It handles:
/// - Git version validation
/// - Repository root discovery
/// - Submodule warnings
/// - Config loading with CLI overrides
pub struct WorkspaceContext {
    /// Path to the git repository root
    pub repo_root: PathBuf,

    /// Loaded configuration with CLI overrides applied
    pub config: Config,
}

impl WorkspaceContext {
    /// Initialize workspace context from CLI arguments
    ///
    /// This method performs all common initialization steps that every
    /// workspace command needs:
    /// 1. Validates git version meets minimum requirements
    /// 2. Discovers the git repository root directory
    /// 3. Checks for and warns about git submodules
    /// 4. Loads configuration from files and applies CLI overrides
    ///
    /// # Arguments
    ///
    /// * `backend` - Optional backend override from CLI
    /// * `worktree_location` - Optional worktree location override from CLI
    /// * `agent` - Optional agent override from CLI
    ///
    /// # Returns
    ///
    /// Returns `Ok(WorkspaceContext)` with initialized context, or an error if:
    /// - Git version is too old
    /// - Not in a git repository
    /// - Config files are invalid
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use agentree::commands::common::WorkspaceContext;
    /// # fn main() -> agentree::error::Result<()> {
    /// let ctx = WorkspaceContext::init(
    ///     Some("local"),
    ///     None,
    ///     None,
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn init(
        backend: Option<&str>,
        worktree_location: Option<&str>,
        agent: Option<&str>,
    ) -> Result<Self> {
        use crate::utils::git::get_git_root;
        use crate::worktree::validation;

        // Check git version
        validation::check_git_version()?;

        // Find repository root
        let repo_root = get_git_root()?.ok_or_else(|| {
            crate::error::AgentreeError::Worktree(
                "Not in a git repository. Run this command from inside a git repository."
                    .to_string(),
            )
        })?;

        // Check for submodules and warn
        validation::check_submodules_and_warn(&repo_root);

        // Load config from files and apply CLI overrides
        let config = crate::config::load(&repo_root)?.with_cli_overrides(
            backend,
            worktree_location,
            agent,
        )?;

        Ok(Self { repo_root, config })
    }
}
