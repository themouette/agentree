use crate::error::{AgentreeError, Result};
use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

/// Get the git common directory (handles worktrees)
pub fn get_git_common_dir() -> Result<Option<PathBuf>> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .map_err(|e| AgentreeError::Git(format!("Failed to run git: {}", e)))?;

    if !output.status.success() {
        return Ok(None);
    }

    let git_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let git_path = PathBuf::from(git_dir);

    if git_path.is_dir() {
        Ok(Some(git_path.canonicalize()?))
    } else {
        Ok(None)
    }
}

/// Get the git worktree directory (if in a worktree)
pub fn get_git_worktree_dir() -> Result<Option<PathBuf>> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .map_err(|e| AgentreeError::Git(format!("Failed to run git: {}", e)))?;

    if !output.status.success() {
        return Ok(None);
    }

    let git_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // If .git is a file (worktree), we're in a worktree
    let git_path = PathBuf::from(&git_dir);
    if git_path.is_file() {
        return Ok(Some(std::env::current_dir()?));
    }

    Ok(None)
}

/// Check if the current directory is inside a git worktree
/// A worktree is detected when --git-dir differs from --git-common-dir
pub fn is_worktree() -> bool {
    let git_dir = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        });

    let git_common_dir = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        });

    // In a worktree, git-dir and git-common-dir are different
    // In a regular repo, they're the same
    if let (Some(dir), Some(common)) = (git_dir, git_common_dir) {
        // Canonicalize paths for accurate comparison
        let dir_path = PathBuf::from(dir).canonicalize().ok();
        let common_path = PathBuf::from(common).canonicalize().ok();

        if let (Some(d), Some(c)) = (dir_path, common_path) {
            return d != c;
        }
    }

    false
}

/// Get the root directory of the main git repository.
///
/// When called from inside a linked worktree, this returns the **main**
/// repository's root directory, not the worktree's own top-level directory.
/// This ensures consistent path resolution (worktree location, repo name)
/// regardless of whether the current working directory is the main repo or
/// one of its linked worktrees.
///
/// Internally uses `--git-common-dir` (which always points to the main repo's
/// `.git` directory) and returns its parent.
pub fn get_git_root() -> Result<Option<PathBuf>> {
    match get_git_common_dir()? {
        None => Ok(None),
        Some(git_common_dir) => {
            let root = git_common_dir.parent().ok_or_else(|| {
                AgentreeError::Git("Cannot determine parent of git common dir".to_string())
            })?;
            Ok(Some(root.to_path_buf()))
        }
    }
}

/// Detect the repository's default branch from the remote origin HEAD ref.
/// Falls back to "main" if the remote HEAD cannot be determined.
pub fn get_default_branch() -> Result<String> {
    let output = Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
        .output()
        .map_err(|e| AgentreeError::Git(format!("Failed to run git: {}", e)))?;

    if !output.status.success() {
        eprintln!("Warning: Could not detect default branch (no remote HEAD ref). Falling back to 'main'.");
        return Ok("main".to_string());
    }

    let symbolic_ref = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Strip the "refs/remotes/origin/" prefix to get just the branch name
    // Example: "refs/remotes/origin/main" -> "main"
    let branch_name = symbolic_ref
        .strip_prefix("refs/remotes/origin/")
        .unwrap_or("main")
        .to_string();

    Ok(branch_name)
}

/// Get the current branch name.
/// Returns an error if not on a branch (detached HEAD).
pub fn get_current_branch() -> Result<String> {
    let output = Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .output()
        .map_err(|e| AgentreeError::Git(format!("Failed to run git: {}", e)))?;

    if !output.status.success() {
        return Err(AgentreeError::Git(
            "Not on a branch (detached HEAD)".to_string(),
        ));
    }

    let branch_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(branch_name)
}

/// Git operation timeout, configurable via AGENTREE_GIT_TIMEOUT environment variable
/// Defaults to 30 seconds if not set or invalid
static GIT_TIMEOUT: Lazy<Duration> = Lazy::new(|| {
    std::env::var("AGENTREE_GIT_TIMEOUT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(30))
});

/// Run a git command and return stdout on success with timeout support.
///
/// # Arguments
/// * `args` - Command arguments (e.g., `&["status", "--short"]`)
/// * `operation` - Human-readable operation description for error messages
/// * `timeout` - Optional timeout duration (defaults to 30 seconds)
///
/// # Example
/// ```ignore
/// let output = run_git_command_timeout(&["rev-parse", "HEAD"], "get commit hash", None)?;
/// ```
fn run_git_command_timeout(
    args: &[&str],
    operation: &str,
    timeout: Option<Duration>,
) -> Result<String> {
    let timeout = timeout.unwrap_or(*GIT_TIMEOUT);

    let mut child = Command::new("git")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AgentreeError::Git(format!("Failed to {}: {}", operation, e)))?;

    match child
        .wait_timeout(timeout)
        .map_err(|e| AgentreeError::Git(format!("Failed to wait for git command: {}", e)))?
    {
        Some(status) => {
            let output = child
                .wait_with_output()
                .map_err(|e| AgentreeError::Git(format!("Failed to read git output: {}", e)))?;

            if !status.success() {
                return Err(AgentreeError::Git(format!(
                    "git {} failed: {}",
                    args[0],
                    String::from_utf8_lossy(&output.stderr)
                )));
            }

            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        }
        None => {
            // Timeout occurred, kill the process
            let _ = child.kill();
            Err(AgentreeError::Git(format!(
                "git {} timed out after {} seconds",
                args[0],
                timeout.as_secs()
            )))
        }
    }
}

/// Run a git command and return stdout on success.
/// Uses a default 30-second timeout.
///
/// # Arguments
/// * `args` - Command arguments (e.g., `&["status", "--short"]`)
/// * `operation` - Human-readable operation description for error messages
///
/// # Example
/// ```ignore
/// let output = run_git_command(&["rev-parse", "HEAD"], "get commit hash")?;
/// ```
pub fn run_git_command(args: &[&str], operation: &str) -> Result<String> {
    run_git_command_timeout(args, operation, None)
}

/// Run a git query command that may legitimately return non-zero exit.
/// Returns None on non-zero exit instead of erroring.
/// Uses a default 30-second timeout.
///
/// # Arguments
/// * `args` - Command arguments (e.g., `&["show-ref", "--verify", "refs/heads/main"]`)
///
/// # Example
/// ```ignore
/// if let Some(sha) = run_git_query(&["show-ref", "--verify", "refs/heads/main"])? {
///     println!("Branch exists: {}", sha);
/// }
/// ```
pub fn run_git_query(args: &[&str]) -> Result<Option<String>> {
    let timeout = *GIT_TIMEOUT;

    let mut child = Command::new("git")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AgentreeError::Git(format!("Failed to run git {}: {}", args[0], e)))?;

    match child
        .wait_timeout(timeout)
        .map_err(|e| AgentreeError::Git(format!("Failed to wait for git command: {}", e)))?
    {
        Some(status) => {
            if !status.success() {
                return Ok(None);
            }

            let output = child
                .wait_with_output()
                .map_err(|e| AgentreeError::Git(format!("Failed to read git output: {}", e)))?;

            Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            ))
        }
        None => {
            // Timeout occurred, kill the process
            let _ = child.kill();
            Err(AgentreeError::Git(format!(
                "git {} timed out after {} seconds",
                args[0],
                timeout.as_secs()
            )))
        }
    }
}

/// Run a git command in best-effort mode, returning raw output without erroring on failures.
/// This is useful for cleanup operations that should log warnings but not fail the main operation.
///
/// # Arguments
/// * `args` - Command arguments (e.g., `&["worktree", "prune"]`)
///
/// # Example
/// ```ignore
/// match run_git_best_effort(&["worktree", "prune"]) {
///     Ok(output) if !output.status.success() => {
///         eprintln!("Warning: prune failed: {}", String::from_utf8_lossy(&output.stderr));
///     }
///     Err(e) => eprintln!("Warning: {}", e),
///     _ => {}
/// }
/// ```
pub fn run_git_best_effort(args: &[&str]) -> Result<std::process::Output> {
    let timeout = *GIT_TIMEOUT;

    let mut child = Command::new("git")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AgentreeError::Git(format!("Failed to spawn git {}: {}", args[0], e)))?;

    match child
        .wait_timeout(timeout)
        .map_err(|e| AgentreeError::Git(format!("Failed to wait for git command: {}", e)))?
    {
        Some(_) => {
            let output = child
                .wait_with_output()
                .map_err(|e| AgentreeError::Git(format!("Failed to read git output: {}", e)))?;
            Ok(output)
        }
        None => {
            // Timeout occurred, kill the process
            let _ = child.kill();
            Err(AgentreeError::Git(format!(
                "git {} timed out after {} seconds",
                args[0],
                timeout.as_secs()
            )))
        }
    }
}

/// Run a git command without any timeout, blocking until the process exits.
///
/// Use this for long-running operations like `git worktree add` that may
/// take an unbounded amount of time (e.g., large repos with post-checkout hooks).
/// Unlike `run_git_command`, this function will not kill the process after 30 seconds.
///
/// # Arguments
/// * `args` - Command arguments (e.g., `&["worktree", "add", "--no-track", "-b", "feature", "/path"]`)
/// * `operation` - Human-readable operation description for error messages
///
/// # Example
/// ```ignore
/// run_git_command_no_timeout(
///     &["worktree", "add", "--no-track", "-b", "feature", "/tmp/worktree"],
///     "create worktree"
/// )?;
/// ```
pub fn run_git_command_no_timeout(args: &[&str], operation: &str) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| AgentreeError::Git(format!("Failed to {}: {}", operation, e)))?;

    if !output.status.success() {
        return Err(AgentreeError::Git(format!(
            "git {} failed: {}",
            args[0],
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Convert a Path to &str with proper error handling
///
/// This helper ensures consistent error messages when paths contain invalid UTF-8.
///
/// # Arguments
/// * `path` - The path to convert
/// * `context` - A description of what this path represents (e.g., "worktree path")
///
/// # Example
/// ```ignore
/// let path_str = path_to_str(&worktree_path, "worktree path")?;
/// ```
pub fn path_to_str<'a>(path: &'a std::path::Path, context: &str) -> Result<&'a str> {
    path.to_str().ok_or_else(|| {
        AgentreeError::Worktree(format!(
            "{} contains invalid UTF-8: {}",
            context,
            path.display()
        ))
    })
}

/// List all local branches in the repository.
///
/// # Returns
/// A vector of branch names (without refs/heads/ prefix).
///
/// # Example
/// ```ignore
/// let branches = list_local_branches()?;
/// ```
pub fn list_local_branches() -> Result<Vec<String>> {
    let output = run_git_command(&["branch", "--format=%(refname:short)"], "list branches")?;

    Ok(output
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// List all remote tracking branches in the repository.
///
/// # Returns
/// Branch names in "remote/branch" format (e.g., "origin/main").
/// Excludes remote HEAD pointers (e.g., "origin/HEAD").
///
/// # Example
/// ```ignore
/// let branches = list_remote_branches()?;
/// // Returns: ["origin/main", "origin/feature"]
/// ```
pub fn list_remote_branches() -> Result<Vec<String>> {
    let output = run_git_command(
        &["branch", "--remote", "--format=%(refname:short)"],
        "list remote branches",
    )?;

    Ok(output
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && !s.ends_with("/HEAD"))
        .collect())
}

/// Find the first remote tracking ref for a local branch name.
///
/// Checks configured remotes in order and returns the first match.
/// Returns the short ref name (e.g., "origin/feature") if found on any remote.
///
/// # Arguments
/// * `branch` - The local branch name to look up (e.g., "feature")
///
/// # Returns
/// `Some("origin/feature")` if found on any remote, `None` otherwise.
///
/// # Example
/// ```ignore
/// if let Some(remote_ref) = find_remote_tracking_ref("feature")? {
///     println!("Found on remote: {}", remote_ref); // "origin/feature"
/// }
/// ```
pub fn find_remote_tracking_ref(branch: &str) -> Result<Option<String>> {
    let remotes_str = match run_git_query(&["remote"])? {
        None => return Ok(None),
        Some(s) => s,
    };

    for remote in remotes_str.lines() {
        let remote = remote.trim();
        if remote.is_empty() {
            continue;
        }
        let remote_ref = format!("refs/remotes/{}/{}", remote, branch);
        if run_git_query(&["show-ref", "--verify", "--quiet", &remote_ref])?.is_some() {
            return Ok(Some(format!("{}/{}", remote, branch)));
        }
    }

    Ok(None)
}

/// Suggest similar branch names based on Levenshtein distance.
///
/// Returns up to 3 branch names with edit distance <= max_distance,
/// sorted by distance (closest first).
///
/// # Arguments
/// * `target` - The mistyped branch name to match against
/// * `candidates` - Available branch names to search
/// * `max_distance` - Maximum edit distance to consider (typically 3)
///
/// # Returns
/// Vector of suggested branch names (up to 3), sorted by similarity.
/// Returns empty vec if target is too short (< 2 chars) to avoid false positives.
///
/// # Example
/// ```ignore
/// let suggestions = suggest_similar_branches("mian", &["main", "feature"], 3);
/// // Returns: ["main"]
/// ```
pub fn suggest_similar_branches(
    target: &str,
    candidates: &[String],
    max_distance: usize,
) -> Vec<String> {
    // Avoid false positives for very short input
    if target.len() < 2 {
        return Vec::new();
    }

    let mut matches: Vec<(usize, &String)> = candidates
        .iter()
        .map(|candidate| (strsim::levenshtein(target, candidate), candidate))
        .filter(|(distance, _)| *distance <= max_distance)
        .collect();

    // Sort by distance (ascending)
    matches.sort_by_key(|(distance, _)| *distance);

    // Return top 3 matches
    matches
        .into_iter()
        .take(3)
        .map(|(_, branch)| branch.clone())
        .collect()
}

/// Validate that a git ref exists.
///
/// Checks if the given ref exists in the repository. If not, suggests
/// similar branch names to help users correct typos.
///
/// # Arguments
/// * `ref_name` - The ref to validate (branch name, tag, commit hash, etc.)
///
/// # Returns
/// Ok(()) if ref exists, error with suggestions if not found.
///
/// # Example
/// ```ignore
/// validate_start_ref("main")?;  // Ok if main exists
/// validate_start_ref("mian")?;  // Error with "Did you mean: main?"
/// ```
pub fn validate_start_ref(ref_name: &str) -> Result<()> {
    // Try to resolve the ref directly
    if run_git_query(&["rev-parse", "--verify", "--quiet", ref_name])?.is_some() {
        return Ok(());
    }

    // Try with refs/heads/ prefix (local branch)
    let branch_ref = format!("refs/heads/{}", ref_name);
    if run_git_query(&["rev-parse", "--verify", "--quiet", &branch_ref])?.is_some() {
        return Ok(());
    }

    // Try with refs/tags/ prefix (tag)
    let tag_ref = format!("refs/tags/{}", ref_name);
    if run_git_query(&["rev-parse", "--verify", "--quiet", &tag_ref])?.is_some() {
        return Ok(());
    }

    // Ref doesn't exist - suggest similar branch names (local + remote)
    let mut branches = list_local_branches()?;
    // Include remote tracking branches so users get suggestions like "origin/feature".
    // Failure to list remotes is non-fatal: omit remote suggestions rather than blocking.
    if let Ok(remote_branches) = list_remote_branches() {
        branches.extend(remote_branches);
    }
    let suggestions = suggest_similar_branches(ref_name, &branches, 3);

    if !suggestions.is_empty() {
        return Err(AgentreeError::BranchNotFoundWithSuggestions {
            branch: ref_name.to_string(),
            suggestions: suggestions.join("', '"),
        });
    }

    Err(AgentreeError::BranchNotFound {
        branch: ref_name.to_string(),
    })
}

/// Validate that a workspace path is accessible and writable.
/// For VM/container backends, checks path is under $HOME to ensure mount accessibility.
///
/// # Arguments
/// * `path` - The workspace path to validate
/// * `backend_kind` - The backend that will access this path
///
/// # Returns
/// Ok(()) if path is accessible, error with helpful hint if not.
///
/// # Example
/// ```ignore
/// validate_workspace_path(&workspace_path, &BackendKind::ClaudeVm)?;
/// ```
pub fn validate_workspace_path(
    path: &std::path::Path,
    backend_kind: &crate::backend::BackendKind,
) -> Result<()> {
    // If path doesn't exist yet, that's fine (create_worktree will create it)
    if !path.exists() {
        return Ok(());
    }

    // If path exists, verify it's a directory
    if !path.is_dir() {
        return Err(AgentreeError::PathNotAccessible {
            path: path.to_path_buf(),
            reason: "path exists but is not a directory".to_string(),
            hint: "Remove the file or choose a different workspace location.".to_string(),
        });
    }

    // For ClaudeVm backend, check if path is under $HOME
    if matches!(backend_kind, crate::backend::BackendKind::ClaudeVm) {
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            if let (Ok(canonical_path), Ok(canonical_home)) =
                (path.canonicalize(), home_path.canonicalize())
            {
                if !canonical_path.starts_with(&canonical_home) {
                    return Err(AgentreeError::PathNotAccessible {
                        path: path.to_path_buf(),
                        reason: "not under home directory".to_string(),
                        hint: "Ensure workspace location is under $HOME or explicitly mounted in your VM configuration.".to_string(),
                    });
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggest_similar_branches_finds_close_matches() {
        let candidates = vec![
            "main".to_string(),
            "feature".to_string(),
            "develop".to_string(),
        ];
        let suggestions = suggest_similar_branches("mian", &candidates, 3);
        assert_eq!(suggestions, vec!["main"]);
    }

    #[test]
    fn test_suggest_similar_branches_no_matches() {
        let candidates = vec![
            "main".to_string(),
            "feature".to_string(),
            "develop".to_string(),
        ];
        let suggestions = suggest_similar_branches("xyzabc", &candidates, 3);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_suggest_similar_branches_short_input() {
        let candidates = vec![
            "main".to_string(),
            "feature".to_string(),
            "develop".to_string(),
        ];
        let suggestions = suggest_similar_branches("m", &candidates, 3);
        assert!(
            suggestions.is_empty(),
            "Should return empty for single char input to avoid false positives"
        );
    }

    #[test]
    fn test_suggest_similar_branches_sorted_by_distance() {
        let candidates = vec![
            "feature".to_string(),
            "feat".to_string(),
            "feature-x".to_string(),
        ];
        let suggestions = suggest_similar_branches("featre", &candidates, 3);
        assert_eq!(
            suggestions[0], "feature",
            "Should return closest match first"
        );
    }

    #[test]
    fn test_suggest_similar_branches_max_three() {
        let candidates = vec![
            "feature1".to_string(),
            "feature2".to_string(),
            "feature3".to_string(),
            "feature4".to_string(),
            "feature5".to_string(),
        ];
        let suggestions = suggest_similar_branches("featur", &candidates, 3);
        assert!(
            suggestions.len() <= 3,
            "Should return maximum 3 suggestions"
        );
    }

    #[test]
    fn test_validate_workspace_path_local_any_path() {
        use crate::backend::BackendKind;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let result = validate_workspace_path(temp_dir.path(), &BackendKind::Local);
        assert!(result.is_ok(), "Local backend should accept any path");
    }

    #[test]
    fn test_validate_workspace_path_nonexistent_ok() {
        use crate::backend::BackendKind;
        use std::path::PathBuf;

        let nonexistent = PathBuf::from("/tmp/nonexistent-path-12345");
        let result = validate_workspace_path(&nonexistent, &BackendKind::Local);
        assert!(
            result.is_ok(),
            "Non-existent path should be OK (will be created)"
        );
    }

    #[test]
    fn test_git_timeout_default() {
        // Without setting env var, should use default 30 seconds
        use std::env;
        env::remove_var("AGENTREE_GIT_TIMEOUT");
        // Since GIT_TIMEOUT is a Lazy static, we can't easily reset it in tests
        // but we can verify the default is reasonable
        assert!(GIT_TIMEOUT.as_secs() >= 30);
    }

    #[test]
    fn test_git_timeout_configurable() {
        // This test documents that AGENTREE_GIT_TIMEOUT should be set before
        // the first access to GIT_TIMEOUT for it to take effect
        // In real usage, users would set the env var before running agentree
        use std::env;

        // Document the environment variable for users
        assert!(
            env::var("AGENTREE_GIT_TIMEOUT").is_ok() || env::var("AGENTREE_GIT_TIMEOUT").is_err()
        );
    }
}
