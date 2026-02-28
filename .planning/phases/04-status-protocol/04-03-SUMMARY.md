---
phase: 04-status-protocol
plan: 03
subsystem: verification
tags: [smoke-test, status-protocol, worktree, tui, rust]

# Dependency graph
requires:
  - phase: 04-01
    provides: AgentStatus struct, setup_agentree_workspace(), templates/CLAUDE.md
  - phase: 04-02
    provides: two-line TUI rows, yellow attention, optimistic clear

provides:
  - Human-verified status protocol working end-to-end in real terminal
  - Confirmed: agentree create produces .agentree/, CLAUDE.md, and clean git status
  - Confirmed: dashboard shows second-line current_task within 2s of writing status.json
  - Confirmed: dashboard shows yellow attention row and first-line text within 2s
  - Confirmed: c key clears attention immediately (optimistic)
  - Confirmed: partial/empty status.json parses without errors

affects: 05-tests-and-hardening

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "CLAUDE.md written to worktree root (not .agentree/) — root-level placement is intentional so agents find it"
    - "Only .agentree/ is git-excluded, not CLAUDE.md — gap noted for follow-up"

key-files:
  created: []
  modified: []

key-decisions:
  - "CLAUDE.md written to worktree root is a minor UX gap: agents see it as untracked in git status. Follow-up: either exclude it too, or skip writing when .claude/CLAUDE.md already exists"
  - "All 4 human smoke tests passed: worktree setup, status display, attention display+clear, empty JSON parsing"

patterns-established:
  - "Human-verify checkpoints log any UX gaps as deferred items — not blocking if all functional tests pass"

requirements-completed:
  - STAT-01
  - STAT-02
  - STAT-03

# Metrics
duration: ~5min (human verification)
completed: 2026-03-01
---

# Phase 04 Plan 03: Status Protocol Human Verification Summary

**All 4 status protocol smoke tests passed: agentree create wires .agentree/ with CLAUDE.md and git exclusion, TUI shows two-line rows for status and yellow attention rows, c key clears optimistically**

## Performance

- **Duration:** ~5 min (human verification session)
- **Started:** 2026-03-01
- **Completed:** 2026-03-01
- **Tasks:** 2 (Task 1: automated build check; Task 2: human smoke test)
- **Files modified:** 0 (verification-only plan)

## Accomplishments
- Confirmed full build is clean (0 errors, 5 automated checks passed)
- Human verified worktree create produces .agentree/ directory, CLAUDE.md at root, and .agentree/ excluded from git status
- Human verified status.json display: dashboard shows second indented DarkGray line within 2s
- Human verified attention display and clear: yellow background + first-line text appears within 2s, c key clears immediately
- Human verified lenient parsing: empty {} and partial JSON both parse without errors

## Task Commits

Task 1 (automated build check) and Task 2 (human verification) were verification-only — no code was changed.

No per-task commits for this plan (pure verification).

**Plan metadata:** see final docs commit (04-03)

## Files Created/Modified

None — this plan was verification-only. All implementation was in plans 04-01 and 04-02.

## Decisions Made

- All 4 smoke tests passed — no blocking failures found
- UX gap noted (see Deviations): CLAUDE.md at worktree root is not git-excluded; agents will see it as untracked
- Follow-up decision deferred: either exclude CLAUDE.md too, or skip writing it when `.claude/CLAUDE.md` already exists in the worktree

## Deviations from Plan

### Observed UX Gap (not a blocking failure)

**[Observation] CLAUDE.md written to worktree root is not git-excluded**

- **Found during:** Task 2 (human smoke test, Test 1)
- **Issue:** `setup_agentree_workspace()` writes `CLAUDE.md` to the worktree root and adds `.agentree/` to `$GIT_COMMON_DIR/info/exclude`, but does NOT exclude `CLAUDE.md` itself. As a result, `git status` in the new worktree shows `CLAUDE.md` as untracked.
- **Impact:** Minor UX gap — agents running `git status` will see CLAUDE.md as an untracked file. Not a correctness failure; all 4 smoke tests still passed.
- **Follow-up options:**
  1. Add `CLAUDE.md` to the git exclude file alongside `.agentree/` in `add_agentree_to_git_exclude()`
  2. Skip writing CLAUDE.md when `.claude/CLAUDE.md` already exists in the worktree (the user notes they use `.claude/CLAUDE.md` for this project)
- **Fix:** Deferred to Phase 5 (tests and hardening) — not blocking current phase
- **Files modified:** None (no code change, gap documented for follow-up)

---

**Total deviations:** 0 auto-fixed, 1 observed UX gap (deferred)
**Impact on plan:** All success criteria met. UX gap noted for follow-up in Phase 5.

## Issues Encountered

None — build was clean, all 4 human smoke tests passed on first attempt.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 4 (Status Protocol) is now complete: all 3 STAT requirements verified end-to-end
- Phase 5 (Tests and Hardening) can proceed
- Deferred item from this plan: CLAUDE.md git-exclusion gap — consider fixing in Phase 5 alongside other hardening work

---
*Phase: 04-status-protocol*
*Completed: 2026-03-01*
