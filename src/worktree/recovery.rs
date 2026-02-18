use crate::error::Result;
use crate::utils::git::run_git_best_effort;
use crate::worktree::state::{list_worktrees, WorktreeEntry};

/// Check for stale worktree metadata and warn the user if any is found.
/// Suggests running `agentree doctor --fix` to resolve issues.
/// Best-effort operation - logs warnings on failure but doesn't error
pub fn check_stale_metadata() -> Result<()> {
    // Do a dry-run to see what would be pruned
    let to_prune = match run_git_best_effort(&["worktree", "prune", "--dry-run", "--verbose"]) {
        Ok(output) => String::from_utf8_lossy(&output.stderr).to_string(),
        Err(e) => {
            eprintln!("Warning: failed to check for orphaned worktrees: {}", e);
            return Ok(());
        }
    };

    // If there's something to prune, suggest running doctor instead of prompting
    if !to_prune.trim().is_empty() {
        eprintln!("Warning: stale worktree metadata detected.");
        eprintln!("Run `agentree doctor --fix` to diagnose and clean up.");
        eprintln!();
    }

    // Always return Ok - prune check is informational, not critical
    Ok(())
}

/// Prune stale worktree metadata unconditionally
/// Best-effort operation - logs warnings on failure but doesn't error
pub fn prune() -> Result<()> {
    match run_git_best_effort(&["worktree", "prune"]) {
        Ok(output) if !output.status.success() => {
            eprintln!(
                "Warning: git worktree prune failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Err(e) => {
            eprintln!("Warning: failed to run git worktree prune: {}", e);
        }
        _ => {}
    }

    // Always return Ok - prune is cleanup, not critical
    Ok(())
}

/// Attempt to repair worktree metadata links
/// Best-effort operation - logs warnings on failure but doesn't error
pub fn try_repair() -> Result<()> {
    match run_git_best_effort(&["worktree", "repair"]) {
        Ok(output) if !output.status.success() => {
            // Log warning but don't fail - repair is best-effort
            eprintln!(
                "Warning: git worktree repair failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Err(e) => {
            eprintln!("Warning: failed to run git worktree repair: {}", e);
        }
        _ => {
            // Success or no error - continue
        }
    }

    // Always return Ok - repair is recovery, not critical
    Ok(())
}

/// Ensure clean state by checking for stale metadata and querying worktrees
/// This is the main entry point Phase 2 will call before operations
pub fn ensure_clean_state() -> Result<Vec<WorktreeEntry>> {
    check_stale_metadata()?;
    list_worktrees()
}

/// Return only linked worktrees (excludes the main repo and detached HEADs).
///
/// `git worktree list` always puts the main repository first. This helper
/// skips that first entry and keeps only entries that have a branch name,
/// so callers never accidentally operate on the main repo.
pub fn list_linked_worktrees() -> Result<Vec<WorktreeEntry>> {
    let all = ensure_clean_state()?;
    Ok(all
        .into_iter()
        .skip(1)
        .filter(|e| e.branch.is_some())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    #[test]
    fn test_check_stale_metadata_does_not_error() {
        // Create a temporary git repo for testing
        let dir = TempDir::new().unwrap();
        let repo_path = dir.path();

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Create initial commit
        fs::write(repo_path.join("test.txt"), "test").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Change to repo directory for git commands
        std::env::set_current_dir(repo_path).unwrap();

        // Should not error even if nothing is stale
        let result = check_stale_metadata();
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_linked_worktrees_empty() {
        let dir = TempDir::new().unwrap();
        let repo_path = dir.path();

        Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        fs::write(repo_path.join("test.txt"), "test").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        std::env::set_current_dir(repo_path).unwrap();

        // No linked worktrees — result must be empty (main repo excluded)
        let result = list_linked_worktrees().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_list_linked_worktrees_excludes_main_repo() {
        let dir = TempDir::new().unwrap();
        let repo_path = dir.path();
        let wt_path = dir.path().join("wt-feature");

        Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        fs::write(repo_path.join("test.txt"), "test").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Add a linked worktree on a new branch
        Command::new("git")
            .args([
                "worktree",
                "add",
                "-b",
                "feature",
                wt_path.to_str().unwrap(),
            ])
            .current_dir(repo_path)
            .output()
            .unwrap();

        std::env::set_current_dir(repo_path).unwrap();

        let result = list_linked_worktrees().unwrap();

        // Only the linked worktree — main repo must not appear
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].branch.as_deref(), Some("feature"));
        assert_ne!(result[0].path, repo_path);
    }

    #[test]
    fn test_try_repair_does_not_error() {
        // Create a temporary git repo for testing
        let dir = TempDir::new().unwrap();
        let repo_path = dir.path();

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Create initial commit
        fs::write(repo_path.join("test.txt"), "test").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Change to repo directory for git commands
        std::env::set_current_dir(repo_path).unwrap();

        // Run try_repair - should not error
        let result = try_repair();
        assert!(result.is_ok());
    }
}
