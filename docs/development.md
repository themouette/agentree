# Development Guide

Guide for contributing to agentree development.

## Table of Contents

- [Getting Started](#getting-started)
- [Architecture Overview](#architecture-overview)
- [Development Workflow](#development-workflow)
- [Adding a Backend](#adding-a-backend)
- [Testing](#testing)
- [Release Process](#release-process)

## Getting Started

### Prerequisites

- **Rust 1.70+** - [Install via rustup](https://rustup.rs/)
- **Git 2.15+** - Required for worktree support
- (Optional) **claude-vm** - For testing VM backend

### Setup

```bash
# Clone repository
git clone https://github.com/themouette/agentree
cd agentree

# Build
cargo build

# Run tests
cargo test

# Run locally
cargo run -- --help

# Install locally for testing
cargo install --path .
```

### Project Structure

```
agentree/
├── src/
│   ├── main.rs              # CLI entry point, command routing
│   ├── lib.rs               # Library exports
│   ├── error.rs             # Error types
│   ├── version.rs           # Version info and platform detection
│   ├── backend/             # Backend abstraction
│   │   ├── mod.rs           # Backend trait and dispatcher
│   │   ├── local.rs         # Local backend (no isolation)
│   │   ├── claude_vm.rs     # VM backend (delegates to claude-vm)
│   │   ├── exec.rs          # Command execution helpers
│   │   └── registry.rs      # Backend validation
│   ├── config/              # Configuration system
│   │   ├── mod.rs           # Config types and merging
│   │   ├── loader.rs        # File discovery and loading
│   │   └── validation.rs    # Config validation
│   ├── commands/            # CLI commands
│   │   ├── mod.rs
│   │   ├── create.rs        # Create worktree
│   │   ├── list.rs          # List worktrees
│   │   ├── remove.rs        # Remove worktrees
│   │   ├── shell.rs         # Open shell
│   │   ├── exec.rs          # Execute command
│   │   ├── agent.rs         # Start AI agent
│   │   ├── cd.rs            # Output cd command
│   │   ├── clean.rs         # Clean orphaned worktrees
│   │   ├── shell_init.rs    # Shell integration setup
│   │   └── update.rs        # Self-update
│   ├── worktree/            # Worktree management
│   │   ├── mod.rs
│   │   ├── config.rs        # Worktree configuration
│   │   ├── operations.rs    # Create/list/remove operations
│   │   ├── metadata.rs      # Metadata tracking
│   │   ├── state.rs         # Worktree state queries
│   │   ├── template.rs      # Path templating
│   │   ├── validation.rs    # Branch/path validation
│   │   └── recovery.rs      # Orphaned worktree cleanup
│   └── utils/               # Utilities
│       ├── mod.rs
│       └── git.rs           # Git command wrappers
├── tests/                   # Integration tests
│   ├── helpers/             # Test helpers
│   └── integration_tests.rs
├── bin/                     # Development scripts
│   ├── setup                # Development setup
│   └── release              # Release automation
├── docs/                    # Documentation
│   ├── configuration.md
│   ├── development.md       # This file
│   └── troubleshooting.md
└── .github/
    └── workflows/           # CI/CD
        ├── test.yml         # Test workflow
        └── release.yml      # Release workflow
```

## Architecture Overview

### High-Level Design

```
┌─────────────────────────────────────────────────┐
│              agentree CLI                       │
│  (workspace management + session coordination)  │
└────────────┬────────────────────────────────────┘
             │
             │ Config System (hierarchical)
             │
     ┌───────┴────────┐
     │                │
┌────▼─────┐    ┌────▼──────┐
│ Worktree │    │  Backend  │
│ Manager  │    │ Abstraction│
└────┬─────┘    └────┬──────┘
     │               │
     │ Git Worktrees │ Backend Trait
     │               │
     │        ┌──────┴───────┬──────────┐
     │        │              │          │
     │   ┌────▼─────┐  ┌────▼──────┐  ...
     │   │  Local   │  │ Claude-VM │
     │   │ Backend  │  │  Backend  │
     │   └──────────┘  └───────────┘
```

### Key Concepts

#### 1. **Separation of Concerns**

- **Worktree Management**: Git operations, path management, metadata
- **Backend Abstraction**: Isolation strategy (local, VM, container)
- **Configuration**: Hierarchical config with precedence rules

This separation allows:
- Users to choose isolation strategy per project/workspace
- Backends to be external binaries (no version coupling)
- Easy addition of new backends without changing core code

#### 2. **Backend Trait**

All backends implement this interface:

```rust
pub trait Backend {
    /// Open interactive shell in workspace
    fn shell(&self, workspace_path: &Path) -> Result<()>;

    /// Execute command and capture output
    fn exec(&self, workspace_path: &Path, command: &[String]) -> Result<ExecOutput>;

    /// Start AI agent session
    fn agent(&self, workspace_path: &Path, agent: &str, flags: &[String]) -> Result<()>;

    /// Get backend name
    fn name(&self) -> &str;
}
```

**Design Decision**: Backends call external CLIs (e.g., `claude-vm`) rather than linking as libraries.

**Why**:
- No version coupling between agentree and backends
- Users install only backends they need
- Backends can be written in any language
- Easier to maintain and test

#### 3. **Configuration Hierarchy**

```
CLI args > Project config > Global config > Built-in defaults
```

**Implementation**: `Config::merge()` performs key-by-key merging:
- If key exists in higher-precedence config, use it
- Otherwise fall back to lower-precedence config
- Built-in defaults always exist

See `src/config/mod.rs` for implementation.

#### 4. **Auto-Create Pattern**

Commands like `shell`, `exec`, `agent` automatically create workspaces if they don't exist:

```rust
// 1. Try to find existing workspace
let workspace = find_workspace(branch)?;

if workspace.is_none() {
    // 2. Create workspace from start-ref
    create_workspace(branch, start_ref)?;
    println!("Created workspace for {}", branch);
}

// 3. Proceed with operation
backend.shell(workspace_path)?;
```

**Benefit**: "Just Works" UX - users don't need to remember to create workspaces.

#### 5. **Metadata Tracking**

Each worktree stores metadata in its gitdir:

```json
{
  "backend": "claude-vm",
  "created_at": "2024-01-15T10:30:00Z",
  "version": "0.1.0"
}
```

**Location**: `.git/agentree-meta.json` (in worktree's gitdir, not working directory)

**Why gitdir**:
- Survives `.gitignore`
- Tied to worktree lifecycle (deleted when worktree removed)
- Not visible in working directory

## Development Workflow

### Making Changes

```bash
# 1. Create feature branch
git checkout -b feature/my-feature

# 2. Make changes
vim src/...

# 3. Run tests
cargo test

# 4. Check formatting
cargo fmt --check

# 5. Run clippy
cargo clippy -- -D warnings

# 6. Test locally
cargo run -- create test-branch
cargo run -- list

# 7. Commit
git commit -am "feat: add my feature"

# 8. Push and create PR
git push origin feature/my-feature
```

### Code Style

- **Follow Rust conventions**: Use `cargo fmt`
- **No warnings**: Code must pass `cargo clippy -- -D warnings`
- **Error handling**: Use `Result<T>` with rich error types (see `src/error.rs`)
- **Testing**: Add unit tests for new functions, integration tests for new commands
- **Documentation**: Document public APIs and complex logic

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add docker backend support
fix: resolve path traversal vulnerability
docs: update configuration guide
test: add integration tests for agent command
refactor: simplify backend trait
```

**Types**: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `chore`

## Adding a Backend

### 1. Create Backend Module

Create `src/backend/my_backend.rs`:

```rust
use crate::backend::exec::{run_interactive, ExecOutput};
use crate::backend::Backend;
use crate::error::Result;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct MyBackend {
    binary: String,
}

impl MyBackend {
    pub fn new() -> Self {
        Self {
            binary: "my-backend-cli".to_string(),
        }
    }
}

impl Backend for MyBackend {
    fn shell(&self, workspace_path: &Path) -> Result<()> {
        // Delegate to backend CLI
        run_interactive(&self.binary, &["shell"], workspace_path)
    }

    fn exec(&self, workspace_path: &Path, command: &[String]) -> Result<ExecOutput> {
        // If backend doesn't isolate exec, run on host
        crate::backend::exec::run_host_command(workspace_path, command, self.name())
    }

    fn agent(&self, workspace_path: &Path, agent: &str, flags: &[String]) -> Result<()> {
        let mut args = vec!["agent"];

        if !agent.is_empty() {
            args.push("--agent");
            args.push(agent);
        }

        // Append user flags
        let flag_refs: Vec<&str> = flags.iter().map(|s| s.as_str()).collect();
        args.extend(flag_refs);

        run_interactive(&self.binary, &args, workspace_path)
    }

    fn name(&self) -> &str {
        "my-backend"
    }
}
```

### 2. Register Backend

Update `src/backend/mod.rs`:

```rust
// Add module
mod my_backend;
pub use my_backend::MyBackend;

// Add to enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendKind {
    Local,
    ClaudeVm,
    MyBackend,  // Add here
}

// Add to Display
impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendKind::Local => write!(f, "local"),
            BackendKind::ClaudeVm => write!(f, "claude-vm"),
            BackendKind::MyBackend => write!(f, "my-backend"),  // Add here
        }
    }
}

// Add to FromStr
impl FromStr for BackendKind {
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "local" => Ok(BackendKind::Local),
            "claude-vm" | "claudevm" => Ok(BackendKind::ClaudeVm),
            "my-backend" => Ok(BackendKind::MyBackend),  // Add here
            _ => Err(/* ... */),
        }
    }
}

// Add to BackendType
pub enum BackendType {
    Local(LocalBackend),
    ClaudeVm(ClaudeVmBackend),
    MyBackend(MyBackend),  // Add here
}

impl BackendType {
    pub fn from_kind(kind: BackendKind) -> Self {
        match kind {
            BackendKind::Local => Self::local(),
            BackendKind::ClaudeVm => Self::claude_vm(),
            BackendKind::MyBackend => Self::MyBackend(MyBackend::new()),  // Add here
        }
    }
}

// Add to Backend trait impl
impl Backend for BackendType {
    fn shell(&self, workspace_path: &Path) -> Result<()> {
        match self {
            BackendType::Local(b) => b.shell(workspace_path),
            BackendType::ClaudeVm(b) => b.shell(workspace_path),
            BackendType::MyBackend(b) => b.shell(workspace_path),  // Add here
        }
    }
    // ... repeat for exec, agent, name
}
```

### 3. Register in Backend Registry

Update `src/backend/registry.rs`:

```rust
impl BackendRegistry {
    pub fn new() -> Self {
        let mut backends = HashMap::new();

        // ... existing backends ...

        // Add your backend
        backends.insert(
            BackendKind::MyBackend,
            BackendInfo {
                binary_name: "my-backend-cli".to_string(),
                min_version: None,  // Or Some(semver::VersionReq::parse(">=1.0.0").unwrap())
                install_instructions: "npm install -g my-backend-cli".to_string(),
                update_instructions: "npm update -g my-backend-cli".to_string(),
            },
        );

        Self { backends }
    }
}
```

### 4. Add Tests

Add to `src/backend/my_backend.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_name() {
        let backend = MyBackend::new();
        assert_eq!(backend.name(), "my-backend");
    }

    #[test]
    fn test_backend_creation() {
        let backend = MyBackend::new();
        assert_eq!(backend.binary, "my-backend-cli");
    }
}
```

### 5. Document Backend

Create `docs/backends/my-backend.md`:

```markdown
# My Backend

Description of your backend and what isolation it provides.

## Installation

\`\`\`bash
npm install -g my-backend-cli
\`\`\`

## Configuration

\`\`\`toml
[backend]
default = "my-backend"
\`\`\`

## Usage

\`\`\`bash
agentree create feature --backend my-backend
\`\`\`
```

### 6. Update Documentation

- Add backend to `README.md` backends table
- Add to `docs/configuration.md` backend options

## Testing

### Unit Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_backend_creation

# Run tests for a module
cargo test backend::

# Show test output
cargo test -- --nocapture
```

### Integration Tests

Located in `tests/integration_tests.rs`:

```rust
#[test]
fn test_my_feature() {
    let repo = TestRepo::new();

    // Test setup
    repo.create_branch("feature");

    // Run command
    let output = run_command(&["create", "feature"]);

    // Assertions
    assert!(output.success());
    assert!(repo.worktree_exists("feature"));
}
```

### Manual Testing

```bash
# Build and install locally
cargo install --path .

# Test commands
agentree create test-branch
agentree list
agentree shell test-branch
agentree remove test-branch
```

### Testing with Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Run with coverage
cargo tarpaulin --out Html
```

## Release Process

### Automated Release

Releases are automated via GitHub Actions:

```bash
# 1. Update version in Cargo.toml
vim Cargo.toml  # Change version = "0.2.0"

# 2. Update CHANGELOG.md
vim CHANGELOG.md  # Add release notes

# 3. Run release script
./bin/release patch  # or minor, major, or specific version

# This will:
# - Run tests
# - Update Cargo.lock
# - Create git tag
# - Push to GitHub
# - Trigger GitHub Actions to build and release
```

### GitHub Actions Workflow

When a tag is pushed (e.g., `v0.2.0`):

1. **Build**: Compiles binaries for:
   - macOS (x86_64, aarch64)
   - Linux (x86_64, aarch64)

2. **Release**: Creates GitHub release with:
   - Changelog from CHANGELOG.md
   - Binary attachments
   - Checksums

3. **Homebrew**: Auto-updates tap formula (if configured)

### Manual Release (Emergency)

```bash
# Create tag
git tag -a v0.2.0 -m "Release v0.2.0"

# Push tag
git push origin v0.2.0

# GitHub Actions will handle the rest
```

## Architecture Decisions

### Why External Backend Binaries?

**Decision**: Backends are external CLIs, not linked libraries.

**Rationale**:
- No version coupling (agentree 0.1.0 works with claude-vm 0.5.0 or 1.0.0)
- Lighter: Users only install backends they need
- Language-agnostic: Backends can be written in any language
- Simpler testing: Mock backends by swapping binary

### Why Git Worktrees?

**Decision**: Use git worktrees instead of multiple clones.

**Rationale**:
- Efficient: Shared object database saves disk space
- Correct: All worktrees share same remotes/branches
- Native: Worktrees are a native git feature (2.5+)

### Why Hierarchical Config?

**Decision**: CLI > Project > Global > Defaults

**Rationale**:
- Flexibility: Override per-command without changing files
- Team Collaboration: Project config for team standards
- Personal Preferences: Global config for individual choices
- Sensible Defaults: Works out-of-the-box

## Contributing Guidelines

### Before Submitting PR

- [ ] Tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy -- -D warnings`)
- [ ] Code formatted (`cargo fmt`)
- [ ] Documentation updated (if adding features)
- [ ] CHANGELOG.md updated (for user-facing changes)

### PR Template

```markdown
## Summary
Brief description of changes.

## Changes
- Added X feature
- Fixed Y bug
- Refactored Z

## Testing
- [ ] Unit tests added/updated
- [ ] Integration tests added/updated
- [ ] Manual testing performed

## Documentation
- [ ] README updated (if needed)
- [ ] docs/ updated (if needed)
- [ ] Code comments added for complex logic
```

## Getting Help

- **Issues**: [github.com/themouette/agentree/issues](https://github.com/themouette/agentree/issues)
- **Discussions**: [github.com/themouette/agentree/discussions](https://github.com/themouette/agentree/discussions)
- **Discord**: Coming soon

## See Also

- [Configuration Guide](configuration.md) - All config options
- [Troubleshooting](troubleshooting.md) - Common issues
- [README](../README.md) - Quick start guide
