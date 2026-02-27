pub mod client;
pub mod tmux;
pub mod ui;

use crate::daemon;
use crate::dashboard::client::{try_connect, DaemonClient};
use crate::dashboard::tmux as tmux_util;
use crate::error::{AgentreeError, Result};
use crate::utils::git::get_git_root;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const DASHBOARD_SESSION: &str = "agentree-dashboard";
const DAEMON_START_TIMEOUT: Duration = Duration::from_secs(5);

/// Entry point for `agentree dashboard`
pub fn execute(tui_mode: bool) -> Result<()> {
    let sock_path = daemon::socket_path()
        .ok_or_else(|| AgentreeError::DaemonError("Could not determine home directory".to_string()))?;

    if tui_mode {
        // We are the TUI process running inside the left pane
        let client = DaemonClient::connect(&sock_path)?;
        return ui::run_tui(client);
    }

    // Ensure tmux is installed
    if !tmux_util::is_available() {
        return Err(AgentreeError::TmuxNotFound);
    }

    // Detect repo root for daemon startup
    let repo_root = get_git_root()?
        .ok_or_else(|| AgentreeError::DaemonError("Not inside a git repository".to_string()))?;

    // Ensure the daemon is running
    ensure_daemon(&sock_path, &repo_root)?;

    // Create or reuse the tmux session
    if !tmux_util::session_exists(DASHBOARD_SESSION) {
        // Start session with a placeholder shell; TUI will be loaded via respawn-pane
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".to_string());
        tmux_util::create_session(DASHBOARD_SESSION, &shell, &repo_root)?;

        // Split into two horizontal panes
        tmux_util::split_horizontal(DASHBOARD_SESSION)?;

        // Set left pane to 44 columns
        tmux_util::resize_pane(DASHBOARD_SESSION, 0, 44)?;

        // Start TUI in the left pane
        let agentree_bin = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("agentree"))
            .to_string_lossy()
            .to_string();
        let tui_cmd = format!("{} dashboard --tui", agentree_bin);
        tmux_util::respawn_pane(DASHBOARD_SESSION, 0, &tui_cmd)?;

        // Bind Ctrl+\ to return to left pane
        tmux_util::bind_key_return_to_dashboard(DASHBOARD_SESSION);

        // Select right pane initially (user will navigate to left pane for list)
        tmux_util::select_pane(DASHBOARD_SESSION, 0)?;
    }

    // Attach to the session (replaces current process)
    tmux_util::attach(DASHBOARD_SESSION)
}

/// Ensure the daemon is running, starting it if necessary.
///
/// Progress display is TTY-aware:
/// - Interactive terminal: spinner + "Starting agentree daemon..." (cleared on success)
/// - Non-TTY (piped / CI): prints "Starting agentree daemon..." once, then polls silently
///
/// On timeout, returns DaemonStartFailed with the log file path.
fn ensure_daemon(sock_path: &Path, repo_root: &Path) -> Result<()> {
    if try_connect(sock_path) {
        return Ok(());
    }

    // Determine log path for error messages
    let log_path = crate::daemon::runtime_dir()
        .map(|d| d.join("daemon.log"))
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "~/.agentree/daemon.log".to_string());

    // Spawn the daemon in the background
    // Note: stdout/stderr are intentionally left as null here. The daemon itself
    // redirects its output to daemon.log via init_logging() in commands/daemon.rs.
    let agentree_bin = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("agentree"))
        .to_string_lossy()
        .to_string();

    // TOCTOU note: two simultaneous dashboard invocations may both spawn a daemon.
    // The second daemon will fail to bind the socket; both will connect to the first.
    std::process::Command::new(&agentree_bin)
        .arg("daemon")
        .arg("--repo-root")
        .arg(repo_root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| AgentreeError::DaemonError(format!("Failed to spawn daemon: {}", e)))?;

    // TTY-aware progress display
    let spinner: Option<ProgressBar> = if std::io::stderr().is_terminal() {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .expect("valid template"),
        );
        pb.set_message("Starting agentree daemon...");
        pb.enable_steady_tick(std::time::Duration::from_millis(80));
        Some(pb)
    } else {
        // No TTY: print static line once, then poll silently
        eprintln!("Starting agentree daemon...");
        None
    };

    // Poll for socket (5s timeout, 50ms interval)
    let deadline = Instant::now() + DAEMON_START_TIMEOUT;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
        if try_connect(sock_path) {
            if let Some(pb) = spinner {
                pb.finish_and_clear();
            }
            return Ok(());
        }
    }

    // Timeout: clear spinner, return actionable error
    if let Some(pb) = spinner {
        pb.finish_and_clear();
    }

    Err(AgentreeError::DaemonStartFailed { log_path })
}
