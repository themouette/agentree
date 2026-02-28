# Requirements: Agentree Dashboard

**Defined:** 2026-02-27
**Core Value:** See all workspaces at a glance and jump into any agent session in one keypress.

## v1 Requirements

### Daemon

- [x] **DMN-01**: `agentree daemon` starts and listens on `~/.agentree/daemon.sock`
- [x] **DMN-02**: Daemon detects and refuses to start if already running (PID check)
- [x] **DMN-03**: Daemon logs startup errors to `~/.agentree/daemon.log` (not /dev/null)
- [x] **DMN-04**: Daemon responds to `{"cmd": "list"}` with workspace list
- [x] **DMN-05**: Daemon watches `.agentree/status.json` in each worktree and updates state
- [x] **DMN-06**: Daemon watches `.agentree/attention.md` in each worktree
- [x] **DMN-07**: Daemon rescans for new worktrees periodically (every 30s)
- [x] **DMN-08**: Daemon cleans up socket and PID file on exit

### Dashboard Launch

- [x] **DASH-01**: `agentree dashboard` starts daemon if not running (with visible progress)
- [x] **DASH-02**: Daemon startup timeout produces actionable error with log file location
- [x] **DASH-03**: `agentree dashboard` creates tmux session `agentree-dashboard`
- [x] **DASH-04**: Session has left pane (44 cols, TUI) and right pane (dynamic content)
- [x] **DASH-05**: Reattaches to existing session if already running (idempotent)
- [x] **DASH-06**: `Ctrl+\` keybinding always returns focus to left pane

### TUI (Left Pane)

- [x] **TUI-01**: Workspace list shows: attention flag, branch name, commits ahead, files changed, last activity
- [x] **TUI-02**: Selected workspace is highlighted; navigate with `↑/↓` or `j/k`
- [x] **TUI-03**: Attention-flagged workspaces show in red with ⚑ indicator
- [x] **TUI-04**: Help bar at bottom shows available keybindings
- [x] **TUI-05**: List refreshes from daemon every 1 second
- [x] **TUI-06**: `q` quits the TUI (TUI only, not the whole dashboard session)

### Right Pane Actions

- [x] **ACT-01**: `a` opens agent session in right pane (creates tmux session if needed)
- [x] **ACT-02**: `t` opens terminal (`$SHELL`) in worktree directory in right pane
- [x] **ACT-03**: `e` opens `$EDITOR` in worktree directory in right pane
- [x] **ACT-04**: `c` clears attention flag for selected workspace

### Agent Sessions

- [x] **AGNT-01**: Agent sessions are named `agentree:{branch}` (with `/` → `-`)
- [x] **AGNT-02**: `a` creates session if not exists, then attaches right pane
- [x] **AGNT-03**: Agent sessions persist after dashboard is closed

### Status Protocol

- [x] **STAT-01**: `.agentree/status.json` format is documented (phase, current_task, last_activity)
- [x] **STAT-02**: CLAUDE.md worktree template instructs agents to write status files
- [x] **STAT-03**: Daemon reads and displays agent status in TUI workspace list

### Tests

- [ ] **TEST-01**: Integration test: daemon starts, socket appears, responds to `list`
- [ ] **TEST-02**: Integration test: daemon detects new worktree after 30s rescan
- [ ] **TEST-03**: Integration test: TUI renders with mock daemon
- [ ] **TEST-04**: Unit test: tmux session name sanitization
- [ ] **TEST-05**: Unit test: `format_age` displays correct relative times

## v2 Requirements

### Enhanced Status

- **V2-STAT-01**: Daemon exposes `{"cmd": "status", "branch": "..."}` for detailed workspace info
- **V2-STAT-02**: Agent writes progress percentage to status file

### Dashboard Polish

- **V2-DASH-01**: `agentree dashboard stop` stops daemon and kills session
- **V2-DASH-02**: Dashboard survives daemon restart without losing state

### Multi-repo

- **V2-REPO-01**: Daemon can track worktrees across multiple repos
- **V2-REPO-02**: Dashboard filters by repo

## Out of Scope

| Feature | Reason |
|---------|--------|
| Windows support | Unix socket IPC required; no PTY abstraction for Windows |
| Tauri/web GUI | tmux handles PTY for free; no need for desktop bundle |
| `agentree agent` standalone tmux session | Standalone mode unchanged; tmux sessions only from dashboard |
| Nested tmux detection | Too complex; assume user isn't already inside tmux |
| Real-time push from daemon | Polling every 1s is sufficient; avoids async complexity |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| DMN-01, DMN-02, DMN-03, DMN-08 | Phase 1 | Pending |
| DASH-01, DASH-02 | Phase 1 | Pending |
| DMN-04, DMN-05, DMN-06, DMN-07 | Phase 2 | Pending |
| DASH-03, DASH-04, DASH-05, DASH-06 | Phase 2 | Pending |
| TUI-01, TUI-02, TUI-03, TUI-04, TUI-05, TUI-06 | Phase 2 | Pending |
| ACT-01, ACT-02, ACT-03, ACT-04 | Phase 3 | Pending |
| AGNT-01, AGNT-02, AGNT-03 | Phase 3 | Pending |
| STAT-01, STAT-02, STAT-03 | Phase 4 | Pending |
| TEST-01, TEST-02, TEST-03, TEST-04, TEST-05 | Phase 5 | Pending |

**Coverage:**
- v1 requirements: 29 total
- Mapped to phases: 29
- Unmapped: 0 ✓

---
*Requirements defined: 2026-02-27*
*Last updated: 2026-02-27 after initial definition*
