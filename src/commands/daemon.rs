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

    /// Check if daemon is running and exit (exit code 0 = running, 1 = not running)
    #[arg(long)]
    pub status: bool,
}

pub fn execute(args: DaemonArgs) -> Result<()> {
    if args.status {
        return execute_status();
    }

    // Resolve repo root
    let repo_root = if let Some(r) = args.repo_root {
        r
    } else {
        get_git_root()?
            .ok_or_else(|| AgentreeError::Git("Not inside a git repository".to_string()))?
    };

    let log_path = daemon::runtime_dir()
        .map(|d| d.join("daemon.log"))
        .unwrap_or_else(|| PathBuf::from("~/.agentree/daemon.log"));

    // Handle PID file: live process = refuse, stale = remove silently
    if let Some(pid_file) = daemon::pid_path() {
        if pid_file.exists() {
            if daemon::is_daemon_running(&pid_file) {
                eprintln!("Daemon already running. Log: {}", log_path.display());
                return Ok(());
            } else {
                // Stale PID file: remove silently and continue startup
                let _ = std::fs::remove_file(&pid_file);
            }
        }
    }

    // Initialize logging BEFORE starting the tokio runtime
    // TTY check: if stderr is a terminal (developer running interactively), log to stderr.
    // Otherwise (spawned by ensure_daemon(), no TTY), log to file.
    use std::io::IsTerminal;
    let _logging_guard = if std::io::stderr().is_terminal() {
        // Foreground / developer mode: log to stderr only
        use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};
        tracing_subscriber::registry()
            .with(
                fmt::layer()
                    .with_ansi(true)
                    .with_target(false)
                    .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339()),
            )
            .init();
        eprintln!("Daemon starting. PID: {}", std::process::id());
        eprintln!("Log file: {}", log_path.display());
        None
    } else {
        // Daemonized mode: log to rolling file
        match daemon::logging::init_logging(&log_path) {
            Ok(guard) => Some(guard),
            Err(e) => {
                eprintln!(
                    "Warning: could not initialize log file ({}). Continuing without logging.",
                    e
                );
                None
            }
        }
    };

    // Start the tokio runtime and run the daemon
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| AgentreeError::Git(format!("Failed to create tokio runtime: {}", e)))?;

    runtime.block_on(daemon::run(repo_root))
}

fn execute_status() -> Result<()> {
    let pid_file = daemon::pid_path().ok_or_else(|| {
        AgentreeError::DaemonError("Could not determine home directory".to_string())
    })?;
    let log_path = daemon::runtime_dir()
        .map(|d| d.join("daemon.log"))
        .unwrap_or_else(|| PathBuf::from("~/.agentree/daemon.log"));

    if pid_file.exists() && daemon::is_daemon_running(&pid_file) {
        let pid = std::fs::read_to_string(&pid_file)
            .unwrap_or_default()
            .trim()
            .to_string();
        println!("Daemon running (PID {})", pid);
        std::process::exit(0);
    } else {
        println!("Daemon not running");
        println!("Log file: {}", log_path.display());
        std::process::exit(1);
    }
}
