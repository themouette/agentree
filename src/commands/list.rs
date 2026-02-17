use crate::config;
use crate::error::Result;
use crate::utils::git::get_git_root;
use crate::worktree::{metadata::WorktreeMetadata, operations, recovery, validation};
use clap::Parser;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
pub struct ListArgs {
    /// Output format
    #[arg(long, value_enum, default_value = "two-lines")]
    pub format: OutputFormat,

    /// Legacy: equivalent to --format=json (deprecated)
    #[arg(long, conflicts_with = "format")]
    pub json: bool,
}

#[derive(clap::ValueEnum, Clone, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum OutputFormat {
    /// Two-line format with indented paths (default)
    TwoLines,
    /// Traditional table format with relative paths (max-width: 120)
    Table,
    /// Card-style with boxes and full details
    Card,
    /// Machine-readable JSON
    Json,
}

#[derive(Serialize)]
struct WorktreeJson {
    branch: String,
    path: String,
    backend: Option<String>,
    created: Option<String>,
    modified: Option<String>,
}

struct WorktreeInfo {
    branch: String,
    path: PathBuf,
    backend: String,
    created: Option<String>,
    modified: Option<std::time::SystemTime>,
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
        OutputFormat::Json
    } else {
        args.format
    };

    // Get worktrees with metadata
    let worktrees = get_worktrees_with_metadata()?;

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
        OutputFormat::TwoLines => render_two_lines(&worktrees, &repo_root),
        OutputFormat::Table => render_table(&worktrees, &repo_root),
        OutputFormat::Card => render_card(&worktrees),
        OutputFormat::Json => render_json(&worktrees),
    }
}

fn get_worktrees_with_metadata() -> Result<Vec<WorktreeInfo>> {
    // Get worktrees with auto-prune
    let worktrees = recovery::ensure_clean_state()?;

    // Skip the first entry (main repo) and get worktrees with branches
    let worktree_list: Vec<_> = worktrees
        .iter()
        .skip(1)
        .filter(|e| e.branch.is_some())
        .collect();

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
            }
        })
        .collect();

    // Sort by last modified (most recent first)
    worktrees_with_time.sort_by(|a, b| {
        match (a.modified, b.modified) {
            (Some(time_a), Some(time_b)) => time_b.cmp(&time_a),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    Ok(worktrees_with_time)
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
            }
        })
        .collect();

    println!("{}", serde_json::to_string_pretty(&json_output)?);
    Ok(())
}

fn render_two_lines(worktrees: &[WorktreeInfo], _repo_root: &Path) -> Result<()> {
    // Header
    println!(
        "{:<30} {:<12} {:<20}",
        "BRANCH", "BACKEND", "MODIFIED"
    );

    for info in worktrees {
        let modified = info
            .modified
            .map(operations::format_activity)
            .unwrap_or_else(|| "Unknown".to_string());

        // First line: branch, backend, modified
        println!(
            "{:<30} {:<12} {:<20}",
            truncate(&info.branch, 30),
            truncate(&info.backend, 12),
            modified
        );

        // Second line: absolute path with arrow
        println!("  → {}", info.path.display());
    }

    Ok(())
}

fn render_table(worktrees: &[WorktreeInfo], repo_root: &Path) -> Result<()> {
    // Header with 120 max width
    println!(
        "{:<25} {:<45} {:<12} {:<18} {:<15}",
        "BRANCH", "PATH", "BACKEND", "CREATED", "MODIFIED"
    );

    for info in worktrees {
        // Convert to relative path
        let path_display = make_relative_path(&info.path, repo_root);

        let created = info
            .created
            .as_ref()
            .and_then(|c| {
                use chrono::DateTime;
                DateTime::parse_from_rfc3339(c)
                    .ok()
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            })
            .unwrap_or_else(|| "-".to_string());

        let modified = info
            .modified
            .map(operations::format_activity)
            .unwrap_or_else(|| "Unknown".to_string());

        println!(
            "{:<25} {:<45} {:<12} {:<18} {:<15}",
            truncate(&info.branch, 25),
            truncate(&path_display, 45),
            truncate(&info.backend, 12),
            created,
            modified
        );
    }

    Ok(())
}

fn render_card(worktrees: &[WorktreeInfo]) -> Result<()> {
    for (i, info) in worktrees.iter().enumerate() {
        if i > 0 {
            println!(); // Blank line between cards
        }

        let created = info
            .created
            .as_ref()
            .and_then(|c| {
                use chrono::DateTime;
                DateTime::parse_from_rfc3339(c)
                    .ok()
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            })
            .unwrap_or_else(|| "-".to_string());

        let modified = info
            .modified
            .map(operations::format_activity)
            .unwrap_or_else(|| "Unknown".to_string());

        println!("┌─ {}", info.branch);
        println!("│  Path:     {}", info.path.display());
        println!("│  Backend:  {}", info.backend);
        println!("│  Created:  {}", created);
        println!("│  Modified: {}", modified);
        println!("└─");
    }

    Ok(())
}

fn make_relative_path(path: &Path, repo_root: &Path) -> String {
    path.strip_prefix(repo_root)
        .ok()
        .and_then(|p| {
            // If it's in a sibling directory, show as ../dirname/...
            if let Ok(p) = p.strip_prefix("..") {
                Some(format!("../{}", p.display()))
            } else {
                Some(p.display().to_string())
            }
        })
        .unwrap_or_else(|| {
            // Fallback: try to make it relative to parent
            repo_root
                .parent()
                .and_then(|parent| path.strip_prefix(parent).ok())
                .map(|p| format!("../{}", p.display()))
                .unwrap_or_else(|| path.display().to_string())
        })
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

