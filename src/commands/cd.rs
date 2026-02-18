use crate::commands::common::{WorkspaceArgs, WorkspaceContext};
use crate::error::Result;
use crate::utils::progress::ensure_workspace_with_progress;
use crate::worktree::metadata::WorktreeMetadata;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct CdArgs {
    #[command(flatten)]
    pub workspace: WorkspaceArgs,
}

pub fn execute(args: CdArgs) -> Result<()> {
    let ctx = WorkspaceContext::init(
        args.workspace.backend.as_deref(),
        args.workspace.worktree_location.as_deref(),
        None,
        None,
    )?;

    let branch = &args.workspace.branch;
    let base = args.workspace.base.as_deref();

    let result =
        ensure_workspace_with_progress(&ctx.config.worktree, &ctx.repo_root, branch, base)?;

    if result.was_created() {
        let metadata = WorktreeMetadata::new(ctx.config.effective_backend().to_string());
        metadata.save(result.path())?;
        // Inform the user something was created; skip this on plain resume to
        // keep navigation noise-free.
        eprintln!("{}", result.message(branch));
    }

    // Print cd command to stdout for shell eval
    println!("cd {}", shell_escape(&result.path().to_string_lossy()));

    Ok(())
}

/// Escape a string for safe use in shell commands.
/// Uses single quotes and handles embedded single quotes.
fn shell_escape(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    let escaped = s.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_escape_simple() {
        assert_eq!(shell_escape("simple"), "'simple'");
    }

    #[test]
    fn test_shell_escape_with_spaces() {
        assert_eq!(shell_escape("path with spaces"), "'path with spaces'");
    }

    #[test]
    fn test_shell_escape_with_single_quote() {
        assert_eq!(shell_escape("path's test"), "'path'\\''s test'");
    }

    #[test]
    fn test_shell_escape_empty() {
        assert_eq!(shell_escape(""), "''");
    }

    #[test]
    fn test_shell_escape_special_chars() {
        assert_eq!(shell_escape("$PATH & stuff"), "'$PATH & stuff'");
    }
}
