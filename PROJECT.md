# Agentree: Workspace Orchestration with Pluggable Isolation

## What We're Building

A CLI tool that manages git worktrees with pluggable isolation backends, enabling developers to work on multiple branches simultaneously with proper environment isolation.

## The Problem

Developers working on multiple features/branches face context-switching pain:
- Can't work on two branches simultaneously (worktrees help but lack isolation)
- Running Claude Code on different branches risks cross-contamination
- No standard way to launch isolated environments per workspace
- Each isolation tool (VMs, containers) requires different workflows

## The Solution

**Agentree** = Workspace management + Backend abstraction

```
User wants to:                    Agentree does:
─────────────────────────────     ──────────────────────────────────
Work on feature-A                 git worktree add + start backend
  └─> with isolation              └─> Delegates to claude-vm/docker/etc.

List all workspaces               Query git worktrees + backend status
Switch to feature-B               cd + activate backend
Remove old workspace              git worktree remove + stop backend
```

**Key insight**: Separate workspace orchestration from isolation implementation.

## Architecture

```
┌─────────────────────────────────────────────────┐
│           agentree (this repo)                  │
│   ┌─────────────────────────────────────┐      │
│   │  Workspace Management (git)         │      │
│   │  - Create/list/remove worktrees     │      │
│   │  - Path templating                  │      │
│   │  - Configuration                    │      │
│   └──────────┬──────────────────────────┘      │
│              │                                  │
│   ┌──────────▼──────────────────────────┐      │
│   │  Backend Abstraction (trait)        │      │
│   │  - start/stop/shell/exec/status     │      │
│   └──────────┬──────────────────────────┘      │
│              │                                  │
│   ┌──────────┴──────────┬─────────────┬──────┐ │
│   │                     │             │      │ │
│   ▼                     ▼             ▼      ▼ │
│ ┌─────────┐  ┌──────────────┐  ┌──────────┐  │ │
│ │ claude  │  │  claude-vm   │  │ opencode │  │ │
│ │ (local) │  │  (Lima VM)   │  │          │  │ │
│ └─────────┘  └──────────────┘  └──────────┘  │ │
└─────────────────────────────────────────────────┘

External Tools (user installs separately):
  - claude (Claude CLI)
  - claude-vm (VM isolation tool)
  - opencode (OpenCode CLI)
  - docker (future)
```

## Core Value Proposition

### For Users
- **One command** to create workspace with isolation: `agentree create feature-A`
- **Choose backend** based on needs: local (fast) vs VM (isolated)
- **Consistent interface** regardless of isolation mechanism
- **Easy switching** between workspaces

### For Backend Developers
- **Simple trait** to implement (7 methods)
- **No workspace logic** - just handle isolation
- **Users discover your tool** through agentree

### For Workspace Ecosystem
- **Standard interface** for isolation backends
- **Mix and match** - use different backends per project
- **Interoperability** - backends work with same workflow

## Success Criteria

### Milestone 1: Foundation (v0.1.0)
- ✅ Extract worktree logic from claude-vm
- ✅ Backend trait defined and documented
- ✅ `claude` backend (local, no isolation) works
- ✅ `claude-vm` backend works (calls claude-vm CLI)
- ✅ Config system (CLI → workspace → global)
- ✅ Basic commands: create, list, remove
- ✅ Tests pass, CI configured

### Milestone 2: Polish (v0.2.0)
- ✅ `opencode` backend implemented
- ✅ Backend auto-detection
- ✅ Comprehensive documentation
- ✅ Error handling and recovery
- ✅ Homebrew formula
- ✅ User guide and tutorials

### Milestone 3: Ecosystem (v1.0.0)
- ✅ Third-party backends emerging
- ✅ Docker backend (official)
- ✅ Session/context management features
- ✅ Shell integrations (completion, prompts)
- ✅ Proven in production use

## Technical Approach

### Phase 1: Extract from claude-vm
**Goal**: Get working workspace management without backends

**Tasks**:
1. Copy worktree operations (create/list/remove) → workspace module
2. Copy git utilities (run_git_command, etc.)
3. Simplify config (remove VM-specific parts)
4. Adapt CLI commands
5. Tests compile and pass

**Output**: `agentree list` shows git worktrees

### Phase 2: Backend Abstraction
**Goal**: Define and implement backend trait

**Tasks**:
1. Define Backend trait (start/stop/shell/exec/status)
2. Define context types (WorkspaceContext, BackendStatus)
3. Create backend registry (get_backend by name)
4. Implement MockBackend for testing

**Output**: Backend trait compiles, tests use mock

### Phase 3: Claude Backend
**Goal**: Simplest backend (local, no isolation)

**Tasks**:
1. Implement ClaudeBackend struct
2. Implement Backend trait methods (mostly no-ops or direct commands)
3. Integration test with real claude binary
4. Update create command to use backend

**Output**: `agentree create feature-A --backend claude` works

### Phase 4: Claude-VM Backend
**Goal**: Full VM isolation via claude-vm CLI

**Tasks**:
1. Implement ClaudeVmBackend struct
2. Delegate to claude-vm CLI (agent, stop, shell, exec)
3. Parse claude-vm info output for status
4. Handle errors (VM not running, claude-vm not installed)
5. Integration test with claude-vm

**Output**: `agentree create feature-A --backend claude-vm` starts VM

### Phase 5: OpenCode Backend
**Goal**: Third backend to validate abstraction

**Tasks**:
1. Implement OpenCodeBackend (similar to Claude)
2. Handle OpenCode-specific quirks
3. Documentation

**Output**: OpenCode users can use agentree

### Phase 6: Polish
**Goal**: Production-ready v0.1.0

**Tasks**:
1. Config precedence (CLI → workspace → global)
2. Backend auto-detection (suggest available backends)
3. Error messages and recovery
4. Documentation (README, architecture, backend guides)
5. CI/CD (copy from claude-vm)
6. Release automation

**Output**: v0.1.0 release on GitHub + Homebrew

## User Experience

### Creating a Workspace
```bash
# With explicit backend
$ agentree create feature-A --backend claude-vm
Creating workspace for branch 'feature-A'...
✓ Created worktree at /Users/me/worktrees/myproject-feature-A
✓ Starting claude-vm backend...
✓ VM ready

# Auto-detect backend
$ agentree create feature-B
? Which backend to use:
  > claude-vm (detected)
    claude (detected)
✓ Workspace created

# In config file (.agentree.toml)
[backend]
default = "claude-vm"

$ agentree create feature-C
✓ Using claude-vm backend (from config)
✓ Workspace created
```

### Listing Workspaces
```bash
$ agentree list
BRANCH         PATH                               BACKEND       STATUS
main           /Users/me/myproject               -             (main repo)
feature-A      ../worktrees/myproject-feature-A  claude-vm     Running
feature-B      ../worktrees/myproject-feature-B  claude        (local)
old-feature    ../worktrees/myproject-old        claude-vm     Stopped
```

### Working in Workspace
```bash
# Open shell
$ agentree shell feature-A
# Opens shell in workspace (via backend)

# Execute command
$ agentree exec feature-A -- cargo test
# Runs test in isolated environment

# Switch workspace (shorthand)
$ cd $(agentree path feature-B)
```

### Removing Workspace
```bash
# Remove specific workspace
$ agentree remove feature-A
Stopping claude-vm backend...
Removing worktree...
✓ Workspace removed

# Bulk remove merged branches
$ agentree remove --merged main
Found 3 merged branches:
  - old-feature-1
  - old-feature-2
  - hotfix-123
? Remove these workspaces? (y/N) y
✓ Removed 3 workspaces
```

## Technical Decisions

### 1. Backend as CLI Caller
**Decision**: Backends call external CLIs, not linked as libraries
**Why**: Independence, no version coupling, lightweight

### 2. Workspace Path Strategy
**Decision**: Worktrees in configurable dir, default `../worktrees/{repo}-{branch}`
**Why**: Clear separation, proven pattern from claude-vm

### 3. Backend Trait Design
**Decision**: 7 methods (start/stop/is_running/shell/exec/status/name)
**Why**: Minimal surface, covers all use cases, easy to implement

### 4. Config Precedence
**Decision**: CLI > workspace > global > defaults
**Why**: Standard pattern, maximum flexibility

### 5. Session Management
**Decision**: Deferred to v0.2.0+
**Why**: Get core working first, session state is complex

## Dependencies

**From claude-vm** (existing):
- clap (CLI)
- serde + toml (config)
- thiserror (errors)

**New**:
- which (backend detection)

**Test**:
- tempfile (test fixtures)
- assert_cmd (CLI testing)

## Files Structure

See `.claude/EXTRACTION_GUIDE.md` for detailed file mapping.

**Core**:
- `src/main.rs` - CLI entry
- `src/workspace/` - Git worktree management
- `src/backend/` - Backend trait + implementations
- `src/config.rs` - Configuration system

**Backends**:
- `src/backend/claude.rs` - Local backend
- `src/backend/claude_vm.rs` - VM backend
- `src/backend/opencode.rs` - OpenCode backend

## Non-Goals

- ❌ Implement isolation (use existing tools)
- ❌ Session/conversation state (future feature)
- ❌ IDE integration (future)
- ❌ Replace git (we wrap, not replace)

## Open Questions

1. **Session state**: How to track Claude conversations per workspace?
   - Future: .agentree/session/{branch}.json
   - Track conversation history, context files

2. **Backend plugins**: Should users be able to add backends?
   - Future: Load backends from ~/.agentree/backends/
   - Requires dynamic loading or subprocess protocol

3. **Workspace templates**: Should we support workspace initialization scripts?
   - Future: .agentree/templates/{template-name}/init.sh
   - Run after workspace creation

## Getting Started

See `.claude/CLAUDE.md` for comprehensive development context.

**Quick start**:
```bash
cd agentree
cargo build
cargo test
cargo run -- --help
```

**Reference**: `../claude-vm` contains proven worktree logic to extract.

## Contact / Community

- GitHub: https://github.com/themouette/agentree
- Issues: https://github.com/themouette/agentree/issues
- Related: claude-vm (isolation backend)
