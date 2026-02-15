# Contributing to Agentree

Thank you for your interest in contributing to agentree! We welcome contributions of all kinds.

## Quick Links

- **[Development Guide](docs/development.md)** - Complete guide to architecture, setup, and development workflow
- **[Configuration Guide](docs/configuration.md)** - Understanding the configuration system
- **[Troubleshooting](docs/troubleshooting.md)** - Common issues and solutions

## Ways to Contribute

### 🐛 Report Bugs

Found a bug? [Open an issue](https://github.com/themouette/agentree/issues/new) with:
- Clear description of the problem
- Steps to reproduce
- Expected vs actual behavior
- System info (OS, agentree version, git version)
- Relevant config files (sanitize sensitive data)

### 💡 Suggest Features

Have an idea? [Start a discussion](https://github.com/themouette/agentree/discussions) or open an issue with:
- Use case description
- How it would work
- Why it would benefit users

### 📖 Improve Documentation

Documentation improvements are always welcome:
- Fix typos or unclear explanations
- Add examples
- Write tutorials
- Improve error messages

### 🔧 Submit Code

See our [Development Guide](docs/development.md) for detailed instructions.

## Quick Start for Contributors

```bash
# 1. Fork and clone
git clone https://github.com/YOUR-USERNAME/agentree
cd agentree

# 2. Create feature branch
git checkout -b feature/your-feature

# 3. Make changes
vim src/...

# 4. Test
cargo test
cargo clippy -- -D warnings
cargo fmt

# 5. Commit
git commit -am "feat: add your feature"

# 6. Push and create PR
git push origin feature/your-feature
```

## Development Setup

### Prerequisites

- **Rust 1.70+** - [Install via rustup](https://rustup.rs/)
- **Git 2.15+** - Required for worktree support
- (Optional) **claude-vm** - For testing VM backend

### Build and Test

```bash
# Build
cargo build

# Run tests
cargo test

# Run locally
cargo run -- --help

# Install for local testing
cargo install --path .
```

## Coding Guidelines

### Code Style

- **Format code**: Run `cargo fmt` before committing
- **No warnings**: Code must pass `cargo clippy -- -D warnings`
- **Write tests**: Add unit tests for functions, integration tests for commands
- **Document**: Add doc comments for public APIs

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add docker backend support
fix: resolve path traversal vulnerability
docs: update configuration guide
test: add integration tests for agent command
refactor: simplify backend trait
perf: optimize git operations
chore: update dependencies
```

**Types**:
- `feat` - New feature
- `fix` - Bug fix
- `docs` - Documentation only
- `test` - Adding or updating tests
- `refactor` - Code change that neither fixes a bug nor adds a feature
- `perf` - Performance improvement
- `chore` - Maintenance (dependencies, build, etc.)

### Pull Request Guidelines

**Before submitting**:
- [ ] Tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy -- -D warnings`)
- [ ] Code formatted (`cargo fmt`)
- [ ] Documentation updated (if adding features)
- [ ] CHANGELOG.md updated (for user-facing changes)

**PR Description Template**:
```markdown
## Summary
Brief description of what this PR does.

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

## Related Issues
Closes #123
Related to #456
```

## Architecture Overview

See the [Development Guide](docs/development.md) for detailed architecture documentation.

**Key principles**:
1. **Separation of Concerns**: Workspace management, backend abstraction, and configuration are separate
2. **Backend Independence**: Backends are external binaries, not linked libraries
3. **User-Friendly Errors**: Rich error types with helpful messages and recovery hints
4. **Testability**: Comprehensive unit and integration tests

## Adding a Backend

See [Development Guide - Adding a Backend](docs/development.md#adding-a-backend) for step-by-step instructions.

**Summary**:
1. Create `src/backend/your_backend.rs`
2. Implement `Backend` trait
3. Register in `src/backend/mod.rs`
4. Add to registry in `src/backend/registry.rs`
5. Write tests
6. Document in `docs/backends/your_backend.md`

## Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_backend_creation

# Run with output
cargo test -- --nocapture

# Run integration tests only
cargo test --test integration_tests
```

### Manual Testing Checklist

Before submitting PR, manually test:
- [ ] `agentree create <branch>` - Creates worktree
- [ ] `agentree list` - Shows worktrees
- [ ] `agentree shell <branch>` - Opens shell
- [ ] `agentree remove <branch>` - Removes worktree
- [ ] Backend-specific features (if adding/modifying backend)

## Documentation

When adding features, update:
- **README.md** - If adding user-facing features
- **docs/configuration.md** - If adding config options
- **docs/development.md** - If changing architecture
- **docs/troubleshooting.md** - If addressing new error cases
- **CHANGELOG.md** - For all user-visible changes

## Release Process

Releases are handled by maintainers. If you're a maintainer:

```bash
# Run release script
./bin/release patch  # or minor, major, or version number

# This will:
# 1. Run tests
# 2. Update Cargo.lock
# 3. Create git tag
# 4. Push to GitHub
# 5. Trigger automated release build
```

See [Development Guide - Release Process](docs/development.md#release-process) for details.

## Code of Conduct

### Our Pledge

We are committed to providing a welcoming and inclusive environment for all contributors.

### Expected Behavior

- Be respectful and considerate
- Welcome newcomers and help them get started
- Provide constructive feedback
- Focus on what is best for the project and community

### Unacceptable Behavior

- Harassment, discrimination, or personal attacks
- Trolling or inflammatory comments
- Publishing others' private information
- Other conduct which could reasonably be considered inappropriate

### Enforcement

Instances of unacceptable behavior may be reported to the project maintainers. All complaints will be reviewed and investigated promptly and fairly.

## Questions?

- **General questions**: [Discussions](https://github.com/themouette/agentree/discussions)
- **Bug reports**: [Issues](https://github.com/themouette/agentree/issues)
- **Feature requests**: [Discussions](https://github.com/themouette/agentree/discussions)

## License

By contributing to agentree, you agree that your contributions will be licensed under the MIT License.

---

Thank you for contributing to agentree! 🎉
