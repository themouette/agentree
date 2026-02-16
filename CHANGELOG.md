# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Agent value completion for `--agent` flag showing available agents (claude, opencode)
- Backend value completion for `--backend` flag showing available backends (local, claude-vm)
- Shell completion improvements for bash, zsh, and fish
- Centralized default agents constant (`DEFAULT_AGENTS`) as single source of truth
- Fallback agent resolution to hardcoded defaults (claude, opencode) when not configured
- 7 new tests for agent resolution including fallback behavior
- 4 new tests for claude-vm backend argument building

### Changed

- Agent requirement is now backend-specific: required for local backend, optional for claude-vm
- Config filename changed from `agentree.toml` to `.agentree.toml` (with leading dot) to match documentation
- Claude-vm backend now uses `--` separator when calling `claude-vm agent` to disambiguate arguments
- Agent command help text now clarifies when `--agent` flag is required vs optional

### Fixed

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
[0.1.0]: https://github.com/themouette/agentree/releases/tag/v0.1.0
