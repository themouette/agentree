# Agentree Project Context

**Agentree** is a workspace orchestration CLI that manages git worktrees with pluggable isolation backends (Claude Code, claude-vm, OpenCode, etc.).

## Project Purpose

**What**: A standalone tool for managing multiple workspace contexts (branches) with automated isolation backend integration.

**Why**: Separate workspace orchestration (git + session management) from isolation implementation (VMs, containers, etc.). This allows:
- Users to work on multiple branches simultaneously with proper isolation
- Flexibility to choose isolation backend (local, VM, container, etc.)
- Backends focus on isolation mechanics, agentree handles workflow

**Architecture**: Workspace orchestrator → Backend abstraction → Isolation providers

## Core Architecture

```
┌─────────────────────────────────────────────────┐
│              agentree CLI                       │
│  (workspace management + session coordination)  │
└────────────┬────────────────────────────────────┘
             │
             │ Backend Trait
             │
     ┌───────┴───────┬──────────────┬──────────────┐
     │               │              │              │
┌────▼─────┐  ┌─────▼──────┐  ┌───▼────────┐  ┌──▼─────┐
│  claude  │  │ claude-vm  │  │  opencode  │  │ docker │
│ (local)  │  │ (Lima VM)  │  │            │  │        │
└──────────┘  └────────────┘  └────────────┘  └────────┘
```

### Backend Trait

Backends must implement:
```rust
pub trait Backend {
    /// Start isolated environment for workspace
    fn start(&self, ctx: &WorkspaceContext) -> Result<()>;

    /// Stop isolated environment
    fn stop(&self, ctx: &WorkspaceContext) -> Result<()>;

    /// Check if environment is running
    fn is_running(&self, ctx: &WorkspaceContext) -> Result<bool>;

    /// Open interactive shell in workspace
    fn shell(&self, ctx: &WorkspaceContext) -> Result<()>;

    /// Execute command in isolated environment
    fn exec(&self, ctx: &WorkspaceContext, cmd: &[&str]) -> Result<ExecOutput>;

    /// Get backend status information
    fn status(&self, ctx: &WorkspaceContext) -> Result<BackendStatus>;
}

pub struct WorkspaceContext {
    pub workspace_path: PathBuf,
    pub main_repo_path: PathBuf,
    pub branch: String,
    pub backend_config: BackendConfig,
}
```

## Backend Implementations

### 1. `claude` Backend (Local)
- Runs Claude Code directly without isolation
- Simplest backend - just `cd` to workspace and run `claude`
- Use case: Trusted code, no isolation needed

### 2. `claude-vm` Backend
- Delegates to claude-vm CLI for Lima VM isolation
- Calls out to `claude-vm agent` in workspace directory
- Use case: Full system isolation, untrusted code

### 3. `opencode` Backend
- Similar to claude backend but for OpenCode
- Integration with OpenCode CLI
- Use case: OpenCode users who want workspace management

### 4. Future: `docker` Backend
- Container-based isolation
- Use case: Consistent environments, easy to reset

## CLI Interface

Keep the proven interface from claude-vm:

```bash
# Create workspace (git worktree + start backend)
agentree create <branch> [--base <base-branch>] [--backend <backend>]

# List all workspaces with status
agentree list

# Remove workspace(s)
agentree remove <branch>...
agentree remove --merged <base-branch>

# Backend control
agentree start [branch]    # Start backend for workspace
agentree stop [branch]     # Stop backend
agentree shell [branch]    # Open shell in workspace

# Info and status
agentree info [branch]     # Show workspace details
agentree status            # Show all workspaces + backend status
```

## What to Extract from claude-vm

The following code is proven and should be extracted:

### Core Worktree Logic (`claude-vm/src/worktree/`)
- `operations.rs` - Git worktree create/list/remove
- `config.rs` - Workspace configuration
- `template.rs` - Path templating for workspace locations
- `validation.rs` - Branch validation, git checks
- `recovery.rs` - Orphaned worktree detection and cleanup

### Git Utilities (`claude-vm/src/utils/git.rs`)
- `run_git_command()`, `run_git_query()`
- `get_git_common_dir()`, `is_worktree()`
- `get_default_branch()`
- All git helper functions

### Worktree Commands (`claude-vm/src/commands/worktree/`)
- Command structure (adapt for backend support)
- User messaging patterns
- Error handling

### Test Patterns
- Integration test setup
- Git repository fixtures
- Command testing helpers

### DO NOT Extract
- VM-specific logic (Lima integration)
- Project detection tied to VM template naming
- Capability system (that's claude-vm specific)
- Config merging for VM contexts

## Configuration System

### Workspace Config (`.agentree.toml` in repo)
```toml
[workspace]
# Where to create worktrees (relative to main repo)
root_dir = "../worktrees"
# Path template: {repo}, {branch}, {timestamp}
template = "{repo}-{branch}"

[backend]
# Default backend: claude, claude-vm, opencode
default = "claude-vm"
# Auto-start backend when creating workspace
auto_start = true

[backend.claude]
# Path to claude binary
binary = "claude"
# Additional args to pass
args = []

[backend.claude-vm]
binary = "claude-vm"
# VM-specific settings passed through
vm_config = { memory = "8GB", disk = "20GB" }

[backend.opencode]
binary = "opencode"
```

### Global Config (`~/.agentree/config.toml`)
```toml
[defaults]
backend = "claude-vm"
workspace_root = "~/workspaces"

[backends]
# Override backend binary paths
claude = "/usr/local/bin/claude"
claude-vm = "/usr/local/bin/claude-vm"
```

## CI and Release Process

**Copy from claude-vm**: Reference `claude-vm/.github/workflows/`

### Required Workflows
1. **CI** (`ci.yml`)
   - Cargo test on Linux, macOS
   - Cargo clippy
   - Cargo fmt check
   - Integration tests

2. **Release** (`release.yml`)
   - Triggered on version tags (v*)
   - Build binaries for:
     - x86_64-apple-darwin (macOS Intel)
     - aarch64-apple-darwin (macOS ARM)
     - x86_64-unknown-linux-gnu (Linux)
   - Create GitHub release with binaries
   - Generate changelog from commits

3. **Homebrew** (post-release)
   - Auto-update tap formula
   - Test installation

### Version Management
- Semantic versioning (MAJOR.MINOR.PATCH)
- CHANGELOG.md updated with each release
- Version in Cargo.toml is source of truth

## Documentation Requirements

### User-Facing Docs
1. **README.md**
   - Quick start guide
   - Installation instructions
   - CLI command reference
   - Backend comparison table
   - Common workflows

2. **docs/architecture.md**
   - System design
   - Backend trait specification
   - Extension guide for new backends

3. **docs/backends/**
   - `claude.md` - Using local Claude backend
   - `claude-vm.md` - VM isolation setup
   - `opencode.md` - OpenCode integration
   - `custom.md` - Building custom backends

4. **docs/troubleshooting.md**
   - Common issues
   - Backend-specific problems
   - Recovery procedures

### Developer Docs
1. **CONTRIBUTING.md**
   - Development setup
   - Testing guidelines
   - PR process

2. **docs/development/**
   - `testing.md` - Test strategy and helpers
   - `release.md` - Release checklist

## Test Infrastructure

### Test Helpers (from claude-vm patterns)
```rust
// tests/helpers/mod.rs
pub struct TestRepo {
    temp_dir: TempDir,
    repo_path: PathBuf,
}

impl TestRepo {
    pub fn new() -> Self { /* ... */ }
    pub fn create_branch(&self, name: &str) { /* ... */ }
    pub fn commit(&self, message: &str) { /* ... */ }
}

pub struct MockBackend {
    // Implement Backend trait for testing
}
```

### Integration Tests
- Workspace create/list/remove
- Backend switching
- Config precedence
- Error scenarios (no git, invalid branch, etc.)
- Recovery (orphaned worktrees)

### Unit Tests
- Path templating
- Git utilities
- Config parsing
- Backend trait implementations

## Development Setup

### Prerequisites
- Rust 1.70+
- Git 2.15+ (worktree support)
- (Optional) claude-vm for testing VM backend

### Quick Start
```bash
# Clone with claude-vm reference
git clone https://github.com/user/agentree
cd agentree

# Reference claude-vm for extraction (mounted at ../claude-vm)

# Build and test
cargo build
cargo test

# Run locally
cargo run -- --help
```

## Migration Strategy

### Phase 1: Extract Foundation (Week 1)
- Set up Rust project structure
- Extract git utilities from claude-vm
- Extract workspace operations
- Create backend trait definition
- Implement `local` backend (simplest)

### Phase 2: Backend Implementations (Week 2)
- Implement `claude-vm` backend (delegates to CLI)
- Implement `claude` backend
- Stub `opencode` backend (TODO)
- Config system for backend selection

### Phase 3: Polish (Week 3)
- CI/CD workflows (from claude-vm)
- Documentation (README, architecture, backend guides)
- Integration tests
- Error handling and user messaging

### Phase 4: Release (Week 4)
- Version 0.1.0 release
- Homebrew formula
- Announce to users

## Key Decisions

### 1. Backend as CLI Caller vs Library
**Decision**: Backends call external CLIs (claude-vm, opencode) rather than linking as libraries.
**Rationale**:
- Keeps agentree lightweight
- No version coupling between tools
- Users install backends they need
- Easier to add new backends

### 2. Workspace Path Strategy
**Decision**: Worktrees created in configurable directory, default `../worktrees/{repo}-{branch}`.
**Rationale**:
- Keeps worktrees separate from main repo
- Easy to identify and clean up
- Matches proven claude-vm pattern

### 3. Session/Context Management
**Decision**: Agentree manages workspace creation/deletion, backends manage isolation lifecycle.
**Rationale**:
- Clear separation of concerns
- Backends don't need to know about git worktrees
- Agentree doesn't need to know about VMs/containers

### 4. Config Precedence
**Decision**: CLI args > Workspace .agentree.toml > Global ~/.agentree/config.toml > Defaults
**Rationale**:
- Standard precedence pattern
- Per-project customization
- User-level defaults
- Sensible fallbacks

## Implementation Notes

### Error Handling
- Use `thiserror` for error types (like claude-vm)
- Provide actionable error messages
- Include recovery suggestions

### User Experience
- Clear progress indicators for long operations
- Confirm destructive operations (delete)
- Show backend status in `list` command
- Auto-detect and suggest backends

### Testing
- Integration tests use real git repositories (temp dirs)
- Mock backends for testing workspace logic
- Test all error paths
- Test config precedence

## Reference: claude-vm Repo

When implementing, reference these claude-vm files:
- `.github/workflows/ci.yml` - CI setup
- `.github/workflows/release.yml` - Release process
- `src/worktree/` - Core worktree logic to extract
- `src/utils/git.rs` - Git utilities to extract
- `tests/` - Test patterns to follow
- `Cargo.toml` - Dependency versions

## Next Steps

When starting work:
1. ✅ Set up Rust project (`cargo init`)
2. ✅ Copy proven code from claude-vm
3. ✅ Define backend trait
4. ✅ Implement first backend (local)
5. ✅ CLI structure with clap
6. ✅ Config system
7. ✅ Tests
8. ✅ CI/CD
9. ✅ Documentation

---

**Working Directory**: `/Users/julien.muetton/Projects/themouette/agentree`
**Reference**: `/Users/julien.muetton/Projects/themouette/claude-vm` (mounted)
