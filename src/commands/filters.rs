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
    #[arg(long = "locked")]
    pub only_locked: bool,

    /// Only include worktrees with uncommitted changes (implies dirty check).
    /// Conflicts with --no-dirty-check when used with `list`.
    #[arg(long = "dirty")]
    pub only_dirty: bool,
}

impl WorktreeFilterArgs {
    /// Returns `true` if any filter flag is set.
    pub fn has_any(&self) -> bool {
        self.merged.is_some() || self.not_merged.is_some() || self.only_locked || self.only_dirty
    }

    /// Returns `true` if a dirty check is required by the active filters.
    pub fn requires_dirty_check(&self) -> bool {
        self.only_dirty
    }
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
}
