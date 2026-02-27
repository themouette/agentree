use crate::daemon;
use crate::error::{AgentreeError, Result};
use crate::utils::git::get_git_root;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct DaemonArgs {
    /// Git repository root (auto-detected from current directory if not specified)
    #[arg(long, hide = true)]
    pub repo_root: Option<PathBuf>,
}

pub fn execute(args: DaemonArgs) -> Result<()> {
    // Resolve repo root
    let repo_root = if let Some(r) = args.repo_root {
        r
    } else {
        get_git_root()?
            .ok_or_else(|| AgentreeError::Git("Not inside a git repository".to_string()))?
    };

    // Check for a stale PID file and warn if another daemon is likely running
    if let Some(pid_file) = daemon::pid_path() {
        if pid_file.exists() && daemon::is_daemon_running(&pid_file) {
            eprintln!("Warning: another agentree daemon appears to be running.");
            eprintln!("If this is stale, remove: {}", pid_file.display());
            return Ok(());
        }
    }

    // Start the tokio runtime and run the daemon
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| AgentreeError::Git(format!("Failed to create tokio runtime: {}", e)))?;

    runtime.block_on(daemon::run(repo_root))
}
