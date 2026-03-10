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

/// Result of workspace setup
///
/// Contains information about the workspace that was created or resumed.
pub struct WorkspaceSetup {
    /// Path to the workspace directory
    pub path: PathBuf,

    /// Whether the workspace was newly created (true) or already existed (false)
    pub was_created: bool,
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
    /// * `editor` - Optional editor binary override from CLI
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
    ///     None,
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn init(
        backend: Option<&str>,
        worktree_location: Option<&str>,
        agent: Option<&str>,
        editor: Option<&str>,
    ) -> Result<Self> {
        use crate::backend::BackendRegistry;
        use crate::utils::git::{get_current_checkout_root, get_git_root};
        use crate::worktree::validation;

        // Check git version
        validation::check_git_version()?;

        // Find repository root (always the main repo, even when in a worktree)
        let repo_root = get_git_root()?.ok_or_else(|| {
            crate::error::AgentreeError::Worktree(
                "Not in a git repository. Run this command from inside a git repository."
                    .to_string(),
            )
        })?;

        // Find the current checkout root (may differ from repo_root when in a worktree)
        let checkout_root = get_current_checkout_root()?;

        // Check for submodules and warn
        validation::check_submodules_and_warn(&repo_root);

        // Load config from files and apply CLI overrides
        let config = crate::config::load(&repo_root, checkout_root.as_deref())?
            .with_cli_overrides(backend, worktree_location, agent, editor)?;

        // Validate the effective backend before proceeding
        let backend_kind = config.effective_backend();
        let registry = BackendRegistry::new();
        registry.validate(&backend_kind)?;

        Ok(Self { repo_root, config })
    }

    /// Ensure workspace exists and set up metadata
    ///
    /// This method performs the common workspace setup operations:
    /// 1. Creates or resumes the workspace (idempotent)
    /// 2. Saves metadata for newly created workspaces
    /// 3. Prints creation message for new workspaces
    /// 4. Validates workspace path accessibility for the backend
    ///
    /// # Arguments
    ///
    /// * `branch` - Name of the branch for the workspace
    /// * `base` - Optional git ref to create branch from
    ///
    /// # Returns
    ///
    /// Returns `Ok(WorkspaceSetup)` with workspace path and creation status.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use agentree::commands::common::WorkspaceContext;
    /// # fn main() -> agentree::error::Result<()> {
    /// # let ctx = WorkspaceContext::init(None, None, None, None)?;
    /// let workspace = ctx.ensure_workspace("feature-branch", None)?;
    /// println!("Workspace at: {}", workspace.path.display());
    /// # Ok(())
    /// # }
    /// ```
    pub fn ensure_workspace(&self, branch: &str, base: Option<&str>) -> Result<WorkspaceSetup> {
        use crate::utils::git::validate_workspace_path;
        use crate::utils::progress::ensure_workspace_with_progress;
        use crate::worktree::metadata::WorktreeMetadata;

        let result =
            ensure_workspace_with_progress(&self.config.worktree, &self.repo_root, branch, base)?;

        let was_created = result.was_created();

        if was_created {
            let metadata = WorktreeMetadata::new(self.config.effective_backend().to_string());
            metadata.save(result.path())?;
            eprintln!("{}", result.message(branch));
        }

        validate_workspace_path(result.path(), &self.config.effective_backend())?;

        Ok(WorkspaceSetup {
            path: result.path().to_path_buf(),
            was_created,
        })
    }

    /// Determine the effective backend for a workspace.
    ///
    /// Priority (highest to lowest):
    /// 1. CLI `--backend` flag (`cli_backend_given = true`) → use config (which has the override)
    /// 2. Workspace metadata stored at creation time (only when resuming an existing workspace)
    /// 3. Config default (`effective_backend()`)
    ///
    /// Reading metadata ensures that a workspace created with `--backend local` continues to
    /// use the local backend even when the global/project config defaults to a different backend.
    pub fn effective_backend_for(
        &self,
        workspace: &WorkspaceSetup,
        cli_backend_given: bool,
    ) -> crate::backend::BackendKind {
        use crate::worktree::metadata::WorktreeMetadata;

        if !cli_backend_given && !workspace.was_created {
            WorktreeMetadata::load(&workspace.path)
                .ok()
                .flatten()
                .and_then(|m| m.backend.parse().ok())
                .unwrap_or_else(|| self.config.effective_backend())
        } else {
            self.config.effective_backend()
        }
    }
}
