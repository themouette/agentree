---
phase: 04-status-protocol
plan: 02
subsystem: dashboard, ui
tags: [ratatui, tui, rust, attention, status]

requires:
  - phase: 04-01
    provides: AgentStatus struct with optional current_task field

provides:
  - Two-line workspace list rows showing agent_status.current_task (DarkGray)
  - Attention rows showing first-line of attention.md (Yellow text) with Yellow background
  - Optimistic attention clear (immediate TUI update before daemon confirmation)
  - truncate_right() helper for right-side truncation

affects: 04-03

tech-stack:
  added: []
  patterns:
    - "Multi-line ListItem: Text::from(vec![line1, line2]) for two-line rows"
    - "Optimistic UI update: mutate local state before async daemon call"
    - "Attention over current_task priority: else-if chain ensures mutual exclusivity"

key-files:
  created: []
  modified:
    - src/dashboard/ui.rs

key-decisions:
  - "Attention takes priority over current_task — when both present, attention first-line is shown"
  - "Attention row background is Yellow (not Red) with Black foreground"
  - "action_clear_attention takes &mut TuiState — optimistic clear before daemon call"
  - "truncate_right appended alongside truncate_middle and shell_quote helpers"

requirements-completed:
  - STAT-03

duration: 2min
completed: 2026-02-28
---

# Phase 04 Plan 02: TUI Two-Line Rows and Attention UI Summary

**TUI workspace list updated with two-line rows showing current_task (DarkGray) and attention (Yellow), attention background changed to Yellow, c key clears attention immediately (optimistic)**

## Performance

- **Duration:** ~2 min
- **Started:** 2026-02-28T22:37:02Z
- **Completed:** 2026-02-28T22:38:42Z
- **Tasks:** 2
- **Files modified:** 1 (src/dashboard/ui.rs)

## Accomplishments
- Added `Text` import; replaced `Line::from(spans)` with `Text::from(lines)` multi-line ListItem
- Attention rows now show Yellow background (Black foreground) instead of Red
- Second row shows attention first-line (Yellow) or `current_task` (DarkGray) — not both
- `action_clear_attention()` now takes `&mut TuiState` and sets `ws.attention = None` before daemon call
- Added `truncate_right()` helper for right-side truncation with ellipsis

## Task Commits

1. **Task 1: Two-line workspace rows + yellow attention highlight** - `6e31563` (feat)
2. **Task 2: Optimistic attention clear** - `196fa7f` (feat)

## Files Created/Modified
- `src/dashboard/ui.rs` - Text import, Yellow attention background, two-line rows, optimistic clear, truncate_right helper

## Decisions Made
- Attention takes priority over current_task: when both present, attention first-line shown
- No second line shown for workspaces with neither attention nor current_task (clean single-line)
- `ws.branch` cloned before `ws.attention = None` to avoid Rust borrow conflicts

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Plan 04-03 (human verification) can now run — all STAT requirements implemented
- Dashboard shows two-line rows with status information
- Attention clear is immediate (optimistic)

## Self-Check: PASSED

- `cargo build --bin agentree` passes (0 errors)
- `Text::from(lines)` used at line 405 in ui.rs
- `Color::Yellow` used at lines 376 (background) and 390 (attention text)
- `ws.attention = None` at line 524 in action_clear_attention
- `fn truncate_right` at line 579

---
*Phase: 04-status-protocol*
*Completed: 2026-02-28*
