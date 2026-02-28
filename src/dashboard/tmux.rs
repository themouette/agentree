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

/// Get the index of the first window in a session.
/// Handles non-default base-index values in user tmux configs.
/// Returns 0 on error (safe fallback for standard configurations).
fn first_window_index(session: &str) -> u8 {
    Command::new("tmux")
        .args(["list-windows", "-t", session, "-F", "#{window_index}"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.lines().next().and_then(|l| l.trim().parse::<u8>().ok()))
        .unwrap_or(0)
}

/// Split the current window horizontally (creates right pane)
pub fn split_horizontal(session: &str) -> Result<()> {
    let window = first_window_index(session);
    let target = format!("{}:{}", session, window);
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
    let window = first_window_index(session);
    let target = format!("{}:{}.{}", session, window, pane);
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
    let window = first_window_index(session);
    let target = format!("{}:{}.{}", session, window, pane);
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
    let window = first_window_index(session);
    let target = format!("{}:{}.{}", session, window, pane);
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

/// Enable focus-events so panes receive FocusIn/FocusOut terminal sequences.
/// Without this, tmux silently drops focus events and TUI focus tracking doesn't work.
/// This sets the global option; it is safe and benign if already enabled.
pub fn enable_focus_events() {
    let _ = Command::new("tmux")
        .args(["set-option", "-g", "focus-events", "on"])
        .output();
}

/// Bind Ctrl+\ (session-scoped) to return focus to pane 0 of the dashboard session
pub fn bind_key_return_to_dashboard(session: &str) {
    let window = first_window_index(session);
    let pane = format!("{}:{}.0", session, window);
    // Bind as a global key (no prefix) for simplicity
    let _ = Command::new("tmux")
        .args(["bind-key", "-n", r"C-\", "select-pane", "-t", &pane])
        .output();
}

/// Canonical tmux session name for an agent workspace
pub fn agent_session_name(branch: &str) -> String {
    // Replace '/' and ':' with '-' — colons cause tmux session:window target ambiguity
    let safe = branch.replace('/', "-").replace(':', "-");
    format!("agentree-{}", safe)
}

/// Canonical tmux session name for a persistent terminal session in a workspace.
pub fn terminal_session_name(branch: &str) -> String {
    let safe = branch.replace('/', "-").replace(':', "-");
    format!("agentree-{}-term", safe)
}

/// Canonical tmux session name for a persistent editor session in a workspace.
pub fn editor_session_name(branch: &str) -> String {
    let safe = branch.replace('/', "-").replace(':', "-");
    format!("agentree-{}-edit", safe)
}

/// List pane IDs in visual (index) order for the first window of a session.
///
/// Returns e.g. `["%0", "%1"]` — the first entry is the left (TUI) pane,
/// the second is the right pane. Uses `#{pane_id}` which is unaffected by
/// `pane-base-index` settings in the user's tmux config.
fn list_pane_ids(session: &str) -> Vec<String> {
    let window = first_window_index(session);
    let target = format!("{}:{}", session, window);
    Command::new("tmux")
        .args(["list-panes", "-t", &target, "-F", "#{pane_id}"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| {
            s.lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Run a command in the right pane of the dashboard session.
///
/// Uses pane IDs (not indices) so it works regardless of `pane-base-index`.
/// If the right pane exists (≥2 panes), respawns it with the new command.
/// If the user closed the right pane, recreates it by splitting $TMUX_PANE
/// (our own pane) and resizes the left pane back to 44 columns.
pub fn run_in_right_pane(session: &str, cmd: &str) -> Result<()> {
    let pane_ids = list_pane_ids(session);

    if pane_ids.len() >= 2 {
        // Right pane exists — respawn it using its pane ID directly
        let right_id = &pane_ids[1];
        let out = Command::new("tmux")
            .args(["respawn-pane", "-k", "-t", right_id, cmd])
            .output()
            .map_err(|e| AgentreeError::TmuxError(format!("respawn-pane failed: {}", e)))?;
        if !out.status.success() {
            return Err(AgentreeError::TmuxError(format!(
                "tmux respawn-pane failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(())
    } else {
        // Right pane was closed — recreate it.
        // Split $TMUX_PANE (our own pane ID) so we always split the correct pane
        // regardless of pane-base-index or how tmux numbers things.
        let split_target = std::env::var("TMUX_PANE").unwrap_or_else(|_| {
            // Fallback: use first pane ID or window target
            pane_ids
                .first()
                .cloned()
                .unwrap_or_else(|| format!("{}:{}", session, first_window_index(session)))
        });

        let out = Command::new("tmux")
            .args(["split-window", "-h", "-d", "-t", &split_target])
            .output()
            .map_err(|e| AgentreeError::TmuxError(format!("split-window failed: {}", e)))?;
        if !out.status.success() {
            return Err(AgentreeError::TmuxError(format!(
                "tmux split-window failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }

        // Re-query panes to get the new right pane ID
        let new_ids = list_pane_ids(session);
        if new_ids.len() < 2 {
            return Err(AgentreeError::TmuxError(
                "split-window did not create a right pane".to_string(),
            ));
        }

        let right_id = &new_ids[1];
        let left_id = &new_ids[0];

        // Run the desired command in the new right pane
        let out = Command::new("tmux")
            .args(["respawn-pane", "-k", "-t", right_id, cmd])
            .output()
            .map_err(|e| AgentreeError::TmuxError(format!("respawn-pane failed: {}", e)))?;
        if !out.status.success() {
            return Err(AgentreeError::TmuxError(format!(
                "tmux respawn-pane (new pane) failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }

        // Restore the left pane to its 44-column width (split halved it)
        let _ = Command::new("tmux")
            .args(["resize-pane", "-x", "44", "-t", left_id])
            .output();

        Ok(())
    }
}

/// Resize the calling pane to 44 columns using $TMUX_PANE.
///
/// Only call this when the right pane (pane 1) exists. When only one pane
/// is present, resize-pane shrinks the entire tmux window, leaving no space
/// for the right pane when it is later recreated.
pub fn resize_self_to_44_cols() {
    if let Ok(pane_id) = std::env::var("TMUX_PANE") {
        let _ = Command::new("tmux")
            .args(["resize-pane", "-x", "44", "-t", &pane_id])
            .output();
    }
}

/// Returns true if the right pane exists in the session (i.e. ≥ 2 panes present).
pub fn right_pane_exists(session: &str) -> bool {
    list_pane_ids(session).len() >= 2
}

/// Returns true if pane 0 in the given session's window 0 has its process exited.
///
/// Uses `tmux list-panes -F '#{pane_dead}'` which outputs "1" for dead panes.
///
/// Assumes pane-base-index 0 (tmux default). Users with pane-base-index 1 will need to adjust.
pub fn is_tui_pane_dead(session: &str) -> bool {
    let window = first_window_index(session);
    let target = format!("{}:{}", session, window);
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

/// Returns true if the named session's first pane process has exited (pane is dead).
///
/// Uses `tmux list-panes -F '#{pane_dead}'` which outputs "1" for dead panes.
/// Returns false on any error — safe default prevents destroying live sessions.
pub fn is_session_pane_dead(session: &str) -> bool {
    Command::new("tmux")
        .args(["list-panes", "-t", session, "-F", "#{pane_dead}"])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .map(|l| l.trim() == "1")
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

/// Ensure a named tmux session exists and its pane process is alive.
///
/// - If session does not exist: creates it running `cmd` in `cwd`.
/// - If session exists but pane is dead: silently respawns pane with `cmd`.
/// - If session exists and alive: no-op (caller will use switch-client to attach).
///
/// Uses the first pane ID from list-panes to avoid pane-base-index assumptions.
pub fn ensure_named_session(session: &str, cmd: &str, cwd: &Path) -> Result<()> {
    if session_exists(session) {
        if is_session_pane_dead(session) {
            // Get the first pane ID to avoid pane-base-index assumptions
            let pane_id = Command::new("tmux")
                .args(["list-panes", "-t", session, "-F", "#{pane_id}"])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|s| s.lines().next().map(|l| l.trim().to_string()))
                .unwrap_or_else(|| format!("{}:0.0", session));

            let status = Command::new("tmux")
                .args(["respawn-pane", "-k", "-t", &pane_id, cmd])
                .status()
                .map_err(|e| {
                    AgentreeError::TmuxError(format!("respawn-pane failed: {}", e))
                })?;
            if !status.success() {
                return Err(AgentreeError::TmuxError(format!(
                    "tmux respawn-pane failed for session '{}'",
                    session
                )));
            }
        }
        // Session alive — caller uses switch-client to attach
    } else {
        create_session(session, cmd, cwd)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_session_name_sanitizes_slash() {
        assert_eq!(agent_session_name("feature/my-auth"), "agentree-feature-my-auth");
    }

    #[test]
    fn test_agent_session_name_sanitizes_colon() {
        assert_eq!(agent_session_name("hotfix:v1.2"), "agentree-hotfix-v1.2");
    }

    #[test]
    fn test_agent_session_name_simple_branch() {
        assert_eq!(agent_session_name("main"), "agentree-main");
    }

    #[test]
    fn test_terminal_session_name() {
        assert_eq!(terminal_session_name("feature/auth"), "agentree-feature-auth-term");
    }

    #[test]
    fn test_editor_session_name() {
        assert_eq!(editor_session_name("feature/auth"), "agentree-feature-auth-edit");
    }
}
