# Configuration Guide

Agentree uses a hierarchical configuration system with clear precedence rules.

## Shell Integration & Completion

Enable both cd command and tab completion with one command:

```bash
# One-liner (recommended): cd command + completion
echo 'eval "$(agentree shell-init --with-completion)"' >> ~/.bashrc
source ~/.bashrc

# Or separately:
# Just cd command
echo 'eval "$(agentree shell-init)"' >> ~/.bashrc

# Just completion
echo 'eval "$(agentree completion bash)"' >> ~/.bashrc

# For zsh: replace ~/.bashrc with ~/.zshrc
# For fish: agentree completion fish > ~/.config/fish/completions/agentree.fish
```

### What Gets Completed

Once enabled, you can tab-complete:

**Static Completions:**
- **Commands**: `agentree <TAB>` shows all subcommands (create, list, shell, agent, etc.)
- **Flags**: `agentree create --<TAB>` shows available flags (--backend, --base, etc.)
- **Help text**: All flags and commands include descriptions

**Dynamic Flag Value Completions:**
- **Branch names for --base flag**: `agentree create new --base <TAB>` shows git branches
- **Agent names for --agent flag**: `agentree agent my-branch --agent <TAB>` shows available agents
- **Backend names for --backend flag**: `agentree create new --backend <TAB>` shows backends

### Example

```bash
# In a git repository with branches: main, feature-auth, hotfix-bug

# Flag value completion (works reliably)
$ agentree create new-feature --base <TAB>
feature-auth    hotfix-bug    main

$ agentree agent my-branch --agent <TAB>
claude    opencode

$ agentree create new --backend <TAB>
local    claude-vm    docker-sandbox

# Positional branch arguments: type the branch name directly (no tab completion)
$ agentree shell my-branch
$ agentree agent feature-auth
```

**Note**: Dynamic completions only work inside git repositories. The `--base` flag completion is especially useful to avoid typos when specifying base branches.

---

## Configuration Precedence

Configuration is loaded and merged in this order (later overrides earlier):

```
Built-in defaults
  ↓
XDG global config (~/.config/agentree/config.toml or ~/Library/Application Support/agentree/config.toml)
  ↓
Home global config (~/.agentree.toml)
  ↓
Project config (.agentree.toml)
  ↓
CLI arguments (highest priority)
```

**Example**: If you set `backend = "local"` in `~/.agentree.toml` but pass `--backend claude-vm` to a command, the CLI argument wins.

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

### Global Config

Agentree supports **two locations** for global configuration (both optional):

1. **XDG-compliant** (recommended for Linux):
   - Linux: `~/.config/agentree/config.toml`
   - macOS: `~/Library/Application Support/agentree/config.toml`

2. **Simple home directory** (recommended for simplicity):
   - All platforms: `~/.agentree.toml`

**Precedence**: If both exist, `~/.agentree.toml` overrides the XDG path.

**Example** (`~/.agentree.toml`):

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
**Options**: `"local"`, `"claude-vm"`, `"docker-sandbox"`
**Example**:
```toml
[backend]
default = "docker-sandbox"
```

**Backend Descriptions**:

| Backend | Isolation | Binary Required | Platform | Use Case |
|---------|-----------|-----------------|----------|----------|
| `local` | None | ❌ No | All | Trusted code, fast iteration |
| `claude-vm` | Lima VM | ✅ `claude-vm` | All | Untrusted code, full system isolation |
| `docker-sandbox` | MicroVM (Docker) | ✅ Docker Desktop 4.58+ | macOS/Windows | AI agents on untrusted code, fast boot |

**Override per command**:
```bash
agentree create feature --backend docker-sandbox
```

**See also**: [Docker Sandbox Backend Documentation](backends/docker-sandbox.md)

### `[docker-sandbox]` Section

Configuration options specific to the Docker Sandbox backend.

#### `binary` (optional)

Custom path to the Docker binary.

**Type**: String (path)
**Default**: `"docker"` (from PATH)
**Example**:
```toml
[docker-sandbox]
binary = "/usr/local/bin/docker"
```

#### `network_policy` (optional)

Network policy for sandboxes. Available policies depend on Docker Desktop configuration.

**Type**: String
**Default**: None (uses Docker default)
**Example**:
```toml
[docker-sandbox]
network_policy = "restricted"
```

#### `persistent` (optional)

Whether to keep sandboxes running between commands. When enabled, subsequent workspace launches are much faster (~1-2s vs ~10-30s).

**Type**: Boolean
**Default**: `true`
**Example**:
```toml
[docker-sandbox]
persistent = true  # Faster launches, uses more resources
# persistent = false  # Clean slate each time, slower
```

#### `mount_main_git` (optional)

⚠️ **Note**: This option has no effect on the `docker-sandbox` backend. Docker Sandboxes do not support custom volume mounts, so the main repo's `.git` directory cannot be mounted separately.

Whether to mount the main repository's `.git` directory for worktrees. When enabled (on supported backends like `claude-vm`), git commands work properly inside the sandbox.

**Type**: Boolean
**Default**: `true`
**Supported backends**: `claude-vm` only (not `docker-sandbox`)
**Example**:
```toml
[claude-vm]
mount_main_git = true  # Enable git commands in worktrees (claude-vm only)
# mount_main_git = false  # Stricter isolation, but git won't work
```

**Full example**:
```toml
[backend]
default = "docker-sandbox"

[docker-sandbox]
binary = "docker"
persistent = true
network_policy = "restricted"

# For claude-vm backend:
[claude-vm]
mount_main_git = true  # Enables git worktree support
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

### `[editor]` Section

Controls which editor to use for the `agentree editor` command.

#### `bin` (optional)

Path or name of the editor binary.

**Type**: String (path or binary name)
**Default**: Falls back to `$EDITOR`, then `$VISUAL`, then `vi`
**Examples**:
```toml
[editor]
bin = "code"  # VS Code

[editor]
bin = "nvim"  # Neovim

[editor]
bin = "/usr/local/bin/sublime"  # Absolute path
```

**Editor Selection Priority**:
1. `--editor` CLI flag (highest)
2. `editor.bin` from config
3. `$EDITOR` environment variable
4. `$VISUAL` environment variable
5. `vi` (fallback)

#### `default_args` (optional)

Default flags to pass to the editor.

**Type**: Array of strings
**Default**: `[]` (no arguments)
**Examples**:
```toml
[editor]
bin = "code"
default_args = ["--wait", "--new-window"]

[editor]
bin = "nvim"
default_args = ["+set number"]
```

**Override per command**:
```bash
# Use different editor for this workspace
agentree editor feature --editor nvim

# Pass additional arguments
agentree editor feature -- --readonly
```

**Note**: The editor command always runs on the **local machine**, not through backend isolation. This ensures your local editor configuration and plugins work correctly.

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

[editor]
bin = "code"
default_args = ["--wait"]
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

[editor]
# I prefer neovim
bin = "nvim"
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

# Override editor
agentree editor feature --editor nvim

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
