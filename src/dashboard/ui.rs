use crate::daemon::protocol::WorkspaceInfo;
use crate::dashboard::client::DaemonClient;
use crate::dashboard::tmux;
use crate::dashboard::DASHBOARD_SESSION;
use crate::error::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::io;
use std::time::{Duration, Instant};

const REFRESH_INTERVAL: Duration = Duration::from_secs(1);
const LEFT_PANE_WIDTH: u16 = 44;

struct TuiState {
    workspaces: Vec<WorkspaceInfo>,
    selected: usize,
    last_refresh: Instant,
}

impl TuiState {
    fn new(workspaces: Vec<WorkspaceInfo>) -> Self {
        TuiState {
            workspaces,
            selected: 0,
            last_refresh: Instant::now(),
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
    execute!(stdout, EnterAlternateScreen).map_err(crate::error::AgentreeError::Io)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(crate::error::AgentreeError::Io)?;

    let workspaces = client.list_workspaces().unwrap_or_default();
    let mut state = TuiState::new(workspaces);

    let result = run_event_loop(&mut terminal, &mut state, &client);

    // Restore terminal unconditionally
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    result
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut TuiState,
    client: &DaemonClient,
) -> Result<()> {
    loop {
        terminal
            .draw(|f| render(f, state))
            .map_err(crate::error::AgentreeError::Io)?;

        // Poll for input with a 1s timeout for refresh
        if event::poll(REFRESH_INTERVAL).map_err(crate::error::AgentreeError::Io)? {
            if let Event::Key(key) = event::read().map_err(crate::error::AgentreeError::Io)? {
                match (key.modifiers, key.code) {
                    // Quit
                    (_, KeyCode::Char('q')) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
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
                    _ => {}
                }
            }
        }

        // Refresh from daemon on interval
        if state.last_refresh.elapsed() >= REFRESH_INTERVAL {
            if let Ok(ws) = client.list_workspaces() {
                let selected_branch = state
                    .selected_workspace()
                    .map(|w| w.branch.clone());
                state.workspaces = ws;
                // Preserve selection by branch name
                if let Some(branch) = selected_branch {
                    if let Some(idx) = state.workspaces.iter().position(|w| w.branch == branch) {
                        state.selected = idx;
                    } else {
                        state.selected = 0;
                    }
                }
            }
            state.last_refresh = Instant::now();
        }
    }
    Ok(())
}

fn render(f: &mut ratatui::Frame, state: &TuiState) {
    let area = f.area();

    // Outer layout: left list pane + right workspace pane
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(LEFT_PANE_WIDTH),
            Constraint::Fill(1),
        ])
        .split(area);

    render_left(f, chunks[0], state);
    render_right(f, chunks[1], state);
}

fn render_left(f: &mut ratatui::Frame, area: ratatui::layout::Rect, state: &TuiState) {
    // Split into list area + help bar at the bottom
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(3)])
        .split(area);

    // ── workspace list ──
    let items: Vec<ListItem> = state
        .workspaces
        .iter()
        .enumerate()
        .map(|(i, ws)| {
            let attention = if ws.attention.is_some() { "⚑ " } else { "  " };
            let selected_marker = if i == state.selected { ">" } else { " " };
            let ahead = format!("↑{}", ws.commits_ahead);
            let changed = format!("{}f", ws.files_changed);
            let age = format_age(ws.last_activity.as_deref());

            let branch = truncate(&ws.branch, 18);
            let status = format!("{:>3} {:>3}", ahead, changed);

            let line_str = format!(
                "{}{} {:<18} {:<8} {:>7}",
                selected_marker, attention, branch, status, age
            );

            let style = if i == state.selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if ws.attention.is_some() {
                Style::default().fg(Color::Red)
            } else {
                Style::default()
            };

            ListItem::new(Line::from(Span::styled(line_str, style)))
        })
        .collect();

    let header = Line::from(vec![
        Span::styled(
            format!("  {:<20} {:<8} {:>7}", "BRANCH", "STATUS", "AGE"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let list = List::new(items).block(
        Block::default()
            .title(header)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected));

    f.render_stateful_widget(list, inner[0], &mut list_state);

    // ── help bar ──
    let help = Paragraph::new(Line::from(vec![
        Span::styled("[a]", Style::default().fg(Color::Yellow)),
        Span::raw("gent "),
        Span::styled("[t]", Style::default().fg(Color::Yellow)),
        Span::raw("erminal "),
        Span::styled("[e]", Style::default().fg(Color::Yellow)),
        Span::raw("ditor "),
        Span::styled("[c]", Style::default().fg(Color::Yellow)),
        Span::raw("lear "),
        Span::styled("[q]", Style::default().fg(Color::Yellow)),
        Span::raw("uit"),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(help, inner[1]);
}

fn render_right(f: &mut ratatui::Frame, area: ratatui::layout::Rect, state: &TuiState) {
    let content = if let Some(ws) = state.selected_workspace() {
        let mut lines = vec![
            Line::from(Span::styled(
                format!(" Branch: {}", ws.branch),
                Style::default().fg(Color::Cyan),
            )),
            Line::from(Span::raw(format!(" Path:   {}", ws.path))),
        ];

        if let Some(status) = &ws.agent_status {
            lines.push(Line::from(Span::raw("")));
            lines.push(Line::from(Span::styled(
                format!(" Agent:  {}", status.phase),
                Style::default().fg(Color::Green),
            )));
            if let Some(task) = &status.current_task {
                lines.push(Line::from(Span::raw(format!(" Task:   {}", task))));
            }
        }

        if ws.attention.is_some() {
            lines.push(Line::from(Span::raw("")));
            lines.push(Line::from(Span::styled(
                " ⚑ Agent needs attention — press [c] to clear",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )));
        }

        lines.push(Line::from(Span::raw("")));
        lines.push(Line::from(Span::raw(format!(
            " Commits ahead: {}  |  Files changed: {}",
            ws.commits_ahead, ws.files_changed
        ))));

        if let Some(act) = &ws.last_activity {
            lines.push(Line::from(Span::raw(format!(" Last activity: {}", act))));
        }

        lines.push(Line::from(Span::raw("")));
        lines.push(Line::from(Span::styled(
            " Press [a] to open agent, [t] terminal, [e] editor",
            Style::default().fg(Color::DarkGray),
        )));

        lines
    } else {
        vec![
            Line::from(Span::raw("")),
            Line::from(Span::styled(
                " No workspaces found.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::raw(" Create one with: agentree create <branch>")),
        ]
    };

    let paragraph = Paragraph::new(content).block(
        Block::default()
            .title(" Workspace ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(paragraph, area);
}

// ---------------------------------------------------------------------------
// Key actions
// ---------------------------------------------------------------------------

fn action_agent(state: &TuiState) {
    if let Some(ws) = state.selected_workspace() {
        let agent_session = tmux::agent_session_name(&ws.branch);
        let worktree_path = std::path::Path::new(&ws.path);
        let agent_cmd = ws.agent_bin.as_deref().unwrap_or("claude");
        let _ = tmux::ensure_agent_session(&ws.branch, worktree_path, agent_cmd);
        // Attach right pane to agent session
        let attach_cmd = format!("tmux attach -t {}", shell_quote(&agent_session));
        let _ = tmux::respawn_pane(DASHBOARD_SESSION, 1, &attach_cmd);
        let _ = tmux::select_pane(DASHBOARD_SESSION, 1);
    }
}

fn action_terminal(state: &TuiState) {
    if let Some(ws) = state.selected_workspace() {
        let cmd = format!("cd {} && exec $SHELL", shell_quote(&ws.path));
        let _ = tmux::respawn_pane(DASHBOARD_SESSION, 1, &cmd);
        let _ = tmux::select_pane(DASHBOARD_SESSION, 1);
    }
}

fn action_editor(state: &TuiState) {
    if let Some(ws) = state.selected_workspace() {
        let editor = std::env::var("EDITOR")
            .or_else(|_| std::env::var("VISUAL"))
            .unwrap_or_else(|_| "vi".to_string());
        let cmd = format!("{} {}", shell_quote(&editor), shell_quote(&ws.path));
        let _ = tmux::respawn_pane(DASHBOARD_SESSION, 1, &cmd);
        let _ = tmux::select_pane(DASHBOARD_SESSION, 1);
    }
}

fn action_clear_attention(state: &TuiState, client: &DaemonClient) {
    if let Some(ws) = state.selected_workspace() {
        let _ = client.clear_attention(&ws.branch);
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

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", cut)
    }
}

/// Wrap a string in single quotes with internal single quotes escaped.
/// Safe for embedding in POSIX shell command strings.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r#"'"'"'"#))
}
