use crate::backend::{BackendKind, DockerSandboxBackend};
use crate::config;
use crate::error::Result;
use crate::utils::git::get_git_root;
use crate::worktree::{operations, recovery, validation};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct RemoveArgs {
    /// Branch names to remove (can specify multiple)
    #[arg(required_unless_present = "merged")]
    pub branches: Vec<String>,

    /// Remove all branches merged into BASE
    #[arg(long, conflicts_with = "branches")]
    pub merged: Option<String>,

    /// Force removal even with uncommitted changes
    #[arg(short, long)]
    pub force: bool,
}

/// Cleanup backend resources for a workspace if applicable
///
/// This function attempts to cleanup backend-specific resources (like Docker sandboxes)
/// after a worktree is removed. It's called opportunistically and errors are logged
/// but not propagated to avoid failing the worktree removal.
fn cleanup_backend_resources(
    workspace_path: &std::path::Path,
    backend_kind: BackendKind,
    config: &config::Config,
) {
    match backend_kind {
        BackendKind::DockerSandbox => {
            // Create docker-sandbox backend and attempt cleanup
            let backend = if let Some(docker_config) = config.docker_sandbox.binary.as_ref() {
                DockerSandboxBackend::with_binary(docker_config.clone())
                    .with_config(config.docker_sandbox.clone())
            } else {
                DockerSandboxBackend::new().with_config(config.docker_sandbox.clone())
            };

            if let Err(e) = backend.remove_sandbox(workspace_path) {
                // Log but don't fail - sandbox may already be removed or not exist
                eprintln!("Note: Could not cleanup Docker sandbox: {}", e);
            }
        }
        BackendKind::Local | BackendKind::ClaudeVm => {
            // No backend-specific cleanup needed
        }
    }
}

pub fn execute(args: RemoveArgs) -> Result<()> {
    // Check git version
    validation::check_git_version()?;

    // Find repository root
    let repo_root = get_git_root()?.ok_or_else(|| {
        crate::error::AgentreeError::Worktree(
            "Not in a git repository. Run this command from inside a git repository.".to_string(),
        )
    })?;

    // Load config to determine backend and for cleanup
    let config = config::load(&repo_root)?;

    // Handle --merged mode
    if let Some(base) = args.merged {
        // Get all merged branches
        let merged_branches = operations::list_merged_branches(&base)?;

        // Get current worktrees
        let worktrees = recovery::ensure_clean_state()?;

        // Find which merged branches have worktrees
        let mut removed_count = 0;
        for branch in merged_branches {
            // Find the worktree path before deleting
            let worktree_entry = worktrees
                .iter()
                .find(|e| e.branch.as_deref() == Some(&branch));

            if let Some(entry) = worktree_entry {
                let workspace_path = entry.path.clone();
                match operations::delete_worktree(&branch) {
                    Ok(_) => {
                        removed_count += 1;
                        println!("Removed worktree for branch '{}'", branch);

                        // Cleanup backend resources (e.g., Docker sandboxes)
                        cleanup_backend_resources(&workspace_path, config.effective_backend(), &config);
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to remove worktree for '{}': {}", branch, e);
                    }
                }
            }
        }

        println!("Removed {} merged worktrees.", removed_count);
        return Ok(());
    }

    // Handle individual branch removal
    if args.branches.is_empty() {
        return Err(crate::error::AgentreeError::Worktree(
            "Specify branches to remove or use --merged <base>".to_string(),
        ));
    }

    // Get worktrees to find paths before deletion
    let worktrees = recovery::ensure_clean_state()?;

    for branch in &args.branches {
        // Find the workspace path before deleting
        let worktree_entry = worktrees
            .iter()
            .find(|e| e.branch.as_deref() == Some(branch.as_str()));

        let workspace_path = worktree_entry.map(|e| e.path.clone());

        operations::delete_worktree(branch)?;
        println!("Removed worktree for branch '{}'", branch);

        // Cleanup backend resources if we found the workspace path
        if let Some(path) = workspace_path {
            cleanup_backend_resources(&path, config.effective_backend(), &config);
        }
    }

    Ok(())
}
