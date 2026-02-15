# Agentree

**Workspace orchestration with pluggable isolation backends**

Manage multiple git branches simultaneously with automatic environment isolation. Choose your backend (local, VM, container) and your AI agent (Claude, OpenCode, etc.).

## Quick Start

```bash
# Create a new workspace with VM isolation
agentree create feature-branch --backend claude-vm

# List all workspaces
agentree list

# Open shell in workspace
agentree shell feature-branch

# Remove workspace when done
agentree remove feature-branch
```

## What is Agentree?

Agentree solves the problem of working on multiple branches simultaneously:
- **Git worktrees** for separate working directories per branch
- **Isolation backends** to prevent cross-contamination between workspaces
- **One command** to create workspace + start environment
- **Consistent interface** regardless of isolation mechanism

```
┌────────────────────────────────────────────┐
│             agentree                       │
│  (workspace + isolation + agent config)    │
└────────────┬───────────────────────────────┘
             │ Backend Trait
    ┌────────┴─────────┬──────────┐
    │                  │          │
┌───▼─────┐    ┌──────▼──────┐  ┌▼────────┐
│  local  │    │  claude-vm  │  │  docker │
│         │    │   (VM)      │  │  (TBD)  │
└─────────┘    └─────────────┘  └─────────┘
     │                │
     └────────┬───────┘
              │ runs any agent
    ┌─────────┼─────────┬──────────┐
┌───▼──────┐ │ ┌───────▼────┐ ┌───▼──────┐
│  claude  │ │ │  opencode  │ │  custom  │
│  (agent) │ │ │  (agent)   │ │  (agent) │
└──────────┘ │ └────────────┘ └──────────┘
```

## Backends (Isolation Strategy)

| Backend | Isolation | Use Case | Setup |
|---------|-----------|----------|-------|
| `local` | None | Trusted code, fast iteration | No setup required |
| `claude-vm` | Lima VM | Untrusted code, full isolation | Install [claude-vm](https://github.com/themouette/claude-vm) |
| `docker` | Container | Consistent environments | Install Docker (coming soon) |

## Agents (AI Tools)

Configure which AI agent to use via config or CLI. Any agent can work with any backend:

| Agent | Binary | Setup |
|-------|--------|-------|
| `claude` | `claude` | Install [Claude Code CLI](https://claude.com/claude-code) |
| `opencode` | `opencode` | Install OpenCode |
| Custom | Any binary | Configure in `.agentree.toml` |

## Installation

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

### Install backends and agents
```bash
# For VM isolation backend
brew install themouette/tap/claude-vm

# For AI agents
# Claude: Install from https://claude.com/claude-code
# OpenCode: Follow OpenCode installation instructions
```

## Usage

### Create Workspace

```bash
# Create with explicit backend
agentree create my-feature --backend claude-vm

# Create with auto-detected backend
agentree create my-feature

# Create from base branch
agentree create my-feature --base develop
```

### List Workspaces

```bash
$ agentree list
BRANCH         PATH                           BACKEND     STATUS
main           /Users/me/myproject           -           (main repo)
my-feature     ../worktrees/project-feature  claude-vm   Running
hotfix         ../worktrees/project-hotfix   claude      (local)
```

### Work in Workspace

```bash
# Open shell in workspace
agentree shell my-feature

# Execute command in workspace
agentree exec my-feature -- cargo test

# Start AI agent in workspace
agentree agent my-feature

# Use specific agent
agentree agent my-feature --agent opencode

# Convenience shortcut (equivalent to above)
agentree claude my-feature
agentree opencode my-feature

# Get workspace path (for scripting)
cd $(agentree path my-feature)
```

### Remove Workspace

```bash
# Remove specific workspace
agentree remove my-feature

# Remove all merged workspaces
agentree remove --merged main
```

### Backend Control

```bash
# Start backend for workspace
agentree start my-feature

# Stop backend
agentree stop my-feature

# Check status
agentree status
```

## Configuration

### Per-Project Config (`.agentree.toml`)

```toml
[workspace]
# Where to create worktrees (relative to repo)
root_dir = "../worktrees"
# Path template
template = "{repo}-{branch}"

[backend]
# Default isolation backend
default = "local"  # or "claude-vm"

[agent]
# Default AI agent to use
default = "claude"

[agent.claude]
bin = "claude"
default_args = []

[agent.opencode]
bin = "opencode"
default_args = ["--quiet"]
```

### Global Config (`~/.agentree/config.toml`)

```toml
[backend]
default = "local"

[agent]
default = "claude"

[agent.claude]
bin = "/usr/local/bin/claude"

[agent.opencode]
bin = "/usr/local/bin/opencode"
```

### CLI Override

```bash
# Use specific backend
agentree create feature --backend claude-vm

# Use specific agent
agentree agent feature --agent opencode

# Override workspace location
agentree create feature --worktree-location ~/my-workspaces
```

**Precedence**: CLI args > Project config > Global config > Defaults

## Architecture

See [PROJECT.md](PROJECT.md) for detailed architecture and design decisions.

**Key concepts**:
- **Workspace management** (agentree) is separate from **isolation** (backends)
- **Backend trait** defines standard interface for isolation providers
- Backends are **external CLIs** (not linked libraries) for independence
- **Git worktrees** provide the foundation for multiple working directories

## Development

### Setup
```bash
# Clone with claude-vm reference for extraction
git clone https://github.com/themouette/agentree
cd agentree

# Build
cargo build

# Test
cargo test

# Run locally
cargo run -- --help
```

### Adding a Backend

1. Create `src/backend/{name}.rs`
2. Implement `Backend` trait
3. Register in `src/backend/mod.rs`
4. Add tests
5. Document in `docs/backends/{name}.md`

See [.claude/BACKEND_SPEC.md](.claude/BACKEND_SPEC.md) for details.

## Documentation

- [PROJECT.md](PROJECT.md) - Vision, architecture, and roadmap
- [.claude/CLAUDE.md](.claude/CLAUDE.md) - Comprehensive development guide
- [.claude/EXTRACTION_GUIDE.md](.claude/EXTRACTION_GUIDE.md) - How code was extracted from claude-vm
- [.claude/BACKEND_SPEC.md](.claude/BACKEND_SPEC.md) - Backend trait specification

## Roadmap

### v0.1.0 (Current)
- ✅ Extract worktree logic from claude-vm
- ✅ Backend trait definition
- ✅ `local` and `claude-vm` backends
- ✅ Agent configuration system
- ✅ Basic commands (create, list, remove, agent)
- ✅ Config system with agent support
- ✅ CI/CD and release automation

### v0.2.0
- [ ] Agent auto-detection and suggestions
- [ ] Backend auto-detection and suggestions
- [ ] Shell completions
- [ ] Comprehensive error messages
- [ ] User guide and tutorials

### v1.0.0
- [ ] `docker` backend
- [ ] Session/context management
- [ ] Workspace templates
- [ ] Plugin system for third-party backends and agents

## Related Projects

- [claude-vm](https://github.com/themouette/claude-vm) - Lima VM isolation for Claude Code
- [Claude Code](https://claude.com/claude-code) - CLI agent from Anthropic
- [OpenCode](https://opencode.com) - Open source code assistant

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

**Quick links**:
- [Issues](https://github.com/themouette/agentree/issues)
- [Pull Requests](https://github.com/themouette/agentree/pulls)
- [Discussions](https://github.com/themouette/agentree/discussions)

## License

MIT License - see [LICENSE](LICENSE) for details.

## Credits

Agentree was created to separate workspace orchestration from isolation implementation, building on proven worktree patterns from the claude-vm project.

The backend abstraction enables the broader ecosystem to integrate isolation tools (VMs, containers, etc.) with a consistent workflow.
