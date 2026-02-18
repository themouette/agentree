# Troubleshooting Guide

Common issues and solutions when using agentree.

## Table of Contents

- [Installation Issues](#installation-issues)
- [Git & Worktree Issues](#git--worktree-issues)
- [Backend Issues](#backend-issues)
- [Configuration Issues](#configuration-issues)
- [Permission Issues](#permission-issues)
- [Agent Issues](#agent-issues)
- [General Issues](#general-issues)

## Installation Issues

### "command not found: agentree"

**Problem**: Shell can't find `agentree` after installation.

**Solution**:

```bash
# Check if binary exists
ls -la ~/.local/bin/agentree

# If it exists, add to PATH
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc

# For macOS with Homebrew
which agentree
# Should show: /opt/homebrew/bin/agentree or /usr/local/bin/agentree
```

### Installation script fails with "Permission denied"

**Problem**: Installing to system directory requires elevated permissions.

**Solution**:

```bash
# Option 1: Install to user directory (recommended)
curl -fsSL https://raw.githubusercontent.com/themouette/agentree/main/install.sh | bash

# Option 2: Install globally (requires sudo)
curl -fsSL https://raw.githubusercontent.com/themouette/agentree/main/install.sh | bash -s -- --global

# Option 3: Custom directory
curl -fsSL https://raw.githubusercontent.com/themouette/agentree/main/install.sh | bash -s -- --destination ~/bin
```

### Shell integration not working

**Problem**: `agentree cd` or `agentree cd <branch>` outputs text instead of changing directory.

**Solution**:

```bash
# Check if shell-init is in your RC file
grep agentree ~/.bashrc  # or ~/.zshrc

# If missing, add it:
echo 'eval "$(agentree shell-init)"' >> ~/.bashrc
source ~/.bashrc

# Test it works
agentree cd        # Should change to main repo
agentree cd main   # Should change to 'main' workspace
```

**Alternative**: Manual installation of shell function:

```bash
# For bash/zsh - add to ~/.bashrc or ~/.zshrc
eval "$(agentree shell-init)"

# For fish - add to ~/.config/fish/config.fish
agentree shell-init | source
```

## Git & Worktree Issues

### "Git version X.X.X is too old"

**Problem**: Git version doesn't support worktrees (need 2.5+).

**Solution**:

```bash
# Check current version
git --version

# Update git
# macOS
brew upgrade git

# Ubuntu/Debian
sudo apt update && sudo apt upgrade git

# Verify
git --version  # Should be 2.5 or higher
```

### "Not in a git repository"

**Problem**: Running agentree outside a git repository.

**Solution**:

```bash
# Navigate to your git repository
cd /path/to/your/repo

# Verify you're in a repo
git status

# Then run agentree
agentree create feature
```

### "Branch 'X' does not exist"

**Problem**: Trying to create workspace for non-existent branch.

**Solution**:

```bash
# Check available branches
git branch -a

# If branch exists remotely, fetch it
git fetch origin

# Create workspace from a base branch
agentree create new-feature main

# Or create branch first, then workspace
git checkout -b new-feature
agentree create new-feature
```

### "Did you mean: <suggestions>?"

**Problem**: Branch name typo detected.

**Example**:
```
❌ Branch 'mainnn' not found. Did you mean: main?
```

**Solution**:

```bash
# Use suggested branch name
agentree create feature --base main  # Not mainnn

# Or list all branches to find correct name
git branch -a
```

### Orphaned worktrees after manual deletion

**Problem**: Deleted worktree directory manually, git still tracks it.

**Symptoms**:
```bash
agentree list
# Shows worktree with broken path
```

**Solution**:

```bash
# Option 1: Let agentree clean up automatically
agentree clean

# Option 2: Manual git cleanup
git worktree prune

# Verify cleanup
agentree list
```

### "Repository contains submodules"

**Problem**: Git worktrees have experimental submodule support.

**Warning**:
```
⚠️  Warning: This repository contains submodules.
   Git worktree support for submodules is experimental.
```

**Solution**: This is just a warning. Worktrees should work but may have issues with submodule updates.

**Workaround**: Update submodules per-worktree:
```bash
agentree shell feature
cd /path/to/submodule
git submodule update --init --recursive
```

### "Worktree is locked"

**Problem**: Git worktree is locked (usually from interrupted operation).

**Solution**:

```bash
# Check lock status
git worktree list

# Unlock manually
git worktree unlock /path/to/worktree

# Or remove the lock file
rm /path/to/main/repo/.git/worktrees/<branch>/locked
```

## Backend Issues

### "Backend 'X' not found"

**Problem**: Configured backend binary doesn't exist.

**Example**:
```
❌ Backend 'claude-vm' not found. Install it using:
   brew install themouette/tap/claude-vm
```

**Solution**:

```bash
# Install the backend
brew install themouette/tap/claude-vm

# Or switch to local backend
agentree create feature --backend local

# Or configure global default
echo '[backend]
default = "local"' > ~/.agentree.toml
```

### "Backend binary not in PATH"

**Problem**: Backend binary exists but isn't in PATH.

**Solution**:

```bash
# Find where binary is installed
which claude-vm

# If not found, check common locations
ls /usr/local/bin/claude-vm
ls ~/.local/bin/claude-vm

# Add to PATH
export PATH="/usr/local/bin:$PATH"
echo 'export PATH="/usr/local/bin:$PATH"' >> ~/.bashrc
```

### "Backend version too old"

**Problem**: Backend version doesn't meet minimum requirement.

**Example**:
```
❌ Backend 'claude-vm' version 0.1.0 is too old.
   Minimum required: 0.5.0
   Update using: brew upgrade claude-vm
```

**Solution**:

```bash
# Update backend
brew upgrade claude-vm

# Or using cargo
cargo install claude-vm --force

# Verify version
claude-vm --version
```

### Backend command hangs

**Problem**: Backend operation appears frozen.

**Solution**:

```bash
# Check if process is running
ps aux | grep agentree

# Git operations have 30s timeout by default
# Increase if needed for large repos
export AGENTREE_GIT_TIMEOUT=60

# Try operation again
agentree shell feature
```

### "Workspace path not accessible"

**Problem**: Backend can't access workspace path (common with VM backends).

**Example**:
```
❌ Workspace path '/Users/me/worktrees/feature' is not accessible.
   Hint: Check that the path is mounted in your VM backend.
```

**Solution for VM backends**:

```bash
# Check VM mounts
claude-vm status

# Workspace must be under a mounted directory
# If using claude-vm, ensure parent is in config:
vim ~/.claude-vm/config.toml

# Add mount:
[[mounts]]
location = "/Users/me/worktrees"
writable = true

# Restart VM
claude-vm stop
claude-vm start
```

### Backend exits with unclear error

**Problem**: Backend fails without helpful message.

**Solution**:

```bash
# Test backend directly
claude-vm shell  # Should it work without agentree?

# Check backend logs
claude-vm logs  # If backend supports logging

# Use local backend for debugging
agentree shell feature --backend local

# Enable verbose output (if available)
agentree --verbose shell feature
```

### Docker Sandbox Issues

#### "Docker is not running"

**Problem**: Docker Desktop is not started.

**Example**:
```
❌ Docker daemon is not running. Please start Docker Desktop and try again.
   Check status with: docker info
```

**Solution**:

```bash
# Start Docker Desktop from Applications (macOS/Windows)

# Verify Docker is running
docker info

# Retry operation
agentree create feature --backend docker-sandbox
```

#### "Docker version does not support sandboxes"

**Problem**: Docker version is too old (need Engine 29.1.5+ / Desktop 4.58+).

**Example**:
```
❌ Docker version 28.0.0 does not support sandboxes.
   Minimum required: Engine 29.1.5 / Desktop 4.58
   Update Docker Desktop: https://www.docker.com/products/docker-desktop/
```

**Solution**:

```bash
# Check current version
docker --version

# Update Docker Desktop
# Download from: https://www.docker.com/products/docker-desktop/

# Verify version after update
docker --version  # Should show 29.1.5 or higher
```

#### "Docker Sandboxes are not supported on Linux"

**Problem**: Trying to use docker-sandbox on Linux (microVMs not available).

**Example**:
```
❌ Docker Sandboxes are not supported on Linux.
   Docker Sandboxes use microVMs which require macOS or Windows.
   Consider using the 'claude-vm' backend instead for VM isolation on Linux.
```

**Solution**:

```bash
# Use claude-vm backend instead
agentree create feature --backend claude-vm

# Or use local backend (no isolation)
agentree create feature --backend local

# Update config to avoid this error
echo '[backend]
default = "claude-vm"' > ~/.agentree.toml
```

#### "Docker sandbox not found"

**Problem**: Sandbox was removed outside of agentree.

**Example**:
```
❌ Docker sandbox 'agentree-feature-a1b2c3d4' not found.
   The sandbox may have been removed outside of agentree.
   Try running: agentree remove feature && agentree create feature
```

**Solution**:

```bash
# Remove workspace and recreate
agentree remove feature
agentree create feature --backend docker-sandbox

# Or list all sandboxes manually
docker sandbox ls | grep agentree

# Clean up manually if needed
docker sandbox rm -f agentree-feature-a1b2c3d4
```

#### Slow first launch with docker-sandbox

**Expected behavior**: First launch takes 10-30 seconds to create the microVM.

**Not an error** - subsequent launches are fast (~1-2s) when using persistent sandboxes.

**To verify persistence is working**:

```toml
# Check config
[docker-sandbox]
persistent = true  # Should be true (default)
```

**To force recreation**:

```bash
agentree remove feature
agentree create feature --backend docker-sandbox
```

#### Manual sandbox cleanup

**Problem**: Want to clean up all docker-sandbox sandboxes.

**Solution**:

```bash
# List all agentree sandboxes
docker sandbox ls | grep agentree

# Remove specific sandbox
docker sandbox rm -f agentree-feature-a1b2c3d4

# Remove all agentree sandboxes
docker sandbox ls --format "{{.Name}}" | grep '^agentree-' | xargs -I {} docker sandbox rm -f {}
```

**See also**: [Docker Sandbox Backend Documentation](backends/docker-sandbox.md)

## Configuration Issues

### "Failed to parse config"

**Problem**: TOML syntax error in config file.

**Example**:
```
❌ Failed to load config from '.agentree.toml': TOML parse error at line 5
```

**Solution**:

```bash
# Check TOML syntax
cat .agentree.toml

# Common issues:
# - Missing quotes around strings
# - Unmatched brackets
# - Invalid keys

# Validate with online tool
# https://www.toml-lint.com/

# Or start with minimal config
cat > .agentree.toml << 'EOF'
[workspace]
location = "../worktrees"
template = "{repo}/{branch}"

[backend]
default = "local"
EOF
```

### Config not being used

**Problem**: Changes to config file don't take effect.

**Solution**:

```bash
# Check config precedence:
# CLI args > Project config > ~/.agentree.toml > ~/.config/agentree/config.toml > Defaults

# Verify which config file is being used
cat .agentree.toml  # Project config
cat ~/.agentree.toml  # Home global config (if exists)
cat ~/.config/agentree/config.toml  # XDG global config (if exists)

# CLI args override config:
agentree create feature --backend local
# Even if config says: default = "claude-vm"

# Debug: Check what agentree sees
agentree create --help  # Shows default values
```

### "Template does not contain {branch}"

**Problem**: Template missing `{branch}` variable may cause path collisions.

**Warning**:
```
⚠️  Template '{repo}' does not contain {branch}.
   Different branches may resolve to the same worktree path.
```

**Solution**:

```toml
# Fix template to include {branch}
[workspace]
template = "{repo}/{branch}"  # Not just "{repo}"
```

### Workspace location doesn't exist

**Problem**: Configured workspace location isn't created.

**Solution**: Agentree creates the directory automatically. This warning is informational:

```
⚠️  Worktree location '/path/to/workspaces' does not exist.
   It will be created when needed.
```

To pre-create:
```bash
mkdir -p /path/to/workspaces
```

## Permission Issues

### "Permission denied" creating workspace

**Problem**: Don't have write permission for workspace location.

**Solution**:

```bash
# Option 1: Use writable location
# Update config to use home directory
cat > .agentree.toml << 'EOF'
[workspace]
location = "~/workspaces"
EOF

# Option 2: Fix permissions
sudo chown -R $USER /path/to/workspaces
```

### "Permission denied" during update

**Problem**: Binary installed in system directory (requires sudo for updates).

**Example**:
```
❌ Permission denied: Cannot write to /usr/local/bin/agentree
   Hint: Try running with sudo if binary is in system directory
```

**Solution**:

```bash
# Update with sudo
sudo agentree update

# Or reinstall to user directory
curl -fsSL https://install.agentree.dev | bash
# Installs to ~/.local/bin (no sudo needed)
```

## Agent Issues

### "Agent binary not found"

**Problem**: Configured agent doesn't exist.

**Solution**:

```bash
# Check which agent is configured
cat .agentree.toml | grep -A5 '^\[agent\]'

# Install missing agent
# For Claude:
# Visit https://claude.com/claude-code

# For OpenCode:
# Follow OpenCode installation

# Or configure different agent
echo '[agent]
default = "claude"

[agent.claude]
bin = "claude"' > .agentree.toml
```

### Agent starts but exits immediately

**Problem**: Agent fails to initialize.

**Debug steps**:

```bash
# Test agent directly
claude  # Should start agent
opencode  # Should start agent

# Check agent logs (if available)
# Claude: ~/.claude/logs/
# OpenCode: check OpenCode docs

# Try with explicit flags
agentree agent feature --agent claude -- --verbose

# Test in local backend first
agentree agent feature --backend local
```

### Wrong agent version

**Problem**: Multiple versions of agent installed.

**Solution**:

```bash
# Check which binary is being used
which claude
which opencode

# Configure explicit path
cat > .agentree.toml << 'EOF'
[agent.claude]
bin = "/opt/homebrew/bin/claude"  # Explicit path
EOF
```

## General Issues

### Commands are slow

**Problem**: Commands take long time to complete.

**Common causes**:
1. **Large repository**: Git operations on large repos are slow
2. **Network issues**: Fetching from remote
3. **Backend overhead**: VM backends have startup time

**Solutions**:

```bash
# Increase git timeout for large repos
export AGENTREE_GIT_TIMEOUT=120

# Use local backend for faster iteration
agentree create feature --backend local

# Disable backend isolation if not needed
echo '[backend]
default = "local"' > .agentree.toml
```

### "Out of disk space"

**Problem**: Worktrees fill up disk.

**Solution**:

```bash
# Check disk usage
du -sh ../worktrees/*

# Remove unused worktrees
agentree remove --merged main

# Clean up orphaned worktrees
agentree clean

# Remove all worktrees except main
agentree list --json | jq -r '.[] | select(.branch != null) | .branch' | xargs agentree remove
```

### JSON output malformed

**Problem**: Parsing `agentree list --json` fails.

**Debug**:

```bash
# Verify JSON is valid
agentree list --json | jq .

# If error, check for warnings in output
agentree list --json 2>/dev/null | jq .

# Save to file for inspection
agentree list --json > worktrees.json
cat worktrees.json
```

### Git hooks prevent operations

**Problem**: Git hooks block worktree operations.

**Solution**:

```bash
# Temporarily disable hooks
git config core.hooksPath /dev/null

# Run operation
agentree create feature

# Re-enable hooks
git config --unset core.hooksPath
```

### Strange characters in output

**Problem**: Color codes or escape sequences in output.

**Solution**:

```bash
# Disable colors (if available in future version)
export NO_COLOR=1
agentree list

# Or strip color codes
agentree list | sed 's/\x1b\[[0-9;]*m//g'
```

## Getting More Help

### Enable Debug Logging

```bash
# Set Rust log level
export RUST_LOG=debug
agentree create feature

# Or for specific modules
export RUST_LOG=agentree::backend=debug
```

### Report a Bug

When reporting issues, include:

```bash
# 1. Version info
agentree --version

# 2. System info
uname -a

# 3. Git version
git --version

# 4. Config (sanitize sensitive data)
cat .agentree.toml

# 5. Output of failing command
agentree create feature 2>&1 | tee error.log
```

### Resources

- **Issues**: [github.com/themouette/agentree/issues](https://github.com/themouette/agentree/issues)
- **Discussions**: [github.com/themouette/agentree/discussions](https://github.com/themouette/agentree/discussions)
- **Documentation**: [github.com/themouette/agentree/tree/main/docs](https://github.com/themouette/agentree/tree/main/docs)

## See Also

- [Configuration Guide](configuration.md) - All config options
- [Development Guide](development.md) - Contributing and architecture
- [README](../README.md) - Quick start guide
