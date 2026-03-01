use crate::backend::{BackendKind, DockerSandboxBackend};
use crate::commands::filters::{check_worktree_dirty, resolve_head_sentinel, WorktreeFilterArgs};
use crate::config;
use crate::error::{AgentreeError, Result};
use crate::utils::git::{get_current_checkout_root, get_git_root};
use crate::utils::progress::with_spinner;
use crate::worktree::operations::ForceLevel;
use crate::worktree::{operations, recovery, validation};
use clap::Parser;

#[derive(Parser, Debug)]
pub struct RemoveArgs {
    /// Branch names to remove (can specify multiple).
    /// Mutually exclusive with filter flags (--merged, --not-merged, etc.).
    #[arg(required_unless_present_any = [
        "merged",
        "not_merged",
        "only_locked",
        "not_locked",
        "only_dirty",
        "only_clean",
        "branch_pattern",
        "stale_days"
    ])]
    pub branches: Vec<String>,

    /// Force removal even with uncommitted changes or locked status
    ///
    /// Use -f to remove worktrees with uncommitted changes.
    /// Use -ff to forcibly remove locked worktrees.
    ///
    /// Locked worktrees typically occur from interrupted operations.
    /// Prefer --unlock over -ff when possible, as -ff bypasses safety checks.
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub force: u8,

    /// Unlock the worktree before attempting normal removal
    ///
    /// Use this for stuck/locked worktrees from interrupted operations.
    /// This attempts to unlock, then removes with normal safety checks.
    /// Safer than -ff, which forces removal and bypasses all checks.
    #[arg(long)]
    pub unlock: bool,

    /// Show what would be removed without actually removing anything
    #[arg(long)]
    pub dry_run: bool,

    #[command(flatten)]
    pub filters: WorktreeFilterArgs,
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
            let mut backend = DockerSandboxBackend::new();

            if let Some(docker_binary) = config.docker_sandbox.binary.as_ref() {
                backend = backend.with_binary(docker_binary.clone());
            }

            let backend = backend.with_config(config.docker_sandbox.clone());

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
    let checkout_root = get_current_checkout_root()?;
    let config = config::load(&repo_root, checkout_root.as_deref())?;

    // Filter mode: no explicit branches given — select candidates via filter flags
    if args.branches.is_empty() {
        return remove_by_filters(
            &args.filters,
            args.force,
            args.unlock,
            args.dry_run,
            &config,
        );
    }

    // Dry-run: show what would be removed without touching anything
    if args.dry_run {
        let worktrees = recovery::ensure_clean_state()?;
        for branch in &args.branches {
            let entry = worktrees
                .iter()
                .find(|e| e.branch.as_deref() == Some(branch.as_str()));
            match entry {
                Some(e) => println!("Would remove: {} ({})", branch, e.path.display()),
                None => eprintln!("Warning: No worktree found for '{}'", branch),
            }
        }
        return Ok(());
    }

    let force_level = ForceLevel::from_count(args.force);
    let mut removed = 0;
    let mut failed_branches: Vec<String> = Vec::new();

    for branch in &args.branches {
        let msg = format!("Removing worktree for '{}'...", branch);
        match with_spinner(&msg, || {
            operations::delete_worktree(branch, force_level, args.unlock)
        }) {
            Ok(removed_path) => {
                removed += 1;
                println!("Removed worktree for branch '{}'", branch);
                cleanup_backend_resources(&removed_path, config.effective_backend(), &config);
            }
            Err(e) => {
                eprintln!("Error: Failed to remove '{}': {}", branch, e);
                failed_branches.push(branch.clone());
            }
        }
    }

    // Fail only when every branch failed (at least one success means partial success)
    if removed == 0 && !failed_branches.is_empty() {
        return Err(AgentreeError::Worktree(
            "No worktrees were removed.".to_string(),
        ));
    }

    Ok(())
}

/// Select worktrees via filter flags and remove them.
fn remove_by_filters(
    filters: &WorktreeFilterArgs,
    force: u8,
    unlock: bool,
    dry_run: bool,
    config: &config::Config,
) -> Result<()> {
    let mut candidates = recovery::list_linked_worktrees()?;

    filters.apply(&mut candidates)?;

    // --dirty / --clean filter: retain by uncommitted-changes status.
    if filters.only_dirty || filters.only_clean {
        candidates.retain(|e| {
            let dirty = check_worktree_dirty(&e.path).unwrap_or(false);
            if filters.only_dirty {
                dirty
            } else {
                !dirty
            }
        });
    }

    if candidates.is_empty() {
        let msg = if let Some(ref base) = filters.merged {
            let resolved = resolve_head_sentinel(base).unwrap_or_else(|_| base.clone());
            format!("No merged worktrees found for '{}'.", resolved)
        } else if let Some(ref base) = filters.not_merged {
            let resolved = resolve_head_sentinel(base).unwrap_or_else(|_| base.clone());
            format!("No unmerged worktrees found for '{}'.", resolved)
        } else {
            "No worktrees match the specified filters.".to_string()
        };
        println!("{}", msg);
        return Ok(());
    }

    // Dry-run: show what would be removed without touching anything
    if dry_run {
        let noun = if candidates.len() == 1 {
            "worktree"
        } else {
            "worktrees"
        };
        println!("Would remove {} {}:", candidates.len(), noun);
        for entry in &candidates {
            if let Some(branch) = &entry.branch {
                println!("  {} ({})", branch, entry.path.display());
            }
        }
        return Ok(());
    }

    // --dirty filter implies at least IgnoreDirty so git can remove the worktree.
    // --clean worktrees have no uncommitted changes so no force escalation needed.
    let force_level = if filters.only_dirty && ForceLevel::from_count(force) == ForceLevel::None {
        ForceLevel::IgnoreDirty
    } else {
        ForceLevel::from_count(force)
    };
    // --locked filter implies automatic unlock so git can remove the worktree
    let effective_unlock = unlock || filters.only_locked;
    let mut removed = 0;

    for entry in candidates {
        let branch = match entry.branch.as_deref() {
            Some(b) => b.to_string(),
            None => continue,
        };
        let msg = format!("Removing worktree for '{}'...", branch);

        match with_spinner(&msg, || {
            operations::delete_worktree(&branch, force_level, effective_unlock)
        }) {
            Ok(removed_path) => {
                removed += 1;
                println!("Removed worktree for branch '{}'", branch);
                cleanup_backend_resources(&removed_path, config.effective_backend(), config);
            }
            Err(e) => {
                eprintln!("Warning: Failed to remove worktree for '{}': {}", branch, e);
            }
        }
    }

    let noun = if removed == 1 {
        "worktree"
    } else {
        "worktrees"
    };
    println!("Removed {} {}.", removed, noun);
    Ok(())
}
