use crate::error::Result;
use crate::utils::git::get_current_branch;
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

impl WorktreeFilterArgs {
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

/// Simple glob matching: `*` matches any sequence of characters, `?` matches one character.
/// Case-sensitive (git branch names are case-sensitive).
pub fn glob_match(pattern: &str, text: &str) -> bool {
    fn matches(p: &[char], t: &[char]) -> bool {
        match (p, t) {
            ([], []) => true,
            ([], _) => false,
            (['*', rest @ ..], _) => {
                // Star matches zero or more chars: try consuming zero (match rest) or advancing one
                matches(rest, t) || (!t.is_empty() && matches(p, &t[1..]))
            }
            (['?', p_rest @ ..], [_, t_rest @ ..]) => matches(p_rest, t_rest),
            (['?', ..], []) => false,
            ([pc, p_rest @ ..], [tc, t_rest @ ..]) if pc == tc => matches(p_rest, t_rest),
            _ => false,
        }
    }

    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    matches(&p, &t)
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
