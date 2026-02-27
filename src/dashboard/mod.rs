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

    // Check if already inside a tmux session and warn the user
    if std::env::var("TMUX").is_ok() {
        eprint!("You are already inside tmux. Nest? [y/N] ");
        use std::io::BufRead;
        let mut line = String::new();
        let stdin = std::io::stdin();
        stdin.lock().read_line(&mut line).ok();
        let answer = line.trim().to_lowercase();
        if answer != "y" && answer != "yes" {
            return Ok(()); // User said no — exit cleanly
        }
        // User said yes — proceed with nesting
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

    // Compute the agentree binary path once (used in both session paths)
    let agentree_bin = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("agentree"))
        .to_string_lossy()
        .to_string();
    let tui_cmd = format!("{} dashboard --tui", agentree_bin);

    // DMN-04: handle_request() in daemon/mod.rs: Request::List => Response::Workspaces(state.snapshot())
    // DMN-05: watcher.rs watches .agentree/ dir; fires on status.json changes → state.update_workspace()
    // DMN-06: same watcher fires on attention.md changes → state.update_workspace() → read_attention()
    // DMN-07: daemon/mod.rs lines 79-93: tokio::time::interval(30s) loop → state.refresh_all()
    // All four are already implemented. No code changes needed for DMN-04..07.

    if tmux_util::session_exists(DASHBOARD_SESSION) {
        // Session exists — check if TUI pane (pane 0) is dead and relaunch if needed
        if tmux_util::is_tui_pane_dead(DASHBOARD_SESSION) {
            tmux_util::respawn_pane(DASHBOARD_SESSION, 0, &tui_cmd)?;
        }
        // Silently attach (no output per CONTEXT.md design)
        return tmux_util::attach(DASHBOARD_SESSION);
    }

    // Session does not exist — create it
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".to_string());
    tmux_util::create_session(DASHBOARD_SESSION, &shell, &repo_root)?;

    // Enable focus-events so the TUI receives FocusLost/FocusGained when the
    // user switches panes. tmux disables this by default.
    tmux_util::enable_focus_events();

    // Split into two horizontal panes
    tmux_util::split_horizontal(DASHBOARD_SESSION)?;

    // Note: resize_pane(0, 44) is NOT called here — the session is still detached
    // so tmux doesn't know the real terminal width. The TUI resizes itself to 44 cols
    // on the first Event::Resize it receives (which fires when the client attaches).

    // Start TUI in the left pane
    tmux_util::respawn_pane(DASHBOARD_SESSION, 0, &tui_cmd)?;

    // Bind Ctrl+\ to return to left pane
    tmux_util::bind_key_return_to_dashboard(DASHBOARD_SESSION);

    // Select left pane initially
    tmux_util::select_pane(DASHBOARD_SESSION, 0)?;

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
