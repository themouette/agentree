# Infrastructure Setup Complete

## Overview

CI/CD and development infrastructure is now in place, following claude-vm patterns.

## What's Been Set Up

### 1. GitHub Actions CI/CD

#### Test Workflow (`.github/workflows/test.yml`)
Runs on every push to main and on pull requests:

- **Matrix testing**: Ubuntu + macOS, Rust stable
- **Cargo caching**: Registry, index, and build artifacts
- **Quality checks**:
  - `cargo test --all` - Unit and integration tests
  - `cargo clippy` - Linting with warnings as errors
  - `cargo fmt --check` - Code formatting verification
- **Security audit**: `cargo-audit` for dependency vulnerabilities
- **Build verification**: Release builds on all platforms

Integration tests only run on main branch to save CI time.

#### Release Workflow (`.github/workflows/release.yml`)
Triggered on version tags (v*.*.*):

- **Multi-platform builds**:
  - macOS Intel (x86_64-apple-darwin)
  - macOS Apple Silicon (aarch64-apple-darwin)
  - Linux x86_64 (x86_64-unknown-linux-gnu)
  - Linux ARM64 (aarch64-unknown-linux-gnu)
- **Automated release creation**:
  - Creates GitHub release
  - Uploads compressed binaries as assets
  - Includes installation instructions in release body
- **Binary optimization**: Stripped for smaller size

### 2. Installation Script (`install.sh`)

Curlable install script for easy distribution:

```bash
# Install latest version
curl -fsSL https://raw.githubusercontent.com/themouette/agentree/main/install.sh | bash

# Install specific version
curl -fsSL https://raw.githubusercontent.com/themouette/agentree/main/install.sh | bash -s -- --version v0.1.0

# Install to system directory
curl -fsSL https://raw.githubusercontent.com/themouette/agentree/main/install.sh | bash -s -- --global
```

Features:
- Platform detection (macOS/Linux, x86_64/ARM64)
- Automatic latest version resolution
- Custom installation directory support
- PATH verification and guidance
- No sudo required for default installation (~/.local/bin)

### 3. Development Scripts (`bin/`)

#### `bin/setup`
One-command development environment setup:

```bash
./bin/setup
```

Installs:
- Rust toolchain via rustup
- Development dependencies (git, jq, build-essential)
- Rust tools (clippy, rustfmt, cargo-watch)
- Builds project and runs tests

Supports: macOS and Debian/Ubuntu Linux

#### `bin/release`
Automated release management:

```bash
./bin/release patch   # 0.1.0 -> 0.1.1
./bin/release minor   # 0.1.0 -> 0.2.0
./bin/release major   # 0.1.0 -> 1.0.0
./bin/release 0.2.0   # Specific version
```

Automated workflow:
1. Version validation
2. Check working tree is clean
3. Run all tests
4. Run clippy with strict warnings
5. Update Cargo.toml and Cargo.lock
6. Update CHANGELOG.md with new version
7. Create git commit and annotated tag
8. Push to remote (triggers GitHub Actions)

Safety features:
- Confirms before releasing
- Validates semantic versioning
- Ensures new version > current version
- Only modifies release-critical files
- Branch verification (warns if not on main)

### 4. Integration Tests (`tests/`)

Test infrastructure with helper utilities:

**Test Helper** (`tests/helpers/mod.rs`):
- `TestRepo` - Temporary git repository for testing
- Git command helpers
- Agentree binary execution
- Automatic cleanup

**Integration Tests** (`tests/integration_tests.rs`):
- Create and list worktrees
- Remove worktrees
- Existing branch handling
- Invalid branch name validation
- Empty repository handling

Run with:
```bash
cargo test --test integration_tests
```

### 5. CHANGELOG.md

Standard changelog following [Keep a Changelog](https://keepachangelog.com) format:
- Semantic versioning links
- Version comparison links
- Unreleased section for ongoing work

Updated automatically by `bin/release` script.

## Release Process

### Automated Release (Recommended)

```bash
# 1. Ensure working tree is clean
git status

# 2. Run release script
./bin/release patch  # or minor/major

# 3. Script handles everything:
#    - Tests
#    - Version bumping
#    - CHANGELOG updates
#    - Git tag creation
#    - Push to remote

# 4. GitHub Actions automatically:
#    - Builds all platform binaries
#    - Creates release
#    - Uploads assets
```

### Manual Release (If Needed)

```bash
# 1. Update version
vim Cargo.toml
cargo build --release  # Updates Cargo.lock

# 2. Update CHANGELOG
vim CHANGELOG.md

# 3. Commit and tag
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "Release version 0.1.0"
git tag -a v0.1.0 -m "Release version 0.1.0"

# 4. Push
git push origin main
git push origin v0.1.0
```

## Testing Strategy

### Unit Tests
```bash
cargo test --lib
```
- Worktree operations
- Path templating
- Configuration parsing
- Git utilities
- Validation logic

### Integration Tests
```bash
cargo test --test integration_tests
```
- End-to-end CLI workflows
- Real git repository interactions
- Error handling
- Multi-worktree scenarios

### CI Testing
- Runs on every push/PR
- Multi-platform verification
- Dependency security audit
- Code quality checks (clippy, fmt)

## Distribution Methods

### 1. Curl Install (Primary)
```bash
curl -fsSL https://raw.githubusercontent.com/themouette/agentree/main/install.sh | bash
```

### 2. GitHub Releases
Download pre-built binaries:
- https://github.com/themouette/agentree/releases

### 3. Cargo Install (For Developers)
```bash
cargo install --path .
```

### 4. Future: Homebrew (Planned)
```bash
brew install themouette/tap/agentree
```

## Monitoring & Maintenance

### GitHub Actions Status
- View at: https://github.com/themouette/agentree/actions
- Check badges in README
- Email notifications on failures

### Security Audits
- Runs on every push
- cargo-audit checks dependencies
- GitHub Dependabot alerts

### Release Checklist

Before each release:
- [ ] All tests passing locally
- [ ] No clippy warnings
- [ ] Code formatted (cargo fmt)
- [ ] CHANGELOG.md updated (done by script)
- [ ] Version bumped (done by script)
- [ ] No uncommitted changes to release files

## Key Differences from claude-vm

### Removed:
- VM integration tests (no Lima dependency)
- Capability-specific tests
- Update checking (will add later if needed)

### Kept:
- Core test/release workflows
- Multi-platform build matrix
- Release automation
- Install script patterns
- Development scripts

### Adapted:
- Binary name: claude-vm → agentree
- Repository URLs
- Integration test focus (worktrees instead of VMs)

## Next Steps

Now that infrastructure is in place, we can:

1. **Refine CLI UX** - Discuss command structure and options
2. **Add more tests** - Expand integration test coverage
3. **Documentation** - User guides and examples
4. **First release** - Cut v0.1.0 when ready
5. **Homebrew formula** - After stable release

## Files Created

```
.github/workflows/
├── test.yml              # CI testing workflow
└── release.yml           # Release automation

bin/
├── setup                 # Development setup script
├── release               # Release automation script
└── README.md             # Script documentation

tests/
├── integration_tests.rs  # Integration test suite
└── helpers/
    └── mod.rs            # Test utilities

install.sh                # User installation script
CHANGELOG.md              # Version history
INFRASTRUCTURE.md         # This document
```

## Documentation

- **bin/README.md** - Development script usage
- **CHANGELOG.md** - Version history
- **INFRASTRUCTURE.md** - This overview

All infrastructure follows proven claude-vm patterns adapted for agentree's use case.
