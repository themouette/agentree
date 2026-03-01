# Phase 7: improve UX - Context

**Gathered:** 2026-03-01
**Status:** Ready for planning

<domain>
## Phase Boundary

Polish the agentree dashboard interaction model. The dashboard is functionally complete (phases 1-6). This phase improves day-to-day usability: clearer quit/detach semantics, better pane sync, help discoverability, a welcome panel, tmux status bar cleanup, and dynamic left pane sizing. No new workspace management features.

</domain>

<decisions>
## Implementation Decisions

### q / d key behavior

- `q` = quit: kill the tmux session. If the dashboard started the daemon, also kill the daemon. If the daemon was pre-existing (started externally), leave it running.
- Confirmation for `q`: inline in the footer — footer changes to `Kill dashboard? [y/N]`. Y executes kill, N cancels back to normal TUI mode.
- `d` = detach: detach from the tmux session. Daemon stays alive regardless of who started it.
- After detaching, print warning to the terminal: "Dashboard running in background. Re-attach: agentree dashboard"
- Daemon ownership: first opener (the session that started the daemon) owns it. Re-attaching to an existing session inherits the original ownership — the re-attaching session does NOT become the new owner.

### Daemon lifecycle

- Dashboard tracks whether it started the daemon (new flag/state).
- On `q` quit: kill daemon only if this session started it.
- On `d` detach: never kill daemon.
- On `q` when daemon was pre-existing: kill tmux session only, leave daemon.

### Right pane active workspace indicator

- A 1-line pane sits at the top of the right side of the layout, always visible.
- This pane shows: "Active: <branch-name>" (or empty/placeholder if no action has been triggered yet).
- The header persists even when the left pane expands (always visible).
- Active follows the last triggered action (`a`, `t`, `e`): when user presses any of these keys, the active indicator updates to the currently selected workspace.
- No auto-sync: navigating the cursor in the left pane does NOT change the right pane content. User must trigger an action to change the active workspace.

### Footer (44-char width)

- Footer shows abbreviated keys only: `a agent  t term  e edit  c clear  d detach  q quit  ? help`
- Full keybinding descriptions are in the welcome panel (accessed via `?`).
- Footer must fit within 44 characters.

### Welcome / help panel

- Content: ASCII agentree logo + quick start guide + full keybinding reference list with descriptions.
- Shows on first dashboard open (right pane starts with this instead of a bare terminal).
- Shows when user presses `?` while in the TUI.
- Auto-respawns: if the right pane dies (user exits its content), dashboard detects the dead pane and respawns the welcome panel automatically.

### Tmux status bar

- Hide entirely on session creation: `set status off` on the agentree-dashboard tmux session.
- Users should not see the tmux window list or session names — these are implementation details.

### Left pane resize on focus

- Use tmux `focus-events` to detect pane focus changes.
- When the left pane (TUI) is focused: expand to 50% of terminal width.
- When focus moves to the right pane: shrink left pane back to 44 columns.
- The 1-line active workspace header above the right pane persists regardless of pane sizes.

### Claude's Discretion

- ASCII logo design for the welcome panel
- Exact wording of the detach warning message
- How focus-events are wired in Rust (tmux hook vs shell wrapper)
- Layout of the welcome panel content (spacing, sections)
- How to store daemon ownership flag (env var, file, or in-memory)

</decisions>

<specifics>
## Specific Ideas

- The footer abbreviation example: `a agent  t term  e edit  c clear  d detach  q quit  ? help` — should fit in 44 chars
- The 1-line header pane is above the right pane content pane, not above the left pane
- When the left pane expands to 50%, the right pane shrinks but remains functional
- The active workspace header should still show even if the right pane has no content yet (show placeholder like "No active workspace")

</specifics>

<deferred>
## Deferred Ideas

- None — discussion stayed within phase scope

</deferred>

---

*Phase: 07-improve-ux*
*Context gathered: 2026-03-01*
