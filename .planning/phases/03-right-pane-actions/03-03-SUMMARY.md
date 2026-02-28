---
phase: 03-right-pane-actions
plan: "03"
subsystem: ui
tags: [tmux, tui, dashboard, ratatui, session-management]

requires:
  - phase: 03-01
    provides: ensure_named_session, is_session_pane_dead, agent_session_name helpers
  - phase: 03-02
    provides: action handlers with persistent sessions, session indicator icons

provides:
  - Human-verified end-to-end right pane actions (a, t, e, c keys)
  - Confirmed session persistence after detach (q)
  - Confirmed Ctrl+\\ returns focus to TUI pane

affects: [04-status-protocol, 05-tests-hardening]

tech-stack:
  added: []
  patterns:
    - "break-pane/join-pane architecture for right pane session management (replaces switch-client)"
    - "Named pane windows per action type — avoids stash-window conflicts"

key-files:
  created: []
  modified:
    - src/dashboard/tmux.rs

key-decisions:
  - "break-pane + join-pane replaces switch-client for right pane action display — switch-client moved entire client, break-pane moves just the target pane into the right pane window"
  - "Kill unnamed right panes before action pane join to prevent layout collisions"

patterns-established:
  - "Human verification plan: build + smoke test steps listed, user types approved"

requirements-completed: [ACT-01, ACT-02, ACT-03, ACT-04, AGNT-01, AGNT-02, AGNT-03]

duration: ~30min
completed: 2026-02-28
---

# Phase 3 Plan 03: Human Verification — Right Pane Actions Summary

**All right pane actions (a/t/e/c) verified end-to-end: persistent named sessions, Ctrl+\\ focus return, and session icons confirmed working after break-pane/join-pane architecture fix.**

## Performance

- **Duration:** ~30 min (including bug fixes during smoke test)
- **Started:** 2026-02-28
- **Completed:** 2026-02-28
- **Tasks:** 2 (build + human verify)
- **Files modified:** 1 (src/dashboard/tmux.rs)

## Accomplishments

- Build passed with all tests before human verification
- Three bug-fix iterations applied during smoke test to fix pane management architecture
- User approved all 7 verification steps (a/t/e/c keys, session persistence, Ctrl+\\, icons)
- Phase 3 right pane actions fully functional end-to-end

## Task Commits

1. **Task 1: Build release binary and prepare test environment** — cargo build + cargo test passed (no code changes, no commit needed)
2. **Bug fix during verification:** `5293162` fix(dashboard): replace switch-client with stash-window pane management
3. **Bug fix during verification:** `b0b3552` fix(dashboard): kill unnamed right panes and use new-window for action panes
4. **Bug fix during verification:** `b16e0f2` fix(dashboard): replace stash window with break-pane per background pane

## Files Created/Modified

- `src/dashboard/tmux.rs` — Pane management architecture changed from switch-client to break-pane/join-pane; unnamed right pane cleanup added

## Decisions Made

- `switch-client` approach was architecturally wrong for right pane display — it moved the entire tmux client to the target session rather than displaying the session's pane in the right pane window. Replaced with `break-pane`/`join-pane` to move named session panes into the dashboard's right pane slot.
- Unnamed right panes must be killed before joining a named action pane, otherwise layout collisions occur.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] switch-client moved entire client instead of populating right pane**
- **Found during:** Task 2 (Human smoke test)
- **Issue:** Pressing `a` switched the entire tmux client to the agent session instead of showing agent output in the right pane of the dashboard window
- **Fix:** Replaced switch-client with break-pane (detach named session pane) + join-pane (attach it into dashboard right pane slot)
- **Files modified:** src/dashboard/tmux.rs
- **Verification:** `a` key shows agent in right pane, TUI stays active in left pane
- **Committed in:** `5293162`, `b0b3552`, `b16e0f2` (three iterations to stabilize)

---

**Total deviations:** 1 auto-fixed (Rule 1 - Bug), required 3 commit iterations to fully resolve
**Impact on plan:** Essential bug fix — the core interaction model required break-pane/join-pane instead of switch-client. No scope creep.

## Issues Encountered

The switch-client architecture flaw required three fix iterations during smoke test:
1. First attempt: stash-window approach — introduced new-window conflicts
2. Second attempt: kill unnamed panes + new-window — resolved collisions but stash still inconsistent
3. Third attempt (final): break-pane per background pane + join-pane — correct and stable

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 3 complete: all right pane actions verified working end-to-end
- Phase 4 (Status Protocol) can proceed: agent status display in TUI, `.agentree/status.json` format
- Phase 5 (Tests and Hardening) can proceed after Phase 4: automated test coverage for core flows

---
*Phase: 03-right-pane-actions*
*Completed: 2026-02-28*
