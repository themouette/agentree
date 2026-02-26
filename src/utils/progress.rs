use crate::error::Result;
use crate::worktree::config::WorktreeConfig;
use crate::worktree::operations::{
    detect_branch_status, ensure_workspace, BranchStatus, CreateResult,
};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use std::time::Duration;

/// Run a closure while displaying an animated spinner on stderr.
///
/// The spinner is only shown when stderr is a TTY. In non-interactive
/// environments (CI, scripts, piped output) the spinner is hidden so
/// existing tooling is unaffected.
///
/// On completion the spinner is cleared regardless of outcome; errors
/// propagate normally to the caller.
///
/// # Example
///
/// ```ignore
/// let result = with_spinner("Creating branch 'feat' and worktree...", || {
///     create_worktree(&config, &root, "feat", None)
/// })?;
/// ```
pub fn with_spinner<F, T, E>(message: &str, f: F) -> std::result::Result<T, E>
where
    F: FnOnce() -> std::result::Result<T, E>,
{
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .expect("valid template"),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));

    let result = f();

    pb.finish_and_clear();
    result
}

/// Ensure a workspace exists, showing an intent spinner before the git operation.
///
/// Detects whether the branch needs to be created or just checked out, shows an
/// appropriate spinner message, then delegates to [`ensure_workspace`].
///
/// - Branch does not exist → `⠙ Creating branch '<name>' and worktree...`
/// - Branch exists, no worktree → `⠙ Creating worktree for '<name>'...`
/// - Worktree already exists → no spinner (instant resume)
///
/// The spinner is suppressed automatically in non-TTY environments.
pub fn ensure_workspace_with_progress(
    config: &WorktreeConfig,
    repo_root: &Path,
    branch: &str,
    base: Option<&str>,
) -> Result<CreateResult> {
    let status = detect_branch_status(branch)?;

    match status {
        BranchStatus::InWorktree(_) => ensure_workspace(config, repo_root, branch, base),
        BranchStatus::ExistsNotCheckedOut => {
            if let Some(ref_name) = base {
                eprintln!(
                    "Warning: '-b {}' ignored — branch '{}' already exists.",
                    ref_name, branch
                );
            }
            let msg = format!("Creating worktree for '{}'...", branch);
            with_spinner(&msg, || ensure_workspace(config, repo_root, branch, base))
        }
        BranchStatus::ExistsOnRemote(ref remote_ref) => {
            if let Some(ref_name) = base {
                eprintln!(
                    "Warning: '-b {}' ignored — branch '{}' already exists on remote ('{}').",
                    ref_name, branch, remote_ref
                );
            }
            let msg = format!(
                "Checking out '{}' from '{}' and creating worktree...",
                branch, remote_ref
            );
            with_spinner(&msg, || ensure_workspace(config, repo_root, branch, base))
        }
        BranchStatus::DoesNotExist => {
            let msg = format!("Creating branch '{}' and worktree...", branch);
            with_spinner(&msg, || ensure_workspace(config, repo_root, branch, base))
        }
    }
}
