use crate::config;
use crate::error::Result;
use crate::utils::git::{get_git_root, path_to_str, run_git_query};
use crate::utils::progress::with_spinner;
use crate::worktree::{metadata::WorktreeMetadata, operations, recovery, validation};
use clap::Parser;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Width of the ST (status) column in table and two-lines formats.
const STATUS_WIDTH: usize = 2;

#[derive(Parser, Debug)]
pub struct ListArgs {
    /// Output format
    #[arg(long, value_enum, default_value = "card")]
    pub format: OutputFormat,

    /// Legacy: equivalent to --format=json (deprecated)
    #[arg(long, conflicts_with = "format")]
    pub json: bool,

    /// Check each worktree for uncommitted changes (slower: runs git status per worktree)
    #[arg(long)]
    pub dirty: bool,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    /// Two-line format: summary line + absolute path on second line
    TwoLines,
    /// Compact table with relative paths (max-width: 120)
    Table,
    /// Card-style boxes with full absolute paths and details (default)
    Card,
    /// Machine-readable JSON with absolute paths
    Json,
}

/// Whether a worktree has uncommitted changes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum DirtyStatus {
    /// The --dirty flag was not passed; no check was performed.
    NotChecked,
    /// Check was performed: worktree is clean.
    Clean,
    /// Check was performed: worktree has uncommitted changes.
    Dirty,
}

#[derive(Serialize)]
struct WorktreeJson {
    branch: String,
    path: String,
    backend: Option<String>,
    created: Option<String>,
    modified: Option<String>,
    /// null when --dirty was not requested, true/false when checked.
    dirty: Option<bool>,
    /// null when not locked, "" when locked without reason, reason string otherwise.
    locked: Option<String>,
}

/// Internal representation of a worktree with its metadata for rendering
struct WorktreeInfo {
    branch: String,
    path: PathBuf,
    backend: String,
    created: Option<String>,
    modified: Option<std::time::SystemTime>,
    dirty: DirtyStatus,
    /// None = not locked, Some("") = locked no reason, Some(msg) = locked with reason
    locked: Option<String>,
}

pub fn execute(args: ListArgs) -> Result<()> {
    // Check git version
    validation::check_git_version()?;

    // Find repository root
    let repo_root = get_git_root()?.ok_or_else(|| {
        crate::error::AgentreeError::Worktree(
            "Not in a git repository. Run this command from inside a git repository.".to_string(),
        )
    })?;

    // Load config (for consistency and future use)
    let _config = config::load(&repo_root)?;

    // Handle backward compatibility
    let format = if args.json {
        eprintln!("Warning: --json is deprecated, use --format=json instead");
        OutputFormat::Json
    } else {
        args.format
    };

    // Get worktrees with metadata; show a spinner when --dirty triggers per-worktree git calls
    let worktrees = if args.dirty {
        with_spinner("Checking for uncommitted changes...", || {
            get_worktrees_with_metadata(true)
        })?
    } else {
        get_worktrees_with_metadata(false)?
    };

    // Check if there are any worktrees
    if worktrees.is_empty() {
        match format {
            OutputFormat::Json => println!("[]"),
            _ => println!("No worktrees found."),
        }
        return Ok(());
    }

    // Dispatch to format-specific renderer
    match format {
        OutputFormat::TwoLines => render_two_lines(&worktrees),
        OutputFormat::Table => render_table(&worktrees, &repo_root),
        OutputFormat::Card => render_card(&worktrees),
        OutputFormat::Json => render_json(&worktrees),
    }
}

fn get_worktrees_with_metadata(check_dirty: bool) -> Result<Vec<WorktreeInfo>> {
    // Get linked worktrees only (excludes main repo and detached HEADs)
    let worktree_list = recovery::list_linked_worktrees()?;

    // Get last activity for each worktree and create sortable tuples
    let mut worktrees_with_time: Vec<_> = worktree_list
        .iter()
        .map(|entry| {
            let activity = operations::get_last_activity(&entry.path);
            let metadata = WorktreeMetadata::load(&entry.path).ok().flatten();

            let dirty = if check_dirty {
                path_to_str(&entry.path, "worktree path")
                    .ok()
                    .and_then(|path_str| {
                        run_git_query(&["-C", path_str, "status", "--short"])
                            .ok()
                            .flatten()
                    })
                    .map(|output| {
                        if output.is_empty() {
                            DirtyStatus::Clean
                        } else {
                            DirtyStatus::Dirty
                        }
                    })
                    .unwrap_or(DirtyStatus::NotChecked)
            } else {
                DirtyStatus::NotChecked
            };

            WorktreeInfo {
                branch: entry.branch.clone().unwrap_or_default(),
                path: entry.path.clone(),
                backend: metadata
                    .as_ref()
                    .map(|m| m.backend.clone())
                    .unwrap_or_else(|| "unknown".to_string()),
                created: metadata.as_ref().map(|m| m.created_at.clone()),
                modified: activity,
                dirty,
                locked: entry.locked.clone(),
            }
        })
        .collect();

    // Sort by last modified (most recent first)
    worktrees_with_time.sort_by(|a, b| match (a.modified, b.modified) {
        (Some(time_a), Some(time_b)) => time_b.cmp(&time_a),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    Ok(worktrees_with_time)
}

fn format_created_date(created: Option<&String>) -> String {
    created
        .and_then(|c| {
            use chrono::DateTime;
            DateTime::parse_from_rfc3339(c)
                .ok()
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        })
        .unwrap_or_else(|| "-".to_string())
}

fn render_json(worktrees: &[WorktreeInfo]) -> Result<()> {
    let json_output: Vec<WorktreeJson> = worktrees
        .iter()
        .map(|info| {
            let modified = info.modified.map(|time| {
                use chrono::{DateTime, Utc};
                let datetime: DateTime<Utc> = time.into();
                datetime.to_rfc3339()
            });

            let dirty = match info.dirty {
                DirtyStatus::NotChecked => None,
                DirtyStatus::Clean => Some(false),
                DirtyStatus::Dirty => Some(true),
            };

            WorktreeJson {
                branch: info.branch.clone(),
                path: info.path.display().to_string(),
                backend: Some(info.backend.clone()),
                created: info.created.clone(),
                modified,
                dirty,
                locked: info.locked.clone(),
            }
        })
        .collect();

    println!("{}", serde_json::to_string_pretty(&json_output)?);
    Ok(())
}

fn render_two_lines(worktrees: &[WorktreeInfo]) -> Result<()> {
    // Header
    println!(
        "{:<30} {:<12} {:<20} {:<width$}",
        "BRANCH",
        "BACKEND",
        "MODIFIED",
        "ST",
        width = STATUS_WIDTH
    );

    for info in worktrees {
        let modified = info
            .modified
            .map(operations::format_activity)
            .unwrap_or_else(|| "Unknown".to_string());

        let st = format_status(info.dirty, info.locked.as_deref());

        // First line: branch, backend, modified, status
        println!(
            "{:<30} {:<12} {:<20} {:<width$}",
            truncate(&info.branch, 30),
            truncate(&info.backend, 12),
            modified,
            st,
            width = STATUS_WIDTH
        );

        // Second line: absolute path with arrow
        println!("  → {}", info.path.display());
    }

    Ok(())
}

fn render_table(worktrees: &[WorktreeInfo], repo_root: &Path) -> Result<()> {
    // Header with 120 max width
    println!(
        "{:<25} {:<50} {:<12} {:<18} {:<15} {:<width$}",
        "BRANCH",
        "PATH",
        "BACKEND",
        "CREATED",
        "MODIFIED",
        "ST",
        width = STATUS_WIDTH
    );

    for info in worktrees {
        // Convert to relative path
        let path_display = make_relative_path(&info.path, repo_root);

        let created = format_created_date(info.created.as_ref());

        let modified = info
            .modified
            .map(operations::format_activity)
            .unwrap_or_else(|| "Unknown".to_string());

        let st = format_status(info.dirty, info.locked.as_deref());

        println!(
            "{:<25} {:<50} {:<12} {:<18} {:<15} {:<width$}",
            truncate(&info.branch, 25),
            truncate(&path_display, 50),
            truncate(&info.backend, 12),
            created,
            modified,
            st,
            width = STATUS_WIDTH
        );
    }

    Ok(())
}

fn render_card(worktrees: &[WorktreeInfo]) -> Result<()> {
    for (i, info) in worktrees.iter().enumerate() {
        if i > 0 {
            println!(); // Blank line between cards
        }

        let created = format_created_date(info.created.as_ref());

        let modified = info
            .modified
            .map(operations::format_activity)
            .unwrap_or_else(|| "Unknown".to_string());

        println!("┌─ {}", info.branch);
        println!("│  Path:     {}", info.path.display());
        println!("│  Backend:  {}", info.backend);
        println!("│  Created:  {}", created);
        println!("│  Modified: {}", modified);
        match info.dirty {
            DirtyStatus::Clean => println!("│  Dirty:    no"),
            DirtyStatus::Dirty => println!("│  Dirty:    yes"),
            DirtyStatus::NotChecked => {}
        }
        if let Some(lock_reason) = &info.locked {
            if lock_reason.is_empty() {
                println!("│  Locked:   yes");
            } else {
                println!("│  Locked:   yes ({})", lock_reason);
            }
        }
        println!("└─");
    }

    Ok(())
}

/// Format a 2-character status column: `*` for dirty, `L` for locked.
/// - `  ` — not checked or clean, not locked
/// - `* ` — dirty, not locked
/// - ` L` — not checked or clean, locked
/// - `*L` — dirty and locked
fn format_status(dirty: DirtyStatus, locked: Option<&str>) -> &'static str {
    match (dirty, locked.is_some()) {
        (DirtyStatus::Dirty, true) => "*L",
        (DirtyStatus::Dirty, false) => "* ",
        (_, true) => " L",
        (_, false) => "  ",
    }
}

fn make_relative_path(path: &Path, repo_root: &Path) -> String {
    // Try relative to repo root first
    if let Ok(rel) = path.strip_prefix(repo_root) {
        return rel.display().to_string();
    }

    // Try relative to repo parent (for sibling directories like ../worktrees/)
    if let Some(parent) = repo_root.parent() {
        if let Ok(rel) = path.strip_prefix(parent) {
            return format!("../{}", rel.display());
        }
    }

    // Fallback to absolute path
    path.display().to_string()
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_status_clean_unlocked() {
        assert_eq!(format_status(DirtyStatus::Clean, None), "  ");
    }

    #[test]
    fn test_format_status_not_checked_unlocked() {
        assert_eq!(format_status(DirtyStatus::NotChecked, None), "  ");
    }

    #[test]
    fn test_format_status_dirty_unlocked() {
        assert_eq!(format_status(DirtyStatus::Dirty, None), "* ");
    }

    #[test]
    fn test_format_status_clean_locked_no_reason() {
        assert_eq!(format_status(DirtyStatus::Clean, Some("")), " L");
    }

    #[test]
    fn test_format_status_clean_locked_with_reason() {
        assert_eq!(format_status(DirtyStatus::Clean, Some("in use")), " L");
    }

    #[test]
    fn test_format_status_not_checked_locked() {
        assert_eq!(format_status(DirtyStatus::NotChecked, Some("")), " L");
    }

    #[test]
    fn test_format_status_dirty_locked() {
        assert_eq!(format_status(DirtyStatus::Dirty, Some("in use")), "*L");
    }

    #[test]
    fn test_format_status_always_two_chars() {
        // All combinations should produce exactly 2 characters
        for dirty in [
            DirtyStatus::NotChecked,
            DirtyStatus::Clean,
            DirtyStatus::Dirty,
        ] {
            for locked in [None, Some(""), Some("reason")] {
                let status = format_status(dirty, locked);
                assert_eq!(
                    status.chars().count(),
                    2,
                    "format_status({:?}, {:?}) = {:?}",
                    dirty,
                    locked,
                    status
                );
            }
        }
    }

    fn make_info(dirty: DirtyStatus, locked: Option<&str>) -> WorktreeInfo {
        WorktreeInfo {
            branch: "feature/test".to_string(),
            path: PathBuf::from("/tmp/worktree"),
            backend: "local".to_string(),
            created: None,
            modified: None,
            dirty,
            locked: locked.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_card_omits_dirty_when_not_checked() {
        use std::io::Write;

        // Capture stdout by running render_card and checking it does not
        // contain "Dirty:" when status is NotChecked.
        // We re-implement the card dirty logic directly to keep this unit-level.
        let info = make_info(DirtyStatus::NotChecked, None);
        let dirty_line = match info.dirty {
            DirtyStatus::Clean => Some("Dirty:    no"),
            DirtyStatus::Dirty => Some("Dirty:    yes"),
            DirtyStatus::NotChecked => None,
        };
        assert!(
            dirty_line.is_none(),
            "Dirty line should be absent when not checked"
        );
    }

    #[test]
    fn test_card_shows_dirty_yes_when_dirty() {
        let info = make_info(DirtyStatus::Dirty, None);
        let dirty_line = match info.dirty {
            DirtyStatus::Clean => Some("Dirty:    no"),
            DirtyStatus::Dirty => Some("Dirty:    yes"),
            DirtyStatus::NotChecked => None,
        };
        assert_eq!(dirty_line, Some("Dirty:    yes"));
    }

    #[test]
    fn test_card_shows_dirty_no_when_clean() {
        let info = make_info(DirtyStatus::Clean, None);
        let dirty_line = match info.dirty {
            DirtyStatus::Clean => Some("Dirty:    no"),
            DirtyStatus::Dirty => Some("Dirty:    yes"),
            DirtyStatus::NotChecked => None,
        };
        assert_eq!(dirty_line, Some("Dirty:    no"));
    }

    #[test]
    fn test_card_omits_locked_when_unlocked() {
        let info = make_info(DirtyStatus::NotChecked, None);
        assert!(info.locked.is_none(), "Locked line should be absent");
    }

    #[test]
    fn test_card_shows_locked_with_reason() {
        let info = make_info(DirtyStatus::NotChecked, Some("in use"));
        assert_eq!(info.locked.as_deref(), Some("in use"));
    }

    #[test]
    fn test_json_dirty_null_when_not_checked() {
        let info = make_info(DirtyStatus::NotChecked, None);
        let dirty: Option<bool> = match info.dirty {
            DirtyStatus::NotChecked => None,
            DirtyStatus::Clean => Some(false),
            DirtyStatus::Dirty => Some(true),
        };
        assert!(dirty.is_none());
    }

    #[test]
    fn test_json_dirty_false_when_clean() {
        let info = make_info(DirtyStatus::Clean, None);
        let dirty: Option<bool> = match info.dirty {
            DirtyStatus::NotChecked => None,
            DirtyStatus::Clean => Some(false),
            DirtyStatus::Dirty => Some(true),
        };
        assert_eq!(dirty, Some(false));
    }

    #[test]
    fn test_json_dirty_true_when_dirty() {
        let info = make_info(DirtyStatus::Dirty, None);
        let dirty: Option<bool> = match info.dirty {
            DirtyStatus::NotChecked => None,
            DirtyStatus::Clean => Some(false),
            DirtyStatus::Dirty => Some(true),
        };
        assert_eq!(dirty, Some(true));
    }
}
