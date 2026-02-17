# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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

[Unreleased]: https://github.com/themouette/agentree/compare/v0.1.0...HEAD
