use crate::error::{AgentreeError, Result};
use crate::utils::git::{run_git_command, run_git_command_no_timeout, run_git_query};
use crate::worktree::config::WorktreeConfig;
use crate::worktree::recovery::ensure_clean_state;
use crate::worktree::template::{compute_worktree_path, TemplateContext};
use crate::worktree::validation;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Represents the status of a branch in relation to worktrees
#[derive(Debug, PartialEq)]
pub enum BranchStatus {
    /// Branch is already checked out in a worktree at the given path
    InWorktree(PathBuf),
    /// Branch exists as a local ref but is not checked out in any worktree
    ExistsNotCheckedOut,
    /// Branch does not exist locally but exists on a remote (e.g., "origin/feature")
    ExistsOnRemote(String),
    /// Branch does not exist as a ref anywhere
    DoesNotExist,
}

/// Represents the result of creating a worktree
#[derive(Debug, PartialEq)]
pub enum CreateResult {
    /// Branch already had a worktree; returning existing path
    Resumed(PathBuf),
    /// New worktree created for an existing local branch
    Created(PathBuf),
    /// New branch and worktree were both created from scratch
    CreatedWithBranch(PathBuf),
    /// Local branch created from a remote tracking branch and worktree added
    CheckedOutFromRemote(PathBuf),
}

impl CreateResult {
    /// Get the worktree path regardless of outcome
    pub fn path(&self) -> &PathBuf {
        match self {
            CreateResult::Resumed(path)
            | CreateResult::Created(path)
            | CreateResult::CreatedWithBranch(path)
            | CreateResult::CheckedOutFromRemote(path) => path,
        }
    }

    /// Returns true if a new worktree (or branch + worktree) was created
    pub fn was_created(&self) -> bool {
        matches!(
            self,
            CreateResult::Created(_)
                | CreateResult::CreatedWithBranch(_)
                | CreateResult::CheckedOutFromRemote(_)
        )
    }

    /// Generate user-facing message for this result
    pub fn message(&self, branch: &str) -> String {
        match self {
            CreateResult::Resumed(path) => {
                format!(
                    "Resuming worktree for branch '{}' at {}",
                    branch,
                    path.display()
                )
            }
            CreateResult::Created(path) => {
                format!(
                    "Created worktree for branch '{}' at {}",
                    branch,
                    path.display()
                )
            }
            CreateResult::CreatedWithBranch(path) => {
                format!(
                    "Created branch '{}' and worktree at {}",
                    branch,
                    path.display()
                )
            }
            CreateResult::CheckedOutFromRemote(path) => {
                format!(
                    "Checked out '{}' from remote and created worktree at {}",
                    branch,
                    path.display()
                )
            }
        }
    }
}

/// Force level for worktree removal operations
///
/// Controls how aggressively to remove a worktree:
/// - `None`: Normal removal (fails on dirty or locked worktrees)
/// - `IgnoreDirty`: Remove even with uncommitted changes (git worktree remove --force)
/// - `IgnoreLocked`: Remove even if locked (git worktree remove --force --force)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForceLevel {
    /// No forcing - normal removal behavior
    None,
    /// Ignore uncommitted changes (equivalent to -f or one --force flag)
    IgnoreDirty,
    /// Ignore locked status (equivalent to -ff or two --force flags)
    IgnoreLocked,
}

impl ForceLevel {
    /// Get the number of --force flags to pass to git worktree remove
    pub fn flag_count(&self) -> usize {
        match self {
            Self::None => 0,
            Self::IgnoreDirty => 1,
            Self::IgnoreLocked => 2,
        }
    }

    /// Create ForceLevel from a count (e.g., from clap's Count action)
    /// 0 => None, 1 => IgnoreDirty, 2+ => IgnoreLocked
    pub fn from_count(count: u8) -> Self {
        match count {
            0 => Self::None,
            1 => Self::IgnoreDirty,
            _ => Self::IgnoreLocked,
        }
    }
}

/// Ensure `.claude/settings.json` contains `allowedTools` entries for `.agentree/**`.
///
/// Both relative (`Write(.agentree/**)`) and absolute path variants are added so
/// that Claude Code grants permission regardless of which form it checks.
///
/// If the file does not exist it is created. If it already exists, any missing
/// entries are appended to the `allowedTools` array so pre-existing project
/// settings are preserved.
fn ensure_agentree_allowed_tools(settings_path: &Path, worktree_path: &Path) -> Result<()> {
    let abs = worktree_path
        .canonicalize()
        .unwrap_or_else(|_| worktree_path.to_path_buf());
    let abs_agentree = abs.join(".agentree").to_string_lossy().into_owned();

    let required: Vec<String> = vec![
        "Write(.agentree/**)".into(),
        "Edit(.agentree/**)".into(),
        format!("Write({}/**)", abs_agentree),
        format!("Edit({}/**)", abs_agentree),
    ];

    // Parse existing file or start from an empty object
    let mut value: serde_json::Value = if settings_path.exists() {
        let raw = std::fs::read_to_string(settings_path).map_err(AgentreeError::Io)?;
        serde_json::from_str(&raw).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let allowed = value
        .as_object_mut()
        .ok_or_else(|| AgentreeError::ConfigError("settings.json root is not an object".into()))?
        .entry("allowedTools")
        .or_insert(serde_json::json!([]))
        .as_array_mut()
        .ok_or_else(|| AgentreeError::ConfigError("allowedTools is not an array".into()))?;

    for entry in &required {
        if !allowed.iter().any(|v: &serde_json::Value| v.as_str() == Some(entry.as_str())) {
            allowed.push(serde_json::json!(entry));
        }
    }

    let updated = serde_json::to_string_pretty(&value)?;
    std::fs::write(settings_path, updated + "\n").map_err(AgentreeError::Io)?;
    Ok(())
}

/// Set up the .agentree/ workspace directory in a newly created worktree.
///
/// Creates the `.agentree/` directory, writes `CLAUDE.md` to the worktree root
/// (only if none already exists), and attempts to exclude `.agentree/` from
/// git tracking via the shared `info/exclude` file.
fn setup_agentree_workspace(worktree_path: &Path) -> Result<()> {
    // 1. Create .agentree/ directory
    std::fs::create_dir_all(worktree_path.join(".agentree")).map_err(AgentreeError::Io)?;

    // 2. Write CLAUDE.md only if no CLAUDE.md exists in worktree root
    let claude_md_path = worktree_path.join("CLAUDE.md");
    if !claude_md_path.exists() {
        std::fs::write(&claude_md_path, include_str!("../../templates/CLAUDE.md"))
            .map_err(AgentreeError::Io)?;
    }

    // 3. Ensure .claude/settings.json grants auto-approval for .agentree/ writes.
    //    If the file already exists, merge the entries rather than overwrite.
    let claude_dir = worktree_path.join(".claude");
    std::fs::create_dir_all(&claude_dir).map_err(AgentreeError::Io)?;
    let settings_path = claude_dir.join("settings.json");
    ensure_agentree_allowed_tools(&settings_path, worktree_path)?;

    // 4. Add .agentree/ to git exclude (non-critical, log warning on failure)
    if let Err(e) = add_agentree_to_git_exclude(worktree_path) {
        eprintln!("Warning: could not add .agentree/ to git exclude: {}", e);
    }

    Ok(())
}

/// Resolve the git common directory for a worktree using `git rev-parse`.
///
/// Runs `git -C worktree_path rev-parse --git-common-dir` and canonicalizes
/// the result (which may be relative to `worktree_path`).
fn resolve_git_common_dir(worktree_path: &Path) -> Result<PathBuf> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(worktree_path)
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .map_err(|e| AgentreeError::Git(format!("Failed to run git: {}", e)))?;

    if !output.status.success() {
        return Err(AgentreeError::Git(
            "git rev-parse --git-common-dir failed".to_string(),
        ));
    }

    let common_dir_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let common_dir = PathBuf::from(&common_dir_str);

    // git may output a relative path (relative to the worktree directory)
    let resolved = if common_dir.is_absolute() {
        common_dir
    } else {
        worktree_path.join(common_dir)
    };

    resolved.canonicalize().map_err(AgentreeError::Io)
}

/// Add `.agentree/` to the shared git `info/exclude` file so it is not tracked.
///
/// Resolves `$GIT_COMMON_DIR` via `git rev-parse --git-common-dir`, then appends
/// `.agentree/` to `$GIT_COMMON_DIR/info/exclude` if not already present.
fn add_agentree_to_git_exclude(worktree_path: &Path) -> Result<()> {
    let common_dir = resolve_git_common_dir(worktree_path)?;

    // Ensure info/ directory exists
    let info_dir = common_dir.join("info");
    std::fs::create_dir_all(&info_dir).map_err(AgentreeError::Io)?;

    // Read existing exclude file content
    let exclude_path = info_dir.join("exclude");
    let existing = std::fs::read_to_string(&exclude_path).unwrap_or_default();

    // Append if not already present
    if !existing.lines().any(|line| line.trim() == ".agentree/") {
        let append = if existing.ends_with('\n') || existing.is_empty() {
            "# agentree status directory (local, not committed)\n.agentree/\n".to_string()
        } else {
            "\n# agentree status directory (local, not committed)\n.agentree/\n".to_string()
        };
        std::fs::write(&exclude_path, format!("{}{}", existing, append))
            .map_err(AgentreeError::Io)?;
    }

    Ok(())
}

/// Remove an orphaned worktree directory if it exists
///
/// An orphaned directory is one that exists on disk but is not tracked by git
/// (e.g., after `git worktree prune` removed the metadata but left the directory).
///
/// At this point in the code flow:
/// - We've already run `git worktree repair` in `ensure_clean_state()`
/// - We've already listed all worktrees that git knows about
/// - The directory exists but is NOT in the worktree list
/// - Therefore: the directory is truly orphaned (no git metadata exists for it)
///
/// This function prompts the user and removes the orphaned directory to allow
/// worktree creation to proceed.
fn remove_orphaned_directory(path: &Path) -> Result<()> {
    use std::io::{self, Write};

    if !path.exists() {
        // Directory doesn't exist, nothing to do
        return Ok(());
    }

    // At this point, we know:
    // 1. Directory exists on disk
    // 2. Git doesn't know about it (we're in create_worktree, not InWorktree case)
    // 3. Repair was already attempted in ensure_clean_state()
    // Therefore: directory is truly orphaned with no git metadata

    eprintln!(
        "Warning: Directory '{}' exists but is not tracked by git.",
        path.display()
    );
    eprintln!("Directory appears to be orphaned (no git metadata found).");
    eprintln!("This may be from a previously pruned worktree.");
    eprintln!();
    eprintln!("Note: Automatic repair was already attempted during initialization.");
    eprintln!();

    // Prompt for confirmation
    print!("Remove directory and continue? [y/N] ");
    let _ = io::stdout().flush();

    let mut input = String::new();
    if let Err(e) = io::stdin().read_line(&mut input) {
        return Err(AgentreeError::Worktree(format!(
            "Failed to read user input: {}",
            e
        )));
    }
    let input = input.trim().to_lowercase();

    if input != "y" && input != "yes" {
        return Err(AgentreeError::Worktree(format!(
            "Directory '{}' already exists. Please remove it manually or use a different location.",
            path.display()
        )));
    }

    // User confirmed, remove the directory
    std::fs::remove_dir_all(path).map_err(|e| {
        AgentreeError::Worktree(format!(
            "Failed to remove orphaned directory '{}': {}",
            path.display(),
            e
        ))
    })?;

    eprintln!("Removed orphaned directory.");
    Ok(())
}

/// Detect the status of a branch
///
/// Returns:
/// - `InWorktree(path)` if the branch is checked out in an existing worktree
/// - `ExistsNotCheckedOut` if the branch exists but is not in a worktree
/// - `DoesNotExist` if the branch does not exist
pub fn detect_branch_status(branch: &str) -> Result<BranchStatus> {
    // Get current worktrees
    let worktrees = ensure_clean_state()?;

    // Check if branch is in any worktree
    for entry in worktrees {
        if let Some(ref entry_branch) = entry.branch {
            if entry_branch == branch {
                return Ok(BranchStatus::InWorktree(entry.path));
            }
        }
    }

    // Check if branch exists as a ref
    let exists = run_git_query(&[
        "show-ref",
        "--verify",
        "--quiet",
        &format!("refs/heads/{}", branch),
    ])?
    .is_some();

    if exists {
        return Ok(BranchStatus::ExistsNotCheckedOut);
    }

    // Branch doesn't exist locally — check if it exists on any remote.
    // This handles the common case where a user passes a branch name that was
    // pushed by a teammate but never fetched as a local branch.
    if let Some(remote_ref) = crate::utils::git::find_remote_tracking_ref(branch)? {
        return Ok(BranchStatus::ExistsOnRemote(remote_ref));
    }

    Ok(BranchStatus::DoesNotExist)
}

/// Ensure a workspace exists for the given branch, creating it if needed.
///
/// This is the auto-create pattern used by shell, exec, and agent commands.
/// Returns CreateResult so callers can use both the workspace path and the result type.
///
/// If start_ref is provided but the branch already exists as a worktree,
/// start_ref is ignored (existing workspace takes precedence).
///
/// # Arguments
/// * `config` - Worktree configuration
/// * `repo_root` - Repository root path
/// * `branch` - Target branch name
/// * `start_ref` - Optional starting ref to validate (e.g., base branch)
///
/// # Example
/// ```ignore
/// let result = ensure_workspace(&config, &repo_root, "feature", Some("main"))?;
/// println!("Workspace at: {}", result.path().display());
/// ```
pub fn ensure_workspace(
    config: &WorktreeConfig,
    repo_root: &Path,
    branch: &str,
    start_ref: Option<&str>,
) -> Result<CreateResult> {
    // Validate branch name first
    validation::validate_branch_name(branch)?;

    // If start_ref is provided, validate it early to get helpful error messages
    if let Some(ref_name) = start_ref {
        crate::utils::git::validate_start_ref(ref_name)?;
    }

    // Delegate to create_worktree which handles all three BranchStatus cases idempotently
    create_worktree(config, repo_root, branch, start_ref)
}

/// Create a worktree for a branch
///
/// This function handles all four branch states:
/// - If the branch is already in a worktree, returns the existing path as Resumed
/// - If the branch exists locally but not in a worktree, checks it out in a new worktree
/// - If the branch exists on a remote but not locally, creates a local tracking branch
/// - If the branch doesn't exist anywhere, creates it from the base and checks it out
///
/// Returns CreateResult indicating whether an existing worktree was resumed or a new one created.
/// The caller (command handler) is responsible for printing user-facing messages.
pub fn create_worktree(
    config: &WorktreeConfig,
    repo_root: &Path,
    branch: &str,
    base: Option<&str>,
) -> Result<CreateResult> {
    // Validate branch name first
    validation::validate_branch_name(branch)?;

    // Fail early with a clear message if the repo has no commits.
    // git worktree add requires at least one commit to function.
    if !has_commits()? {
        return Err(AgentreeError::Worktree(
            "Repository has no commits yet.\n\
             Create an initial commit first:\n  git commit --allow-empty -m 'Initial commit'"
                .to_string(),
        ));
    }

    let status = detect_branch_status(branch)?;

    match status {
        BranchStatus::InWorktree(path) => {
            // Branch already has a worktree - return existing path
            Ok(CreateResult::Resumed(path))
        }
        status @ (BranchStatus::ExistsNotCheckedOut | BranchStatus::ExistsOnRemote(_)) => {
            // Both cases use the same git DWIM command: no -b flag lets git resolve the
            // branch as either a local ref or a remote-tracking branch transparently.
            // We only differ in the CreateResult variant returned to the caller.
            let is_remote_checkout = matches!(status, BranchStatus::ExistsOnRemote(_));

            let short_hash = get_short_hash()?;
            let repo_name = repo_root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("repo");
            let context = TemplateContext::new(repo_name, branch, &short_hash);
            let worktree_path = compute_worktree_path(config, repo_root, &context)?;

            // Clean up orphaned directory if it exists (from a previous pruned worktree)
            remove_orphaned_directory(&worktree_path)?;

            let path_str = crate::utils::git::path_to_str(&worktree_path, "worktree path")?;
            run_git_command_no_timeout(&["worktree", "add", path_str, branch], "create worktree")
                .map_err(|e| improve_worktree_add_error(e, branch))?;

            if let Err(e) = setup_agentree_workspace(&worktree_path) {
                eprintln!("Warning: could not set up .agentree/ workspace: {}", e);
                // Non-fatal: worktree was created, status protocol just won't work
            }

            Ok(if is_remote_checkout {
                CreateResult::CheckedOutFromRemote(worktree_path)
            } else {
                CreateResult::Created(worktree_path)
            })
        }
        BranchStatus::DoesNotExist => {
            // Branch doesn't exist anywhere, create it from base
            let short_hash = get_short_hash()?;
            let repo_name = repo_root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("repo");
            let context = TemplateContext::new(repo_name, branch, &short_hash);
            let worktree_path = compute_worktree_path(config, repo_root, &context)?;

            // Clean up orphaned directory if it exists (from previous pruned worktree)
            remove_orphaned_directory(&worktree_path)?;

            let path_str = crate::utils::git::path_to_str(&worktree_path, "worktree path")?;
            // --no-track prevents git from setting the new branch's upstream to the
            // base ref (which happens automatically when base is a remote tracking branch
            // like origin/preprod, causing confusing push behaviour)
            let mut args = vec!["worktree", "add", "--no-track", "-b", branch, path_str];
            if let Some(base_branch) = base {
                args.push(base_branch);
            }

            run_git_command_no_timeout(&args, "create worktree")
                .map_err(|e| improve_worktree_add_error(e, branch))?;

            if let Err(e) = setup_agentree_workspace(&worktree_path) {
                eprintln!("Warning: could not set up .agentree/ workspace: {}", e);
                // Non-fatal: worktree was created, status protocol just won't work
            }

            Ok(CreateResult::CreatedWithBranch(worktree_path))
        }
    }
}

/// Delete a worktree by branch name
///
/// This removes the worktree directory and updates git metadata, but preserves the branch.
/// Returns the path of the removed worktree so callers can use it for cleanup.
///
/// # Arguments
/// * `branch` - Branch name to remove
/// * `force_level` - How aggressively to remove (None/IgnoreDirty/IgnoreLocked)
/// * `unlock` - If true, attempt to unlock the worktree before removing
pub fn delete_worktree(branch: &str, force_level: ForceLevel, unlock: bool) -> Result<PathBuf> {
    // Validate branch name first
    validation::validate_branch_name(branch)?;

    let worktrees = ensure_clean_state()?;

    // Find worktree by branch
    let worktree = worktrees
        .iter()
        .find(|e| e.branch.as_deref() == Some(branch))
        .ok_or_else(|| AgentreeError::WorktreeNotFound {
            branch: branch.to_string(),
        })?;

    // Refuse to remove the main repository checkout (always the first entry in
    // `git worktree list`). Path equality catches both the named-branch case and
    // the detached-HEAD case where main.branch is None.
    if worktrees
        .first()
        .is_some_and(|main| main.path == worktree.path)
    {
        return Err(AgentreeError::Worktree(format!(
            "Cannot remove the main repository checkout for branch '{}'.\n\
             Only linked worktrees can be removed.\n\
             To switch the main repo to a different branch:\n\
               git checkout <other-branch>",
            branch
        )));
    }

    let path = worktree.path.clone();
    // Obtain an owned string so we can move `path` freely after the git call
    let path_str = crate::utils::git::path_to_str(&path, "worktree path")?.to_string();

    // If unlock flag is set, try to unlock first
    if unlock {
        match run_git_command_no_timeout(&["worktree", "unlock", &path_str], "unlock worktree") {
            Ok(_) => {
                eprintln!("Unlocked worktree for branch '{}'", branch);
            }
            Err(e) => {
                let error_msg = e.to_string();
                // "not locked" or "unlocked" in error message means it's already unlocked
                if !error_msg.contains("not locked") && !error_msg.contains("unlocked") {
                    eprintln!("Warning: Could not unlock worktree: {}", e);
                    eprintln!("         Attempting removal anyway...");
                }
                // If it's already unlocked, that's fine - continue silently
            }
        }
    }

    // Build git command with appropriate force flags
    let mut args = vec!["worktree", "remove"];

    // Add force flags based on level (compatible with older Rust versions)
    // Note: Using repeat().take() instead of repeat_n() for broader Rust version compatibility
    #[allow(clippy::manual_repeat_n)]
    args.extend(std::iter::repeat("--force").take(force_level.flag_count()));

    args.push(&path_str);

    // Try to remove the worktree
    match run_git_command_no_timeout(&args, "remove worktree") {
        Ok(_) => Ok(path),
        Err(e) => {
            // Parse the error to provide helpful guidance
            let error_msg = e.to_string();

            if error_msg.contains("contains the current working directory") {
                Err(AgentreeError::Worktree(format!(
                    "Cannot remove worktree for '{}': your shell is currently inside it.\n\
                     Navigate away first, then retry:\n\
                       agentree cd        # return to main repository\n\
                       agentree remove {}",
                    branch, branch
                )))
            } else if error_msg.contains("is a main working tree") {
                Err(AgentreeError::Worktree(format!(
                    "Cannot remove the main repository checkout for branch '{}'.\n\
                     Only linked worktrees can be removed.\n\
                     To switch the main repo to a different branch:\n\
                       git checkout <other-branch>",
                    branch
                )))
            } else if error_msg.contains("locked working tree") {
                Err(AgentreeError::Worktree(format!(
                    r#"Cannot remove locked worktree for branch '{branch}'.

The worktree is locked (likely due to interrupted initialization).
Location: {path}

To fix this, try one of these options:
1. Unlock and remove: agentree remove --unlock {branch}
2. Force remove: agentree remove -ff {branch}
3. Manual unlock: git worktree unlock "{path}" && agentree remove {branch}"#,
                    branch = branch,
                    path = path_str
                )))
            } else if error_msg.contains("uncommitted changes")
                || error_msg.contains("modified files")
                || error_msg.contains("untracked files")
                || error_msg.contains("modified or untracked")
            {
                Err(AgentreeError::Worktree(format!(
                    r#"Cannot remove worktree for branch '{branch}' with uncommitted changes.

Location: {path}

To fix this, try one of these options:
1. Force remove: agentree remove -f {branch}
2. Commit changes first: cd "{path}" && git commit
3. Stash changes: cd "{path}" && git stash"#,
                    branch = branch,
                    path = path_str
                )))
            } else if error_msg.to_lowercase().contains("permission denied") {
                Err(AgentreeError::Worktree(format!(
                    "Cannot remove worktree for '{}': permission denied.\n\
                     Location: {}\n\n\
                     Fix directory permissions:\n\
                       chmod -R u+w \"{}\"\n\
                     Or remove manually with elevated privileges:\n\
                       sudo rm -rf \"{}\"",
                    branch, path_str, path_str, path_str
                )))
            } else {
                // Return the original error if we can't provide specific guidance
                Err(e)
            }
        }
    }
}

/// List branches that have been merged into the base branch
///
/// Returns a list of branch names (excluding the base branch itself)
pub fn list_merged_branches(base: &str) -> Result<Vec<String>> {
    // First validate that base branch exists (check both local and remote)
    let ref_paths = vec![
        format!("refs/heads/{}", base),   // Local branch
        format!("refs/remotes/{}", base), // Remote branch (e.g., origin/main)
    ];

    let mut branch_exists = false;
    for ref_path in &ref_paths {
        if run_git_query(&["show-ref", "--verify", ref_path])?.is_some() {
            branch_exists = true;
            break;
        }
    }

    if !branch_exists {
        return Err(AgentreeError::BranchNotFound {
            branch: base.to_string(),
        });
    }

    // Get merged branches
    let output_str = run_git_command(
        &["branch", "--merged", base, "--format=%(refname:short)"],
        "list merged branches",
    )?;

    let branches: Vec<String> = output_str
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != base)
        .collect();

    Ok(branches)
}

/// Check if a specific branch is merged into the base branch
pub fn is_branch_merged(branch: &str, base: &str) -> Result<bool> {
    let merged = list_merged_branches(base)?;
    Ok(merged.contains(&branch.to_string()))
}

/// Get the last activity time for a worktree directory
pub fn get_last_activity(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

/// Format a SystemTime as a human-readable timestamp
pub fn format_activity(time: SystemTime) -> String {
    use chrono::{DateTime, Local};
    let datetime: DateTime<Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M").to_string()
}

/// Get the short hash of the current HEAD
fn get_short_hash() -> Result<String> {
    run_git_command(&["rev-parse", "--short", "HEAD"], "get short hash")
}

/// Check whether the repository has at least one commit.
fn has_commits() -> Result<bool> {
    Ok(run_git_query(&["rev-parse", "--verify", "HEAD"])?.is_some())
}

/// Translate a raw `git worktree add` error into an actionable message.
///
/// Git's error messages are often terse. This function maps the most common
/// failure patterns to clearer, user-friendly alternatives while keeping
/// the original error as context.
fn improve_worktree_add_error(
    err: crate::error::AgentreeError,
    branch: &str,
) -> crate::error::AgentreeError {
    let msg = err.to_string();

    if msg.contains("already checked out") || msg.contains("is already checked out") {
        return AgentreeError::Worktree(format!(
            "Branch '{branch}' is already checked out in another worktree.\n\
             Use `agentree list` to see existing worktrees."
        ));
    }

    if msg.contains("already exists") {
        return AgentreeError::Worktree(
            "Worktree path already exists and is not managed by git.\n\
             Run `agentree doctor` to detect and fix orphaned directories."
                .to_string(),
        );
    }

    if msg.contains("is not a valid branch name") || msg.contains("not a valid object name") {
        return AgentreeError::Worktree(format!(
            "Cannot create worktree: '{branch}' is not a valid git ref.\n\
             Ensure the base branch or commit exists: `git log --oneline -5`"
        ));
    }

    if msg.contains("is not a commit") {
        return AgentreeError::Worktree(
            "Cannot create worktree: the specified ref does not point to a commit.\n\
             Check the ref with: `git rev-parse <ref>`"
                .to_string(),
        );
    }

    // Return original error unchanged if we cannot provide better guidance
    err
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branch_status_variants() {
        // Test that BranchStatus variants exist and can be compared
        let status1 = BranchStatus::InWorktree(PathBuf::from("/test"));
        let status2 = BranchStatus::InWorktree(PathBuf::from("/test"));
        let status3 = BranchStatus::ExistsNotCheckedOut;
        let status4 = BranchStatus::DoesNotExist;
        let status5 = BranchStatus::ExistsOnRemote("origin/feature".to_string());

        assert_eq!(status1, status2);
        assert_ne!(status1, status3);
        assert_ne!(status3, status4);
        assert_ne!(status4, status5);
    }

    #[test]
    fn test_create_result_variants() {
        let result1 = CreateResult::Resumed(PathBuf::from("/test"));
        let result2 = CreateResult::Resumed(PathBuf::from("/test"));
        let result3 = CreateResult::Created(PathBuf::from("/test"));
        let result4 = CreateResult::CreatedWithBranch(PathBuf::from("/test"));
        let result5 = CreateResult::CheckedOutFromRemote(PathBuf::from("/test"));

        assert_eq!(result1, result2);
        assert_ne!(result1, result3);
        assert_ne!(result3, result4);
        assert_ne!(result4, result5);
    }

    #[test]
    fn test_create_result_path() {
        let path = PathBuf::from("/test");
        assert_eq!(CreateResult::Resumed(path.clone()).path(), &path);
        assert_eq!(CreateResult::Created(path.clone()).path(), &path);
        assert_eq!(CreateResult::CreatedWithBranch(path.clone()).path(), &path);
        assert_eq!(
            CreateResult::CheckedOutFromRemote(path.clone()).path(),
            &path
        );
    }

    #[test]
    fn test_create_result_was_created() {
        let path = PathBuf::from("/test");
        assert!(!CreateResult::Resumed(path.clone()).was_created());
        assert!(CreateResult::Created(path.clone()).was_created());
        assert!(CreateResult::CreatedWithBranch(path.clone()).was_created());
        assert!(CreateResult::CheckedOutFromRemote(path.clone()).was_created());
    }

    #[test]
    fn test_create_result_message() {
        let path = PathBuf::from("/tmp/worktrees/feature");

        let msg = CreateResult::Resumed(path.clone()).message("feature");
        assert!(msg.contains("Resuming") && msg.contains("feature"));

        let msg = CreateResult::Created(path.clone()).message("feature");
        assert!(msg.contains("Created worktree") && msg.contains("feature"));

        let msg = CreateResult::CreatedWithBranch(path.clone()).message("feature");
        assert!(msg.contains("Created branch") && msg.contains("feature"));

        let msg = CreateResult::CheckedOutFromRemote(path.clone()).message("feature");
        assert!(msg.contains("Checked out") && msg.contains("remote") && msg.contains("feature"));
    }

    #[test]
    fn test_get_last_activity_nonexistent_path() {
        let result = get_last_activity(Path::new("/nonexistent/path"));
        assert!(result.is_none());
    }

    #[test]
    fn test_format_activity() {
        use std::time::{Duration, UNIX_EPOCH};

        // Create a known timestamp: 2024-01-15 12:34:56 UTC
        let timestamp = UNIX_EPOCH + Duration::from_secs(1705322096);
        let formatted = format_activity(timestamp);

        // Format should be YYYY-MM-DD HH:MM (length 16)
        assert_eq!(formatted.len(), 16);
        // Should start with date
        assert!(formatted.starts_with("2024-01-15"));
    }

    #[test]
    fn test_ensure_workspace_delegates_to_create() {
        // This is a behavioral test that verifies the function signature
        // and return type match CreateResult. Since it requires a git repo,
        // we just verify the function exists and has the correct type.

        // Type assertion: ensure_workspace returns Result<CreateResult>
        fn _type_check() {
            fn assert_returns_create_result<F>(f: F)
            where
                F: Fn(&WorktreeConfig, &Path, &str, Option<&str>) -> Result<CreateResult>,
            {
                let _ = f;
            }
            assert_returns_create_result(ensure_workspace);
        }
    }
}
