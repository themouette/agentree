use agentree::commands::{agent, cd, clean, create, exec, list, remove, shell, shell_init, update};
use agentree::version;
use clap::{Parser, Subcommand};
use std::process;

#[derive(Parser)]
#[command(name = "agentree")]
#[command(about = "Workspace orchestration CLI with pluggable isolation backends", long_about = None)]
#[command(version = version::VERSION)]
#[command(after_help = "\
AGENT SHORTCUTS:
    agentree claude <branch> [flags]     Equivalent to: agentree agent <branch> --agent claude
    agentree opencode <branch> [flags]   Equivalent to: agentree agent <branch> --agent opencode

COMMAND ALIASES:
    ls      List all worktrees (alias for 'list')
    rm      Remove worktree(s) (alias for 'remove')

EXAMPLES:
    agentree claude feature              Start Claude in feature branch workspace
    agentree opencode hotfix main        Start OpenCode in hotfix branch (from main)
    agentree ls --json                   List worktrees in JSON format
    agentree rm --merged main            Remove all merged worktrees
")]
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
    #[command(alias = "ls")]
    List(list::ListArgs),

    /// Remove worktree(s)
    #[command(alias = "rm")]
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
    // Collect command-line arguments
    let mut args: Vec<String> = std::env::args().collect();

    // Convenience command routing: agentree <agent-name> <branch> [flags]
    // Transforms to: agentree agent <branch> --agent <agent-name> [flags]
    // Supported agent shortcuts: claude, opencode
    if args.len() >= 2 {
        let convenience_agents = ["claude", "opencode"];

        if convenience_agents.contains(&args[1].as_str()) {
            let agent_name = args[1].clone();
            args.remove(1); // Remove agent name from position 1

            // Insert "agent" subcommand at position 1
            args.insert(1, "agent".to_string());

            // Find the position to insert --agent flag (before any flag that starts with -)
            // or at the end if no flags present
            let flag_pos = args
                .iter()
                .position(|a| a.starts_with('-'))
                .unwrap_or(args.len());

            args.insert(flag_pos, agent_name);
            args.insert(flag_pos, "--agent".to_string());
        }
    }

    let cli = Cli::parse_from(args);

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
