pub mod logging;
pub mod protocol;
pub mod state;
pub mod watcher;

use crate::daemon::protocol::{Request, Response};
use crate::daemon::state::DaemonState;
use crate::error::{AgentreeError, Result};
use notify::RecommendedWatcher;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::net::UnixListener;
use tokio::time::{interval, Duration};

/// Return the path to the agentree runtime directory (`~/.agentree/`)
pub fn runtime_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".agentree"))
}

/// Path to the daemon Unix socket
pub fn socket_path() -> Option<PathBuf> {
    runtime_dir().map(|d| d.join("daemon.sock"))
}

/// Path to the daemon PID file
pub fn pid_path() -> Option<PathBuf> {
    runtime_dir().map(|d| d.join("daemon.pid"))
}

/// Run the daemon (blocking — consumes the calling thread via tokio runtime).
pub async fn run(repo_root: PathBuf) -> Result<()> {
    let runtime_dir = runtime_dir()
        .ok_or_else(|| AgentreeError::DaemonError("Could not determine home directory".to_string()))?;

    std::fs::create_dir_all(&runtime_dir).map_err(AgentreeError::Io)?;

    // Write PID file
    let pid = std::process::id();
    let pid_file = runtime_dir.join("daemon.pid");
    std::fs::write(&pid_file, pid.to_string()).map_err(AgentreeError::Io)?;

    let sock_path = runtime_dir.join("daemon.sock");

    // Remove stale socket if present
    if sock_path.exists() {
        std::fs::remove_file(&sock_path).map_err(AgentreeError::Io)?;
    }

    // Build shared state
    let state = Arc::new(DaemonState::new(repo_root));
    let _ = state.refresh_all();

    // Start file watcher
    let (watcher_tx, mut watcher_rx) = tokio::sync::mpsc::channel::<PathBuf>(256);
    let initial_watch_paths = state.get_all_agentree_paths();
    let paths_to_watch: Vec<PathBuf> = initial_watch_paths
        .into_iter()
        .map(|(_, p)| p)
        .collect();

    let raw_watcher = watcher::start_watcher(paths_to_watch, watcher_tx.clone())
        .map_err(|e| AgentreeError::DaemonError(format!("File watcher error: {}", e)))?;
    let shared_watcher: Arc<Mutex<RecommendedWatcher>> = Arc::new(Mutex::new(raw_watcher));

    // Periodic re-scan task (every 30s to pick up new worktrees)
    let state_rescan = Arc::clone(&state);
    let watcher_rescan = Arc::clone(&shared_watcher);
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(30));
        tick.tick().await; // skip first immediate tick
        loop {
            tick.tick().await;
            let _ = state_rescan.refresh_all();
            // Add any new worktree .agentree/ paths to the file watcher
            let new_paths: Vec<PathBuf> = state_rescan
                .get_all_agentree_paths()
                .into_iter()
                .map(|(_, p)| p)
                .collect();
            if let Ok(mut w) = watcher_rescan.lock() {
                watcher::add_watch_paths(&mut w, &new_paths);
            }
        }
    });

    // File-change handler task
    let state_watch = Arc::clone(&state);
    tokio::spawn(async move {
        while let Some(changed_path) = watcher_rx.recv().await {
            if let Some(branch) = state_watch.find_branch_for_path(&changed_path) {
                state_watch.update_workspace(&branch);
            }
        }
    });

    // Bind Unix socket and accept connections
    let listener = UnixListener::bind(&sock_path)
        .map_err(|e| AgentreeError::DaemonError(format!("Failed to bind socket: {}", e)))?;

    eprintln!("agentree daemon listening on {}", sock_path.display());

    loop {
        let (stream, _) = listener
            .accept()
            .await
            .map_err(|e| AgentreeError::DaemonError(format!("Accept error: {}", e)))?;

        let state_conn = Arc::clone(&state);
        tokio::spawn(async move {
            handle_connection(stream, state_conn).await;
        });
    }
}

async fn handle_connection(stream: tokio::net::UnixStream, state: Arc<DaemonState>) {
    // We convert to std stream for synchronous line I/O. This is acceptable because
    // the protocol is one-shot (one request → one response) and all state operations
    // are in-memory. Blocking the task briefly is not a concern here.
    let std_stream = match stream.into_std() {
        Ok(s) => s,
        Err(_) => return,
    };

    let mut write_stream = match std_stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };

    // One-shot protocol: read exactly one request, send one response, close.
    let mut reader = BufReader::new(&std_stream);
    let mut line = String::new();

    if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
        return;
    }

    let request: Request = match serde_json::from_str(line.trim()) {
        Ok(r) => r,
        Err(e) => {
            let response = Response::Err(format!("Invalid request: {}", e));
            let _ = write_response(&mut write_stream, &response);
            return;
        }
    };

    let response = handle_request(request, &state);
    let _ = write_response(&mut write_stream, &response);
}

fn handle_request(request: Request, state: &DaemonState) -> Response {
    match request {
        Request::List => Response::Workspaces(state.snapshot()),
        Request::ClearAttention { branch } => match state.clear_attention(&branch) {
            Ok(()) => Response::Ok,
            Err(e) => Response::Err(e.to_string()),
        },
    }
}

fn write_response(stream: &mut impl Write, response: &Response) -> std::io::Result<()> {
    let json = serde_json::to_string(response)
        .unwrap_or_else(|_| r#"{"Err":"serialize"}"#.into());
    stream.write_all(json.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()
}

/// Check if a daemon is already running by reading the PID file.
///
/// Returns `true` if the process identified by the PID file is alive.
/// `kill -0` checks process existence without sending a signal.
pub fn is_daemon_running(pid_file: &Path) -> bool {
    if let Ok(content) = std::fs::read_to_string(pid_file) {
        if let Ok(pid) = content.trim().parse::<u32>() {
            #[cfg(unix)]
            {
                let status = std::process::Command::new("kill")
                    .args(["-0", &pid.to_string()])
                    .output();
                return status.map(|o| o.status.success()).unwrap_or(false);
            }
            #[cfg(not(unix))]
            {
                let _ = pid;
                return false;
            }
        }
    }
    false
}
