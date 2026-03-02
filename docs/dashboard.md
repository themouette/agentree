# Dashboard & Daemon

Monitor and interact with all your active workspaces from a single view.

## Overview

When multiple agents run in parallel across worktrees, `agentree dashboard` gives you a unified view: which agents are active, which workspaces need attention, how far ahead each branch is, and quick shortcuts to jump into any workspace.

The system has two components:

- **`agentree daemon`** — background process that tracks workspace state via file watching
- **`agentree dashboard`** — tmux session with a list pane (left) and a dynamic workspace pane (right)

```
┌──────────────────────────────────────┬──────────────────────────────────────┐
│  BRANCH            STATUS    AGE     │                                      │
│⚑ feature/auth       ↑3 12f    2m     │  Branch: feature/auth                │
│  fix/perf           ↑1  4f    8m     │  Path:   /home/user/worktrees/...    │
│> feat/ui            ↑0 23f   just   │                                      │
│──────────────────────────────────────│  Agent:  implementing                │
│ [a]gent [t]erminal [e]ditor [q]uit  │  Task:   OAuth handler               │
└──────────────────────────────────────┴──────────────────────────────────────┘
```

## Quick Start

```bash
# Start the dashboard (launches daemon automatically if needed)
agentree dashboard

# The dashboard opens a tmux session named "agentree-dashboard"
# Press Ctrl+\ at any time to return focus to the list pane

# Kill an existing dashboard session from outside the TUI
agentree dashboard --kill
```

## Requirements

- **tmux** — `brew install tmux` (macOS) or `apt install tmux` (Linux)
- An active git repository with at least one worktree

## Using the Dashboard

### Navigation

| Key | Action |
|-----|--------|
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `a` | Open agent session in right pane |
| `t` | Open terminal in right pane |
| `e` | Open editor (`$EDITOR`) in right pane |
| `c` | Clear attention flag for selected workspace |
| `d` | Detach — leave the dashboard session running in the background |
| `q` | Quit — prompts for confirmation (`y/N`), then kills all workspace panes and the tmux session |
| `?` | Show the welcome / help panel in the right pane |
| `Ctrl+\` | Return focus to list pane from anywhere |

### Status Column

```
  BRANCH              STATUS    AGE
⚑ feature/auth        ↑3 12f    2m
  fix/perf            ↑1  4f    8m
> feat/ui             ↑0 23f    just now
```

- `>` — currently selected workspace
- `⚑` — workspace has an attention request from its agent
- `↑N` — commits ahead of upstream tracking branch
- `Nf` — files with uncommitted changes
- Age — time since last git commit in the worktree

### Agent Sessions

Pressing `a` attaches the right pane to the workspace's agent tmux session (named `agentree:<branch>`). If no agent session exists, one is created by running `agentree agent <branch>` in the worktree.

A 🤖 icon appears next to a workspace row when an agent session is live for that branch. The icon disappears as soon as the session exits.

Before launching Claude Code, `agentree agent` automatically:
- Injects an agentree-specific block into `CLAUDE.md` explaining the status protocol and how to request human attention.
- Merges `.agentree/**` `allowedTools` entries into `.claude/settings.json` so Claude Code can write status files without triggering tool-use prompts.

Both changes are reverted when the agent session exits.

Agent sessions persist independently — pressing `d` to detach from the dashboard leaves all agents running. Pressing `q` (and confirming with `y`) sends SIGTERM to all workspace panes before tearing down the session.

### Right Pane

The right pane shows live information about the selected workspace and accepts any command launched via `a`, `t`, or `e`. Press `Ctrl+\` to return to the list pane.

## Agent Status Protocol

Any agent or script can report its status by writing files into the worktree's `.agentree/` directory. The daemon picks up changes within one second.

### `.agentree/status.json`

Reports what the agent is currently doing:

```json
{
  "phase": "implementing",
  "current_task": "OAuth handler",
  "last_activity": "2026-02-27T10:23:00Z"
}
```

Fields:

| Field | Type | Description |
|-------|------|-------------|
| `phase` | string | Current activity (e.g. `"planning"`, `"implementing"`, `"testing"`, `"waiting"`) |
| `current_task` | string? | Short description of what is being worked on |
| `last_activity` | string? | RFC3339 timestamp of last agent action |

### `.agentree/attention.md`

When an agent needs human input, it writes a non-empty file here:

```markdown
I need clarification on the database schema before proceeding.
Should `user_id` be UUID or integer?
```

The dashboard shows `⚑` next to the branch name. Press `c` to clear the flag after responding. Clearing deletes the file.

### Adding Status Reporting to an Agent

Any tool can participate — shell scripts, custom agents, Claude Code hooks, etc.:

```bash
# Report current phase
mkdir -p .agentree
echo '{"phase":"implementing","current_task":"auth middleware"}' > .agentree/status.json

# Request human attention
echo "Need input on API design — see QUESTIONS.md" > .agentree/attention.md

# Clear attention after human responds
rm .agentree/attention.md
```

For Claude Code, add a hook that writes `status.json` on tool use events and `attention.md` when waiting for input.

## Daemon

The daemon is started automatically by `agentree dashboard` but can also be run manually.

```bash
# Start the daemon manually (runs in foreground; redirect output to manage it)
agentree daemon

# With explicit repo root (useful if not in the repo directory)
agentree daemon --repo-root /path/to/repo
```

### What the Daemon Does

1. Writes its PID to `~/.agentree/daemon.pid`
2. Scans all linked worktrees for the current repository
3. Reads `.agentree/status.json` and `.agentree/attention.md` for each worktree
4. Watches each `.agentree/` directory for file changes (sub-second response)
5. Re-scans the worktree list every 30 seconds to pick up newly created worktrees
6. Listens on `~/.agentree/daemon.sock` for requests from the dashboard

### Socket Protocol

The daemon uses a simple newline-delimited JSON protocol over a Unix socket. Each connection is one-shot: send one request, receive one response, close.

**Request:**
```json
{"cmd": "list"}
{"cmd": "clear_attention", "branch": "feature/auth"}
```

**Response:**
```json
{"Workspaces": [...]}
{"Ok": null}
{"Err": "error message"}
```

You can query the daemon directly for scripting:

```bash
echo '{"cmd":"list"}' | socat - UNIX-CONNECT:~/.agentree/daemon.sock | jq .
```

### Runtime Files

| Path | Description |
|------|-------------|
| `~/.agentree/daemon.sock` | Unix socket for dashboard–daemon communication |
| `~/.agentree/daemon.pid` | PID of the running daemon |

## Architecture

```
agentree dashboard
       │
       ├── starts agentree daemon (if not running)
       │         │
       │         ├── reads: {worktree}/.agentree/status.json
       │         ├── reads: {worktree}/.agentree/attention.md
       │         ├── watches: {worktree}/.agentree/ (notify, <1s)
       │         ├── rescans worktrees every 30s
       │         └── serves: ~/.agentree/daemon.sock
       │
       └── opens tmux session "agentree-dashboard"
                 │
                 ├── pane 0 (left, 44 cols): agentree dashboard --tui
                 │         │
                 │         └── polls daemon every 1s via Unix socket
                 │
                 └── pane 1 (right): agent / terminal / editor
```

## Troubleshooting

**Dashboard says daemon is not running**

Try starting it manually to see error output:
```bash
agentree daemon
```

**`⚑` flag appears but does not go away after pressing `c`**

Check that the daemon has write permission to the worktree directory. The `c` key deletes `.agentree/attention.md`.

**Right pane is empty / shows wrong content**

The right pane is a regular tmux pane. If it shows stale content, press `t` to open a fresh shell or `a` to reattach to the agent session.

**tmux session already exists with wrong layout**

Kill the session and restart:
```bash
tmux kill-session -t agentree-dashboard
agentree dashboard
```

**Commits-ahead shows 0 when it should not**

The daemon queries `git rev-list --count HEAD ^@{u}` which requires a configured upstream tracking branch. Set one with:
```bash
git branch --set-upstream-to=origin/<branch> <branch>
```
