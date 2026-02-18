use crate::config;
use crate::error::Result;
use crate::utils::git::{get_git_root, path_to_str, run_git_query};
use crate::worktree::{metadata::WorktreeMetadata, operations, recovery, validation};
use clap::Parser;
use serde::Serialize;
use std::path::{Path, PathBuf};

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

#[derive(Serialize)]
struct WorktreeJson {
    branch: String,
    path: String,
    backend: Option<String>,
    created: Option<String>,
    modified: Option<String>,
    dirty: Option<bool>,
    locked: Option<String>,
}

/// Internal representation of a worktree with its metadata for rendering
struct WorktreeInfo {
    branch: String,
    path: PathBuf,
    backend: String,
    created: Option<String>,
    modified: Option<std::time::SystemTime>,
    /// None = not checked, Some(true/false) = checked
    is_dirty: Option<bool>,
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

    // Get worktrees with metadata
    let worktrees = get_worktrees_with_metadata(args.dirty)?;

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

            let is_dirty = if check_dirty {
                let dirty = path_to_str(&entry.path, "worktree path")
                    .ok()
                    .and_then(|path_str| {
                        run_git_query(&["-C", path_str, "status", "--short"])
                            .ok()
                            .flatten()
                    })
                    .map(|output| !output.is_empty())
                    .unwrap_or(false);
                Some(dirty)
            } else {
                None
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
                is_dirty,
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

            WorktreeJson {
                branch: info.branch.clone(),
                path: info.path.display().to_string(),
                backend: Some(info.backend.clone()),
                created: info.created.clone(),
                modified,
                dirty: info.is_dirty,
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
        "{:<30} {:<12} {:<20} {:<2}",
        "BRANCH", "BACKEND", "MODIFIED", "ST"
    );

    for info in worktrees {
        let modified = info
            .modified
            .map(operations::format_activity)
            .unwrap_or_else(|| "Unknown".to_string());

        let st = format_status(info.is_dirty, info.locked.as_deref());

        // First line: branch, backend, modified, status
        println!(
            "{:<30} {:<12} {:<20} {:<2}",
            truncate(&info.branch, 30),
            truncate(&info.backend, 12),
            modified,
            st
        );

        // Second line: absolute path with arrow
        println!("  → {}", info.path.display());
    }

    Ok(())
}

fn render_table(worktrees: &[WorktreeInfo], repo_root: &Path) -> Result<()> {
    // Header with 120 max width
    println!(
        "{:<25} {:<50} {:<12} {:<18} {:<15} {:<2}",
        "BRANCH", "PATH", "BACKEND", "CREATED", "MODIFIED", "ST"
    );

    for info in worktrees {
        // Convert to relative path
        let path_display = make_relative_path(&info.path, repo_root);

        let created = format_created_date(info.created.as_ref());

        let modified = info
            .modified
            .map(operations::format_activity)
            .unwrap_or_else(|| "Unknown".to_string());

        let st = format_status(info.is_dirty, info.locked.as_deref());

        println!(
            "{:<25} {:<50} {:<12} {:<18} {:<15} {:<2}",
            truncate(&info.branch, 25),
            truncate(&path_display, 50),
            truncate(&info.backend, 12),
            created,
            modified,
            st
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
        if let Some(dirty) = info.is_dirty {
            println!("│  Dirty:    {}", if dirty { "yes" } else { "no" });
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
/// - `  ` — clean, not locked
/// - `* ` — dirty, not locked
/// - ` L` — clean, locked
/// - `*L` — dirty and locked
fn format_status(is_dirty: Option<bool>, locked: Option<&str>) -> String {
    let dirty_char = match is_dirty {
        Some(true) => '*',
        _ => ' ',
    };
    let lock_char = if locked.is_some() { 'L' } else { ' ' };
    format!("{}{}", dirty_char, lock_char)
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
        assert_eq!(format_status(Some(false), None), "  ");
    }

    #[test]
    fn test_format_status_not_checked_unlocked() {
        assert_eq!(format_status(None, None), "  ");
    }

    #[test]
    fn test_format_status_dirty_unlocked() {
        assert_eq!(format_status(Some(true), None), "* ");
    }

    #[test]
    fn test_format_status_clean_locked_no_reason() {
        assert_eq!(format_status(Some(false), Some("")), " L");
    }

    #[test]
    fn test_format_status_clean_locked_with_reason() {
        assert_eq!(format_status(Some(false), Some("in use")), " L");
    }

    #[test]
    fn test_format_status_not_checked_locked() {
        assert_eq!(format_status(None, Some("")), " L");
    }

    #[test]
    fn test_format_status_dirty_locked() {
        assert_eq!(format_status(Some(true), Some("in use")), "*L");
    }

    #[test]
    fn test_format_status_always_two_chars() {
        // All combinations should produce exactly 2 characters
        for is_dirty in [None, Some(false), Some(true)] {
            for locked in [None, Some(""), Some("reason")] {
                let status = format_status(is_dirty, locked);
                assert_eq!(
                    status.len(),
                    2,
                    "format_status({:?}, {:?}) = {:?} (len {})",
                    is_dirty,
                    locked,
                    status,
                    status.len()
                );
            }
        }
    }
}
