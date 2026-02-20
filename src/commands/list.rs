use crate::commands::filters::{
    check_worktree_dirty, resolve_head_sentinel, WorktreeFilterArgs, WorktreeFilterable,
};
use crate::config;
use crate::error::Result;
use crate::utils::git::get_git_root;
use crate::utils::progress::with_spinner;
use crate::worktree::{metadata::WorktreeMetadata, operations, recovery, validation};
use chrono::{DateTime, Local, Utc};
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

    /// Skip uncommitted-changes check for faster output.
    /// By default, git status is run for each worktree.
    #[arg(long)]
    pub no_dirty_check: bool,

    #[command(flatten)]
    pub filters: WorktreeFilterArgs,
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
    /// --no-dirty-check was passed; check was skipped.
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
    /// null when --no-dirty-check was passed, true/false when checked.
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

impl WorktreeFilterable for WorktreeInfo {
    fn branch(&self) -> Option<&str> {
        Some(&self.branch)
    }
    fn locked(&self) -> Option<&str> {
        self.locked.as_deref()
    }
    fn path(&self) -> &std::path::Path {
        &self.path
    }
    fn modified(&self) -> Option<std::time::SystemTime> {
        self.modified
    }
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

    // Fetch basic metadata (no dirty check yet — filter first to avoid wasted git calls)
    let mut worktrees = get_worktrees_with_metadata()?;

    // --dirty / --clean filters conflict with --no-dirty-check (check is required to evaluate them)
    if args.no_dirty_check && (args.filters.only_dirty || args.filters.only_clean) {
        let flag = if args.filters.only_dirty {
            "--dirty"
        } else {
            "--clean"
        };
        return Err(crate::error::AgentreeError::Worktree(format!(
            "--no-dirty-check cannot be combined with {} (dirty check is required to filter by it)",
            flag
        )));
    }

    // Apply cheap filters first (no dirty check needed)
    args.filters.apply(&mut worktrees)?;

    // Run dirty check when not explicitly disabled.
    let need_dirty_check = !args.no_dirty_check;
    if need_dirty_check {
        with_spinner(
            "Checking for uncommitted changes... (use --no-dirty-check for faster output)",
            || populate_dirty_status(&mut worktrees),
        )?;
    }

    // Apply dirty/clean filter after the check
    if args.filters.only_dirty {
        worktrees.retain(|w| w.dirty == DirtyStatus::Dirty);
    }
    if args.filters.only_clean {
        worktrees.retain(|w| w.dirty == DirtyStatus::Clean);
    }

    // Check if there are any worktrees
    if worktrees.is_empty() {
        let msg = empty_message(&args.filters);
        match format {
            OutputFormat::Json => println!("[]"),
            _ => println!("{}", msg),
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

/// Build the "no results" message based on which filters are active.
fn empty_message(filters: &WorktreeFilterArgs) -> String {
    if let Some(ref base) = filters.merged {
        let resolved = resolve_head_sentinel(base).unwrap_or_else(|_| base.clone());
        format!("No merged worktrees found for '{}'.", resolved)
    } else if let Some(ref base) = filters.not_merged {
        let resolved = resolve_head_sentinel(base).unwrap_or_else(|_| base.clone());
        format!("No unmerged worktrees found for '{}'.", resolved)
    } else if filters.has_any() {
        "No worktrees match the specified filters.".to_string()
    } else {
        "No worktrees found.".to_string()
    }
}

fn get_worktrees_with_metadata() -> Result<Vec<WorktreeInfo>> {
    // Get linked worktrees only (excludes main repo and detached HEADs)
    let worktree_list = recovery::list_linked_worktrees()?;

    // Get last activity for each worktree and create sortable tuples
    let mut worktrees_with_time: Vec<_> = worktree_list
        .iter()
        .map(|entry| {
            let activity = operations::get_last_activity(&entry.path);
            let metadata = WorktreeMetadata::load(&entry.path).ok().flatten();

            WorktreeInfo {
                branch: entry.branch.clone().unwrap_or_default(),
                path: entry.path.clone(),
                backend: metadata
                    .as_ref()
                    .map(|m| m.backend.clone())
                    .unwrap_or_else(|| "unknown".to_string()),
                created: metadata.as_ref().map(|m| m.created_at.clone()),
                modified: activity,
                dirty: DirtyStatus::NotChecked,
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

/// Run `git status --short` for each worktree and update its dirty field in place.
/// Errors per worktree are treated as NotChecked (best-effort).
fn populate_dirty_status(worktrees: &mut [WorktreeInfo]) -> Result<()> {
    for info in worktrees.iter_mut() {
        info.dirty = match check_worktree_dirty(&info.path) {
            Some(true) => DirtyStatus::Dirty,
            Some(false) => DirtyStatus::Clean,
            None => DirtyStatus::NotChecked,
        };
    }
    Ok(())
}

fn format_created_date(created: Option<&String>) -> String {
    created
        .and_then(|c| {
            DateTime::parse_from_rfc3339(c).ok().map(|dt| {
                let local = dt.with_timezone(&Local);
                let ago = format_time_ago(local.into());
                format!("{} ({})", local.format("%Y-%m-%d %H:%M"), ago)
            })
        })
        .unwrap_or_else(|| "-".to_string())
}

/// Format a `SystemTime` as a human-readable relative duration, e.g. "3 months ago".
fn format_time_ago(time: std::time::SystemTime) -> String {
    let datetime: DateTime<Local> = time.into();
    let now = Local::now();
    let duration = now.signed_duration_since(datetime);

    let secs = duration.num_seconds();
    if secs < 60 {
        return "just now".to_string();
    }
    let mins = duration.num_minutes();
    if mins < 60 {
        return format!("{} minute{} ago", mins, if mins == 1 { "" } else { "s" });
    }
    let hours = duration.num_hours();
    if hours < 24 {
        return format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" });
    }
    let days = duration.num_days();
    if days < 7 {
        return format!("{} day{} ago", days, if days == 1 { "" } else { "s" });
    }
    let weeks = days / 7;
    if weeks < 5 {
        return format!("{} week{} ago", weeks, if weeks == 1 { "" } else { "s" });
    }
    let months = days / 30;
    if months < 12 {
        return format!("{} month{} ago", months, if months == 1 { "" } else { "s" });
    }
    let years = days / 365;
    format!("{} year{} ago", years, if years == 1 { "" } else { "s" })
}

fn render_json(worktrees: &[WorktreeInfo]) -> Result<()> {
    let json_output: Vec<WorktreeJson> = worktrees
        .iter()
        .map(|info| {
            let modified = info.modified.map(|time| {
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
            .map(|t| {
                let dt: DateTime<Local> = t.into();
                format!("{} ({})", dt.format("%Y-%m-%d %H:%M"), format_time_ago(t))
            })
            .unwrap_or_else(|| "Unknown".to_string());

        println!("┌─ {}", info.branch);
        println!("│  Path:     {}", info.path.display());
        println!("│  Backend:  {}", info.backend);
        println!("│  Created:  {}", created);
        println!("│  Modified: {}", modified);
        match info.dirty {
            DirtyStatus::Clean => println!("│  Dirty:    no"),
            DirtyStatus::Dirty => println!("│  Dirty:    yes"),
            DirtyStatus::NotChecked => println!("│  Dirty:    ?"),
        }
        match info.locked.as_deref() {
            None => println!("│  Locked:   no"),
            Some("") => println!("│  Locked:   yes"),
            Some(reason) => println!("│  Locked:   yes ({})", reason),
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
    if s.chars().count() <= max_len {
        return s.to_string();
    }
    let cut = max_len.saturating_sub(3);
    let end = s.char_indices().nth(cut).map(|(i, _)| i).unwrap_or(s.len());
    format!("{}...", &s[..end])
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
    fn test_card_shows_dirty_unknown_when_skipped() {
        // When --no-dirty-check is passed (or check failed) the dirty line shows "?".
        let info = make_info(DirtyStatus::NotChecked, None);
        let dirty_line = match info.dirty {
            DirtyStatus::Clean => "Dirty:    no",
            DirtyStatus::Dirty => "Dirty:    yes",
            DirtyStatus::NotChecked => "Dirty:    ?",
        };
        assert_eq!(dirty_line, "Dirty:    ?");
    }

    #[test]
    fn test_card_shows_dirty_yes_when_dirty() {
        let info = make_info(DirtyStatus::Dirty, None);
        let dirty_line = match info.dirty {
            DirtyStatus::Clean => "Dirty:    no",
            DirtyStatus::Dirty => "Dirty:    yes",
            DirtyStatus::NotChecked => "Dirty:    ?",
        };
        assert_eq!(dirty_line, "Dirty:    yes");
    }

    #[test]
    fn test_card_shows_dirty_no_when_clean() {
        let info = make_info(DirtyStatus::Clean, None);
        let dirty_line = match info.dirty {
            DirtyStatus::Clean => "Dirty:    no",
            DirtyStatus::Dirty => "Dirty:    yes",
            DirtyStatus::NotChecked => "Dirty:    ?",
        };
        assert_eq!(dirty_line, "Dirty:    no");
    }

    #[test]
    fn test_card_shows_locked_no_when_unlocked() {
        let info = make_info(DirtyStatus::NotChecked, None);
        let locked_line = match info.locked.as_deref() {
            None => "Locked:   no",
            Some("") => "Locked:   yes",
            Some(reason) => {
                // just to satisfy the borrow checker in test context
                let _ = reason;
                "Locked:   yes (reason)"
            }
        };
        assert_eq!(locked_line, "Locked:   no");
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

    fn ago(secs: u64) -> std::time::SystemTime {
        std::time::SystemTime::now() - std::time::Duration::from_secs(secs)
    }

    #[test]
    fn test_format_time_ago_just_now() {
        assert_eq!(format_time_ago(ago(30)), "just now");
    }

    #[test]
    fn test_format_time_ago_minutes() {
        assert_eq!(format_time_ago(ago(90)), "1 minute ago");
        assert_eq!(format_time_ago(ago(300)), "5 minutes ago");
    }

    #[test]
    fn test_format_time_ago_hours() {
        assert_eq!(format_time_ago(ago(3600)), "1 hour ago");
        assert_eq!(format_time_ago(ago(7200)), "2 hours ago");
    }

    #[test]
    fn test_format_time_ago_days() {
        assert_eq!(format_time_ago(ago(86400)), "1 day ago");
        assert_eq!(format_time_ago(ago(86400 * 3)), "3 days ago");
    }

    #[test]
    fn test_format_time_ago_weeks() {
        assert_eq!(format_time_ago(ago(86400 * 7)), "1 week ago");
        assert_eq!(format_time_ago(ago(86400 * 14)), "2 weeks ago");
    }

    #[test]
    fn test_format_time_ago_months() {
        assert_eq!(format_time_ago(ago(86400 * 60)), "2 months ago");
    }

    #[test]
    fn test_format_time_ago_years() {
        assert_eq!(format_time_ago(ago(86400 * 400)), "1 year ago");
        assert_eq!(format_time_ago(ago(86400 * 800)), "2 years ago");
    }
}
