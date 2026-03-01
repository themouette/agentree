use crate::daemon::protocol::WorkspaceInfo;
use crate::dashboard::client::DaemonClient;
use crate::dashboard::tmux;
use crate::dashboard::DASHBOARD_SESSION;
use crate::error::Result;
use crossterm::{
    event::{self, DisableFocusChange, EnableFocusChange, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
    Terminal,
};
use std::collections::HashMap;
use std::io;
use std::time::{Duration, Instant};

const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

#[derive(PartialEq)]
enum TuiStartupState {
    Connecting,
    Connected,
    ConnectionLost,
}

/// Cached pane-open status for a single workspace, refreshed once per poll cycle.
#[derive(Default, Clone)]
struct WorkspacePaneStatus {
    agent: bool,
    term: bool,
    edit: bool,
}

struct TuiState {
    workspaces: Vec<WorkspaceInfo>,
    selected: usize,
    last_refresh: Instant,
    focused: bool,
    startup: TuiStartupState,
    /// Frame counter for spinner animation
    frame: u64,
    /// Ephemeral error message shown in the status bar. Auto-clears after 3 seconds.
    status_message: Option<(String, std::time::Instant)>,
    /// True if this TUI session started the daemon (and should kill it on quit).
    started_daemon: bool,
    /// True when the footer shows "Kill dashboard? [y/N]" confirmation.
    quit_pending: bool,
    /// Branch of the last-actioned workspace (used by indicator pane).
    active_workspace: Option<String>,
    /// Pane open/running status per workspace branch. Refreshed on each poll cycle.
    pane_status: HashMap<String, WorkspacePaneStatus>,
}

impl TuiState {
    fn new() -> Self {
        TuiState {
            workspaces: vec![],
            selected: 0,
            last_refresh: Instant::now(),
            focused: true,
            startup: TuiStartupState::Connecting,
            frame: 0,
            status_message: None,
            started_daemon: false,
            quit_pending: false,
            active_workspace: None,
            pane_status: HashMap::new(),
        }
    }

    fn next(&mut self) {
        if !self.workspaces.is_empty() {
            self.selected = (self.selected + 1) % self.workspaces.len();
        }
    }

    fn prev(&mut self) {
        if !self.workspaces.is_empty() {
            if self.selected == 0 {
                self.selected = self.workspaces.len() - 1;
            } else {
                self.selected -= 1;
            }
        }
    }

    fn selected_workspace(&self) -> Option<&WorkspaceInfo> {
        self.workspaces.get(self.selected)
    }
}

/// Run the ratatui TUI in the left pane of the dashboard
pub fn run_tui(client: DaemonClient, started_daemon: bool) -> Result<()> {
    enable_raw_mode().map_err(crate::error::AgentreeError::Io)?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableFocusChange)
        .map_err(crate::error::AgentreeError::Io)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(crate::error::AgentreeError::Io)?;

    // Start in Connecting state; transition to Connected on first successful list
    let mut state = TuiState::new();
    state.started_daemon = started_daemon;

    let result = run_event_loop(&mut terminal, &mut state, &client);

    // Restore terminal unconditionally
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        DisableFocusChange,
        LeaveAlternateScreen
    );
    let _ = terminal.show_cursor();

    result
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut TuiState,
    client: &DaemonClient,
) -> Result<()> {
    loop {
        state.frame = state.frame.wrapping_add(1);

        // Clear expired status message at start of each frame
        if let Some((_, shown_at)) = &state.status_message {
            if shown_at.elapsed() >= std::time::Duration::from_secs(3) {
                state.status_message = None;
            }
        }

        terminal
            .draw(|f| render(f, state))
            .map_err(crate::error::AgentreeError::Io)?;

        // When Connecting, poll frequently to transition fast
        let poll_timeout = if state.startup == TuiStartupState::Connecting {
            Duration::from_millis(100)
        } else {
            REFRESH_INTERVAL
        };

        if event::poll(poll_timeout).map_err(crate::error::AgentreeError::Io)? {
            match event::read().map_err(crate::error::AgentreeError::Io)? {
                Event::Key(key) => {
                    match (key.modifiers, key.code) {
                        // Quit: first press shows confirmation footer, second q cancels
                        (_, KeyCode::Char('q')) => {
                            if !state.quit_pending {
                                state.quit_pending = true;
                            } else {
                                // Second q press — cancel (treat as 'n')
                                state.quit_pending = false;
                            }
                        }
                        // Confirm quit
                        (_, KeyCode::Char('y')) | (_, KeyCode::Char('Y')) if state.quit_pending => {
                            execute_quit(state);
                            break;
                        }
                        // Cancel quit
                        (_, KeyCode::Char('n')) | (_, KeyCode::Char('N')) | (_, KeyCode::Esc)
                            if state.quit_pending =>
                        {
                            state.quit_pending = false;
                        }
                        // Detach: put dashboard in background, session + TUI stay alive
                        (_, KeyCode::Char('d')) => {
                            execute_detach();
                            // Don't break — TUI keeps running; `agentree dashboard` reattaches
                        }
                        // Force-quit: actually exit the TUI process
                        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                            break;
                        }
                        // Navigation
                        (_, KeyCode::Down) | (_, KeyCode::Char('j')) => state.next(),
                        (_, KeyCode::Up) | (_, KeyCode::Char('k')) => state.prev(),
                        // Actions
                        (_, KeyCode::Char('a')) => action_agent(state),
                        (_, KeyCode::Char('t')) => action_terminal(state),
                        (_, KeyCode::Char('e')) => action_editor(state),
                        (_, KeyCode::Char('c')) => action_clear_attention(state, client),
                        // Help: show welcome/help panel
                        (_, KeyCode::Char('?')) => {
                            tmux::show_welcome_panel(DASHBOARD_SESSION);
                        }
                        // Retry connection
                        (_, KeyCode::Char('r')) => {
                            if state.startup == TuiStartupState::ConnectionLost {
                                state.startup = TuiStartupState::Connecting;
                            }
                        }
                        _ => {}
                    }
                }
                Event::FocusLost => {
                    state.focused = false;
                    // Shrink back to fixed width when right pane exists
                    if tmux::right_pane_exists(DASHBOARD_SESSION) {
                        tmux::resize_self_to_44_cols();
                    }
                }
                Event::FocusGained => {
                    state.focused = true;
                    // Expand to configured percentage when right pane exists
                    if tmux::right_pane_exists(DASHBOARD_SESSION) {
                        tmux::resize_self_to_percent(tmux::TUI_PANE_WIDTH_PERCENT);
                    }
                }
                Event::Resize(_, _) => {
                    // Restore fixed-col width only when unfocused and the right pane exists.
                    // When focused, the pane should stay at 50% — don't undo FocusGained.
                    // When pane 1 just closed, we are the only pane — resizing
                    // would shrink the entire tmux window to 44 cols, leaving no
                    // space when the right pane is recreated.
                    if !state.focused && tmux::right_pane_exists(DASHBOARD_SESSION) {
                        tmux::resize_self_to_44_cols();
                    }
                }
                _ => {}
            }
        }

        // Refresh from daemon on interval (or every poll when Connecting)
        if state.startup == TuiStartupState::Connecting
            || state.last_refresh.elapsed() >= REFRESH_INTERVAL
        {
            match client.list_workspaces() {
                Ok(ws) => {
                    let selected_branch = state.selected_workspace().map(|w| w.branch.clone());
                    state.workspaces = ws;
                    // Preserve selection by branch name
                    if let Some(branch) = selected_branch {
                        if let Some(idx) = state.workspaces.iter().position(|w| w.branch == branch)
                        {
                            state.selected = idx;
                        } else {
                            state.selected = 0;
                        }
                    }
                    state.startup = TuiStartupState::Connected;
                }
                Err(_) => {
                    if state.startup == TuiStartupState::Connected {
                        state.startup = TuiStartupState::ConnectionLost;
                        state.quit_pending = false; // cancel any pending quit on connection loss
                    }
                }
            }
            state.last_refresh = Instant::now();

            // Update pane status cache (avoids 3× tmux subprocess per workspace per render frame)
            let mut pane_status = HashMap::new();
            for ws in &state.workspaces {
                pane_status.insert(
                    ws.branch.clone(),
                    WorkspacePaneStatus {
                        agent: tmux::pane_exists_in_session(
                            DASHBOARD_SESSION,
                            &tmux::agent_session_name(&ws.branch),
                        ),
                        term: tmux::pane_exists_in_session(
                            DASHBOARD_SESSION,
                            &tmux::terminal_session_name(&ws.branch),
                        ),
                        edit: tmux::pane_exists_in_session(
                            DASHBOARD_SESSION,
                            &tmux::editor_session_name(&ws.branch),
                        ),
                    },
                );
            }
            state.pane_status = pane_status;

            // Auto-respawn welcome panel if the content pane has died
            if state.startup == TuiStartupState::Connected
                && tmux::is_content_pane_dead(DASHBOARD_SESSION)
            {
                tmux::show_welcome_panel(DASHBOARD_SESSION);
            }
        }
    }
    Ok(())
}

fn render(f: &mut ratatui::Frame, state: &TuiState) {
    match state.startup {
        TuiStartupState::Connecting => render_connecting(f, f.area(), state.frame),
        TuiStartupState::ConnectionLost => render_connection_lost(f, f.area()),
        TuiStartupState::Connected => render_workspace_list(f, f.area(), state),
    }
}

fn render_connecting(f: &mut ratatui::Frame, area: ratatui::layout::Rect, frame: u64) {
    const SPINNER_FRAMES: [char; 4] = ['|', '/', '-', '\\'];
    let spinner = SPINNER_FRAMES[(frame / 5) as usize % 4];
    let text = format!("{} Connecting...", spinner);

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Cyan));

    // Vertically center by splitting
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(area);

    f.render_widget(paragraph, chunks[1]);
}

fn render_connection_lost(f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
    let text = "Lost connection to daemon. Press r to retry.";

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Red));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(area);

    f.render_widget(paragraph, chunks[1]);
}

fn render_workspace_list(f: &mut ratatui::Frame, area: ratatui::layout::Rect, state: &TuiState) {
    // Split into: header line + list area + active indicator + footer
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Fill(1),   // list
            Constraint::Length(1), // active workspace indicator
            Constraint::Length(1), // footer (status + hints combined)
        ])
        .split(area);

    // ── header line ──
    let header = Paragraph::new(Line::from(vec![Span::styled(
        " \u{1F916}  BRANCH             \u{2191}  \u{00B1}   AGE",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]));
    f.render_widget(header, inner[0]);

    // ── workspace list ──
    if state.workspaces.is_empty() {
        let empty_msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "No workspaces yet.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "Run: agentree create <branch>",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        f.render_widget(empty_msg, inner[1]);
    } else {
        // Available width: inner[1].width minus 2 for borders
        let list_width = inner[1].width.saturating_sub(2) as usize;
        // Layout within each row (approximate):
        // " 🤖 "(4) + ">_ "(3) + "E "(2) + attention(2) + branch(variable) + stats(8) + age(7) + spaces(3)
        // robot=4 (space+emoji+space = 4 visible), term=3, edit=2, attention=2, stats=8, age=7, spaces=3
        let branch_width = list_width.saturating_sub(4 + 3 + 2 + 2 + 8 + 7 + 3).max(6);

        let items: Vec<ListItem> = state
            .workspaces
            .iter()
            .map(|ws| {
                // Read pane status from cache (updated once per poll cycle)
                let pane_status = state
                    .pane_status
                    .get(&ws.branch)
                    .cloned()
                    .unwrap_or_default();
                let agent_running = pane_status.agent;
                let term_running = pane_status.term;
                let edit_running = pane_status.edit;

                // Robot icon: green if agent running, DarkGray otherwise
                let robot_style = if agent_running {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                let term_style = if term_running {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                let edit_style = if edit_running {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                // Attention icon
                let attention_str = if ws.attention.is_some() {
                    "\u{2691} "
                } else {
                    "  "
                };
                let attention_style = if ws.attention.is_some() {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default()
                };

                // Branch name (middle-truncated)
                let branch = truncate_middle(&ws.branch, branch_width);

                // Phase tag: shown after branch if agent_status.phase is present
                let phase_tag = ws
                    .agent_status
                    .as_ref()
                    .and_then(|s| s.phase.as_ref())
                    .map(|p| {
                        // Truncate phase string to 12 chars
                        let truncated = if p.chars().count() > 12 {
                            format!("{}…", p.chars().take(11).collect::<String>())
                        } else {
                            p.clone()
                        };
                        format!(" [{}]", truncated)
                    })
                    .unwrap_or_default();

                // Conditional git stats
                let ahead_str = if ws.commits_ahead > 0 {
                    format!("\u{2191}{}", ws.commits_ahead)
                } else {
                    String::new()
                };
                let changed_str = if ws.files_changed > 0 {
                    format!("\u{00B1}{}", ws.files_changed)
                } else {
                    String::new()
                };

                let age = format_age(ws.last_activity.as_deref());

                let stats_str = format!("{:>3} {:>3}", ahead_str, changed_str);

                let spans = vec![
                    Span::styled(" \u{1F916} ", robot_style),
                    Span::styled(">_ ", term_style),
                    Span::styled("E ", edit_style),
                    Span::styled(attention_str, attention_style),
                    Span::raw(format!("{:<width$}", branch, width = branch_width)),
                    Span::styled(phase_tag, Style::default().fg(Color::DarkGray)), // phase after branch
                    Span::raw(" "),
                    Span::styled(stats_str, Style::default().fg(Color::DarkGray)),
                    Span::raw(" "),
                    Span::styled(format!("{:>7}", age), Style::default().fg(Color::DarkGray)),
                ];

                let item_style = Style::default(); // No row background — selection highlight always wins cleanly

                let mut lines: Vec<Line> = vec![Line::from(spans)];

                // Second line: attention first-line takes priority over current_task
                if let Some(ref attention_content) = ws.attention {
                    let first_line = attention_content.lines().next().unwrap_or("").to_string();
                    if !first_line.trim().is_empty() {
                        let truncated = truncate_right(&first_line, list_width.saturating_sub(4));
                        lines.push(Line::from(vec![
                            Span::raw("    "),
                            Span::styled(truncated, Style::default().fg(Color::Yellow)),
                        ]));
                    }
                } else if let Some(ref status) = ws.agent_status {
                    if let Some(ref task) = status.current_task {
                        if !task.trim().is_empty() {
                            let truncated = truncate_right(task, list_width.saturating_sub(4));
                            lines.push(Line::from(vec![
                                Span::raw("    "),
                                Span::styled(truncated, Style::default().fg(Color::DarkGray)),
                            ]));
                        }
                    }
                }

                ListItem::new(Text::from(lines)).style(item_style)
            })
            .collect();

        let highlight_style = if state.focused {
            Style::default()
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::DIM)
        };

        let list = List::new(items)
            .highlight_style(highlight_style)
            .highlight_symbol("")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            );

        let mut list_state = ListState::default();
        list_state.select(Some(state.selected));

        f.render_stateful_widget(list, inner[1], &mut list_state);

        // Scrollbar: only when list has more items than visible area
        // Each workspace is 2 rows tall (two-line rows).
        // Visible item count = floor(inner[1].height / 2), accounting for 2-row borders.
        let inner_height = inner[1].height.saturating_sub(2); // subtract top+bottom borders
        let rows_per_item: u16 = 2;
        let visible_items = (inner_height / rows_per_item) as usize;
        if state.workspaces.len() > visible_items {
            let mut scrollbar_state =
                ScrollbarState::new(state.workspaces.len()).position(state.selected);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .end_symbol(None),
                inner[1],
                &mut scrollbar_state,
            );
        }
    }

    // ── active workspace indicator ──
    let active_line = match &state.active_workspace {
        Some(branch) => Line::from(vec![
            Span::styled("Active: ", Style::default().fg(Color::DarkGray)),
            Span::styled(branch.clone(), Style::default().fg(Color::Cyan)),
        ]),
        None => Line::from(Span::styled(
            "No active workspace",
            Style::default().fg(Color::DarkGray),
        )),
    };
    f.render_widget(Paragraph::new(active_line), inner[2]);

    // ── footer: quit confirmation or status message or key hints ──
    let footer_line = if state.quit_pending {
        // Quit confirmation takes priority over everything
        Line::from(vec![
            Span::styled("Kill dashboard? ", Style::default().fg(Color::Red)),
            Span::styled(
                "[y/N]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        // Check for active status message (errors, etc.) — takes priority over hints
        let active_msg = state.status_message.as_ref().and_then(|(msg, shown_at)| {
            if shown_at.elapsed() < std::time::Duration::from_secs(3) {
                Some(msg.as_str())
            } else {
                None
            }
        });

        if let Some(msg) = active_msg {
            Line::from(Span::styled(
                format!(" {} ", msg),
                Style::default().fg(Color::Red),
            ))
        } else {
            Line::from(vec![
                Span::styled("a", Style::default().fg(Color::Yellow)),
                Span::raw(" agent  "),
                Span::styled("t", Style::default().fg(Color::Yellow)),
                Span::raw(" term  "),
                Span::styled("e", Style::default().fg(Color::Yellow)),
                Span::raw(" edit  "),
                Span::styled("c", Style::default().fg(Color::Yellow)),
                Span::raw(" clear  "),
                Span::styled("d", Style::default().fg(Color::Yellow)),
                Span::raw(" detach  "),
                Span::styled("q", Style::default().fg(Color::Yellow)),
                Span::raw(" quit  "),
                Span::styled("?", Style::default().fg(Color::Yellow)),
                Span::raw(" help"),
            ])
        }
    };
    let footer = Paragraph::new(footer_line).style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, inner[3]);
}

// ---------------------------------------------------------------------------
// Key actions
// ---------------------------------------------------------------------------

/// Open a named pane for a workspace in the right content slot.
///
/// Calls `show_pane`, updates `active_workspace` and the right-pane display
/// title on success, or sets an ephemeral `status_message` on error.
fn open_pane_for_workspace(
    state: &mut TuiState,
    title: String,
    cmd: String,
    cwd: std::path::PathBuf,
    branch: String,
) {
    match tmux::show_pane(DASHBOARD_SESSION, &title, &cmd, &cwd) {
        Ok(()) => {
            state.active_workspace = Some(branch.clone());
            tmux::set_right_pane_display_title(DASHBOARD_SESSION, &format!("Active: {}", branch));
        }
        Err(e) => {
            state.status_message = Some((format!("tmux: {}", e), std::time::Instant::now()));
        }
    }
}

fn action_agent(state: &mut TuiState) {
    if let Some(ws) = state.selected_workspace().cloned() {
        let title = tmux::agent_session_name(&ws.branch);
        let cmd = ws.agent_bin.as_deref().unwrap_or("claude").to_string();
        let cwd = std::path::PathBuf::from(&ws.path);
        open_pane_for_workspace(state, title, cmd, cwd, ws.branch.clone());
    }
}

fn action_terminal(state: &mut TuiState) {
    if let Some(ws) = state.selected_workspace().cloned() {
        let title = tmux::terminal_session_name(&ws.branch);
        let cmd = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let cwd = std::path::PathBuf::from(&ws.path);
        open_pane_for_workspace(state, title, cmd, cwd, ws.branch.clone());
    }
}

fn action_editor(state: &mut TuiState) {
    if let Some(ws) = state.selected_workspace().cloned() {
        let editor = std::env::var("EDITOR")
            .or_else(|_| std::env::var("VISUAL"))
            .unwrap_or_else(|_| "vi".to_string());

        // Split $EDITOR into binary + optional extra args (e.g. "vim -u config")
        let parts: Vec<&str> = editor.splitn(2, ' ').collect();
        let editor_bin = parts[0];

        if which::which(editor_bin).is_err() {
            state.status_message = Some((
                "No editor found — set $EDITOR".to_string(),
                std::time::Instant::now(),
            ));
            return;
        }

        let title = tmux::editor_session_name(&ws.branch);
        let cwd = std::path::PathBuf::from(&ws.path);
        // Shell-quote only the binary; append extra args unquoted, then "."
        let bin = shell_quote(editor_bin);
        let edit_cmd = if parts.len() > 1 {
            format!("{} {} .", bin, parts[1])
        } else {
            format!("{} .", bin)
        };
        open_pane_for_workspace(state, title, edit_cmd, cwd, ws.branch.clone());
    }
}

fn action_clear_attention(state: &mut TuiState, client: &DaemonClient) {
    if let Some(ws) = state.workspaces.get_mut(state.selected) {
        if ws.attention.is_some() {
            // Optimistic clear: update local TUI state immediately.
            // Next 1s poll will confirm; flag reappears only if daemon delete failed.
            let branch = ws.branch.clone();
            ws.attention = None;
            let _ = client.clear_attention(&branch);
        }
    }
}

/// Kill the dashboard tmux session (and the daemon if this session started it).
///
/// Call this before breaking out of the event loop. Terminal cleanup is irrelevant
/// since killing the tmux session destroys the pane.
fn execute_quit(state: &mut TuiState) {
    // Kill the daemon only if this session started it
    if state.started_daemon {
        if let Some(pid_path) = crate::daemon::runtime_dir().map(|d| d.join("daemon.pid")) {
            if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    #[cfg(unix)]
                    let _ = std::process::Command::new("kill")
                        .args(["-TERM", &pid.to_string()])
                        .status();
                }
            }
        }
    }

    // Kill the tmux session — this kills the TUI process itself
    let _ = tmux::kill_session(DASHBOARD_SESSION);
}

/// Detach from the tmux session without killing the dashboard.
///
/// The session and TUI remain alive in the background.
/// `agentree dashboard` will reattach on next invocation.
fn execute_detach() {
    eprintln!("Dashboard running in background. Re-attach: agentree dashboard");
    let _ = std::process::Command::new("tmux")
        .args(["detach-client"])
        .status();
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn format_age(last_activity: Option<&str>) -> String {
    let time_str = match last_activity {
        Some(s) => s,
        None => return "-".to_string(),
    };

    let parsed = match chrono::DateTime::parse_from_rfc3339(time_str) {
        Ok(t) => t,
        Err(_) => return "-".to_string(),
    };

    let secs = chrono::Utc::now()
        .signed_duration_since(parsed)
        .num_seconds();

    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

/// Middle-truncate a string to at most `max` chars, using "…" in the middle.
///
/// # Precondition
///
/// `max` must be ≥ 2. Smaller values are not meaningful (you cannot fit both
/// the ellipsis character and at least one content character).
fn truncate_middle(s: &str, max: usize) -> String {
    debug_assert!(max >= 2, "truncate_middle: max must be >= 2");
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    let half = (max.saturating_sub(1)) / 2;
    let prefix: String = chars[..half].iter().collect();
    let suffix: String = chars[chars.len() - (max - 1 - half)..].iter().collect();
    format!("{}\u{2026}{}", prefix, suffix)
}

/// Wrap a string in single quotes with internal single quotes escaped.
/// Safe for embedding in POSIX shell command strings.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r#"'"'"'"#))
}

/// Right-trim a string to at most `max` chars, appending "…" if truncated.
fn truncate_right(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    let trimmed: String = chars[..max.saturating_sub(1)].iter().collect();
    format!("{}\u{2026}", trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── format_age ──────────────────────────────────────────────────────────

    fn rfc3339_ago(secs: i64) -> String {
        use chrono::{Duration, Utc};
        (Utc::now() - Duration::seconds(secs)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }

    #[test]
    fn test_format_age_none() {
        assert_eq!(format_age(None), "-");
    }

    #[test]
    fn test_format_age_invalid() {
        assert_eq!(format_age(Some("not-a-date")), "-");
    }

    #[test]
    fn test_format_age_just_now() {
        assert_eq!(format_age(Some(&rfc3339_ago(30))), "just now");
    }

    #[test]
    fn test_format_age_minutes() {
        assert_eq!(format_age(Some(&rfc3339_ago(90))), "1m");
        assert_eq!(format_age(Some(&rfc3339_ago(3599))), "59m");
    }

    #[test]
    fn test_format_age_hours() {
        assert_eq!(format_age(Some(&rfc3339_ago(3600))), "1h");
        assert_eq!(format_age(Some(&rfc3339_ago(7200))), "2h");
    }

    #[test]
    fn test_format_age_days() {
        assert_eq!(format_age(Some(&rfc3339_ago(86400))), "1d");
        assert_eq!(format_age(Some(&rfc3339_ago(172800))), "2d");
    }

    // ── truncate_middle ─────────────────────────────────────────────────────

    #[test]
    fn test_truncate_middle_no_op_when_short() {
        assert_eq!(truncate_middle("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_middle_no_op_at_exact_max() {
        assert_eq!(truncate_middle("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_middle_truncates() {
        let result = truncate_middle("hello world!", 8);
        assert_eq!(result.chars().count(), 8);
        assert!(result.contains('\u{2026}'));
    }

    #[test]
    fn test_truncate_middle_min_valid_max() {
        // max=2: one prefix char (0 half) + ellipsis + one suffix char
        let result = truncate_middle("abc", 2);
        assert_eq!(result.chars().count(), 2);
        assert!(result.contains('\u{2026}'));
    }

    // ── truncate_right ──────────────────────────────────────────────────────

    #[test]
    fn test_truncate_right_no_op_when_short() {
        assert_eq!(truncate_right("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_right_no_op_at_exact_max() {
        assert_eq!(truncate_right("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_right_truncates() {
        let result = truncate_right("hello world!", 8);
        assert_eq!(result.chars().count(), 8);
        assert!(result.ends_with('\u{2026}'));
    }

    // ── shell_quote ─────────────────────────────────────────────────────────

    #[test]
    fn test_shell_quote_plain() {
        assert_eq!(shell_quote("vim"), "'vim'");
    }

    #[test]
    fn test_shell_quote_with_spaces() {
        assert_eq!(shell_quote("vim -u config"), "'vim -u config'");
    }

    #[test]
    fn test_shell_quote_with_single_quote() {
        // it's → 'it'"'"'s'
        assert_eq!(shell_quote("it's"), r#"'it'"'"'s'"#);
    }
}
