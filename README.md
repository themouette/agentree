# Agentree

**Worktree management for the AI era**

Agentree manages multiple git branches simultaneously with automatic environment isolation. Work on a feature branch, hotfix, and experiment in parallel - each in its own isolated workspace.

## What Does It Do?

Agentree combines:

- **Git worktrees** - Separate working directories for each branch
- **Isolation backends** - Choose your isolation level (none, VM, sandbox)
- **Agent management** - Work with any AI coding assistant

```
main repo          ┌─> feature-auth (isolated)
  ├─ src/          │   ├─ src/
  ├─ tests/        │   ├─ tests/
  └─ README.md     │   └─ README.md
                   │
                   ├─> hotfix-bug (isolated)
                   │   ├─ src/
                   │   └─ tests/
                   │
                   └─> experiment (isolated)
```

## Installation

**One-line install** (macOS/Linux):

```bash
curl -fsSL https://raw.githubusercontent.com/themouette/agentree/main/install.sh | bash
```

<details>
<summary>Other installation methods</summary>

### Homebrew (macOS)

```bash
brew install themouette/tap/agentree
```

### From source

```bash
git clone https://github.com/themouette/agentree
cd agentree
cargo install --path .
```

### Shell integration & completion (optional but recommended)

```bash
# One-liner: enables cd command + tab completion
echo 'eval "$(agentree shell-init --with-completion)"' >> ~/.bashrc  # or ~/.zshrc

# Or separately:
# Just cd command: eval "$(agentree shell-init)"
# Just completion: eval "$(agentree completion bash)"

# After sourcing, you get:
# - cd command: agentree cd <branch> changes directory
# - Tab completion: agentree create new --base <TAB> shows your git branches
# - Tab completion: agentree agent my-branch --agent <TAB> shows available agents
```

### Shell prompt customization (optional)

When using `agentree shell`, environment variables are set that you can use to customize your prompt:

<details>
<summary><b>Bash/Zsh Example</b></summary>

Add to your `~/.bashrc` or `~/.zshrc`:

```bash
# Show agentree workspace in prompt
if [ -n "$AGENTREE_WORKSPACE" ]; then
    PS1="(🌳 $AGENTREE_BRANCH) $PS1"
fi
```

Result: `(🌳 feature-auth) user@host ~/workspace $`

</details>

<details>
<summary><b>Starship Example</b></summary>

Add to your `~/.config/starship.toml`:

```toml
[env_var.AGENTREE_BRANCH]
symbol = "🌳 "
format = "[$symbol$env_value]($style) "
style = "cyan bold"
```

</details>

**Available environment variables:**
- `AGENTREE_WORKSPACE=1` - Indicates you're in an agentree workspace
- `AGENTREE_BRANCH=<branch>` - The workspace branch name
- `AGENTREE_WORKSPACE_PATH=<path>` - Full path to workspace

</details>

## Quick Start

### Typical Workflow

```bash
# 1. Start working on a feature
agentree agent feature-auth

# Your workspace is auto-created if it doesn't exist!
# Now you're in an isolated environment with your AI agent

# 2. See all your workspaces
agentree list

# Output:
# BRANCH         PATH                              BACKEND  CREATED
# main           /Users/me/myproject              -        (main)
# feature-auth   ../worktrees/myproject/feature   local    2024-01-15

# 3. Done with work? Clean up merged branches
agentree remove --merged main

# Removes all branches that have been merged into main
```

### Common Commands

```bash
# Start AI agent in a workspace (creates if needed)
agentree agent <branch>
agentree agent <branch> --base main    # Create from specific branch
agentree claude <branch>               # Shortcut for Claude
agentree opencode <branch>             # Shortcut for OpenCode

# Create workspace explicitly
agentree create <branch>
agentree create <branch> -b develop    # Create from develop branch

# List all workspaces
agentree list
agentree ls                    # Alias

# Remove workspace(s)
agentree remove <branch>
agentree rm feature-*          # Supports wildcards

# Open shell in workspace
agentree shell <branch>
agentree shell <branch> --base main    # Create from main if doesn't exist

# Open editor in workspace (runs on local machine)
agentree editor <branch>

# Execute command in workspace
agentree exec <branch> -- npm test
agentree exec <branch> -b hotfix -- npm test  # Create from hotfix

# Go to workspace directory
agentree cd <branch>

# Remove merged branches (cleanup)
agentree remove --merged main
```

## Backends (Isolation Levels)

Choose your isolation strategy:

| Backend          | Isolation          | Use Case                              | Setup                                                        |
| ---------------- | ------------------ | ------------------------------------- | ------------------------------------------------------------ |
| `local`          | None               | Trusted code, fast iteration          | ✅ Built-in (default)                                        |
| `claude-vm`      | Lima VM            | Untrusted code, full system isolation | Install [claude-vm](https://github.com/themouette/claude-vm) |
| `docker sandbox` | Use Docker sandbox | Coming soon                           | -                                                            |

**Set backend per workspace:**

```bash
agentree create feature --backend claude-vm
```

**Or configure default** in `.agentree.toml`:

```toml
[backend]
default = "claude-vm"
```

## Agents (AI Tools)

Any AI agent can work with any backend:

```bash
# Use Claude
agentree claude feature-branch

# Use OpenCode
agentree opencode feature-branch

# Use custom agent
agentree agent feature-branch --agent my-custom-agent
```

Configure agents in `.agentree.toml`:

```toml
[agent]
default = "claude"

[agent.opencode]
bin = "opencode"
default_args = ["--quiet"]
```

## Configuration

Create `.agentree.toml` in your project:

```toml
[workspace]
# Where to create worktrees (relative to repo root)
location = "../worktrees"
# Path template: {repo}, {branch}, {user}, {date}, {short_hash}
template = "{repo}/{branch}"

[backend]
default = "local"  # or "claude-vm"

[agent]
default = "claude"

[editor]
bin = "code"  # Use VS Code, falls back to $EDITOR
default_args = ["--wait"]
```

See [docs/configuration.md](docs/configuration.md) for all options.

## Documentation

- **[Configuration Guide](docs/configuration.md)** - All configuration options
- **[Development Guide](docs/development.md)** - Contributing and architecture
- **[Troubleshooting](docs/troubleshooting.md)** - Common issues and solutions

## Why Agentree?

**Problem**: Working on multiple branches means either:

- Constant `git stash` + `git checkout` (slow, error-prone)
- Multiple clones (wastes disk space, out-of-sync remotes)

**Solution**: Git worktrees give you multiple working directories, but managing them manually is tedious. Agentree automates:

- ✅ Worktree creation with proper path management
- ✅ Backend isolation (VM, container, or none)
- ✅ Agent integration (Claude, OpenCode, etc.)
- ✅ Cleanup of merged/stale branches

## Examples

### Feature Development

```bash
# Start new feature from main
agentree claude feature-auth main

# Work on it...
# (agentree auto-creates workspace from main branch)

# When done, clean up
git checkout main
git pull
agentree remove --merged main  # Removes feature-auth if merged
```

### Multiple Branches

```bash
# Work on feature
agentree shell feature-auth

# Switch to hotfix (in another terminal)
agentree shell hotfix-bug

# Check status of all
agentree list
```

### Custom Workflow

```bash
# Create workspace without starting agent
agentree create experiment

# Run tests in that workspace
agentree exec experiment -- cargo test

# Open shell to investigate
agentree shell experiment

# Remove when done
agentree remove experiment
```

## Contributing

We welcome contributions! See [docs/development.md](docs/development.md) for:

- Development setup
- Architecture overview
- Adding new backends
- Testing guidelines

## License

MIT License - see [LICENSE](LICENSE) for details.

## Related Projects

- [claude-vm](https://github.com/themouette/claude-vm) - Lima VM isolation for AI agents
