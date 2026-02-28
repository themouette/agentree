---
phase: 06-improve-the-ui
plan: 02
subsystem: ui
tags: [tmux, dashboard, daemon, teardown, kill]

# Dependency graph
requires:
  - phase: 02-dashboard-launch
    provides: dashboard tmux session management and DASHBOARD_SESSION constant
  - phase: 01-daemon-startup
    provides: daemon PID file at runtime_dir()/daemon.pid and socket_path()
provides:
  - "`agentree dashboard --kill` command that tears down dashboard session and stops daemon"
  - "`kill_session()` tmux helper for killing sessions by name"
  - "`kill_dashboard()` orchestration function in dashboard/mod.rs"
affects: [06-03, any future dashboard teardown workflows]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "kill_session ignores tmux exit code (non-zero = session didn't exist = success)"
    - "SIGTERM via `kill -TERM <pid>` for daemon shutdown (POSIX, Unix-only — safe since tmux is Unix-only)"
    - "sanitize_branch() extracted helper for DRY branch-name sanitization in tmux session names"

key-files:
  created: []
  modified:
    - src/commands/dashboard.rs
    - src/dashboard/mod.rs
    - src/dashboard/tmux.rs
    - src/dashboard/ui.rs
    - src/worktree/operations.rs

key-decisions:
  - "kill_session ignores tmux exit code — non-zero means session not found, goal already met"
  - "Daemon stopped via SIGTERM to PID file rather than socket protocol — simpler, avoids auth"
  - "500ms sleep after SIGTERM before checking socket — gives daemon time to cleanup files"
  - "Pre-existing clippy issues fixed inline (sanitize_branch, unused Scrollbar imports, redundant closures)"

patterns-established:
  - "kill_session: treat non-zero exit as success (session gone = goal achieved)"
  - "kill_dashboard: check + kill session first, then stop daemon via PID file"

requirements-completed: [UI-05]

# Metrics
duration: 3min
completed: 2026-03-01
---

# Phase 06 Plan 02: Add Dashboard --kill Command Summary

**`agentree dashboard --kill` flag that kills the tmux session and stops the daemon via SIGTERM to the PID file**

## Performance

- **Duration:** ~3 min
- **Started:** 2026-02-28T23:54:15Z
- **Completed:** 2026-03-01T00:00:00Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Added `--kill` flag to `DashboardArgs` with routing to `kill_dashboard()` in `execute()`
- Implemented `kill_session(session: &str) -> Result<()>` in `tmux.rs` — idempotent tmux session teardown
- Implemented `kill_dashboard() -> Result<()>` in `dashboard/mod.rs` — full teardown sequence: session kill then daemon SIGTERM
- Fixed pre-existing clippy issues: extracted `sanitize_branch()` helper, removed unused Scrollbar imports, fixed redundant closures

## Task Commits

Each task was committed atomically:

1. **Task 1: Add --kill flag and routing in DashboardArgs** - `a5712a9` (feat)
2. **Task 2: Implement kill_session and kill_dashboard** - `3e93268` (feat)

**Plan metadata:** (docs commit to follow)

## Files Created/Modified
- `src/commands/dashboard.rs` - Added `kill: bool` field and routing logic
- `src/dashboard/mod.rs` - Added `kill_dashboard()` public function
- `src/dashboard/tmux.rs` - Added `kill_session()` and `sanitize_branch()` helper
- `src/dashboard/ui.rs` - Removed unused Scrollbar imports (clippy fix)
- `src/worktree/operations.rs` - Fixed redundant closures (clippy fix)

## Decisions Made
- `kill_session()` ignores tmux exit code — exit 1 means session didn't exist, which is already the desired state. No error propagated.
- Daemon stopped via SIGTERM to PID in `~/.agentree/daemon.pid` rather than a socket-level stop request. Simpler and works even when socket is unavailable.
- 500ms sleep after SIGTERM gives daemon time to cleanup its socket and PID files before checking if it stopped.
- `sanitize_branch()` extracted as a private helper to fix the consecutive `str::replace` clippy lint across all session name functions.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed pre-existing clippy errors to meet plan success criteria**
- **Found during:** Task 2 (verification step)
- **Issue:** `cargo clippy -- -D warnings` failed due to pre-existing issues: unused `Scrollbar`/`ScrollbarOrientation`/`ScrollbarState` imports in `ui.rs`, consecutive `str::replace` calls in `tmux.rs`, and redundant closures in `operations.rs`
- **Fix:** Extracted `sanitize_branch()` helper in `tmux.rs`, removed unused ratatui widget imports from `ui.rs`, simplified `map_err` closures in `operations.rs`
- **Files modified:** `src/dashboard/tmux.rs`, `src/dashboard/ui.rs`, `src/worktree/operations.rs`
- **Verification:** `cargo clippy --quiet -- -D warnings` exits 0
- **Committed in:** `3e93268` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - pre-existing clippy errors blocking verification)
**Impact on plan:** Required for plan success criteria ("cargo clippy -- -D warnings pass"). No scope creep.

## Issues Encountered
- Task 1 alone does not compile without `kill_dashboard()` (undefined symbol). The two tasks were committed sequentially with Task 1 committed first, immediately followed by Task 2 to restore build integrity.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `agentree dashboard --kill` is ready to use
- Full teardown sequence: session kill + daemon SIGTERM + confirmation messages
- Edge cases handled: no session (prints "No dashboard session running."), no PID file (prints appropriate message)
- Phase 06-03 can proceed — kill command is complete

---
*Phase: 06-improve-the-ui*
*Completed: 2026-03-01*

## Self-Check: PASSED

- FOUND: src/commands/dashboard.rs
- FOUND: src/dashboard/mod.rs
- FOUND: src/dashboard/tmux.rs
- FOUND: .planning/phases/06-improve-the-ui/06-02-SUMMARY.md
- FOUND: commit a5712a9 (Task 1)
- FOUND: commit 3e93268 (Task 2)
