# Roadmap: Agentree Dashboard

**Milestone:** Dashboard v1 — working end-to-end
**Goal:** `agentree dashboard` opens reliably, shows workspace status, and lets you
jump into any agent session in one keypress.

---

## Phase 1: Fix Daemon Startup Reliability

**Goal:** `agentree daemon` starts, stays running, and `agentree dashboard` doesn't
hang while waiting for it.

**Why first:** Everything downstream (TUI, actions) requires a stable daemon. The
current "hang" comes from the daemon failing silently.

**Covers:** DMN-01, DMN-02, DMN-03, DMN-08, DASH-01, DASH-02

**Plans:** 2/2 plans complete

Plans:
- [x] 01-01-PLAN.md — Daemon reliability: logging module, stale PID fix, signal handlers, --status command
- [x] 01-02-PLAN.md — Dashboard ensure_daemon: TTY-aware spinner, 50ms poll, actionable timeout error

### Changes

1. **Daemon log file** — redirect daemon stdout/stderr to `~/.agentree/daemon.log`
   instead of `/dev/null`. This is the single most important debug tool.

2. **Fix `ensure_daemon()` hang** — replace the silent 5-second poll with:
   - Print "Starting agentree daemon..." to stderr while polling
   - On timeout: show log file location in error message
   - Reduce poll interval to 50ms (5s timeout stays)

3. **PID file robustness** — on daemon startup, if a stale PID file exists for a dead
   process: silently remove it. Current code warns and exits, leaving dashboard stuck.

4. **Socket cleanup on exit** — register a signal handler (SIGTERM, SIGINT) to
   remove `daemon.sock` and `daemon.pid` before exiting.

5. **Daemon health check command** — `agentree daemon --status` prints whether daemon
   is running (reads PID file, checks process).

### Validation

- `agentree daemon` starts, prints log file path, and PID
- `agentree daemon` (second run) exits with "already running" message
- Kill daemon with Ctrl+C → socket and PID file are removed
- `agentree dashboard` on a cold start shows progress, not silence

---

## Phase 2: Dashboard Launch + TUI Workspace List

**Goal:** `agentree dashboard` opens the tmux session with a working workspace list
in the left pane.

**Why after Phase 1:** Daemon must be reliable before we can test the dashboard end-to-end.

**Covers:** DMN-04, DMN-05, DMN-06, DMN-07, DASH-03, DASH-04, DASH-05, DASH-06, TUI-01..TUI-06

**Plans:** 3 plans

**Plans: 3/3 complete ✓**

Plans:
- [x] 02-01-PLAN.md — TUI rendering fixes: full-screen mode, full-row highlight, middle-truncation, robot icon, startup states
- [x] 02-02-PLAN.md — Dashboard session management: nested tmux warning, idempotent reattach, dead TUI respawn, daemon verification
- [x] 02-03-PLAN.md — Human verification: end-to-end dashboard smoke test

### Changes

1. **Fix `--tui` mode rendering** — in `--tui` mode (running inside 44-col left pane),
   render ONLY the workspace list (full pane height/width). Remove the `render_right()`
   call from `run_tui()`. The static info panel is unnecessary; the tmux right pane
   is what the user sees.

2. **Verify session layout** — test that `create_session` + `split_horizontal` +
   `resize_pane` produces the expected layout (left: 44 cols TUI, right: dynamic).
   The current `split_horizontal` target is `session:0` which may need to be
   `session:0.0` for correct pane targeting after session creation.

3. **Fix pane numbering** — `split_window -h` creates pane 1; `respawn_pane` targets
   `session:0.0` and `session:0.1`. Verify these indices are correct after the split.

4. **Empty workspace state** — when no worktrees exist, TUI shows a helpful message:
   `No workspaces yet. Run: agentree create <branch>`

5. **Git stats in TUI** — commits ahead and files changed already use `git rev-list`
   and `git status --porcelain`. Verify these work for worktrees that don't have an
   upstream branch (commits ahead shows 0, not error).

6. **Idempotent dashboard** — `DASH-05`: if `agentree-dashboard` session already
   exists, just attach to it instead of erroring.

### Validation

- `agentree dashboard` opens a tmux session (visible in `tmux ls`)
- Left pane shows workspace list with correct data
- Navigation (`j/k`, `↑/↓`) works
- `q` quits TUI, leaving tmux session intact
- `Ctrl+\` returns focus to left pane

---

## Phase 3: Right Pane Actions

**Goal:** `a`, `t`, `e` keys populate the right pane correctly.

**Why after Phase 2:** Right pane actions require a working session layout.

**Covers:** ACT-01, ACT-02, ACT-03, ACT-04, AGNT-01, AGNT-02, AGNT-03

**Plans:** 3/3 complete

Plans:
- [x] 03-01-PLAN.md — tmux.rs: fix session naming (dash not colon), add ensure_named_session, is_session_pane_dead, unit tests
- [x] 03-02-PLAN.md — ui.rs: fix action handlers with persistent sessions + switch-client, session indicator icons, status bar error
- [x] 03-03-PLAN.md — Human verification: end-to-end right pane actions smoke test

### Changes

1. **Agent action (`a`)** — `ensure_agent_session()` creates `agentree:{safe-branch}`
   if not exists, then `respawn_pane(session, 1, "tmux attach -t agentree:{branch}")`.
   Fix: tmux session names with `:` are valid but tmux parses `agentree:{branch}` as
   `session:window`. Use `agentree-{branch}` or prefix differently to avoid conflict.

2. **Terminal action (`t`)** — `cd {path} && exec $SHELL` piped through `respawn_pane`.
   Verify shell quoting handles paths with spaces.

3. **Editor action (`e`)** — `$EDITOR {path}` in right pane. Verify `$EDITOR` fallback
   chain: EDITOR → VISUAL → vi.

4. **Clear attention (`c`)** — sends `ClearAttention` to daemon, removes
   `.agentree/attention.md`. Verify daemon state updates immediately.

5. **Return to left pane** — after triggering a right pane action, the TUI stays
   active in the left pane. Verify `Ctrl+\` binding works.

### Validation

- Select a workspace, press `a` → right pane shows agent (or creates session)
- Press `t` → right pane shows shell in worktree directory
- Press `e` → right pane shows editor in worktree directory
- Press `c` on attention-flagged workspace → flag cleared, TUI updates within 1s
- Agent session persists after detaching from dashboard

---

## Phase 4: Status Protocol

**Goal:** Agents can report their status to the dashboard; the TUI shows it.

**Why after Phase 3:** Core interaction model must work before adding the status layer.

**Covers:** STAT-01, STAT-02, STAT-03

**Plans:** 3 plans

**Plans: 3/3 complete**

Plans:
- [x] 04-01-PLAN.md — Protocol + worktree setup: fix AgentStatus struct, create templates/CLAUDE.md, wire .agentree/ dir + git exclude in create_worktree()
- [x] 04-02-PLAN.md — TUI status display: two-line workspace rows, yellow attention highlight, optimistic c-key clear
- [x] 04-03-PLAN.md — Human verification: status protocol end-to-end smoke test

### Changes

1. **Document `.agentree/status.json` format:**
   ```json
   {
     "phase": "implementing",
     "current_task": "Writing tests for auth module",
     "last_activity": "2026-02-27T14:30:00Z"
   }
   ```

2. **Update CLAUDE.md worktree template** — add a section instructing agents to write
   status files. The template is injected by `agentree create` into new worktrees.
   Find the template location and add the protocol instructions.

3. **Verify daemon reads status** — `DaemonState::read_agent_status()` already reads
   `.agentree/status.json`. Verify the file watcher picks up changes and triggers
   `update_workspace()`. Add a test with a mock status file.

4. **TUI status display** — the current `render_left()` shows branch + git stats but
   not agent status text. Add a truncated `current_task` display in the workspace list
   row (if available).

### Validation

- Write a mock `.agentree/status.json` in a worktree
- Dashboard shows the status within 2 seconds (watcher picks it up)
- Write a mock `.agentree/attention.md` → dashboard shows ⚑ flag
- Remove the file → flag clears within 2 seconds

---

## Phase 5: Tests and Hardening

**Goal:** Automated tests covering the core flows; edge cases handled gracefully.

**Why last:** Tests are most valuable once the happy path is stable.

**Covers:** TEST-01, TEST-02, TEST-03, TEST-04, TEST-05

### Changes

1. **Daemon integration test** (`TEST-01`) — spin up a real daemon against a temp
   git repo, send a `list` request via the Unix socket, verify response.

2. **Daemon rescan test** (`TEST-02`) — create a worktree after daemon starts, wait
   for 30s rescan (or trigger it), verify new workspace appears.

3. **TUI unit tests** (`TEST-03`) — mock `DaemonClient` returning canned `WorkspaceInfo`
   data, render TUI into a `TestBackend`, assert output contains expected text.

4. **Tmux name sanitization** (`TEST-04`) — unit test that `agent_session_name`
   converts `feature/my-branch` to a valid tmux name.

5. **`format_age` tests** (`TEST-05`) — unit tests for all time buckets (just now,
   2m, 3h, 1d).

6. **Edge cases:**
   - Worktree with no upstream branch (commits ahead = 0, not error)
   - Worktree path doesn't exist (daemon skips it gracefully)
   - Daemon socket permission error (actionable error message)
   - Dashboard inside tmux already (`$TMUX` set) — either nest or warn

### Validation

- `cargo test` passes with no failures
- `cargo clippy` produces no warnings
- Manual smoke test: start dashboard, navigate workspaces, launch agent, clear attention

---

## Success Criteria

The milestone is complete when:
1. `agentree dashboard` opens on a real terminal without hanging
2. Workspace list shows all worktrees with git stats
3. `a` / `t` / `e` populate the right pane correctly
4. Writing `.agentree/status.json` in a worktree → updates in TUI within 2s
5. All `cargo test` pass

### Phase 6: Improve the UI

**Goal:** Polish the TUI workspace list with a compact footer, clean attention visuals, agent phase display, scrollbar, and a kill-dashboard command.
**Requirements**: UI-01 (compact footer), UI-02 (attention per-span color), UI-03 (phase display), UI-04 (scrollbar), UI-05 (kill-dashboard)
**Depends on:** Phase 5
**Plans:** 3 plans

Plans:
- [ ] 06-01-PLAN.md — TUI polish: compact footer, attention per-span color, phase tag on row, scrollbar
- [ ] 06-02-PLAN.md — kill-dashboard: `agentree dashboard --kill` tears down session + stops daemon
- [ ] 06-03-PLAN.md — Human verification: Phase 6 UI improvements smoke test

---
*Roadmap created: 2026-02-27*
