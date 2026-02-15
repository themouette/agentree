use agentree::commands::{cd, clean, create, list, remove};
use clap::{Parser, Subcommand};
use std::process;

#[derive(Parser)]
#[command(name = "agentree")]
#[command(about = "Workspace orchestration CLI with pluggable isolation backends", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print cd command for shell wrapper (use with eval)
    Cd(cd::CdArgs),

    /// Create a worktree for a branch
    Create(create::CreateArgs),

    /// List all worktrees
    List(list::ListArgs),

    /// Remove worktree(s)
    Remove(remove::RemoveArgs),

    /// Clean orphaned worktrees
    Clean(clean::CleanArgs),
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Cd(args) => cd::execute(args),
        Command::Create(args) => create::execute(args),
        Command::List(args) => list::execute(args),
        Command::Remove(args) => remove::execute(args),
        Command::Clean(args) => clean::execute(args),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
