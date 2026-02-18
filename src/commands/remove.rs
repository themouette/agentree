use crate::backend::{BackendKind, DockerSandboxBackend};
use crate::commands::filters::{glob_match, resolve_head_sentinel, WorktreeFilterArgs};
use crate::config;
use crate::error::Result;
use crate::utils::git::get_git_root;
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
        "only_dirty",
        "branch_pattern"
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
    let config = config::load(&repo_root)?;

    // Filter mode: no explicit branches given — select candidates via filter flags
    if args.branches.is_empty() {
        return remove_by_filters(&args.filters, args.force, args.unlock, &config);
    }

    // Get worktrees to find paths before deletion
    let worktrees = recovery::ensure_clean_state()?;

    for branch in &args.branches {
        // Find the workspace path before deleting
        let worktree_entry = worktrees
            .iter()
            .find(|e| e.branch.as_deref() == Some(branch.as_str()));

        let workspace_path = worktree_entry.map(|e| e.path.clone());

        let force_level = ForceLevel::from_count(args.force);
        let msg = format!("Removing worktree for '{}'...", branch);
        with_spinner(&msg, || {
            operations::delete_worktree(branch, force_level, args.unlock)
        })?;
        println!("Removed worktree for branch '{}'", branch);

        // Cleanup backend resources if we found the workspace path
        if let Some(path) = workspace_path {
            cleanup_backend_resources(&path, config.effective_backend(), &config);
        }
    }

    Ok(())
}

/// Apply filter flags to a list of worktree entries, retaining only those that match.
///
/// Filters are applied cheapest-first (in-memory checks before additional git calls).
/// The dirty filter is handled separately in `remove_by_filters` because it requires
/// per-worktree git calls.
fn apply_entry_filters(
    entries: &mut Vec<crate::worktree::state::WorktreeEntry>,
    filters: &WorktreeFilterArgs,
) -> Result<()> {
    // --merged: keep only branches merged into BASE
    if let Some(ref base) = filters.merged {
        let base = resolve_head_sentinel(base)?;
        let merged_branches = operations::list_merged_branches(&base)?;
        entries.retain(|e| {
            e.branch
                .as_deref()
                .map(|b| merged_branches.contains(&b.to_string()))
                .unwrap_or(false)
        });
    }
    // --not-merged: keep only branches NOT merged into BASE
    if let Some(ref base) = filters.not_merged {
        let base = resolve_head_sentinel(base)?;
        let merged_branches = operations::list_merged_branches(&base)?;
        entries.retain(|e| {
            e.branch
                .as_deref()
                .map(|b| !merged_branches.contains(&b.to_string()))
                .unwrap_or(false)
        });
    }
    // --locked: keep only locked worktrees
    if filters.only_locked {
        entries.retain(|e| e.locked.is_some());
    }
    // --branch: keep only branches matching PATTERN
    if let Some(ref pattern) = filters.branch_pattern {
        entries.retain(|e| {
            e.branch
                .as_deref()
                .map(|b| glob_match(pattern, b))
                .unwrap_or(false)
        });
    }
    Ok(())
}

/// Returns `true` when the worktree at `path` has uncommitted changes.
/// Best-effort: returns `false` on error (missing path, git failure, etc.).
fn is_worktree_dirty(path: &std::path::Path) -> bool {
    use crate::utils::git::{path_to_str, run_git_query};
    path_to_str(path, "worktree path")
        .ok()
        .and_then(|p| {
            run_git_query(&["-C", p, "status", "--short"])
                .ok()
                .flatten()
        })
        .map(|output| !output.is_empty())
        .unwrap_or(false)
}

/// Select worktrees via filter flags and remove them.
fn remove_by_filters(
    filters: &WorktreeFilterArgs,
    force: u8,
    unlock: bool,
    config: &config::Config,
) -> Result<()> {
    let mut candidates = recovery::list_linked_worktrees()?;

    apply_entry_filters(&mut candidates, filters)?;

    // --dirty filter: keep only worktrees with uncommitted changes.
    // Auto-imply IgnoreDirty force level so git allows the removal.
    if filters.only_dirty {
        candidates.retain(|e| is_worktree_dirty(&e.path));
    }

    if candidates.is_empty() {
        let msg = if let Some(ref base) = filters.merged {
            format!("No merged worktrees found for '{}'.", base)
        } else if let Some(ref base) = filters.not_merged {
            format!("No unmerged worktrees found for '{}'.", base)
        } else {
            "No worktrees match the specified filters.".to_string()
        };
        println!("{}", msg);
        return Ok(());
    }

    // --dirty filter implies at least IgnoreDirty so git can remove the worktree
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
        let workspace_path = entry.path.clone();
        let msg = format!("Removing worktree for '{}'...", branch);

        match with_spinner(&msg, || {
            operations::delete_worktree(&branch, force_level, effective_unlock)
        }) {
            Ok(_) => {
                removed += 1;
                println!("Removed worktree for branch '{}'", branch);
                cleanup_backend_resources(&workspace_path, config.effective_backend(), config);
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
