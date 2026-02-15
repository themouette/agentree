use crate::error::Result;
use crate::utils::git::get_git_root;
use crate::worktree::{operations, recovery, validation};
use clap::Parser;
use serde::Serialize;

#[derive(Parser, Debug)]
pub struct ListArgs {
    /// Output in JSON format
    #[arg(long)]
    pub json: bool,
}

#[derive(Serialize)]
struct WorktreeJson {
    branch: String,
    path: String,
    modified: Option<String>,
}

pub fn execute(args: ListArgs) -> Result<()> {
    // Check git version
    validation::check_git_version()?;

    // Find repository root
    let _repo_root = get_git_root()?.ok_or_else(|| {
        crate::error::AgentreeError::Worktree(
            "Not in a git repository. Run this command from inside a git repository.".to_string(),
        )
    })?;

    // Get worktrees with auto-prune
    let worktrees = recovery::ensure_clean_state()?;

    // Skip the first entry (main repo) and get worktrees with branches
    let worktree_list: Vec<_> = worktrees
        .iter()
        .skip(1)
        .filter(|e| e.branch.is_some())
        .collect();

    // Check if there are any worktrees
    if worktree_list.is_empty() {
        if !args.json {
            println!("No worktrees found.");
        } else {
            println!("[]");
        }
        return Ok(());
    }

    // Get last activity for each worktree and create sortable tuples
    let mut worktrees_with_time: Vec<_> = worktree_list
        .iter()
        .map(|entry| {
            let activity = operations::get_last_activity(&entry.path);
            (*entry, activity)
        })
        .collect();

    // Sort by last modified (most recent first)
    worktrees_with_time.sort_by(|a, b| {
        match (a.1, b.1) {
            (Some(time_a), Some(time_b)) => time_b.cmp(&time_a), // Reverse order for most recent first
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    if args.json {
        // JSON output
        let json_output: Vec<WorktreeJson> = worktrees_with_time
            .iter()
            .map(|(entry, activity)| {
                let modified = activity.map(|time| {
                    use chrono::{DateTime, Utc};
                    let datetime: DateTime<Utc> = time.into();
                    datetime.to_rfc3339()
                });

                WorktreeJson {
                    branch: entry.branch.clone().unwrap_or_default(),
                    path: entry.path.display().to_string(),
                    modified,
                }
            })
            .collect();

        println!("{}", serde_json::to_string_pretty(&json_output)?);
    } else {
        // Table output
        println!("{:<20} {:<40} {:<16}", "BRANCH", "PATH", "MODIFIED");

        for (entry, activity) in worktrees_with_time {
            let branch = entry.branch.as_ref().unwrap();
            let path = entry.path.display().to_string();
            let modified = activity
                .map(operations::format_activity)
                .unwrap_or_else(|| "Unknown".to_string());

            // Truncate long values
            let branch_display = if branch.len() > 20 {
                format!("{}...", &branch[..17])
            } else {
                branch.clone()
            };

            let path_display = if path.len() > 40 {
                format!("{}...", &path[..37])
            } else {
                path
            };

            println!("{:<20} {:<40} {:<16}", branch_display, path_display, modified);
        }
    }

    Ok(())
}
