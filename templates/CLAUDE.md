# Agentree Status Protocol

Your current directory is a git worktree managed by `agentree`. The dashboard
monitors this workspace via `.agentree/` files.

## Reporting Status

Write `.agentree/status.json` on meaningful transitions — do NOT write on every
tool call. Approximately every 5–10 minutes during long tasks, or whenever you
switch phases (planning → implementing → testing).

```json
{
  "phase": "implementing",
  "current_task": "Writing auth tests for oauth flow"
}
```

Phases: `planning`, `implementing`, `testing`, `reviewing`, `waiting`, or any
descriptive string.

**End of session:** Delete `.agentree/status.json`. The dashboard row returns to
single-line display (clean slate for next session).

## Requesting Human Attention

Write `.agentree/attention.md` when blocked or needing human input **— including
before any tool call that requires user approval**. Do not silently wait for
the permission prompt to be noticed; signal first, then make the call. Example:

```
Need permission to run: cargo fmt
```

More generally, write `.agentree/attention.md` when blocked or needing human input:

```
Need OPENAI_API_KEY to proceed with integration tests.
```

First line must be a clear, actionable message — it appears in the dashboard.
The rest of the file can provide full context.

**After human responds:** The human presses `c` in the dashboard to clear the
flag. You may also delete the file yourself once unblocked.

## File Paths (relative to worktree root)

- `.agentree/status.json` — current phase and task
- `.agentree/attention.md` — attention request (create when blocked)

These files are excluded from git tracking automatically.
