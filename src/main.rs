use agentree::commands::{agent, cd, clean, create, exec, list, remove, shell, shell_init, update};
use agentree::version;
use clap::{Parser, Subcommand};
use std::process;

#[derive(Parser)]
#[command(name = "agentree")]
#[command(about = "Workspace orchestration CLI with pluggable isolation backends", long_about = None)]
#[command(version = version::VERSION)]
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

    /// Open a shell in workspace (auto-creates if needed)
    Shell(shell::ShellArgs),

    /// Output shell initialization code (use with eval)
    ShellInit(shell_init::ShellInitArgs),

    /// Execute a command in workspace (auto-creates if needed, runs on host)
    Exec(exec::ExecArgs),

    /// Start AI agent in workspace (auto-creates if needed)
    Agent(agent::AgentArgs),

    /// Update agentree to latest or specified version
    Update(update::UpdateArgs),
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Cd(args) => cd::execute(args),
        Command::Create(args) => create::execute(args),
        Command::List(args) => list::execute(args),
        Command::Remove(args) => remove::execute(args),
        Command::Clean(args) => clean::execute(args),
        Command::Shell(args) => shell::execute(args),
        Command::ShellInit(args) => shell_init::execute(args),
        Command::Exec(args) => exec::execute(args),
        Command::Agent(args) => agent::execute(args),
        Command::Update(args) => update::execute(args),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
