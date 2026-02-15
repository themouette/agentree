# Configuration Examples

This directory contains example configurations for different use cases.

## Quick Start

1. Choose an example that matches your needs
2. Copy it to your project root as `.agentree.toml`
3. Customize as needed

```bash
# Use basic config
cp examples/basic.toml .agentree.toml

# Or start with team-shared config
cp examples/team-shared.toml .agentree.toml
```

## Available Examples

### [`basic.toml`](basic.toml)
Minimal configuration with sensible defaults. Good starting point for most projects.

**Use when**: You want simple, straightforward setup with local backend.

### [`team-shared.toml`](team-shared.toml)
Configuration designed to be checked into git and shared with your team.

**Use when**: You want consistent workspace paths across your team.

### [`isolated.toml`](isolated.toml)
Configuration using VM backend for full isolation.

**Use when**: Working with untrusted code or requiring system-level isolation.

**Requires**: `brew install themouette/tap/claude-vm`

### [`custom-paths.toml`](custom-paths.toml)
Demonstrates different workspace path templates and locations.

**Use when**: You need custom workspace organization (by user, by date, etc.).

### [`global-config.toml`](global-config.toml)
Personal configuration for `~/.config/agentree/config.toml`.

**Use when**: Setting your personal preferences across all projects.

**Note**: Place at `~/.config/agentree/config.toml`, not in project.

### [`multi-agent.toml`](multi-agent.toml)
Configuration with multiple AI agents (Claude, OpenCode, custom agents).

**Use when**: You work with different AI tools for different tasks.

## Configuration Hierarchy

Agentree uses this precedence order:

```
CLI arguments > Project .agentree.toml > Global ~/.config/agentree/config.toml > Defaults
```

**Example workflow**:
1. Set personal preferences in `~/.config/agentree/config.toml` (use [`global-config.toml`](global-config.toml))
2. Set team standards in project `.agentree.toml` (use [`team-shared.toml`](team-shared.toml))
3. Override per-command: `agentree create feature --backend claude-vm`

## Template Variables

Use these in the `template` setting:

- `{repo}` - Repository name (e.g., "myproject")
- `{branch}` - Branch name (e.g., "feature-auth")
- `{user}` - Current username (from `$USER`)
- `{date}` - Current date (YYYY-MM-DD format)
- `{short_hash}` - Short git hash (8 characters)

**Examples**:
```toml
# Group by repo
template = "{repo}/{branch}"
# → ../worktrees/myproject/feature-auth

# Flat structure
template = "{branch}"
# → ../worktrees/feature-auth

# Include user for shared systems
template = "{user}/{repo}/{branch}"
# → ../worktrees/alice/myproject/feature-auth

# With timestamp
template = "{repo}/{branch}-{date}"
# → ../worktrees/myproject/feature-auth-2024-01-15
```

## Backend Options

| Backend | Isolation | Use Case | Setup |
|---------|-----------|----------|-------|
| `local` | None | Trusted code, fast iteration | ✅ Built-in |
| `claude-vm` | Lima VM | Untrusted code, system isolation | `brew install themouette/tap/claude-vm` |

## Testing Configuration

Test your configuration before committing:

```bash
# Test workspace creation
agentree create test-branch

# Check resulting path
agentree list

# Clean up test
agentree remove test-branch
```

## Common Patterns

### Pattern 1: Team Standardization
**Project** `.agentree.toml`:
```toml
[workspace]
location = "../workspaces"
template = "{repo}/{branch}"
```

**Personal** `~/.config/agentree/config.toml`:
```toml
[backend]
default = "claude-vm"  # You prefer isolation
```

**Result**: Team gets consistent paths, you get your preferred backend.

### Pattern 2: Per-Feature Backend
**Project** `.agentree.toml`:
```toml
[backend]
default = "local"
```

**Command**:
```bash
# Use local for trusted features
agentree create feature-ui

# Use VM for untrusted external code
agentree create feature-plugin --backend claude-vm
```

### Pattern 3: Multi-Agent Workflow
**Project** `.agentree.toml`:
```toml
[agent]
default = "claude"

[agent.claude]
bin = "claude"

[agent.specialist]
bin = "opencode"
default_args = ["--model", "specialized"]
```

**Usage**:
```bash
# General development with Claude
agentree claude feature-general

# Specialized tasks with different agent
agentree agent feature-special --agent specialist
```

## See Also

- [Configuration Guide](../docs/configuration.md) - Complete reference
- [Development Guide](../docs/development.md) - Architecture and contributing
- [Troubleshooting](../docs/troubleshooting.md) - Common issues
