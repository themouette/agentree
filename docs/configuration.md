# Configuration Guide

Agentree uses a hierarchical configuration system with clear precedence rules.

## Shell Completion

Enable tab completion for agentree commands with **dynamic branch suggestions**:

```bash
# Bash
agentree completion bash >> ~/.bashrc
source ~/.bashrc

# Zsh
agentree completion zsh >> ~/.zshrc
source ~/.zshrc

# Fish
agentree completion fish > ~/.config/fish/completions/agentree.fish
```

### What Gets Completed

Once enabled, you can tab-complete:

**Static Completions:**
- **Commands**: `agentree <TAB>` shows all subcommands (create, list, shell, agent, etc.)
- **Flags**: `agentree create --<TAB>` shows available flags (--backend, --base, etc.)
- **Aliases**: `ls` and `rm` aliases work

**Dynamic Completions:**
- **Branch names**: `agentree shell <TAB>` shows your actual git branches
- **Commands with branches**: Works for `shell`, `agent`, `exec`, `remove`, `cd`
- **Base branches**: `agentree create new --base <TAB>` shows branches

### Example

```bash
# In a git repository with branches: main, feature-auth, hotfix-bug

$ agentree shell <TAB>
feature-auth    hotfix-bug    main

$ agentree remove <TAB>
feature-auth    hotfix-bug    main

$ agentree create new-feature --base <TAB>
feature-auth    hotfix-bug    main
```

**Note**: Dynamic branch completion only works when you're inside a git repository. Outside git repos, you get static completions only.

---

## Configuration Precedence

```
CLI arguments > Project config > Global config > Built-in defaults
```

**Example**: If you set `backend = "local"` globally but pass `--backend claude-vm` to a command, the CLI argument wins.

## Configuration Files

### Project Config: `.agentree.toml`

Place in your repository root to configure workspace defaults for all contributors:

```toml
[workspace]
location = "../worktrees"
template = "{repo}/{branch}"

[backend]
default = "local"

[agent]
default = "claude"

[agent.claude]
bin = "claude"
default_args = []

[agent.opencode]
bin = "opencode"
default_args = ["--quiet"]
```

**When to use**: Project-specific settings that all team members should share.

### Global Config: `~/.config/agentree/config.toml`

Your personal defaults across all projects:

```toml
[backend]
default = "claude-vm"

[agent]
default = "claude"

[agent.claude]
bin = "/opt/homebrew/bin/claude"
default_args = []
```

**When to use**: Personal preferences (e.g., you prefer VM isolation by default).

## Configuration Options

### `[workspace]` Section

Controls where and how worktrees are created.

#### `location` (optional)

Where to create worktree directories.

**Type**: String (path)
**Default**: `../worktrees` (sibling to repository root)
**Examples**:
```toml
# Relative to repo root
location = "../my-workspaces"

# Absolute path
location = "/Users/me/workspaces"

# Home directory
location = "~/workspaces"
```

**Notes**:
- Relative paths are resolved from repository root
- Directory is created automatically if it doesn't exist
- Use `~` for home directory expansion

#### `template` (optional)

Path template for worktree subdirectories.

**Type**: String (template)
**Default**: `"{repo}/{branch}"`
**Variables**:
- `{repo}` - Repository name (e.g., "myproject")
- `{branch}` - Branch name (e.g., "feature-auth")
- `{user}` - Current username (from `$USER`)
- `{date}` - Current date (YYYY-MM-DD format)
- `{short_hash}` - Short git hash (8 characters)

**Examples**:
```toml
# Group by repo
template = "{repo}/{branch}"
# Result: ../worktrees/myproject/feature-auth

# Flat structure
template = "{branch}"
# Result: ../worktrees/feature-auth

# Include user for shared systems
template = "{user}/{repo}/{branch}"
# Result: ../worktrees/alice/myproject/feature-auth

# With timestamp for experiments
template = "{repo}/{branch}-{date}"
# Result: ../worktrees/myproject/feature-auth-2024-01-15
```

**Sanitization**:
- `/` and `\` are replaced with `-`
- Spaces and control characters become `_`
- Path traversal (`..`) is blocked for security

**Validation Warnings**:
- ⚠️ Template without `{branch}` may cause collisions
- ⚠️ Template with `..` is unsafe
- ⚠️ Template starting with `/` may be unsafe

### `[backend]` Section

Controls workspace isolation strategy.

#### `default` (optional)

Which backend to use by default.

**Type**: String (enum)
**Default**: `"local"`
**Options**: `"local"`, `"claude-vm"`
**Example**:
```toml
[backend]
default = "claude-vm"
```

**Backend Descriptions**:

| Backend | Isolation | Binary Required | Use Case |
|---------|-----------|-----------------|----------|
| `local` | None | ❌ No | Trusted code, fast iteration |
| `claude-vm` | Lima VM | ✅ `claude-vm` | Untrusted code, full system isolation |

**Override per command**:
```bash
agentree create feature --backend claude-vm
```

### `[agent]` Section

Controls which AI agent to use.

#### `default` (optional)

Which agent to use when `--agent` is not specified.

**Type**: String
**Default**: `"claude"`
**Example**:
```toml
[agent]
default = "opencode"
```

**Override per command**:
```bash
agentree agent feature --agent claude
# Or use shortcuts:
agentree claude feature
agentree opencode feature
```

### `[agent.<name>]` Sections

Define custom agent configurations.

#### `bin` (required)

Path or name of the agent binary.

**Type**: String (path or binary name)
**Examples**:
```toml
[agent.claude]
bin = "claude"  # Use PATH lookup

[agent.opencode]
bin = "/usr/local/bin/opencode"  # Absolute path

[agent.custom]
bin = "~/bin/my-agent"  # Home directory
```

#### `default_args` (optional)

Default flags to pass to the agent.

**Type**: Array of strings
**Default**: `[]` (no arguments)
**Examples**:
```toml
[agent.claude]
bin = "claude"
default_args = []

[agent.opencode]
bin = "opencode"
default_args = ["--quiet", "--no-color"]

[agent.custom]
bin = "custom-agent"
default_args = ["--model", "gpt-4", "--temperature", "0.7"]
```

**Note**: CLI flags are appended to `default_args`:
```bash
# With default_args = ["--quiet"]
agentree agent feature --verbose
# Runs: opencode --quiet --verbose
```

## Complete Example

A production-ready configuration:

**Project `.agentree.toml`**:
```toml
# Shared team settings
[workspace]
location = "../workspaces"
template = "{repo}/{branch}"

[backend]
# Default to local for speed
default = "local"

[agent]
default = "claude"

[agent.claude]
bin = "claude"
default_args = []

[agent.opencode]
bin = "opencode"
default_args = ["--quiet"]
```

**Personal `~/.config/agentree/config.toml`**:
```toml
# Personal overrides
[backend]
# I prefer VM isolation for untrusted code
default = "claude-vm"

[agent]
default = "claude"

[agent.claude]
# My custom Claude installation
bin = "/opt/claude-code/bin/claude"
default_args = []
```

**Result**: You get VM isolation by default (your preference) but worktrees are created in team's shared location.

## CLI Overrides

All configuration can be overridden via CLI:

```bash
# Override backend
agentree create feature --backend local

# Override workspace location
agentree create feature --location ~/tmp-workspaces

# Override agent
agentree agent feature --agent opencode

# Multiple overrides
agentree create feature \
  --backend claude-vm \
  --location ~/experiments
```

## Environment Variables

### `AGENTREE_GIT_TIMEOUT`

Timeout for git operations in seconds.

**Default**: `30`
**Example**:
```bash
export AGENTREE_GIT_TIMEOUT=60
agentree create slow-repo-branch
```

**Use cases**:
- Large repositories with slow git operations
- Network operations (fetching large repos)
- Debugging git hangs

## Validation

Agentree validates configuration and shows helpful warnings:

### Warnings (Non-Fatal)

```toml
[workspace]
template = "{repo}"  # Missing {branch}
```
```
⚠️  Warning: Template does not contain {branch}.
    Different branches may resolve to the same worktree path.
```

### Errors (Fatal)

```toml
[backend]
default = "unknown-backend"
```
```
❌ Error: Unknown backend 'unknown-backend'.
   Available backends: local, claude-vm
```

## Tips & Best Practices

### 1. **Use Project Config for Team Standards**
```toml
# .agentree.toml (checked into git)
[workspace]
location = "../workspaces"
template = "{repo}/{branch}"
```
Everyone on the team gets consistent workspace paths.

### 2. **Use Global Config for Personal Preferences**
```toml
# ~/.config/agentree/config.toml
[backend]
default = "claude-vm"  # You prefer isolation
```
Your personal choice doesn't affect teammates.

### 3. **Include {branch} in Templates**
```toml
# Good: Each branch gets unique path
template = "{repo}/{branch}"

# Risky: Different branches may collide
template = "{repo}"
```

### 4. **Use Descriptive Agent Names**
```toml
[agent.claude-latest]
bin = "/opt/claude-v2/claude"

[agent.claude-stable]
bin = "/opt/claude-v1/claude"
```
Easily switch between versions.

### 5. **Keep Security in Mind**
```toml
# ❌ Avoid: Allows path traversal
template = "../{branch}"

# ✅ Good: Safe, scoped to workspace
template = "{repo}/{branch}"
```

## Troubleshooting Config Issues

### "Config file not found"

**Cause**: Agentree can't locate `.agentree.toml` or `~/.config/agentree/config.toml`.

**Solution**: This is OK! Agentree uses built-in defaults. Create config file only if you need customization.

### "Backend binary not found"

**Cause**: Configured backend binary doesn't exist in PATH.

**Example**:
```
❌ Backend 'claude-vm' not found. Install it using:
   brew install themouette/tap/claude-vm
```

**Solution**: Install the backend or change `default` backend:
```toml
[backend]
default = "local"  # Use built-in local backend
```

### "Worktree path collision"

**Cause**: Template doesn't include `{branch}`, so multiple branches map to same path.

**Solution**: Include `{branch}` in template:
```toml
[workspace]
template = "{repo}/{branch}"  # Not just "{repo}"
```

### "Permission denied creating workspace"

**Cause**: Workspace location isn't writable.

**Solution**: Use a location you have write access to:
```toml
[workspace]
location = "~/workspaces"  # Use home directory
```

## See Also

- [README](../README.md) - Quick start guide
- [Development Guide](development.md) - Architecture and contributing
- [Troubleshooting](troubleshooting.md) - Common issues and solutions
