use crate::error::{AgentreeError, Result};
use crate::utils::git::run_git_command;
use crate::worktree::config::WorktreeConfig;
use crate::worktree::state::{list_worktrees, WorktreeEntry};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Maximum recursion depth when scanning for orphaned directories
/// This prevents unbounded recursion and handles symlink cycles
const MAX_SCAN_DEPTH: usize = 10;

/// Box-drawing separator line for visual output formatting
pub const SEPARATOR: &str = "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━";

/// Types of worktree issues that can be detected
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueType {
    /// Directory exists but is not tracked by git
    OrphanedDirectory,
    /// Git knows about worktree but directory is missing or corrupt
    BrokenMetadata,
}

impl std::fmt::Display for IssueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IssueType::OrphanedDirectory => write!(f, "Orphaned Directory"),
            IssueType::BrokenMetadata => write!(f, "Broken Metadata"),
        }
    }
}

/// Represents a detected worktree issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeIssue {
    pub issue_type: IssueType,
    pub path: PathBuf,
    pub branch: Option<String>,
    pub description: String,
    pub fix_description: String,
}

/// Summary of a diagnostic scan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub issues: Vec<WorktreeIssue>,
    pub scan_time: SystemTime,
}

impl DiagnosticReport {
    /// Count issues by severity (all orphaned directories are warnings, broken metadata are errors)
    pub fn count_by_severity(&self) -> (usize, usize) {
        let errors = self
            .issues
            .iter()
            .filter(|i| i.issue_type == IssueType::BrokenMetadata)
            .count();
        let warnings = self
            .issues
            .iter()
            .filter(|i| i.issue_type == IssueType::OrphanedDirectory)
            .count();
        (errors, warnings)
    }
}

/// Result of fixing issues
#[derive(Debug)]
pub struct FixReport {
    pub fixed: usize,
    pub skipped: usize,
    pub failed: Vec<(WorktreeIssue, String)>,
}

/// Run diagnostic scan to detect worktree issues
pub fn diagnose(repo_root: &Path, config: &WorktreeConfig) -> Result<DiagnosticReport> {
    let mut issues = Vec::new();

    // Get current worktree list (WITHOUT repair or auto-prune - we want to see issues)
    let worktrees = list_worktrees()?;

    // Find orphaned directories
    let orphaned = find_orphaned_directories(repo_root, config, &worktrees)?;
    issues.extend(orphaned);

    // Find broken metadata
    let broken = find_broken_metadata(&worktrees)?;
    issues.extend(broken);

    Ok(DiagnosticReport {
        issues,
        scan_time: SystemTime::now(),
    })
}

/// Find directories that exist but are not tracked by git
fn find_orphaned_directories(
    repo_root: &Path,
    config: &WorktreeConfig,
    worktrees: &[WorktreeEntry],
) -> Result<Vec<WorktreeIssue>> {
    let mut issues = Vec::new();

    // Determine scan location using config method
    let scan_dir = config.resolve_location(repo_root)?;

    // If scan directory doesn't exist, no orphaned directories to find
    if !scan_dir.exists() {
        return Ok(issues);
    }

    // Get list of tracked worktree paths (canonicalized for comparison)
    let tracked_paths: Vec<PathBuf> = worktrees
        .iter()
        .filter_map(|e| e.path.canonicalize().ok())
        .collect();

    // Recursively scan for worktree directories
    scan_for_orphaned(&scan_dir, &tracked_paths, &mut issues, 0)?;

    Ok(issues)
}

/// Recursively scan a directory for orphaned worktrees
fn scan_for_orphaned(
    dir: &Path,
    tracked_paths: &[PathBuf],
    issues: &mut Vec<WorktreeIssue>,
    depth: usize,
) -> Result<()> {
    // Prevent unbounded recursion
    if depth >= MAX_SCAN_DEPTH {
        return Ok(());
    }

    let entries = std::fs::read_dir(dir)
        .map_err(|e| {
            AgentreeError::Worktree(format!(
                "Failed to read directory '{}': {}",
                dir.display(),
                e
            ))
        })?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir());

    for path in entries {
        // Check if this directory has a .git file/directory (looks like a worktree)
        let git_path = path.join(".git");
        if git_path.exists() {
            // This looks like a worktree - check if it's tracked
            let canonical_path = match path.canonicalize() {
                Ok(p) => p,
                Err(_) => continue, // Skip if we can't canonicalize
            };

            if !tracked_paths.contains(&canonical_path) {
                // Found an orphaned worktree!
                let branch = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string());

                issues.push(WorktreeIssue {
                    issue_type: IssueType::OrphanedDirectory,
                    path: path.clone(),
                    branch,
                    description: "Directory exists but is not tracked by git".to_string(),
                    fix_description: "Remove orphaned directory".to_string(),
                });
            }
        } else {
            // No .git file, but might contain subdirectories with worktrees
            // Recurse into this directory
            if let Err(e) = scan_for_orphaned(&path, tracked_paths, issues, depth + 1) {
                // Log error but continue scanning other directories
                eprintln!(
                    "Warning: Failed to scan directory '{}': {}",
                    path.display(),
                    e
                );
            }
        }
    }

    Ok(())
}

/// Find worktree metadata entries that point to missing or corrupt directories
fn find_broken_metadata(worktrees: &[WorktreeEntry]) -> Result<Vec<WorktreeIssue>> {
    let mut issues = Vec::new();

    for entry in worktrees {
        // Skip the main repository (first entry, usually has no branch or is bare)
        if entry.is_bare || entry.branch.is_none() {
            continue;
        }

        let path = &entry.path;

        // Check if directory exists
        if !path.exists() {
            issues.push(WorktreeIssue {
                issue_type: IssueType::BrokenMetadata,
                path: path.clone(),
                branch: entry.branch.clone(),
                description: "Git metadata exists but directory is missing".to_string(),
                fix_description: "Prune git worktree metadata".to_string(),
            });
            continue;
        }

        // Check if .git metadata is valid
        let git_path = path.join(".git");
        if !git_path.exists() {
            issues.push(WorktreeIssue {
                issue_type: IssueType::BrokenMetadata,
                path: path.clone(),
                branch: entry.branch.clone(),
                description: "Git metadata is corrupt (.git file missing)".to_string(),
                fix_description: "Prune git worktree metadata".to_string(),
            });
        }
    }

    Ok(issues)
}

/// Display a single issue to the user
pub fn display_issue(issue: &WorktreeIssue, index: usize, total: usize) {
    let severity = match issue.issue_type {
        IssueType::OrphanedDirectory => "[WARNING]",
        IssueType::BrokenMetadata => "[ERROR]",
    };

    eprintln!("{}", SEPARATOR);
    eprintln!(
        "Issue {}/{}: {} {}",
        index + 1,
        total,
        severity,
        issue.issue_type
    );
    eprintln!("  Path: {}", issue.path.display());
    if let Some(ref branch) = issue.branch {
        eprintln!("  Branch: {}", branch);
    }
    eprintln!();
    eprintln!("  {}", issue.description);
    eprintln!("  Fix: {}", issue.fix_description);
    eprintln!("{}", SEPARATOR);
    eprintln!();
}

/// Prompt user for yes/no confirmation
fn prompt_yes_no(prompt: &str) -> Result<bool> {
    print!("{} [y/N] ", prompt);
    let _ = io::stdout().flush();

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            AgentreeError::Worktree("User cancelled (Ctrl+C or Ctrl+D)".to_string())
        } else {
            AgentreeError::Worktree(format!("Failed to read user input: {}", e))
        }
    })?;

    let input = input.trim().to_lowercase();
    Ok(input == "y" || input == "yes")
}

/// Apply fix for a single issue
fn apply_fix(issue: &WorktreeIssue) -> Result<()> {
    match issue.issue_type {
        IssueType::OrphanedDirectory => {
            // Remove orphaned directory
            std::fs::remove_dir_all(&issue.path).map_err(|e| {
                AgentreeError::Worktree(format!(
                    "Failed to remove directory '{}': {}",
                    issue.path.display(),
                    e
                ))
            })?;
            eprintln!("✓ Removed orphaned directory");
        }
        IssueType::BrokenMetadata => {
            // Prune git metadata
            run_git_command(&["worktree", "prune"], "prune broken worktree metadata")?;
            eprintln!("✓ Pruned broken metadata");
        }
    }
    Ok(())
}

/// Interactively fix detected issues
pub fn fix_issues_interactive(report: &DiagnosticReport) -> Result<FixReport> {
    let mut fixed = 0;
    let mut skipped = 0;
    let mut failed = Vec::new();

    let total = report.issues.len();

    for (index, issue) in report.issues.iter().enumerate() {
        display_issue(issue, index, total);

        // Prompt user for confirmation
        match prompt_yes_no("Fix this issue?") {
            Ok(true) => {
                // User confirmed, apply fix
                match apply_fix(issue) {
                    Ok(()) => {
                        fixed += 1;
                        eprintln!();
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to fix: {}", e);
                        failed.push((issue.clone(), e.to_string()));
                        eprintln!();
                    }
                }
            }
            Ok(false) => {
                eprintln!("Skipped.");
                skipped += 1;
                eprintln!();
            }
            Err(e) => {
                eprintln!("Failed to read input: {}", e);
                skipped += 1;
                eprintln!();
            }
        }
    }

    Ok(FixReport {
        fixed,
        skipped,
        failed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_type_display() {
        assert_eq!(
            format!("{}", IssueType::OrphanedDirectory),
            "Orphaned Directory"
        );
        assert_eq!(format!("{}", IssueType::BrokenMetadata), "Broken Metadata");
    }

    #[test]
    fn test_diagnostic_report_count_by_severity() {
        let report = DiagnosticReport {
            issues: vec![
                WorktreeIssue {
                    issue_type: IssueType::OrphanedDirectory,
                    path: PathBuf::from("/tmp/orphaned"),
                    branch: Some("feature".to_string()),
                    description: "test".to_string(),
                    fix_description: "test fix".to_string(),
                },
                WorktreeIssue {
                    issue_type: IssueType::BrokenMetadata,
                    path: PathBuf::from("/tmp/broken"),
                    branch: Some("hotfix".to_string()),
                    description: "test".to_string(),
                    fix_description: "test fix".to_string(),
                },
                WorktreeIssue {
                    issue_type: IssueType::OrphanedDirectory,
                    path: PathBuf::from("/tmp/orphaned2"),
                    branch: None,
                    description: "test".to_string(),
                    fix_description: "test fix".to_string(),
                },
            ],
            scan_time: SystemTime::now(),
        };

        let (errors, warnings) = report.count_by_severity();
        assert_eq!(errors, 1);
        assert_eq!(warnings, 2);
    }

    #[test]
    fn test_find_orphaned_directories_empty_dir() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let repo_root = temp_dir.path();

        // Use a worktree location inside the temp directory to avoid shared /tmp/worktrees
        let worktrees_dir = temp_dir.path().join("worktrees");
        let config = WorktreeConfig {
            location: Some(worktrees_dir.to_string_lossy().to_string()),
            ..Default::default()
        };
        let worktrees = vec![];

        let result = find_orphaned_directories(repo_root, &config, &worktrees);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_find_broken_metadata_missing_directory() {
        let worktrees = vec![WorktreeEntry {
            path: PathBuf::from("/nonexistent/path"),
            head: "abc123".to_string(),
            branch: Some("feature".to_string()),
            is_bare: false,
            is_detached: false,
            locked: None,
        }];

        let result = find_broken_metadata(&worktrees).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].issue_type, IssueType::BrokenMetadata);
        assert_eq!(result[0].branch, Some("feature".to_string()));
    }

    #[test]
    fn test_find_broken_metadata_skips_bare() {
        let worktrees = vec![WorktreeEntry {
            path: PathBuf::from("/nonexistent/path"),
            head: "abc123".to_string(),
            branch: None,
            is_bare: true,
            is_detached: false,
            locked: None,
        }];

        let result = find_broken_metadata(&worktrees).unwrap();
        assert_eq!(result.len(), 0);
    }
}
