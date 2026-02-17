# Docker Sandbox Backend

The `docker-sandbox` backend provides **microVM-based isolation** for running AI agents on untrusted code. It uses Docker Sandboxes, which offer hypervisor-level isolation stronger than regular containers.

## Overview

**What it provides:**
- Hypervisor-level isolation (microVMs, not containers)
- Safe execution of AI-generated code
- Support for Docker-in-Docker workflows
- Faster startup than full VM solutions (~10-30s cold start)
- Persistent sandboxes for better performance

**When to use:**
- Running AI agents on untrusted code
- Need stronger isolation than processes/containers
- Want Docker-in-Docker capability
- macOS or Windows environment

**Limitations:**
- **Linux not supported** - microVM sandboxes require macOS/Windows
- **Limited git worktree support** - main repo's `.git` directory cannot be mounted (Docker Sandboxes don't support custom volume mounts)
- Requires Docker Desktop 4.58+ (Engine 29.1.5+)
- Higher resource usage than `local` backend
- Slower than `local` (but faster than full VMs)

## Installation

### Prerequisites

**Docker Desktop 4.58 or newer** (includes Engine 29.1.5+)

Download from: https://www.docker.com/products/docker-desktop/

### Verify Installation

```bash
# Check Docker version
docker --version
# Should show: Docker version 29.1.5 or higher

# Check if Docker is running
docker info

# Test sandbox support (Docker Desktop 4.58+)
docker sandbox --help
```

## Usage

### Basic Usage

```bash
# Create workspace with docker-sandbox backend
agentree create feature-branch --backend docker-sandbox

# Start AI agent in sandbox (creates sandbox on first run)
agentree agent feature-branch --backend docker-sandbox

# Open shell in sandbox
agentree shell feature-branch --backend docker-sandbox
```

### Set as Default Backend

Configure in `.agentree.toml`:

```toml
[backend]
default = "docker-sandbox"

[docker-sandbox]
# Optional: custom docker binary path
binary = "docker"

# Optional: network policy
network_policy = "restricted"

# Optional: keep sandboxes running (default: true)
# Setting to true improves performance for subsequent launches
persistent = true
```

### Configuration Options

```toml
[docker-sandbox]
# Docker binary path (default: "docker")
binary = "/usr/local/bin/docker"

# Network policy for sandboxes (optional)
# Options depend on Docker Sandbox configuration
network_policy = "restricted"

# Keep sandboxes running between commands (default: true)
# true = faster subsequent launches, uses more resources
# false = clean slate each time, slower
persistent = true
```

## How It Works

### Sandbox Lifecycle

1. **First shell/agent call**: Sandbox is created using `docker sandbox create`
2. **Shell access**: Uses `docker sandbox exec -it` to run bash in the existing sandbox
3. **Agent execution**: Uses `docker sandbox run` in the existing sandbox
4. **Subsequent calls**: Reuses existing sandbox (fast, ~1-2s)
5. **Workspace removal**: Sandbox is automatically destroyed with `docker sandbox rm`

Each workspace gets its own persistent sandbox, identified by a deterministic name based on the workspace path.

### Naming Convention

Sandboxes are named deterministically from the workspace path:
```
agentree-{branch}-{hash}
```

Examples:
- Workspace: `/worktrees/agentree-feature` → Sandbox: `agentree-feature-a1b2c3d4`
- Workspace: `/worktrees/agentree-bugfix` → Sandbox: `agentree-bugfix-e5f6a7b8`

### Workspace Mounting

The workspace directory is automatically mounted at the **same absolute path** inside the sandbox:

```bash
# On host
/Users/me/worktrees/myproject/feature-auth

# In sandbox (same path)
/Users/me/worktrees/myproject/feature-auth
```

This is automatic - Docker Sandboxes handle the mounting internally.

### Git Worktree Limitations

⚠️ **Important**: Docker Sandboxes do not support custom volume mounts (the `-v` flag used by regular `docker run`).

**What this means for worktrees:**
- The workspace itself is always mounted and accessible
- **However**, the main repository's `.git` directory cannot be separately mounted
- Some git operations may have limited functionality inside the sandbox

**Example:**
```bash
# Main repo:          /Users/me/repos/myproject/.git
# Worktree:           /Users/me/worktrees/myproject/feature-branch
# Worktree .git file: points to main repo's .git

# Inside sandbox: worktree files are accessible, but .git link may not resolve
```

**Workaround:**
- Use `agentree exec` for git operations (runs on host, has full git access)
- Or use the `claude-vm` backend which supports full git worktree mounting

**Regular repos work fine:**
If you're not using git worktrees (just a regular cloned repository), this limitation doesn't affect you.

## Supported Agents

### Whitelisted Agents

These agents have native Docker Sandbox integration:

- `claude` - Claude AI
- `gemini` - Google Gemini

Usage:
```bash
agentree agent feature-branch --agent claude --backend docker-sandbox
```

### Custom Agents

Any agent binary can run in the sandbox:

```bash
agentree agent feature-branch --agent /usr/local/bin/my-agent --backend docker-sandbox
```

Custom agents are executed via `docker sandbox exec`.

## Commands

### Shell Access

Open an interactive shell in the sandbox:

```bash
agentree shell feature-branch --backend docker-sandbox
```

**How it works:**
- If the sandbox doesn't exist yet, it's automatically created
- Uses `docker sandbox exec -it` to run an interactive bash shell
- The sandbox persists between shell and agent sessions

Inside the sandbox:
- Your workspace is at the same absolute path as on the host
- You have full system access within the microVM
- Changes to files in the workspace are synced to the host

### Execute Commands

The `exec` command runs **on the host** (per BACK-08 decision), not in the sandbox. This is for quick inspection:

```bash
# Runs on host (fast, no isolation)
agentree exec feature-branch -- git status
agentree exec feature-branch -- ls -la
```

For isolated execution, use `shell` or `agent`.

### Agent Work

Run AI agents in isolated sandbox:

```bash
# Whitelisted agent (native integration)
agentree agent feature-branch --agent claude --backend docker-sandbox

# Custom agent
agentree agent feature-branch --agent opencode --backend docker-sandbox
```

## Troubleshooting

### Error: "Docker is not running"

**Solution**: Start Docker Desktop

```bash
# Check Docker status
docker info

# If not running, start Docker Desktop from Applications
```

### Error: "Docker version X.X.X does not support sandboxes"

**Solution**: Update Docker Desktop to 4.58 or newer

- Download from: https://www.docker.com/products/docker-desktop/
- Check version: `docker --version`
- Required: Engine 29.1.5+ / Desktop 4.58+

### Error: "Docker Sandboxes are not supported on Linux"

**Reason**: Docker Sandboxes use microVMs which are macOS/Windows only.

**Solution**: Use `claude-vm` backend instead, which works on all platforms:

```bash
agentree create feature-branch --backend claude-vm
```

### Sandbox Not Found

If you see "Docker sandbox 'agentree-X-XXXXXXXX' not found", the sandbox may have been removed outside of agentree.

**Solution**: Recreate the workspace:

```bash
agentree remove feature-branch
agentree create feature-branch --backend docker-sandbox
```

### Slow First Launch

**Expected behavior**: The first launch takes 10-30 seconds to create the microVM.

**Subsequent launches are fast** (~1-2s) because sandboxes persist.

To force recreation:
```bash
# Remove and recreate workspace
agentree remove feature-branch
agentree create feature-branch --backend docker-sandbox
```

### List All Sandboxes

See all Docker Sandboxes created by agentree:

```bash
docker sandbox ls | grep agentree
```

### Manual Cleanup

Remove a specific sandbox manually:

```bash
docker sandbox rm -f agentree-feature-a1b2c3d4
```

Remove all agentree sandboxes:

```bash
docker sandbox ls --format "{{.Name}}" | grep '^agentree-' | xargs -I {} docker sandbox rm -f {}
```

## Comparison with Other Backends

| Feature                  | local        | docker-sandbox     | claude-vm      |
| ------------------------ | ------------ | ------------------ | -------------- |
| **Isolation**            | None         | MicroVM            | Lima VM        |
| **Platform**             | All          | macOS/Windows      | All            |
| **Setup**                | Built-in     | Docker Desktop     | claude-vm CLI  |
| **Startup Time**         | Instant      | 10-30s (cold)      | 30-60s (cold)  |
| **Subsequent Launches**  | Instant      | ~1-2s              | ~5-10s         |
| **Resource Usage**       | Minimal      | Medium             | High           |
| **Security**             | None         | Hypervisor-level   | Hypervisor-level |
| **Docker-in-Docker**     | Host's Docker| ✅ Yes             | ✅ Yes         |
| **Git Worktrees**        | ✅ Full      | ⚠️ Limited         | ✅ Full        |
| **Agent Support**        | Any          | Any                | Any            |

### When to Choose docker-sandbox

✅ **Choose docker-sandbox if:**
- You're on macOS or Windows
- You need isolation for AI-generated code
- You want faster startup than full VMs
- You already have Docker Desktop installed

❌ **Don't choose docker-sandbox if:**
- You're on Linux (use `claude-vm` instead)
- You don't have Docker Desktop 4.58+
- You need the fastest possible performance (use `local`)
- You trust the code you're working with (use `local`)

## Advanced Usage

### Custom Network Policies

Configure network restrictions for sandboxes:

```toml
[docker-sandbox]
network_policy = "restricted"
```

Consult Docker Sandbox documentation for available policies.

### Persistent vs Ephemeral Sandboxes

**Persistent (default, recommended):**
```toml
[docker-sandbox]
persistent = true
```

- ✅ Fast subsequent launches
- ✅ Preserves installed packages
- ✅ Better developer experience
- ❌ Uses more resources

**Ephemeral:**
```toml
[docker-sandbox]
persistent = false
```

- ✅ Clean slate every time
- ✅ Lower resource usage
- ❌ Very slow (30s+ per launch)
- ❌ Loses all state

### Per-Project Configuration

Override global settings per project:

```toml
# .agentree.toml in project root
[backend]
default = "docker-sandbox"

[docker-sandbox]
persistent = true
network_policy = "restricted"
```

## Security Considerations

**What docker-sandbox protects against:**
- Malicious code execution on host
- Filesystem access outside workspace
- Network attacks on host
- Resource exhaustion attacks

**What it doesn't protect against:**
- Data exfiltration (code can see workspace files)
- Denial of service (can still consume resources)
- All vulnerabilities (microVMs are not perfect)

**Best practices:**
- Review AI-generated code before running
- Don't store secrets in workspace files
- Use network policies to restrict outbound traffic
- Monitor resource usage

## FAQ

### Does this work with Docker Desktop for Mac/Windows?

Yes! Docker Sandboxes are part of Docker Desktop 4.58+.

### Can I use this with Podman or other Docker alternatives?

No. Docker Sandboxes require Docker Desktop specifically.

### Why not Linux?

Docker Sandboxes use microVMs (hypervisor-level isolation) which are only available in Docker Desktop for macOS/Windows. On Linux, use the `claude-vm` backend for similar isolation.

### How is this different from regular Docker containers?

Docker Sandboxes use **microVMs** (hypervisor-level isolation) rather than containers (namespace isolation). This provides:
- Stronger security guarantees
- Full system isolation
- Ability to run Docker-in-Docker safely

### Can I ssh into the sandbox?

Use `agentree shell` instead:

```bash
agentree shell feature-branch --backend docker-sandbox
```

This opens an interactive bash shell inside the sandbox.

### Do I need to stop sandboxes manually?

No. Agentree manages the sandbox lifecycle:
- Created on first use
- Reused for subsequent commands
- Destroyed when workspace is removed

### Can multiple workspaces share a sandbox?

No. Each workspace gets its own isolated sandbox, ensuring no cross-contamination.

### Why don't git worktrees work fully in docker-sandbox?

Docker Sandboxes don't support custom volume mounts (`-v` flag). The workspace itself is mounted automatically, but we can't mount the main repo's `.git` directory separately.

**Solution**: Use `agentree exec` for git operations (runs on host), or switch to `claude-vm` backend for full worktree support:

```bash
# Git operations on host (full access)
agentree exec feature-branch -- git status
agentree exec feature-branch -- git commit -m "message"

# Or use claude-vm for full isolation + worktree support
agentree create feature-branch --backend claude-vm
```

## See Also

- [Configuration Guide](../configuration.md) - All configuration options
- [Troubleshooting](../troubleshooting.md) - Common issues and solutions
- [claude-vm Backend](./claude-vm.md) - Alternative VM isolation (works on Linux)
