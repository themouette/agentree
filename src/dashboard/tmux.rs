use crate::error::{AgentreeError, Result};
use std::path::Path;
use std::process::Command;

/// Fixed column width of the TUI (left) pane in the dashboard layout.
pub const TUI_PANE_WIDTH: u16 = 44;

/// Percentage of terminal width the TUI pane expands to when focused.
pub const TUI_PANE_WIDTH_PERCENT: u8 = 50;

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

/// Kill a tmux session by name. Returns Ok(()) whether or not the session existed.
///
/// `tmux kill-session` exits 1 when the session does not exist — that is treated
/// as success here because the caller's goal (no running session) is already met.
pub fn kill_session(session: &str) -> Result<()> {
    let _status = Command::new("tmux")
        .args(["kill-session", "-t", session])
        .status()
        .map_err(AgentreeError::Io)?;
    // Intentionally ignore exit code — non-zero means session didn't exist, which is fine.
    Ok(())
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
        .args(["resize-pane", "-x", &width.to_string(), "-t", &target])
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

/// Hide the tmux status bar for the dashboard session.
/// Users should not see the tmux window list — these are implementation details.
pub fn disable_status_bar(session: &str) {
    let _ = Command::new("tmux")
        .args(["set-option", "-t", session, "status", "off"])
        .output();
}

/// Bind Ctrl+\ to return focus to pane 0 of the dashboard session.
///
/// The binding is technically global (tmux has no session-scoped key bindings),
/// but it is a silent no-op in other sessions because it checks the current
/// session name before acting. A `session-closed` hook auto-unbinds the key
/// when the dashboard session is killed.
pub fn bind_key_return_to_dashboard(session: &str) {
    let window = first_window_index(session);
    let pane = format!("{}:{}.0", session, window);
    let check_and_select = format!(
        "[ \"$(tmux display-message -p '#S')\" = '{}' ] && tmux select-pane -t '{}'",
        session, pane
    );
    let _ = Command::new("tmux")
        .args(["bind-key", "-n", r"C-\", "run-shell", &check_and_select])
        .output();
    // Auto-unbind when the dashboard session closes
    let _ = Command::new("tmux")
        .args([
            "set-hook",
            "-t",
            session,
            "session-closed",
            r"unbind-key -n C-\",
        ])
        .output();
}

/// Sanitize a branch name for use as a tmux session name component.
///
/// Replaces '/' and ':' with '-'. Colons cause tmux session:window target ambiguity.
fn sanitize_branch(branch: &str) -> String {
    branch.replace(['/', ':'], "-")
}

/// Canonical tmux session name for an agent workspace
pub fn agent_session_name(branch: &str) -> String {
    format!("agentree-{}", sanitize_branch(branch))
}

/// Canonical tmux session name for a persistent terminal session in a workspace.
pub fn terminal_session_name(branch: &str) -> String {
    format!("agentree-{}-term", sanitize_branch(branch))
}

/// Canonical tmux session name for a persistent editor session in a workspace.
pub fn editor_session_name(branch: &str) -> String {
    format!("agentree-{}-edit", sanitize_branch(branch))
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

/// Resize the calling pane to TUI_PANE_WIDTH columns using $TMUX_PANE.
///
/// Only call this when the right pane (pane 1) exists. When only one pane
/// is present, resize-pane shrinks the entire tmux window, leaving no space
/// for the right pane when it is later recreated.
pub fn resize_self_to_44_cols() {
    if let Ok(pane_id) = std::env::var("TMUX_PANE") {
        let _ = Command::new("tmux")
            .args([
                "resize-pane",
                "-x",
                &TUI_PANE_WIDTH.to_string(),
                "-t",
                &pane_id,
            ])
            .output();
    }
}

/// Returns true if a right pane exists in the session (≥ 2 panes in main window).
pub fn right_pane_exists(session: &str) -> bool {
    list_pane_ids(session).len() >= 2
}

/// Check whether pane at `index` in the session's main window has its process exited.
///
/// For `index > 0`, returns `true` when fewer panes than `index + 1` exist
/// (missing pane is treated as dead). For `index == 0`, returns `false` when
/// no panes exist (avoids spurious dead detection on the TUI pane).
fn is_pane_dead_at_index(session: &str, index: usize) -> bool {
    let window = first_window_index(session);
    let target = format!("{}:{}", session, window);
    Command::new("tmux")
        .args(["list-panes", "-t", &target, "-F", "#{pane_dead}"])
        .output()
        .map(|o| {
            let output = String::from_utf8_lossy(&o.stdout);
            let mut lines = output.lines();
            // Skip panes before the requested index
            for _ in 0..index {
                if lines.next().is_none() {
                    // Not enough panes — for index > 0, treat as dead
                    return index > 0;
                }
            }
            match lines.next() {
                Some(l) => l.trim() == "1",
                // No pane at this index
                None => index > 0,
            }
        })
        .unwrap_or(false)
}

/// Returns true if pane 0 in the given session's window has its process exited.
///
/// Uses `tmux list-panes -F '#{pane_dead}'` which outputs "1" for dead panes.
pub fn is_tui_pane_dead(session: &str) -> bool {
    is_pane_dead_at_index(session, 0)
}

/// Returns true if the right content pane (pane 1) has exited (pane_dead = 1).
///
/// Returns `true` when fewer than 2 panes exist (no content pane = dead).
/// Returns `false` on any error (safe default — avoid spurious respawn).
pub fn is_content_pane_dead(session: &str) -> bool {
    is_pane_dead_at_index(session, 1)
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
                .map_err(|e| AgentreeError::TmuxError(format!("respawn-pane failed: {}", e)))?;
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

/// Enable pane border status for the dashboard window.
///
/// Sets `pane-border-status top` so each pane shows a 1-row title bar, and
/// `pane-border-format` to display the per-pane `@agentree_title` user option.
/// Also initialises the left pane title to "agentree".
///
/// Call this once after split_horizontal() during session creation.
pub fn setup_pane_border_status(session: &str) {
    let window = first_window_index(session);
    let target = format!("{}:{}", session, window);
    let _ = Command::new("tmux")
        .args(["set-option", "-t", &target, "pane-border-status", "top"])
        .output();
    let _ = Command::new("tmux")
        .args([
            "set-option",
            "-t",
            &target,
            "pane-border-format",
            " #{@agentree_title}",
        ])
        .output();
    // Set left pane display title to "agentree"
    let pane_ids = list_pane_ids(session);
    if let Some(left_id) = pane_ids.first() {
        let _ = Command::new("tmux")
            .args([
                "set-option",
                "-p",
                "-t",
                left_id,
                "@agentree_title",
                "agentree",
            ])
            .output();
    }
}

/// Update the display title shown in the right pane's border.
///
/// Uses the per-pane tmux user option `@agentree_title` so the pane's
/// identity title (used by find_pane_in_session) is not affected.
///
/// Safe to call if no right pane exists — silently does nothing.
pub fn set_right_pane_display_title(session: &str, display: &str) {
    let pane_ids = list_pane_ids(session);
    if let Some(right_id) = pane_ids.get(1) {
        let _ = Command::new("tmux")
            .args([
                "set-option",
                "-p",
                "-t",
                right_id,
                "@agentree_title",
                display,
            ])
            .output();
    }
}

const WELCOME_PANE_TITLE: &str = "agentree-welcome";

/// Welcome/help panel content displayed on dashboard open and via the ? key.
const WELCOME_CONTENT: &str = r#"
  ▗▄▖  ▗▄▄▖▗▄▄▄▖▗▖  ▗▖▗▄▄▄▖▗▄▄▖ ▗▄▄▄▖▗▄▄▄▖
 ▐▌ ▐▌▐▌   ▐▌   ▐▛▚▖▐▌  █  ▐▌ ▐▌▐▌   ▐▌
 ▐▛▀▜▌▐▌▝▜▌▐▛▀▀▘▐▌ ▝▜▌  █  ▐▛▀▚▖▐▛▀▀▘▐▛▀▀▘
 ▐▌ ▐▌▝▚▄▞▘▐▙▄▄▖▐▌  ▐▌  █  ▐▌ ▐▌▐▙▄▄▖▐▙▄▄▖


  QUICK START

  agentree create <branch>  — create new workspace


  KEYBINDINGS

  j / down  navigate down       k / up    navigate up
  a         open agent          t         open terminal
  e         open editor         c         clear attention
  d         detach (background) q         quit dashboard
  ?         show this help


  Ctrl+\  return focus to left pane
"#;

/// Show the welcome/help panel in the right content area.
///
/// Writes the welcome content to a temp file and displays it via `cat` in a
/// persistent pane. The pane is titled WELCOME_PANE_TITLE so show_pane
/// recognizes it as a named pane and preserves it when switching actions.
pub fn show_welcome_panel(session: &str) {
    // Determine welcome file path
    let welcome_path = crate::daemon::runtime_dir()
        .map(|d| d.join("welcome.txt"))
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/agentree-welcome.txt"));

    // Write welcome content to file
    let _ = std::fs::write(&welcome_path, WELCOME_CONTENT);
    let welcome_path_str = welcome_path.to_string_lossy().to_string();

    // Command: clear screen, cat the file, read to keep pane alive showing content
    // `tail -f /dev/null` blocks portably across bash/zsh/sh/fish without
    // requiring shell-specific read flags. The pane stays alive until replaced
    // by show_pane (respawn-pane -k) when the user presses an action key.
    let cmd = format!("clear; cat '{}'; tail -f /dev/null", welcome_path_str);

    // Use HOME as cwd fallback
    let cwd = std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));

    let _ = show_pane(session, WELCOME_PANE_TITLE, &cmd, &cwd);
    set_right_pane_display_title(session, "Help");
}

/// Resize the calling pane to a percentage of the terminal width.
///
/// Uses $TMUX_PANE to identify the calling pane. Only call when a right pane exists.
pub fn resize_self_to_percent(percent: u8) {
    if let Ok(pane_id) = std::env::var("TMUX_PANE") {
        let _ = Command::new("tmux")
            .args([
                "resize-pane",
                "-x",
                &format!("{}%", percent),
                "-t",
                &pane_id,
            ])
            .output();
    }
}

/// Set the title of a tmux pane by its pane ID (e.g. "%3").
///
/// Sets both the terminal title (`select-pane -T`) and the persistent
/// `@agentree_title` user option so `find_pane_in_session` can locate it
/// reliably even when an agent overwrites the terminal title.
fn set_pane_title(pane_id: &str, title: &str) -> Result<()> {
    let out = Command::new("tmux")
        .args(["select-pane", "-t", pane_id, "-T", title])
        .output()
        .map_err(|e| AgentreeError::TmuxError(format!("select-pane -T failed: {}", e)))?;
    if !out.status.success() {
        return Err(AgentreeError::TmuxError(format!(
            "tmux select-pane -T failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    // Also set the persistent user option used by find_pane_in_session
    let _ = Command::new("tmux")
        .args(["set-option", "-p", "-t", pane_id, "@agentree_title", title])
        .output();
    Ok(())
}

/// Get the agentree title of a specific pane by its pane ID.
fn get_pane_title(pane_id: &str) -> String {
    Command::new("tmux")
        .args(["display-message", "-t", pane_id, "-p", "#{@agentree_title}"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Find a pane anywhere in the session by its agentree title. Returns the pane ID if found.
fn find_pane_in_session(session: &str, title: &str) -> Option<String> {
    let out = Command::new("tmux")
        .args([
            "list-panes",
            "-s",
            "-t",
            session,
            "-F",
            "#{pane_id} #{@agentree_title}",
        ])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        let mut parts = line.splitn(2, ' ');
        let pane_id = parts.next()?;
        let pane_title = parts.next()?.trim();
        if pane_title == title {
            return Some(pane_id.to_string());
        }
    }
    None
}

/// Check whether a pane with the given agentree title exists anywhere in the session.
///
/// Used for session icon indicators in the workspace list.
pub fn pane_exists_in_session(session: &str, title: &str) -> bool {
    Command::new("tmux")
        .args([
            "list-panes",
            "-s",
            "-t",
            session,
            "-F",
            "#{@agentree_title}",
        ])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().any(|l| l.trim() == title))
        .unwrap_or(false)
}

/// Show a named action pane in the right slot of the dashboard's main window.
///
/// Each background pane lives in its own detached window (via break-pane).
/// No shared stash window — avoids all size constraint failures.
///
/// - Named right pane (agentree-*): parked via break-pane into its own window.
/// - Unnamed right pane (plain shell): killed — no need to preserve it.
/// - Requested pane found anywhere in session: join-pane'd into main.
/// - Requested pane not found: created via new-window -d, then join-pane'd.
///
/// After this call, the right pane shows the requested content with keyboard focus.
pub fn show_pane(session: &str, title: &str, cmd: &str, cwd: &Path) -> Result<()> {
    // 1. Snapshot main window pane layout
    let main_panes = list_pane_ids(session);
    let left_id = main_panes
        .first()
        .ok_or_else(|| AgentreeError::TmuxError("Dashboard main window has no panes".to_string()))?
        .clone();

    // 2. If a right pane exists, park named panes via break-pane, kill unnamed ones.
    let content_pane_id = main_panes.get(1).cloned();

    if let Some(right_id) = content_pane_id {
        let right_title = get_pane_title(&right_id);
        if right_title.starts_with("agentree-") {
            // break-pane creates a new detached window for this pane — no size constraints.
            let out = Command::new("tmux")
                .args(["break-pane", "-d", "-s", &right_id])
                .output()
                .map_err(|e| AgentreeError::TmuxError(format!("break-pane failed: {}", e)))?;
            if !out.status.success() {
                return Err(AgentreeError::TmuxError(format!(
                    "tmux break-pane failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                )));
            }
        } else {
            let _ = Command::new("tmux")
                .args(["kill-pane", "-t", &right_id])
                .output();
        }
    }

    // 3. Restore a parked pane or create a fresh one in the main window.
    if let Some(existing_id) = find_pane_in_session(session, title) {
        // Bring back the parked pane via join-pane
        let out = Command::new("tmux")
            .args(["join-pane", "-h", "-s", &existing_id, "-t", &left_id])
            .output()
            .map_err(|e| AgentreeError::TmuxError(format!("join-pane to main failed: {}", e)))?;
        if !out.status.success() {
            return Err(AgentreeError::TmuxError(format!(
                "tmux join-pane to main failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
    } else {
        // Create a new right pane directly via split-window — simpler than new-window + join-pane
        let cwd_str = cwd.to_string_lossy();
        let out = Command::new("tmux")
            .args([
                "split-window",
                "-h",
                "-d",
                "-P",
                "-F",
                "#{pane_id}",
                "-t",
                &left_id,
                "-c",
                &cwd_str,
                cmd,
            ])
            .output()
            .map_err(|e| {
                AgentreeError::TmuxError(format!("split-window for pane failed: {}", e))
            })?;
        if !out.status.success() {
            return Err(AgentreeError::TmuxError(format!(
                "tmux split-window for pane failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        let new_pane_id = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let _ = set_pane_title(&new_pane_id, title);
    }

    // 4. Restore the TUI pane to its fixed column width
    let _ = Command::new("tmux")
        .args([
            "resize-pane",
            "-x",
            &TUI_PANE_WIDTH.to_string(),
            "-t",
            &left_id,
        ])
        .output();

    // 5. Give keyboard focus to the right pane
    focus_right_pane(session);

    Ok(())
}

/// Give keyboard focus to the right content pane of the main window.
pub fn focus_right_pane(session: &str) {
    let pane_ids = list_pane_ids(session);
    if let Some(right_id) = pane_ids.get(1) {
        let _ = Command::new("tmux")
            .args(["select-pane", "-t", right_id])
            .output();
    }
}

/// Kill all stash panes belonging to a workspace branch.
///
/// Matches panes whose title starts with `agentree-{safe_branch}`.
/// Called when a workspace is removed to clean up background panes.
pub fn kill_workspace_panes(session: &str, branch: &str) {
    let safe = sanitize_branch(branch);
    let prefix = format!("agentree-{}", safe);

    let pane_infos: Vec<(String, String)> = Command::new("tmux")
        .args([
            "list-panes",
            "-s",
            "-t",
            session,
            "-F",
            "#{pane_id} #{@agentree_title}",
        ])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| {
            s.lines()
                .filter_map(|line| {
                    let mut parts = line.splitn(2, ' ');
                    let pane_id = parts.next()?.to_string();
                    let pane_title = parts.next()?.trim().to_string();
                    Some((pane_id, pane_title))
                })
                .collect()
        })
        .unwrap_or_default();

    for (pane_id, pane_title) in pane_infos {
        if pane_title.starts_with(&prefix) {
            let _ = Command::new("tmux")
                .args(["kill-pane", "-t", &pane_id])
                .output();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_session_name_sanitizes_slash() {
        assert_eq!(
            agent_session_name("feature/my-auth"),
            "agentree-feature-my-auth"
        );
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
        assert_eq!(
            terminal_session_name("feature/auth"),
            "agentree-feature-auth-term"
        );
    }

    #[test]
    fn test_editor_session_name() {
        assert_eq!(
            editor_session_name("feature/auth"),
            "agentree-feature-auth-edit"
        );
    }
}
