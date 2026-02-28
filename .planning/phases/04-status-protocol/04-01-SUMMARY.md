---
phase: 04-status-protocol
plan: 01
subsystem: daemon, backend, worktree
tags: [agentree, status-protocol, serde, rust]

requires:
  - phase: 03-right-pane-actions
    provides: dashboard infrastructure and backend trait

provides:
  - AgentStatus struct with optional phase (STAT-01, STAT-02 coverage)
  - templates/CLAUDE.md status protocol documentation for agents
  - setup_agentree_workspace() creating .agentree/ dir in new worktrees
  - add_agentree_to_git_exclude() adding .agentree/ to $GIT_COMMON_DIR/info/exclude
  - Backend trait status_dir() default method returning workspace_path/.agentree/

affects: 04-03, dashboard, daemon

tech-stack:
  added: []
  patterns:
    - "Lenient serde deserialization: #[serde(default)] on AgentStatus for partial JSON"
    - "Non-fatal setup steps: warn and continue if .agentree/ setup fails"
    - "include_str!() for compile-time template embedding"
    - "commondir resolution: gitdir -> commondir -> $GIT_COMMON_DIR for shared git exclude"

key-files:
  created:
    - templates/CLAUDE.md
  modified:
    - src/daemon/protocol.rs
    - src/worktree/operations.rs
    - src/backend/mod.rs

key-decisions:
  - "AgentStatus.phase is Option<String> — absent field parses as None, not error"
  - "AgentStatus has no last_activity field — daemon derives it from git log"
  - "#[serde(default)] on AgentStatus enables lenient parsing of partial status.json"
  - "setup_agentree_workspace() is non-fatal: worktree created successfully, warns if setup fails"
  - "add_agentree_to_git_exclude() writes to $GIT_COMMON_DIR/info/exclude via commondir resolution"
  - "Backend trait status_dir() has default impl; daemon wiring deferred (architectural change)"

requirements-completed:
  - STAT-01
  - STAT-02

duration: 3min
completed: 2026-02-28
---

# Phase 04 Plan 01: AgentStatus Struct Fix and Worktree Setup Summary

**AgentStatus protocol fixed (phase: Option, no last_activity), worktree setup wires .agentree/ creation with templates/CLAUDE.md and git exclusion, Backend trait gains status_dir() default method**

## Performance

- **Duration:** ~3 min
- **Started:** 2026-02-28T22:32:44Z
- **Completed:** 2026-02-28T22:35:22Z
- **Tasks:** 3
- **Files modified:** 4 (protocol.rs, operations.rs, backend/mod.rs, templates/CLAUDE.md)

## Accomplishments
- Fixed `AgentStatus` struct: `phase` is now `Option<String>`, `last_activity` removed, `#[serde(default)]` added for lenient deserialization
- Created `templates/CLAUDE.md` with status protocol documentation for AI agents (embedded via `include_str!()`)
- Added `setup_agentree_workspace()` and `add_agentree_to_git_exclude()` helpers in `operations.rs`
- Updated `create_worktree()` to call `setup_agentree_workspace()` in both creation arms (not `Resumed`)
- Added `status_dir()` default method to `Backend` trait returning `workspace_path.join(".agentree")`

## Task Commits

1. **Task 1: Fix AgentStatus struct** - `b46ee1c` (fix)
2. **Task 2: Create CLAUDE.md template and wire worktree setup** - `7cdb029` (feat)
3. **Task 3: Add status_dir() to Backend trait** - `8955362` (feat)

## Files Created/Modified
- `src/daemon/protocol.rs` - AgentStatus struct: phase optional, last_activity removed, serde(default)
- `templates/CLAUDE.md` - Status protocol documentation for agents
- `src/worktree/operations.rs` - setup_agentree_workspace(), add_agentree_to_git_exclude(), create_worktree() updated
- `src/backend/mod.rs` - Backend trait: PathBuf import, status_dir() default method

## Decisions Made
- `#[serde(default)]` on `AgentStatus` enables any subset of fields to be present in status.json
- `setup_agentree_workspace()` is non-fatal: if it fails, only status protocol doesn't work — worktree was created successfully
- `add_agentree_to_git_exclude()` resolves `$GIT_COMMON_DIR` via `.git` file → `gitdir/commondir` → canonicalize
- Daemon wiring (calling `backend.status_dir()`) is explicitly deferred — daemon doesn't currently have backend instances in scope

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Plan 04-02 (TUI two-line rows) can proceed independently (no dependency on these changes)
- Plan 04-03 (human verification) requires both 04-01 and 04-02 to be complete
- All success criteria met: cargo build passes, AgentStatus correct, templates/CLAUDE.md exists, setup_agentree_workspace called in both arms, status_dir() on Backend trait

## Self-Check: PASSED

- `cargo build --bin agentree` passes (0 errors)
- `AgentStatus` has `#[serde(default)]`, `phase: Option<String>`, `current_task: Option<String>`, no `last_activity`
- `templates/CLAUDE.md` exists (44 lines)
- `setup_agentree_workspace` called at lines 433 and 469 in operations.rs (both creation arms)
- `fn status_dir(&self, workspace_path: &Path) -> PathBuf` in Backend trait with default impl

---
*Phase: 04-status-protocol*
*Completed: 2026-02-28*
