# Development Scripts

This directory contains scripts for development and release automation.

## Scripts

### `setup`

Development environment setup script that installs all required dependencies.

**Usage:**
```bash
./bin/setup
```

**What it does:**
- Detects OS (macOS or Linux)
- Installs Homebrew (macOS only)
- Installs Rust toolchain
- Installs development dependencies (git, jq, build tools)
- Installs Rust development tools (clippy, rustfmt, cargo-watch)
- Builds the project
- Runs tests to verify setup

**Requirements:**
- Internet connection for downloading dependencies
- macOS or Debian/Ubuntu Linux
- sudo access may be required for system packages

### `release`

Automated release script that handles version bumping, testing, and GitHub release creation.

**Usage:**
```bash
# Interactive mode (prompts for version)
./bin/release

# Bump patch version (0.1.0 -> 0.1.1)
./bin/release patch

# Bump minor version (0.1.0 -> 0.2.0)
./bin/release minor

# Bump major version (0.1.0 -> 1.0.0)
./bin/release major

# Set specific version
./bin/release 0.2.0

# Show help
./bin/release --help
```

**What it does:**
1. Validates version format
2. Checks release-critical files are clean
3. Verifies you're on main branch
4. Runs all tests (`cargo test --all`)
5. Runs clippy (`cargo clippy -- -D warnings`)
6. Updates version in Cargo.toml and Cargo.lock
7. Updates CHANGELOG.md with new version
8. Creates git commit and annotated tag
9. Pushes to remote (triggers GitHub Actions release)

**Requirements:**
- No uncommitted changes to Cargo.toml, Cargo.lock, or CHANGELOG.md
- All tests passing
- No clippy warnings
- Git remote configured

**GitHub Integration:**
When the tag is pushed, GitHub Actions automatically:
- Builds binaries for all platforms (macOS x86_64/ARM64, Linux x86_64/ARM64)
- Creates a GitHub release
- Uploads binaries as release assets

## Development Workflow

### Initial Setup

```bash
# Clone repository
git clone https://github.com/themouette/agentree
cd agentree

# Run setup script
./bin/setup
```

### Daily Development

```bash
# Run tests
cargo test

# Run linter
cargo clippy

# Format code
cargo fmt

# Build release binary
cargo build --release

# Install locally for testing
cargo install --path .
```

### Creating a Release

```bash
# Ensure working tree is clean
git status

# Bump version and create release
./bin/release patch  # or minor/major

# Script will:
# 1. Run tests and clippy
# 2. Update version and CHANGELOG
# 3. Create git tag
# 4. Push to remote
# 5. Trigger GitHub Actions build
```

### Manual Release (if needed)

If the release script fails or you need to release manually:

```bash
# Update version in Cargo.toml
vim Cargo.toml

# Update CHANGELOG.md
vim CHANGELOG.md

# Build to update Cargo.lock
cargo build --release

# Commit changes
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "Release version 0.1.0"

# Create tag
git tag -a v0.1.0 -m "Release version 0.1.0"

# Push
git push origin main
git push origin v0.1.0
```

## Troubleshooting

### Setup Script Issues

**Rust installation fails:**
- Check internet connection
- Ensure curl is installed: `brew install curl` (macOS) or `sudo apt-get install curl` (Linux)
- Try manual installation: https://rustup.rs

**Build fails:**
- Check Rust version: `rustc --version` (should be 1.70+)
- Update Rust: `rustup update`
- Clean build: `cargo clean && cargo build`

### Release Script Issues

**Tests fail:**
```bash
cargo test --all --verbose
```
Fix failing tests before releasing.

**Clippy warnings:**
```bash
cargo clippy --all-targets --all-features -- -D warnings
```
Fix all warnings before releasing.

**Version already exists:**
Check existing tags: `git tag -l`
Delete tag if needed: `git tag -d v0.1.0 && git push origin :v0.1.0`

**Working tree not clean:**
```bash
git status
git add <files>  # or git stash
```

### GitHub Actions Issues

**Build fails:**
- Check workflow logs: https://github.com/themouette/agentree/actions
- Test build locally: `cargo build --release --target x86_64-unknown-linux-gnu`
- Check Cargo.lock is committed

**Release not created:**
- Verify tag was pushed: `git ls-remote --tags origin`
- Check GitHub Actions workflow status
- Verify GITHUB_TOKEN permissions in repository settings

## Additional Resources

- [Rust Installation](https://rustup.rs)
- [Cargo Book](https://doc.rust-lang.org/cargo/)
- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Semantic Versioning](https://semver.org)
- [Keep a Changelog](https://keepachangelog.com)
