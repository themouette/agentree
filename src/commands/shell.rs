use crate::backend::{Backend, BackendType};
use crate::commands::common::{WorkspaceArgs, WorkspaceContext};
use crate::error::Result;
use clap::Parser;
use console::{style, Term};

#[derive(Parser, Debug)]
pub struct ShellArgs {
    #[command(flatten)]
    pub workspace: WorkspaceArgs,
}

pub fn execute(args: ShellArgs) -> Result<()> {
    // Initialize workspace context (validation, git root discovery, config loading)
    let ctx = WorkspaceContext::init(
        args.workspace.backend.as_deref(),
        args.workspace.worktree_location.as_deref(),
        None,
        None,
    )?;

    // Ensure workspace exists (create or resume) and set up metadata
    let workspace = ctx.ensure_workspace(&args.workspace.branch, args.workspace.base.as_deref())?;

    // Print welcome banner before opening shell
    print_welcome_banner(
        &args.workspace.branch,
        &workspace.path,
        &ctx.config.effective_backend(),
    );

    // Create backend and open shell (respects backend isolation per BACK-07)
    let backend = BackendType::from_kind(ctx.config.effective_backend());
    backend.shell(&workspace.path)?;

    Ok(())
}

/// Print a welcome banner when entering a workspace shell
fn print_welcome_banner(
    branch: &str,
    workspace_path: &std::path::Path,
    backend: &crate::backend::BackendKind,
) {
    let use_colors = Term::stdout().is_term();

    println!();
    if use_colors {
        println!(
            "{}",
            style("╭─────────────────────────────────────────────────╮").cyan()
        );
        println!(
            "{} {}",
            style("│").cyan(),
            style(format!("  Agentree Workspace: {}", branch)).bold()
        );
        println!(
            "{} {}",
            style("│").cyan(),
            style(format!("  Location: {}", workspace_path.display())).dim()
        );
        println!(
            "{} {}",
            style("│").cyan(),
            style(format!("  Backend: {}", backend)).dim()
        );
        println!("{}", style("│").cyan());
        println!(
            "{} {}",
            style("│").cyan(),
            style("  Type 'exit' or press Ctrl+D to return").dim()
        );
        println!(
            "{}",
            style("╰─────────────────────────────────────────────────╯").cyan()
        );
    } else {
        // Fallback for non-TTY environments (no colors)
        println!("+-------------------------------------------------+");
        println!("|  Agentree Workspace: {}", branch);
        println!("|  Location: {}", workspace_path.display());
        println!("|  Backend: {}", backend);
        println!("|");
        println!("|  Type 'exit' or press Ctrl+D to return");
        println!("+-------------------------------------------------+");
    }
    println!();
}
