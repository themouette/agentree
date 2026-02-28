use crate::daemon::protocol::WorkspaceInfo;
use crate::dashboard::client::DaemonClient;
use crate::dashboard::tmux;
use crate::dashboard::DASHBOARD_SESSION;
use crate::error::Result;
use crossterm::{
    event::{self, EnableFocusChange, DisableFocusChange, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::io;
use std::time::{Duration, Instant};

const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

#[derive(PartialEq)]
enum TuiStartupState {
    Connecting,
    Connected,
    ConnectionLost,
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
pub fn run_tui(client: DaemonClient) -> Result<()> {
    enable_raw_mode().map_err(crate::error::AgentreeError::Io)?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableFocusChange)
        .map_err(crate::error::AgentreeError::Io)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(crate::error::AgentreeError::Io)?;

    // Start in Connecting state; transition to Connected on first successful list
    let mut state = TuiState::new();

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
                        // Detach: put dashboard in background, session + TUI stay alive
                        (_, KeyCode::Char('q')) => {
                            let _ = std::process::Command::new("tmux")
                                .args(["detach-client"])
                                .status();
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
                }
                Event::FocusGained => {
                    state.focused = true;
                }
                Event::Resize(_, _) => {
                    // Restore 44-col width only when the right pane exists.
                    // When pane 1 just closed, we are the only pane — resizing
                    // would shrink the entire tmux window to 44 cols, leaving no
                    // space when the right pane is recreated.
                    if tmux::right_pane_exists(DASHBOARD_SESSION) {
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
                    }
                }
            }
            state.last_refresh = Instant::now();
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
    // Split into: header line + list area + status bar + help bar at the bottom
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),   // header
            Constraint::Fill(1),     // list
            Constraint::Length(1),   // status bar (1 line, hidden when empty)
            Constraint::Length(3),   // help bar
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
            .enumerate()
            .map(|(i, ws)| {
                let agent_running = tmux::pane_exists_in_session(DASHBOARD_SESSION, &tmux::agent_session_name(&ws.branch));
                let term_running = tmux::pane_exists_in_session(DASHBOARD_SESSION, &tmux::terminal_session_name(&ws.branch));
                let edit_running = tmux::pane_exists_in_session(DASHBOARD_SESSION, &tmux::editor_session_name(&ws.branch));

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
                let attention_str = if ws.attention.is_some() { "\u{2691} " } else { "  " };
                let attention_style = if ws.attention.is_some() {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default()
                };

                // Branch name (middle-truncated)
                let branch = truncate_middle(&ws.branch, branch_width);

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
                    Span::raw(format!("{:<width$} ", branch, width = branch_width)),
                    Span::styled(stats_str, Style::default().fg(Color::DarkGray)),
                    Span::raw(" "),
                    Span::styled(format!("{:>7}", age), Style::default().fg(Color::DarkGray)),
                ];

                // Attention rows (not selected) get red background
                let item_style = if ws.attention.is_some() && i != state.selected {
                    Style::default().bg(Color::Red)
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(spans)).style(item_style)
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
    }

    // ── status bar — show ephemeral messages (errors, confirmations) ──
    let status_text = match &state.status_message {
        Some((msg, shown_at)) if shown_at.elapsed() < std::time::Duration::from_secs(3) => {
            Span::styled(format!(" {}", msg), Style::default().fg(Color::Red))
        }
        _ => Span::raw(""),
    };
    let status_bar = Paragraph::new(Line::from(vec![status_text]));
    f.render_widget(status_bar, inner[2]);

    // ── help bar ──
    let help = Paragraph::new(Line::from(vec![
        Span::styled("[j/k]", Style::default().fg(Color::Yellow)),
        Span::raw(" navigate  "),
        Span::styled("[a]", Style::default().fg(Color::Yellow)),
        Span::raw("gent  "),
        Span::styled("[t]", Style::default().fg(Color::Yellow)),
        Span::raw("erminal  "),
        Span::styled("[e]", Style::default().fg(Color::Yellow)),
        Span::raw("ditor  "),
        Span::styled("[c]", Style::default().fg(Color::Yellow)),
        Span::raw("lear  "),
        Span::styled("[q]", Style::default().fg(Color::Yellow)),
        Span::raw("etach"),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(help, inner[3]);
}

// ---------------------------------------------------------------------------
// Key actions
// ---------------------------------------------------------------------------

fn action_agent(state: &mut TuiState) {
    if let Some(ws) = state.selected_workspace().cloned() {
        let title = tmux::agent_session_name(&ws.branch);
        let worktree_path = std::path::Path::new(&ws.path);
        let agent_cmd = ws.agent_bin.as_deref().unwrap_or("claude");
        if let Err(e) = tmux::show_pane(DASHBOARD_SESSION, &title, agent_cmd, worktree_path) {
            state.status_message = Some((format!("tmux: {}", e), std::time::Instant::now()));
        }
    }
}

fn action_terminal(state: &mut TuiState) {
    if let Some(ws) = state.selected_workspace().cloned() {
        let title = tmux::terminal_session_name(&ws.branch);
        let worktree_path = std::path::Path::new(&ws.path);
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        if let Err(e) = tmux::show_pane(DASHBOARD_SESSION, &title, &shell, worktree_path) {
            state.status_message = Some((format!("tmux: {}", e), std::time::Instant::now()));
        }
    }
}

fn action_editor(state: &mut TuiState) {
    if let Some(ws) = state.selected_workspace().cloned() {
        let editor = std::env::var("EDITOR")
            .or_else(|_| std::env::var("VISUAL"))
            .unwrap_or_else(|_| "vi".to_string());

        if which::which(&editor).is_err() {
            state.status_message = Some((
                "No editor found — set $EDITOR".to_string(),
                std::time::Instant::now(),
            ));
            return;
        }

        let title = tmux::editor_session_name(&ws.branch);
        let worktree_path = std::path::Path::new(&ws.path);
        // Open editor in workspace directory; split-window -c handles the cwd
        let edit_cmd = format!("{} .", shell_quote(&editor));
        let _ = tmux::show_pane(DASHBOARD_SESSION, &title, &edit_cmd, worktree_path);
    }
}

fn action_clear_attention(state: &TuiState, client: &DaemonClient) {
    if let Some(ws) = state.selected_workspace() {
        // Silent no-op if workspace has no attention flag
        if ws.attention.is_some() {
            let _ = client.clear_attention(&ws.branch);
            // Daemon deletes .agentree/attention.md and updates in-memory state.
            // TUI picks up the cleared flag within the next 1s poll cycle — no extra work needed.
        }
    }
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
fn truncate_middle(s: &str, max: usize) -> String {
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
