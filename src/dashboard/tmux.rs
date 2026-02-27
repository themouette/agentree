use crate::error::{AgentreeError, Result};
use std::path::Path;
use std::process::Command;

/// Check if tmux is installed and available
pub fn is_available() -> bool {
    which::which("tmux").is_ok()
}

/// Check if a tmux session exists
pub fn session_exists(name: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Create a new detached tmux session running `cmd` in `cwd`
pub fn create_session(name: &str, cmd: &str, cwd: &Path) -> Result<()> {
    let cwd_str = cwd.to_string_lossy();
    let status = Command::new("tmux")
        .args(["new-session", "-d", "-s", name, "-c", &cwd_str, cmd])
        .status()
        .map_err(|e| AgentreeError::TmuxError(format!("Failed to start tmux: {}", e)))?;

    if !status.success() {
        return Err(AgentreeError::TmuxError(format!(
            "tmux new-session failed for session '{}'",
            name
        )));
    }
    Ok(())
}

/// Split the current window horizontally (creates right pane)
pub fn split_horizontal(session: &str) -> Result<()> {
    let target = format!("{}:0", session);
    let status = Command::new("tmux")
        .args(["split-window", "-h", "-t", &target])
        .status()
        .map_err(|e| AgentreeError::TmuxError(format!("split-window failed: {}", e)))?;

    if !status.success() {
        return Err(AgentreeError::TmuxError(
            "tmux split-window failed".to_string(),
        ));
    }
    Ok(())
}

/// Resize pane to a fixed column width
pub fn resize_pane(session: &str, pane: u8, width: u16) -> Result<()> {
    let target = format!("{}:0.{}", session, pane);
    Command::new("tmux")
        .args([
            "resize-pane",
            "-x",
            &width.to_string(),
            "-t",
            &target,
        ])
        .status()
        .map_err(|e| AgentreeError::TmuxError(format!("resize-pane failed: {}", e)))?;
    Ok(())
}

/// Kill existing command in pane and respawn with new command
pub fn respawn_pane(session: &str, pane: u8, cmd: &str) -> Result<()> {
    let target = format!("{}:0.{}", session, pane);
    let status = Command::new("tmux")
        .args(["respawn-pane", "-k", "-t", &target, cmd])
        .status()
        .map_err(|e| AgentreeError::TmuxError(format!("respawn-pane failed: {}", e)))?;

    if !status.success() {
        return Err(AgentreeError::TmuxError(format!(
            "tmux respawn-pane failed for pane {}",
            pane
        )));
    }
    Ok(())
}

/// Select (focus) a pane
pub fn select_pane(session: &str, pane: u8) -> Result<()> {
    let target = format!("{}:0.{}", session, pane);
    Command::new("tmux")
        .args(["select-pane", "-t", &target])
        .status()
        .map_err(|e| AgentreeError::TmuxError(format!("select-pane failed: {}", e)))?;
    Ok(())
}

/// Attach to a tmux session (replaces the current process via exec)
pub fn attach(session: &str) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = Command::new("tmux")
            .arg("attach")
            .arg("-t")
            .arg(session)
            .exec();
        // exec() only returns on error
        Err(AgentreeError::TmuxError(format!(
            "Failed to exec tmux attach: {}",
            err
        )))
    }
    #[cfg(not(unix))]
    Err(AgentreeError::TmuxNotFound)
}

/// Bind Ctrl+\ (session-scoped) to return focus to pane 0 of the dashboard session
pub fn bind_key_return_to_dashboard(session: &str) {
    let pane = format!("{}:0.0", session);
    // Bind as a global key (no prefix) for simplicity
    let _ = Command::new("tmux")
        .args(["bind-key", "-n", r"C-\", "select-pane", "-t", &pane])
        .output();
}

/// Canonical tmux session name for an agent workspace
pub fn agent_session_name(branch: &str) -> String {
    // Replace '/' with '-' to get a valid tmux session name
    let safe = branch.replace('/', "-");
    format!("agentree:{}", safe)
}

/// Returns true if pane 0 in the given session's window 0 has its process exited.
///
/// Uses `tmux list-panes -F '#{pane_dead}'` which outputs "1" for dead panes.
///
/// Assumes pane-base-index 0 (tmux default). Users with pane-base-index 1 will need to adjust.
pub fn is_tui_pane_dead(session: &str) -> bool {
    let target = format!("{}:0", session);
    let output = Command::new("tmux")
        .args(["list-panes", "-t", &target, "-F", "#{pane_dead}"])
        .output();
    output
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next() // pane 0 is the first line
                .map(|l| l.trim() == "1")
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

/// Ensure a named tmux session exists for an agent in the given worktree.
/// If the session already exists, does nothing.
pub fn ensure_agent_session(branch: &str, worktree_path: &Path, agent_cmd: &str) -> Result<()> {
    let session = agent_session_name(branch);
    if !session_exists(&session) {
        create_session(&session, agent_cmd, worktree_path)?;
    }
    Ok(())
}
