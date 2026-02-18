use crate::error::Result;
use crate::utils::git::{get_current_branch, path_to_str, run_git_query};
use crate::worktree::{operations, state::WorktreeEntry};
use clap::Parser;

/// Shared worktree filter flags used by `list` and `remove`.
///
/// Flatten this into a command's args with `#[command(flatten)]`.
#[derive(Parser, Debug, Clone, Default)]
pub struct WorktreeFilterArgs {
    /// Only include worktrees whose branch is merged into BASE.
    /// Defaults to the current branch when BASE is omitted.
    #[arg(
        long,
        num_args(0..=1),
        default_missing_value = "HEAD",
        value_name = "BASE",
        conflicts_with = "not_merged"
    )]
    pub merged: Option<String>,

    /// Only include worktrees whose branch is NOT merged into BASE.
    /// Defaults to the current branch when BASE is omitted.
    #[arg(
        long,
        num_args(0..=1),
        default_missing_value = "HEAD",
        value_name = "BASE",
        conflicts_with = "merged"
    )]
    pub not_merged: Option<String>,

    /// Only include locked worktrees.
    #[arg(long = "locked", conflicts_with = "not_locked")]
    pub only_locked: bool,

    /// Only include unlocked worktrees.
    #[arg(long = "not-locked", conflicts_with = "only_locked")]
    pub not_locked: bool,

    /// Only include worktrees with uncommitted changes (implies dirty check).
    /// Conflicts with --no-dirty-check when used with `list`.
    #[arg(long = "dirty", conflicts_with = "only_clean")]
    pub only_dirty: bool,

    /// Only include worktrees with no uncommitted changes (implies dirty check).
    /// Conflicts with --no-dirty-check when used with `list`.
    #[arg(long = "clean", conflicts_with = "only_dirty")]
    pub only_clean: bool,

    /// Only include worktrees whose branch name matches PATTERN.
    /// Supports `*` (any sequence of characters) and `?` (any single character).
    /// Example: --branch "feature/*"
    #[arg(long = "branch", value_name = "PATTERN")]
    pub branch_pattern: Option<String>,

    /// Only include worktrees not modified within the last N days.
    /// Defaults to 30 days when the flag is given without a value.
    /// Example: --stale 7 (show worktrees idle for 7+ days)
    #[arg(long = "stale", num_args(0..=1), default_missing_value = "30", value_name = "DAYS")]
    pub stale_days: Option<u32>,
}

/// Trait for types that can be filtered by `WorktreeFilterArgs`.
/// Implement this for any type that represents a worktree entry.
pub trait WorktreeFilterable {
    fn branch(&self) -> Option<&str>;
    fn locked(&self) -> Option<&str>;
    fn path(&self) -> &std::path::Path;
    fn modified(&self) -> Option<std::time::SystemTime>;
}

impl WorktreeFilterable for WorktreeEntry {
    fn branch(&self) -> Option<&str> {
        self.branch.as_deref()
    }
    fn locked(&self) -> Option<&str> {
        self.locked.as_deref()
    }
    fn path(&self) -> &std::path::Path {
        &self.path
    }
    fn modified(&self) -> Option<std::time::SystemTime> {
        operations::get_last_activity(&self.path)
    }
}

impl WorktreeFilterArgs {
    /// Apply all non-dirty filters in place.
    ///
    /// Dirty/clean filtering is excluded because it requires command-specific
    /// two-phase handling (check then filter).
    pub fn apply<T: WorktreeFilterable>(&self, items: &mut Vec<T>) -> Result<()> {
        if let Some(ref base) = self.merged {
            let base = resolve_head_sentinel(base)?;
            let merged = operations::list_merged_branches(&base)?;
            items.retain(|e| {
                e.branch()
                    .map(|b| merged.contains(&b.to_string()))
                    .unwrap_or(false)
            });
        }
        if let Some(ref base) = self.not_merged {
            let base = resolve_head_sentinel(base)?;
            let merged = operations::list_merged_branches(&base)?;
            items.retain(|e| {
                e.branch()
                    .map(|b| !merged.contains(&b.to_string()))
                    .unwrap_or(false)
            });
        }
        if self.only_locked {
            items.retain(|e| e.locked().is_some());
        }
        if self.not_locked {
            items.retain(|e| e.locked().is_none());
        }
        if let Some(ref pat) = self.branch_pattern {
            items.retain(|e| e.branch().map(|b| glob_match(pat, b)).unwrap_or(false));
        }
        if let Some(days) = self.stale_days {
            let threshold = std::time::Duration::from_secs(u64::from(days) * 86_400);
            items.retain(|e| {
                e.modified()
                    .and_then(|t| t.elapsed().ok())
                    .map(|d| d >= threshold)
                    .unwrap_or(false)
            });
        }
        Ok(())
    }

    /// Returns `true` if any filter flag is set.
    pub fn has_any(&self) -> bool {
        self.merged.is_some()
            || self.not_merged.is_some()
            || self.only_locked
            || self.not_locked
            || self.only_dirty
            || self.only_clean
            || self.branch_pattern.is_some()
            || self.stale_days.is_some()
    }

    /// Returns `true` if a dirty check is required by the active filters.
    pub fn requires_dirty_check(&self) -> bool {
        self.only_dirty || self.only_clean
    }
}

/// Returns `Some(true)` if dirty, `Some(false)` if clean, `None` if the check failed.
pub fn check_worktree_dirty(path: &std::path::Path) -> Option<bool> {
    path_to_str(path, "worktree path")
        .ok()
        .and_then(|p| {
            run_git_query(&["-C", p, "status", "--short"])
                .ok()
                .flatten()
        })
        .map(|output| !output.is_empty())
}

/// Simple glob matching: `*` matches any sequence of characters, `?` matches one character.
/// Case-sensitive (git branch names are case-sensitive).
pub fn glob_match(pattern: &str, text: &str) -> bool {
    fn matches(p: &str, t: &str) -> bool {
        let mut pc = p.chars();
        match pc.next() {
            None => t.is_empty(),
            Some('*') => {
                let p_rest = pc.as_str();
                matches(p_rest, t)
                    || t.chars()
                        .next()
                        .map(|c| matches(p, &t[c.len_utf8()..]))
                        .unwrap_or(false)
            }
            Some('?') => t
                .chars()
                .next()
                .map(|c| matches(pc.as_str(), &t[c.len_utf8()..]))
                .unwrap_or(false),
            Some(a) => {
                let mut tc = t.chars();
                tc.next()
                    .map(|b| a == b && matches(pc.as_str(), tc.as_str()))
                    .unwrap_or(false)
            }
        }
    }
    matches(pattern, text)
}

/// Resolve the `"HEAD"` sentinel to the current branch name.
///
/// Used because clap's `default_missing_value = "HEAD"` is a sentinel for
/// "use current branch when flag is passed with no value".
pub fn resolve_head_sentinel(base: &str) -> Result<String> {
    if base == "HEAD" {
        get_current_branch()
    } else {
        Ok(base.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_head_sentinel_passthrough() {
        // Non-HEAD values pass through unchanged.
        // (Cannot test HEAD without a git repo; integration tests cover that.)
        assert_eq!(resolve_head_sentinel("main").unwrap(), "main");
        assert_eq!(resolve_head_sentinel("origin/main").unwrap(), "origin/main");
    }

    // ─── glob_match ──────────────────────────────────────────────────────────

    #[test]
    fn test_glob_exact_match() {
        assert!(glob_match("main", "main"));
        assert!(!glob_match("main", "master"));
    }

    #[test]
    fn test_glob_star_prefix() {
        assert!(glob_match("feature/*", "feature/auth"));
        assert!(glob_match("feature/*", "feature/auth-v2"));
        assert!(!glob_match("feature/*", "bugfix/auth"));
    }

    #[test]
    fn test_glob_star_matches_empty() {
        assert!(glob_match("feature/*", "feature/"));
        assert!(glob_match("*", ""));
        assert!(glob_match("*", "anything"));
    }

    #[test]
    fn test_glob_star_suffix() {
        assert!(glob_match("*-wip", "feature-wip"));
        assert!(glob_match("*-wip", "my-long-name-wip"));
        assert!(!glob_match("*-wip", "wip")); // no "-wip" suffix
        assert!(!glob_match("*-wip", "wip-done")); // suffix is "-done", not "-wip"
    }

    #[test]
    fn test_glob_question_mark() {
        assert!(glob_match("feat-?", "feat-1"));
        assert!(glob_match("feat-?", "feat-x"));
        assert!(!glob_match("feat-?", "feat-"));
        assert!(!glob_match("feat-?", "feat-10"));
    }

    #[test]
    fn test_glob_multiple_stars() {
        assert!(glob_match("*feat*", "my-feat-branch"));
        assert!(glob_match("*/*", "feature/auth"));
        assert!(!glob_match("*/*", "no-slash"));
    }

    #[test]
    fn test_glob_no_wildcard() {
        assert!(glob_match("exact", "exact"));
        assert!(!glob_match("exact", "exact-extra"));
    }

    #[test]
    fn test_glob_case_sensitive() {
        assert!(!glob_match("Feature/*", "feature/auth"));
        assert!(glob_match("feature/*", "feature/auth"));
    }
}
