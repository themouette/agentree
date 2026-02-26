# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- `agentree create <branch> -b <ref>` now errors immediately when the branch
  already exists locally or on a remote. Previously `-b` was silently ignored,
  which could mislead users into thinking the worktree was created from their
  specified ref.
- `agentree cd <branch> -b <ref>` (and the `agent`/`editor` equivalents) now
  emit a warning on stderr when the branch already exists and `-b` has no
  effect, instead of silently discarding the flag.

## [0.7.2] - 2026-02-26

### Added

- `agentree create <branch>` now detects branches that exist only on a remote
  (e.g. pushed by a teammate but never fetched locally). Previously agentree
  would silently create a new disconnected branch from HEAD. It now recognises
  the remote tracking ref and uses git DWIM to check out a proper local
  tracking branch, reporting `"Checked out '<branch>' from remote and created
  worktree at <path>"`.
- Typo suggestions for `--base` now include remote tracking branches (e.g.
  `origin/main`) in addition to local ones, making it easier to correct
  mistyped remote refs.

### Fixed

- `agentree remove` now detects when the shell is inside the target worktree
  and emits an actionable error instead of raw git output, telling the user to
  navigate away first.
- `agentree remove` now refuses to remove the main repository checkout with a
  clear error. Only linked worktrees can be removed.
- Removing multiple branches explicitly now continues past individual failures,
  consistent with `--merged` / filter mode. A warning is printed per failure;
  the command succeeds if at least one branch was removed.
- Backend cleanup (Docker sandbox) no longer silently skips when the worktree
  path lookup and the deletion call disagree on state. `delete_worktree` now
  returns the canonical path it removed, which is used directly for cleanup.
- Permission denied errors during worktree removal are now translated to an
  actionable message with `chmod` and `sudo rm` remediation hints.
- Creating a worktree on a repository with no commits now fails immediately
  with a clear, actionable error instead of a cryptic
  `"ambiguous argument 'HEAD': unknown revision"` git message.
- Common `git worktree add` failures (branch already checked out elsewhere,
  path already exists, invalid ref) are now translated to actionable error
  messages with remediation hints instead of raw git stderr.

## [0.7.1] - 2026-02-20

### Fixed

- `agentree remove` (and `remove --unlock`) no longer times out after 30 seconds on
  slow filesystems or locked worktrees. `git worktree remove` and `git worktree unlock`
  are now run without a timeout, matching the treatment already applied to
  `git worktree add`.
- Card view (`--format card`) now always shows the `Dirty` field. Previously the line
  was silently omitted when the dirty check was skipped (`--no-dirty-check`) or when the
  check failed (e.g. after an interrupted removal leaves the worktree in an inconsistent
  state). It now displays `?` in those cases so the unknown state is visible.

## [0.7.0] - 2026-02-19

### Changed

- `list` command now defaults to `card` format instead of `two-lines`, showing full
  branch names, absolute paths, and all metadata without truncation.
- Creation messages now distinguish between two cases:
  - Worktree added for an existing branch: `Created worktree for branch 'X' at <path>`
  - Branch and worktree both created: `Created branch 'X' and worktree at <path>`
- `clean` command now runs `git worktree repair` before `git worktree prune`, fixing
  the case where broken gitdir links would cause valid worktrees to be incorrectly pruned.

### Fixed

- `remove --merged` no longer attempts to remove the main repository. `git worktree
  list` always includes the main repo as its first entry; the `--merged` path was
  iterating all entries without skipping it, so if the main repo's branch appeared in
  the merged-branch list the command would try (and fail) to delete it. A new
  `list_linked_worktrees()` helper in `recovery` centralises the skip-first-entry
  logic and is now used by both `remove --merged` and `list`.
- `agentree create` (and any command that creates a worktree) no longer times out
  after 30 seconds on large repositories or repos with slow post-checkout hooks.
  `git worktree add` is now run without a timeout, matching the behaviour of running
  git directly; other git operations (list, prune, ref queries) retain the 30-second
  safety timeout. The timeout on those operations remains configurable via the
  `AGENTREE_GIT_TIMEOUT` environment variable.
- Creating a branch from a remote tracking ref (e.g. `agentree create my-branch -b origin/preprod`)
  no longer silently sets the new branch's upstream to the remote branch. `--no-track` is now
  passed to `git worktree add` so the new branch is always free-standing, and `git push` behaves
  as expected.  
- Significant performance regression introduced in 0.5.0: `git worktree repair` was
  being run before every command (list, cd, remove, create, etc.) via `ensure_clean_state()`.
  `git worktree repair` is a filesystem-intensive operation and caused noticeable slowdowns
  on each invocation.
- Interactive prune prompt in `ensure_clean_state()` could block non-interactive use
  (scripts, CI) waiting for user input. The prompt has been removed from the hot path.
- `ensure_clean_state()` now only runs a lightweight `git worktree prune --dry-run` check
  and prints a warning with `agentree doctor --fix` guidance when stale metadata is found.
  Actual repair and pruning are left to the explicit `clean` and `doctor` commands.

### Added

- `list` command now shows dirty and locked status in all formats:
  - **card**: `Dirty: yes/no` and `Locked: yes [reason]` lines added to each card
  - **table / two-lines**: `ST` column — `*` for dirty, `L` for locked, `*L` for both
  - **json**: `dirty` (`true`/`false`, or `null` when skipped) and `locked` fields always present
  - Dirty check runs `git status --short` per worktree with a progress spinner; errors (e.g.
    missing path) are treated as `NotChecked` and do not abort the list
  - Locked status is free — data comes from the existing `git worktree list --porcelain` call
- New `--no-dirty-check` flag for `list` command: skips the per-worktree `git status` calls for
  faster output (useful for large repos or scripts where dirty status is not needed)
- `remove --dry-run` flag to preview what would be removed without making any changes:
  - Works with explicit branches (`agentree remove --dry-run feat-x`) and all filter flags
  - Prints "Would remove N worktrees:" with each candidate branch and path
  - Exits successfully without touching git metadata or the filesystem
  - Most useful combined with filter flags: `agentree remove --dry-run --merged main`
- Filter flags for both `list` and `remove` commands to select worktrees by status:
  - `--merged [BASE]` — only worktrees whose branch is merged into BASE (defaults to current branch)
  - `--not-merged [BASE]` — inverse of `--merged`
  - `--dirty` — only worktrees with uncommitted changes (implies dirty check; conflicts with `--no-dirty-check`)
  - `--clean` — only worktrees with no uncommitted changes (implies dirty check; conflicts with `--no-dirty-check`)
  - `--locked` — only locked worktrees
  - `--not-locked` — only unlocked worktrees
  - `--branch PATTERN` — only branches matching a glob pattern (`*` and `?` wildcards supported)
  - `--stale [DAYS]` — only worktrees not modified in the last DAYS days (default: 30)
  - Filters combine (AND logic); e.g. `agentree rm --merged main --clean` removes only merged, clean worktrees
  - For `remove`: `--dirty` automatically escalates to `IgnoreDirty` force level; `--locked` automatically unlocks
- `agentree cd <branch>` now warns on stderr when the requested branch is
  checked out in the main repository rather than in a dedicated worktree (git
  forbids the same branch from being checked out in two places). Two variants:
  - **Navigating from elsewhere**: warns that the branch lives in the main repo,
    shows the destination path, and suggests switching the main repo to another
    branch first if an isolated worktree is needed.
  - **Already in the main repo**: warns that you're already on that branch in
    the main repo, and suggests using `agentree cd` (no argument) to navigate
    there intentionally. The `cd` command is still emitted in both cases so the
    shell wrapper continues to work transparently.
- `agentree cd` (no branch argument) now navigates to the main repository root,
  whether called from a worktree or from the main repo itself. This makes it easy
  to return to the main repo before deleting a worktree or switching contexts,
  without relying on `cd -`. The shell wrapper (`shell-init`) was updated to forward
  all arguments after `cd` so that `agentree cd`, `agentree cd <branch>`, and
  `agentree cd <branch> --base <ref>` all work correctly.
- Animated progress spinner shown before long git operations:
  - During worktree **creation**: `⠙ Creating branch 'X' and worktree...` or `⠙ Creating worktree for 'X'...`
  - During worktree **removal**: `⠙ Removing worktree for 'X'...`
  - No spinner for instant resume (worktree already exists)
  - Spinner is automatically hidden in non-TTY environments (CI, scripts, piped output)
  - Applies to: `create`, `remove`, `shell`, `agent`, `exec`, `cd`
- `cd` now auto-creates the branch and worktree when they do not exist, accepting the
  same `-b/--base`, `--backend`, and `--worktree-location` flags as other workspace commands.
  Previously `cd` required the worktree to already exist.
- New `doctor` command to check worktree health and fix issues:
  - Detects orphaned directories (exist but not tracked by git)
  - Detects broken metadata (git knows about but directory missing/corrupt)
  - Interactive fix mode with `--fix` flag (prompts for each issue)
  - Two output formats: human-readable (default) and JSON (`--format json`)
  - Recursive scanning for nested worktree structures
  - Exit codes for CI integration (returns error when issues found in human mode)
  - Use case: Clean up leftover worktree directories and stale git metadata
- `remove --merged` now defaults to current branch when no base branch is specified
  - Use `agentree rm --merged` to remove all worktrees merged into your current branch
  - Explicitly specify a base with `agentree rm --merged main` if needed
- Docker Sandbox backend (`docker-sandbox`) for microVM-based isolation:
  - Hypervisor-level isolation using Docker Desktop's sandbox feature (Engine 29.1.5+)
  - Platform support: macOS and Windows (microVMs not available on Linux)
  - Fast startup: ~10-30s cold start, ~1-2s for subsequent launches with persistent sandboxes
  - Configuration options: custom binary path, network policies, persistence mode
  - Platform validation at initialization (fails fast on Linux with clear error)
  - Automatic sandbox cleanup when removing workspaces
  - Comprehensive documentation in `docs/backends/docker-sandbox.md`
  - **Note**: Limited git worktree support (Docker Sandboxes don't support custom volume mounts)
- Backend validation during workspace initialization for all commands
- Backend value completion now includes `docker-sandbox` option
- Multiple output formats for `list` command via `--format` flag:
  - `two-lines` (default): Summary line with absolute path on second line
  - `table`: Compact table format with relative paths (120 char width)
  - `card`: Card-style boxes with full details and absolute paths
  - `json`: Machine-readable JSON output with absolute paths
- New `editor` command to open an editor in a workspace
  - Auto-creates workspace if it doesn't exist
  - Supports optional start_ref for workspace creation
  - Forwards all trailing arguments to the editor
  - Runs directly on local machine (not through backend isolation)
  - Respects editor precedence: --editor flag, config, $EDITOR, $VISUAL, vi
  - Includes tab completion for branch names in bash, zsh, and fish
- EditorConfig section in configuration system
  - `editor.bin`: Configure default editor binary
  - `editor.default_args`: Configure default arguments passed to editor
- Environment variable support for editor selection ($EDITOR, $VISUAL)
- Hybrid global config support with two locations:
  - Simple: `~/.agentree.toml` (cross-platform, easy to find)
  - XDG-compliant: `~/.config/agentree/config.toml` (Linux) or `~/Library/Application Support/agentree/config.toml` (macOS)
- `home_config_path()` function for `~/.agentree.toml` path resolution
- `xdg_config_path()` function (renamed from `global_config_path()`) for XDG-compliant path resolution
- Config loader now checks both global config locations with proper precedence
- Tests for both XDG and home config path functions
- Agent value completion for `--agent` flag showing available agents (claude, opencode)
- Backend value completion for `--backend` flag showing available backends (local, claude-vm)
- Branch value completion for `--base` flag in shell, agent, and exec commands
- Shell completion improvements for bash, zsh, and fish
- Centralized default agents constant (`DEFAULT_AGENTS`) as single source of truth
- Fallback agent resolution to hardcoded defaults (claude, opencode) when not configured
- 7 new tests for agent resolution including fallback behavior
- 4 new tests for claude-vm backend argument building
- Welcome banner when entering a workspace shell showing:
  - Workspace name (branch)
  - Full path to workspace
  - Backend being used
  - Exit instructions (exit or Ctrl+D)
  - Uses colors when running in a TTY, plain text fallback for pipes/redirects
- Environment variables for shell prompt customization:
  - `AGENTREE_WORKSPACE=1` - Indicates you're in an agentree workspace
  - `AGENTREE_BRANCH=<branch>` - The workspace branch name
  - `AGENTREE_WORKSPACE_PATH=<path>` - Full path to workspace
  - Enables users to customize their shell prompt to show workspace context

### Changed

- Default `list` output format changed from table to two-lines format for better readability with long paths
- Shell completion simplified to only complete flag values (--base, --agent, --backend), removed brittle positional branch argument completion
  - **What works**: Tab completion for flag values reliably suggests branches, agents, and backends
  - **What changed**: Positional branch arguments (e.g., `agentree agent <TAB>`) no longer suggest branches
  - **Why**: Positional detection logic was fragile and conflicted with clap's static completions, causing broken behavior
  - **Impact**: Flag value completion (the most useful feature) is now reliable across all shells (bash, zsh, fish)
- **BREAKING**: Base branch specification changed from positional argument to `--base`/`-b` flag in `create`, `shell`, `agent`, and `exec` commands
  - **Old syntax**:
    - `agentree create feature main`
    - `agentree agent feature-branch main`
    - `agentree shell my-branch develop`
    - `agentree exec test-branch main -- npm test`
  - **New syntax**:
    - `agentree create feature --base main` or `agentree create feature -b main`
    - `agentree agent feature-branch --base main` or `agentree agent feature-branch -b main`
    - `agentree shell my-branch --base develop` or `agentree shell my-branch -b develop`
    - `agentree exec test-branch --base main -- npm test` or `agentree exec test-branch -b main -- npm test`
  - **Why**: This removes ambiguity when passing arguments to agents (e.g., `agentree agent feat /clear` now correctly passes `/clear` to the agent instead of treating it as a base branch)
  - **Migration**: All commands now use consistent `--base`/`-b` flag syntax. Scripts using the old positional syntax will fail with clear error messages
- Config precedence order updated: `CLI args > Project .agentree.toml > ~/.agentree.toml > XDG config > Defaults`
- Global config filename in XDG directory changed from `agentree.toml` to `config.toml` (e.g., `~/.config/agentree/config.toml`)
- Documentation updated to recommend `~/.agentree.toml` for simplicity while noting XDG option
- Agent requirement is now backend-specific: required for local backend, optional for claude-vm
- Config filename changed from `agentree.toml` to `.agentree.toml` (with leading dot) to match documentation
- Claude-vm backend now uses `--` separator when calling `claude-vm agent` to disambiguate arguments
- Agent command help text now clarifies when `--agent` flag is required vs optional

### Deprecated

- `--json` flag for `list` command; use `--format=json` instead (backward compatible with deprecation warning)

### Fixed

- Bash completion was missing `--base` flag value completion (zsh and fish had it)
- Zsh completion now shows both flags and branches (previously only showed branches)
- Zsh completion array subscript syntax error that caused "invalid subscript" error
- Config file not being loaded because code looked for `agentree.toml` instead of `.agentree.toml`
- Agent resolution error when no agent specified with claude-vm backend
- Zsh completion not falling back to static completion for flags

## [0.1.0] - 2026-02-16

### Added

- Initial release with core worktree management
- Backend abstraction with claude and claude-vm backends
- CLI commands: create, list, remove, shell, start
- Configuration system with hierarchical precedence
- Path templating with variable expansion
- Git utilities and worktree operations extracted from claude-vm
- Integration tests foundation
- CI/CD workflows for automated testing and releases
- Install script for easy installation

[Unreleased]: https://github.com/themouette/agentree/compare/v0.7.2...HEAD
[0.7.2]: https://github.com/themouette/agentree/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/themouette/agentree/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/themouette/agentree/compare/v0.1.0...v0.7.0
[0.1.0]: https://github.com/themouette/agentree/releases/tag/v0.1.0
