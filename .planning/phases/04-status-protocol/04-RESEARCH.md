# Phase 4: Status Protocol - Research

**Researched:** 2026-02-28
**Domain:** Rust/Ratatui TUI status display, git worktree file exclusion, daemon file watching, agent protocol documentation
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Communication mechanism:**
- Agent writes to `.agentree/status.json` relative to the worktree root (fixed path, no env var needed)
- On `agentree create`, write `.agentree/` to the worktree's per-worktree git exclude file: `main-repo/.git/worktrees/{branch}/info/exclude`
- The `.agentree/` directory itself should be created by `agentree create` so the agent doesn't need to mkdir
- Backend trait gets a `status_dir(ctx: &WorkspaceContext) -> PathBuf` method with a default impl returning `ctx.workspace_path.join(".agentree")`. Backends that have broader filesystem access can override
- Daemon calls `backend.status_dir(ctx)` when determining what paths to watch — not hardcoded to worktree path

**Attention flag:**
- Agent writes `.agentree/attention.md` when it needs human attention
- Presence of the file = flagged. First line is shown in TUI as the status text
- File is deleted by pressing `c` in the TUI (via `ClearAttention` daemon command)
- Daemon watcher detects deletion and clears the flag in state

**TUI status display:**
- Workspace rows have an optional second line showing `current_task` text, truncated to fit pane width
- When `status.json` does not exist: single-line row (no placeholder text)
- Attention state: `⚑` icon before branch name + full row background highlighted (yellow/orange tint). Second line shows first line of `attention.md` content
- `c` key behavior: optimistic clear — TUI removes `⚑` and highlight immediately on keypress

**Status file format:**
- Two fields: `phase` (free text) and `current_task` (free text)
- `last_activity` is NOT agent-written — daemon derives it from file mtime
- No format versioning. Lenient parsing: missing fields treated as `None`, unknown fields silently ignored
- Both fields are optional in the struct

**TUI rendering of status fields:**
- Second line shows `current_task` only (no `phase` prefix)
- `phase` field is not displayed in left pane for now

**Agent instructions (CLAUDE.md worktree template):**
- Write status on meaningful transitions (new task, phase switch, blocked, completing session)
- Do NOT write on every tool call (~every 5-10 minutes during long tasks)
- End of session: delete `status.json`
- Attention: write `attention.md` when blocked or needing human input. First line must be clear and actionable
- Path is always `.agentree/` relative to worktree root

### Claude's Discretion
- Exact color/style for attention row highlight (yellow, orange, red)
- Truncation strategy for `current_task` (mid-truncation with `…` or right-trim)
- How daemon discovers per-backend status dirs when there is no active worktree session

### Deferred Ideas (OUT OF SCOPE)
- Remote VM backend status communication (no shared filesystem)
- Displaying `phase` field in the TUI
- `agentree status set` CLI helper for agents
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| STAT-01 | `.agentree/status.json` format is documented (phase, current_task, last_activity) | Protocol format clarified: both fields optional; `last_activity` is daemon-derived from mtime, not agent-written |
| STAT-02 | CLAUDE.md worktree template instructs agents to write status files | No existing template injection mechanism in `create.rs` — new code needed to write CLAUDE.md template to worktree |
| STAT-03 | Daemon reads and displays agent status in TUI workspace list | `read_agent_status()` already exists; `AgentStatus` struct needs `phase` made optional; TUI needs a second line in ListItem |
</phase_requirements>

---

## Summary

Phase 4 is a focused integration phase: the protocol infrastructure (file watcher, status reading, attention clearing) is already implemented in the daemon layer. The work is mostly about wiring up the remaining pieces: fixing the `AgentStatus` struct to match the agreed-upon format, creating the `.agentree/` directory and writing a CLAUDE.md template on `agentree create`, and adding a second display line to the TUI workspace list for `current_task`.

There is one critical gap between the CONTEXT.md design and git behavior: the per-worktree `$GIT_DIR/info/exclude` path (`main-repo/.git/worktrees/{branch}/info/exclude`) is **not** independently readable by git in linked worktrees. Git resolves `$GIT_DIR/info/` through `$GIT_COMMON_DIR/info/` (the shared main repo), making per-worktree exclusions via `info/exclude` ineffective. The implementation must write to the **shared** `$GIT_COMMON_DIR/info/exclude` instead. This is a minor deviation from the decision text but achieves the same goal (clean `git status`).

The TUI change is well-supported by ratatui 0.29.0's `ListItem` API, which natively accepts `Text` with multiple `Line`s for variable-height rows.

**Primary recommendation:** Fix `AgentStatus` struct first (make `phase` optional, remove agent-written `last_activity`), then add `.agentree/` setup in `create_worktree()`, then update TUI for two-line rows, then write the CLAUDE.md template content.

---

## Standard Stack

### Core (already in Cargo.toml)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ratatui | 0.29.0 | TUI rendering | Already used; `ListItem::new(Text)` supports multi-line rows natively |
| serde_json | 1.0 | Status file parsing | Already used in protocol.rs for `AgentStatus` |
| notify | 6 | File watching | Already used in daemon/watcher.rs; watches `.agentree/` dirs |
| std::fs | stdlib | Directory creation, file writes | Used throughout codebase |

### No New Dependencies
All work for this phase uses existing dependencies. No `cargo add` required.

---

## Architecture Patterns

### Existing Code to Modify (not create)

**1. `src/daemon/protocol.rs` — `AgentStatus` struct**

Current (incorrect per CONTEXT.md):
```rust
pub struct AgentStatus {
    pub phase: String,              // Should be Option<String>
    pub current_task: Option<String>,
    pub last_activity: Option<String>,  // Should be removed (daemon-derived from mtime)
}
```

Required per CONTEXT.md decisions:
```rust
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(default)]  // Lenient: missing fields = None
pub struct AgentStatus {
    pub phase: Option<String>,
    pub current_task: Option<String>,
    // last_activity removed — daemon reads file mtime, not this field
}
```

**2. `src/daemon/state.rs` — `build_workspace_info()` and `get_last_activity()`**

Current `last_activity` reads the last git commit timestamp. The file-mtime approach (for `last_activity` derived from `.agentree/status.json` mtime) is an enhancement on top of this. The `WorkspaceInfo.last_activity` field currently shows git commit time — the CONTEXT.md says daemon derives `last_activity` from file mtime, but this is for the agent status domain. The existing `last_activity` in `WorkspaceInfo` uses git log timestamps.

Action: Keep `WorkspaceInfo.last_activity` as git-based. The daemon can optionally update it from status file mtime when status exists.

**3. `src/worktree/operations.rs` — `create_worktree()`**

Add after successful git worktree creation:
```rust
// Create .agentree/ directory
let agentree_dir = worktree_path.join(".agentree");
std::fs::create_dir_all(&agentree_dir)?;

// Write CLAUDE.md with status protocol instructions
let claude_md_content = include_str!("../../templates/CLAUDE.md");
std::fs::write(agentree_dir.join("../CLAUDE.md"), claude_md_content)?;
// Or: write to a known path in the worktree

// Add .agentree/ to shared info/exclude (see git exclude section below)
add_agentree_to_exclude(&worktree_path)?;
```

**4. `src/dashboard/ui.rs` — `render_workspace_list()`**

Change `ListItem` construction from single-line to two-line:
```rust
// Instead of:
ListItem::new(Line::from(spans)).style(item_style)

// Use:
let mut lines = vec![Line::from(spans)];
if let Some(ref status) = ws.agent_status {
    if let Some(ref task) = status.current_task {
        let truncated = truncate_right(task, list_width.saturating_sub(4));
        lines.push(Line::from(vec![
            Span::raw("    "),  // indent to align under branch name
            Span::styled(truncated, Style::default().fg(Color::DarkGray)),
        ]));
    }
} else if let Some(ref attention) = ws.attention {
    // Show first line of attention.md as second line
    let first_line = attention.lines().next().unwrap_or("").to_string();
    let truncated = truncate_right(&first_line, list_width.saturating_sub(4));
    lines.push(Line::from(vec![
        Span::raw("    "),
        Span::styled(truncated, Style::default().fg(Color::Yellow)),
    ]));
}
let text = ratatui::text::Text::from(lines);
ListItem::new(text).style(item_style)
```

**Note on attention display:** Per CONTEXT.md decisions, attention row shows first line of `attention.md` as the second line. When attention is set, `current_task` is NOT shown (attention takes priority).

### Attention Row Highlight Colors

Per Claude's Discretion (research recommendation): Use `Color::Yellow` for background (not red — red is already used for the `⚑` icon; yellow gives a distinct "needs attention" look without alarm). For the `⚑` icon color: keep it `Color::Red` as currently implemented. For the row background: `Color::Yellow` with dark text.

Current code uses `Style::default().bg(Color::Red)` for non-selected attention rows. Change to `Color::Yellow` with black foreground:
```rust
let item_style = if ws.attention.is_some() && i != state.selected {
    Style::default().bg(Color::Yellow).fg(Color::Black)
} else {
    Style::default()
};
```

### Git Exclude: The Critical Implementation Detail

**CONTEXT.md says:** Write `.agentree/` to `main-repo/.git/worktrees/{branch}/info/exclude`

**Reality (verified by testing):** Git resolves `$GIT_DIR/info/` to `$GIT_COMMON_DIR/info/` in linked worktrees. The per-worktree `info/` directory is ignored by git. Writing to `main-repo/.git/worktrees/{branch}/info/exclude` has NO EFFECT on git status.

**Correct implementation:** Write `.agentree/` to the **shared** `$GIT_COMMON_DIR/info/exclude` = `main-repo/.git/info/exclude`. This is shared across all worktrees of the same repo, which is acceptable because:
- `.agentree/` will be created in ALL worktrees by `agentree create`
- All worktrees benefit from the exclusion
- The pattern is per-repo (correct scope), not per-team (not `.gitignore`)

Implementation:
```rust
fn add_agentree_to_exclude(worktree_path: &Path) -> Result<()> {
    // Resolve GIT_COMMON_DIR: follow worktree's .git file to gitdir,
    // then read commondir file to find the shared .git
    let gitdir = resolve_gitdir(worktree_path)?;
    let commondir = resolve_commondir(&gitdir)?;
    let info_dir = commondir.join("info");
    let exclude_file = info_dir.join("exclude");

    std::fs::create_dir_all(&info_dir).map_err(AgentreeError::Io)?;

    // Read existing exclude content; add .agentree/ if not already present
    let existing = std::fs::read_to_string(&exclude_file).unwrap_or_default();
    if !existing.lines().any(|l| l.trim() == ".agentree/") {
        let mut content = existing;
        if !content.ends_with('\n') && !content.is_empty() {
            content.push('\n');
        }
        content.push_str("# agentree status directory (local, per-worktree)\n");
        content.push_str(".agentree/\n");
        std::fs::write(&exclude_file, content).map_err(AgentreeError::Io)?;
    }
    Ok(())
}
```

Where `resolve_commondir()` reads the `commondir` file:
```rust
fn resolve_commondir(gitdir: &Path) -> Result<PathBuf> {
    let commondir_file = gitdir.join("commondir");
    if commondir_file.exists() {
        let rel = std::fs::read_to_string(&commondir_file)
            .map_err(AgentreeError::Io)?;
        let rel = rel.trim();
        // commondir contains "../.." relative path to main .git
        let resolved = gitdir.join(rel).canonicalize()
            .map_err(AgentreeError::Io)?;
        Ok(resolved)
    } else {
        // Already at the main repo gitdir
        Ok(gitdir.to_path_buf())
    }
}
```

**Alternative approach if the shared exclude is undesirable:** Accept that `.agentree/` shows as untracked and document it in the CLAUDE.md template ("These files are intentionally untracked. Run `agentree create` to configure git to hide them."). LOW preference — the shared exclude is cleaner.

### CLAUDE.md Worktree Template

Currently there is NO template injection mechanism in `create.rs` or `operations.rs`. The CLAUDE.md content needs to be embedded in the binary or read from a template file.

**Recommended approach:** Use `include_str!()` at compile time to embed the template content. Place the template at `templates/CLAUDE.md` in the project root.

The template content (per CONTEXT.md decisions):
```markdown
# Agentree Status Protocol

Your current directory is a git worktree managed by `agentree`. The dashboard
monitors this workspace via `.agentree/` files.

## Reporting Status

Write `.agentree/status.json` on meaningful transitions (do NOT write on every
tool call — approximately every 5–10 minutes during long tasks):

```json
{
  "phase": "implementing",
  "current_task": "Writing auth tests for oauth flow"
}
```

Phases: `planning`, `implementing`, `testing`, `reviewing`, `waiting`, or any
descriptive string.

**End of session:** Delete `status.json`. The dashboard row returns to
single-line display.

## Requesting Human Attention

Write `.agentree/attention.md` when blocked or needing human input:

```
Need OPENAI_API_KEY to proceed with integration tests.
```

First line must be a clear, actionable message (shown in dashboard).
The file content can provide full context.

**After human responds:** The human presses `c` in the dashboard to clear
the flag. You may also delete the file yourself once unblocked.

## File Paths (relative to worktree root)

- `.agentree/status.json` — current phase and task
- `.agentree/attention.md` — attention request (create when blocked)

These files are excluded from git tracking automatically.
```

**Placement of CLAUDE.md write in `create_worktree()`:**

Write CLAUDE.md to the worktree root (not inside `.agentree/`) — this is the standard location that Claude Code reads on startup:
```rust
let claude_md_path = worktree_path.join("CLAUDE.md");
// Append to existing CLAUDE.md if present, or create new
// Alternatively: only write if no CLAUDE.md exists
if !claude_md_path.exists() {
    std::fs::write(&claude_md_path, AGENTREE_CLAUDE_MD_TEMPLATE)?;
}
```

**Note:** `CLAUDE.md` is typically in `.gitignore` or tracked by the project. Writing it on `agentree create` will show as an untracked file unless the project already has `.gitignore` rules for it. This is acceptable — agents will see the instructions immediately. The developer may choose to `.gitignore` it.

### Daemon: `status_dir()` Backend Trait Method

Per CONTEXT.md decisions, the `Backend` trait needs a `status_dir()` method:

```rust
pub trait Backend {
    // ... existing methods ...

    /// Returns the path to the status directory for a given workspace.
    /// Daemon watches this directory for status.json and attention.md.
    /// Default implementation returns `workspace_path/.agentree/`.
    /// Backends with broader filesystem access (e.g. non-sandboxed local)
    /// may override to return a central path.
    fn status_dir(&self, workspace_path: &Path) -> PathBuf {
        workspace_path.join(".agentree")
    }
}
```

This is a default-impl method on the trait — no existing backend needs to override it in v1. The daemon currently hardcodes `.agentree/` via `get_all_agentree_paths()` using `info.path + "/.agentree"`. The daemon doesn't instantiate backends; it uses `DaemonState` directly.

**Practical implementation for v1:** The daemon does not have easy access to backend instances (it uses `DaemonState` which knows workspace paths, not backends). For v1, keep the current hardcoded `.agentree/` approach in `get_all_agentree_paths()` — the `status_dir()` trait method is added to the trait for future use but the daemon doesn't call it yet. This satisfies the STAT-03 requirement without over-engineering.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Multi-line list items | Custom widget or manual cursor positioning | `ListItem::new(Text::from(vec![line1, line2]))` | ratatui 0.29 supports natively |
| JSON parsing with unknown fields | Custom parser | `#[serde(default)]` on AgentStatus + serde_json | `deny_unknown_fields` must NOT be used; unknown fields must be silently ignored |
| Checking if a pattern is already in exclude | Regex | Simple `lines().any(|l| l.trim() == ".agentree/")` | File is human-readable text; line comparison is sufficient |

---

## Common Pitfalls

### Pitfall 1: AgentStatus Phase Field
**What goes wrong:** `phase: String` (non-optional) fails deserialization when `phase` is absent from the JSON. Agent may write `{}` or `{"current_task": "..."}` without `phase`.
**Why it happens:** Current struct uses non-optional `String` for `phase`.
**How to avoid:** Change to `Option<String>` and add `#[serde(default)]` on the struct.
**Warning signs:** `read_agent_status()` returns `None` even when `status.json` exists — this silently swallows parse errors via `.ok()`.

### Pitfall 2: Per-Worktree info/exclude Does Not Work
**What goes wrong:** Writing to `main-repo/.git/worktrees/{branch}/info/exclude` has NO EFFECT. Git resolves `$GIT_DIR/info/` to `$GIT_COMMON_DIR/info/` for linked worktrees.
**Why it happens:** Git worktree documentation (gitrepository-layout) states: "This directory is ignored if $GIT_COMMON_DIR is set and `$GIT_COMMON_DIR/info` will be used instead."
**How to avoid:** Write `.agentree/` pattern to `$GIT_COMMON_DIR/info/exclude` = `main-repo/.git/info/exclude` (shared across worktrees of the same repo).
**Warning signs:** `git status` in the worktree still shows `.agentree/` as `??` untracked even after writing the per-worktree exclude file.

### Pitfall 3: ListItem Multi-Line and Variable Row Height
**What goes wrong:** If rows have variable height (1 line vs 2 lines), the `selected` index in `ListState` may jump visually when switching between single and double-line items during scrolling.
**Why it happens:** ratatui 0.29 fixed multi-line list scrolling (PR #1553) but selection tracking uses item count, not line count.
**How to avoid:** Test with a mix of workspaces (some with status, some without) and verify selection doesn't jump. The fix in 0.29 addresses this.
**Warning signs:** Selected workspace highlight appears on wrong row after navigating past a two-line item.

### Pitfall 4: Attention Second Line vs current_task Second Line
**What goes wrong:** When attention is set AND current_task exists, both want the second line.
**Why it happens:** The design says attention takes priority — show first line of `attention.md` as the second row line.
**How to avoid:** Check `ws.attention` first; only show `current_task` if attention is None.
**Warning signs:** Second line shows task text instead of attention message when workspace has both.

### Pitfall 5: CLAUDE.md Template File Not Found at Compile Time
**What goes wrong:** `include_str!("../../templates/CLAUDE.md")` fails compilation if file doesn't exist.
**Why it happens:** `include_str!()` is evaluated at compile time.
**How to avoid:** Create the template file before adding the `include_str!()` call. Make Wave 0 task create the file first.

### Pitfall 6: Writing CLAUDE.md Overwrites Existing Project CLAUDE.md
**What goes wrong:** `agentree create` on a branch that already has a committed `CLAUDE.md` will overwrite it with the status template.
**Why it happens:** `create_worktree()` writes the file without checking if it exists.
**How to avoid:** Check if `CLAUDE.md` already exists; append or skip if present. Decision: skip (don't overwrite). The agent can find the status protocol docs elsewhere.
**Warning signs:** Agent sees old CLAUDE.md replaced by boilerplate.

### Pitfall 7: Optimistic Clear — TUI State vs Daemon State
**What goes wrong:** `action_clear_attention()` sends to daemon but doesn't clear local TUI state immediately. Flag persists until next 1s poll.
**Why it happens:** Current `action_clear_attention()` sends `clear_attention` to daemon and relies on next poll. CONTEXT.md says "optimistic clear" — remove `⚑` immediately on keypress.
**How to avoid:** After calling `client.clear_attention()`, immediately mutate `state.workspaces[i].attention = None`. The next poll confirms it.
**Warning signs:** After pressing `c`, the `⚑` stays visible for up to 1 second.

---

## Code Examples

### Multi-Line ListItem with Text

```rust
// Source: docs.rs/ratatui/0.29.0/ratatui/widgets/struct.ListItem.html
use ratatui::text::{Line, Text};
use ratatui::widgets::ListItem;

// Single-line item (no status)
let item = ListItem::new(Line::from(spans));

// Two-line item (with current_task)
let lines = vec![
    Line::from(spans),
    Line::from(vec![
        Span::raw("    "),  // 4-space indent
        Span::styled(task_text, Style::default().fg(Color::DarkGray)),
    ]),
];
let item = ListItem::new(Text::from(lines));
```

### AgentStatus with Optional Fields and Lenient Parsing

```rust
// Source: serde documentation + CONTEXT.md decisions
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(default)]  // All missing fields become None/Default
pub struct AgentStatus {
    pub phase: Option<String>,
    pub current_task: Option<String>,
    // last_activity intentionally removed — daemon reads file mtime
}

// Parsing: missing fields become None, unknown fields silently ignored
// (serde default behavior — no deny_unknown_fields)
fn read_agent_status(path: &Path) -> Option<AgentStatus> {
    let status_file = path.join(".agentree").join("status.json");
    let content = std::fs::read_to_string(status_file).ok()?;
    let status: AgentStatus = serde_json::from_str(&content).ok()?;
    // Treat as "no status" if both fields are None
    if status.phase.is_none() && status.current_task.is_none() {
        None
    } else {
        Some(status)
    }
}
```

### Writing to Shared Git Exclude

```rust
// Verified by manual testing: per-worktree info/exclude has NO EFFECT
// Must write to $GIT_COMMON_DIR/info/exclude
fn add_agentree_to_exclude(worktree_path: &Path) -> Result<()> {
    // Read the .git file to find gitdir
    let dot_git = worktree_path.join(".git");
    let gitdir = if dot_git.is_file() {
        let content = std::fs::read_to_string(&dot_git).map_err(AgentreeError::Io)?;
        let gitdir_line = content.lines()
            .find(|l| l.starts_with("gitdir: "))
            .ok_or_else(|| AgentreeError::Git("Invalid .git file".into()))?;
        PathBuf::from(gitdir_line.trim_start_matches("gitdir: ").trim())
    } else {
        dot_git  // Main repo: .git is a directory
    };

    // Read commondir to find $GIT_COMMON_DIR
    let commondir_file = gitdir.join("commondir");
    let common_git = if commondir_file.exists() {
        let rel = std::fs::read_to_string(&commondir_file)
            .map_err(AgentreeError::Io)?;
        gitdir.join(rel.trim()).canonicalize()
            .map_err(AgentreeError::Io)?
    } else {
        gitdir.clone()
    };

    let exclude_path = common_git.join("info").join("exclude");
    std::fs::create_dir_all(exclude_path.parent().unwrap()).map_err(AgentreeError::Io)?;

    let existing = std::fs::read_to_string(&exclude_path).unwrap_or_default();
    if !existing.lines().any(|l| l.trim() == ".agentree/") {
        let mut content = existing;
        if !content.ends_with('\n') && !content.is_empty() { content.push('\n'); }
        content.push_str("# agentree status directory (local, not committed)\n");
        content.push_str(".agentree/\n");
        std::fs::write(&exclude_path, content).map_err(AgentreeError::Io)?;
    }
    Ok(())
}
```

### Optimistic Attention Clear in TUI

```rust
fn action_clear_attention(state: &mut TuiState, client: &DaemonClient) {
    if let Some(ws) = state.workspaces.get_mut(state.selected) {
        if ws.attention.is_some() {
            // Optimistic clear: update local state immediately
            ws.attention = None;
            // Send to daemon in background (fire and forget)
            let branch = ws.branch.clone();
            let _ = client.clear_attention(&branch);
            // Next 1s poll will confirm; flag reappears only if delete failed
        }
    }
}
```

Note: `action_clear_attention()` currently takes `&TuiState` (immutable). Must change to `&mut TuiState`.

### `.agentree/` Directory Creation in `create_worktree()`

```rust
// After git worktree add succeeds, in create_worktree():
let agentree_dir = worktree_path.join(".agentree");
std::fs::create_dir_all(&agentree_dir).map_err(AgentreeError::Io)?;

// Write CLAUDE.md template to worktree root (if not already present)
let claude_md_path = worktree_path.join("CLAUDE.md");
if !claude_md_path.exists() {
    let template = include_str!("../../templates/CLAUDE.md");
    std::fs::write(&claude_md_path, template).map_err(AgentreeError::Io)?;
}

// Add .agentree/ to git exclude (shared info/exclude)
add_agentree_to_exclude(&worktree_path)?;
```

---

## Existing Infrastructure (Already Done)

These items from the phase description are already implemented and only need verification + test:

| Item | Location | Status |
|------|----------|--------|
| `read_agent_status()` reads `.agentree/status.json` | `daemon/state.rs:168` | Done |
| `read_attention()` reads `.agentree/attention.md` | `daemon/state.rs:174` | Done |
| File watcher watches `.agentree/` dirs | `daemon/mod.rs:66-93` | Done |
| `update_workspace()` called on file change | `daemon/mod.rs:98-103` | Done |
| `clear_attention()` deletes `attention.md` | `daemon/state.rs:66-89` | Done |
| `build_workspace_info()` populates `agent_status` | `daemon/state.rs:155-166` | Done |
| `get_all_agentree_paths()` for watcher init | `daemon/state.rs:114-128` | Done |

The daemon infrastructure is complete. The issues are:
1. `AgentStatus` struct format mismatch (non-optional `phase`)
2. No `.agentree/` directory creation on `agentree create`
3. No CLAUDE.md template injection on `agentree create`
4. TUI shows no `current_task` (only single-line items)
5. Attention highlight is red background — CONTEXT.md says yellow/orange

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| `phase: String` (required) | `phase: Option<String>` with `#[serde(default)]` | Lenient parsing; empty file is valid |
| Single-line `ListItem::new(Line::from(spans))` | `ListItem::new(Text::from(vec![line1, line2]))` | Variable-height rows in workspace list |
| Per-worktree `info/exclude` (non-functional) | Shared `$GIT_COMMON_DIR/info/exclude` | Actually works to hide `.agentree/` |
| Attention highlight: red background | Yellow/orange background (distinguishable from error) | Better visual distinction |

---

## Open Questions

1. **CLAUDE.md placement: worktree root vs `.agentree/` subdirectory**
   - What we know: Claude Code reads `CLAUDE.md` from the CWD and parent directories on startup
   - What's unclear: Whether agents other than Claude Code will find instructions in `CLAUDE.md`
   - Recommendation: Write to worktree root (`{worktree}/CLAUDE.md`) — this is the Claude Code convention and works immediately

2. **What happens when `.agentree/` watcher path doesn't exist at daemon startup?**
   - What we know: `add_watch_paths()` skips non-existent paths (`if path.exists()`)
   - What's unclear: If `agentree create` was run AFTER daemon started, the new `.agentree/` directory won't be watched until the 30s rescan
   - Recommendation: This is acceptable for v1 — the 30s rescan adds new paths. Verify in test.

3. **Truncation strategy for `current_task` (Claude's Discretion)**
   - What we know: The branch name already uses middle-truncation (`truncate_middle()`)
   - Recommendation: Use **right-trim** (not middle-truncation) for `current_task` since it reads as a sentence — cutting the end is more natural than cutting the middle. Append `…` if truncated.

4. **Should `add_agentree_to_exclude()` fail silently or propagate errors?**
   - What we know: Failing to write the exclude file is non-critical (`.agentree/` still works; just shows as untracked)
   - Recommendation: Log a warning with `eprintln!` but don't fail the create command. Git status is cosmetic.

---

## Sources

### Primary (HIGH confidence)
- Direct source code analysis of `src/daemon/state.rs`, `src/daemon/protocol.rs`, `src/daemon/mod.rs`, `src/daemon/watcher.rs`, `src/dashboard/ui.rs` — all at `/Users/julien.muetton/Projects/themouette/worktrees/agentree/discuss-future/src/`
- docs.rs/ratatui/0.29.0 — `ListItem` multi-line support, `Text::from(Vec<Line>)` API
- Live testing in the actual git repository: verified that `$GIT_DIR/info/` is overridden by `$GIT_COMMON_DIR/info/` in linked worktrees

### Secondary (MEDIUM confidence)
- [git-repository-layout documentation](https://git-scm.com/docs/gitrepository-layout) — confirmed `info/` in worktrees is ignored in favor of common dir
- [ratatui PR #1553](https://github.com/ratatui/ratatui/pull/1553) — multi-line list scrolling fix (in ratatui 0.29)
- [ratatui ListItem docs](https://docs.rs/ratatui/latest/ratatui/widgets/struct.ListItem.html) — confirmed `ListItem::new(Text)` accepts multi-line `Text`

### Tertiary (LOW confidence)
- None

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries already in use, no new dependencies
- Architecture: HIGH — actual source code analyzed line-by-line
- Pitfalls: HIGH — git exclude behavior verified by live testing; struct mismatch verified by code inspection
- Git exclude behavior: HIGH — verified by direct testing in the actual repo

**Research date:** 2026-02-28
**Valid until:** 2026-03-30 (stable domain — Rust crates don't change breaking APIs frequently)
