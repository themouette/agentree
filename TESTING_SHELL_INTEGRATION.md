# Shell Integration Testing

This document describes how the `agentree shell-init` feature is tested.

## Testing Strategy

### 1. Unit Tests (Rust)
**Location**: `src/commands/shell_init.rs`

**Tests**:
- Shell type parsing (bash, zsh, fish, unknown)
- Default behavior for unknown shells (falls back to POSIX)
- Function output generation

**Run**:
```bash
cargo test shell_init
```

### 2. Integration Tests (Shell Script)
**Location**: `test_shell_integration.sh`

**Tests**:
- **Syntax Validation**: Shell-init output is syntactically valid for each shell type
- **RC File Modification**: Install script correctly adds initialization line
- **Idempotency**: Running install twice doesn't create duplicates
- **Function Loading**: Wrapper function can be sourced and is callable
- **Auto-Detection**: Shell type auto-detection works

**Run**:
```bash
./test_shell_integration.sh
```

**Expected Output**:
```
ℹ Building agentree...
ℹ Running shell integration tests...

ℹ Test 1: Validating shell-init output
✓ Bash output syntax valid
✓ Zsh output syntax valid
✓ Fish output contains function definition

ℹ Test 2: Testing RC file modification
✓ RC file contains initialization line
✓ Idempotent: no duplicate lines

ℹ Test 3: Testing function loading
✓ Wrapper function loads correctly

ℹ Test 4: Testing auto-detection
✓ Auto-detection produces output

✓ All tests passed!
```

### 3. Manual Testing

For end-to-end verification:

#### Test 1: Manual Installation
```bash
# In a test shell
echo 'eval "$(./target/debug/agentree shell-init)"' >> ~/.bashrc_test
source ~/.bashrc_test

# Verify function exists
type agentree
# Should show: agentree is a function

# Test it doesn't break normal commands
./target/debug/agentree --help
```

#### Test 2: CD Command Integration
```bash
# Create a test worktree
./target/debug/agentree create test-branch

# Try cd command (requires sourced wrapper)
agentree cd test-branch
pwd
# Should show path to test-branch worktree
```

#### Test 3: Install Script with --shell-integration
```bash
# Test install script (dry run on test rc file)
export HOME=/tmp/test-home-$$
mkdir -p $HOME
echo "Testing..." | ./install.sh --shell-integration --help

# Check what would be added
cat $HOME/.bashrc 2>/dev/null || echo "RC file not created (expected for --help)"
```

### 4. Cross-Shell Testing

Test on actual shell environments:

**Bash**:
```bash
bash -c 'eval "$(./target/debug/agentree shell-init)"; declare -f agentree'
```

**Zsh**:
```zsh
zsh -c 'eval "$(./target/debug/agentree shell-init)"; declare -f agentree'
```

**Fish**:
```fish
fish -c 'eval (./target/debug/agentree shell-init); functions agentree'
```

## What Each Test Validates

### Syntax Validation
- **Purpose**: Ensure shell-init output is syntactically correct
- **Method**: Pipe output to `bash -n` (syntax check mode)
- **Passes if**: No syntax errors reported

### RC File Modification
- **Purpose**: Verify install script adds correct line to rc file
- **Method**: Run `add_shell_init` function, check file contents
- **Passes if**: File contains exactly `eval "$(agentree shell-init)"`

### Idempotency
- **Purpose**: Ensure running install twice doesn't duplicate lines
- **Method**: Run `add_shell_init` twice, count occurrences
- **Passes if**: Only 1 occurrence of "agentree shell-init" in file

### Function Loading
- **Purpose**: Verify wrapper function can be sourced and works
- **Method**: Source output in subprocess, check function exists
- **Passes if**: `declare -f agentree` succeeds

### Auto-Detection
- **Purpose**: Verify shell type detection from environment
- **Method**: Run without `--shell` flag, check output
- **Passes if**: Produces valid function definition

## Continuous Integration

Add to `.github/workflows/test.yml`:

```yaml
- name: Test shell integration
  run: |
    chmod +x test_shell_integration.sh
    ./test_shell_integration.sh
```

## Known Limitations

1. **Fish Syntax Validation**: Fish has different syntax from bash/zsh, so we can't use `bash -n` to validate it. We check for expected keywords instead.

2. **Actual CD Testing**: The test suite doesn't test the actual `agentree cd` command changing directories because that requires:
   - A git repository with worktrees
   - The wrapper function modifying the parent shell's state
   - Complex subprocess management

   This is covered by manual testing instead.

3. **Shell Process Testing**: We test that the function *loads* but not that it actually *works* in all edge cases (paths with spaces, special characters, etc.). Full integration testing requires actual shell sessions.

## Troubleshooting Tests

If tests fail:

1. **Build Errors**: Ensure `cargo build` succeeds first
2. **Function Sourcing Errors**: Check `install.sh` syntax with `bash -n install.sh`
3. **Grep Issues**: Verify install script has the expected functions
4. **Idempotency Failures**: Check if marker comment changed in `install.sh`

## Future Improvements

1. **Docker-based Testing**: Test across shell versions in containers
2. **Real Worktree Testing**: Create git repos and worktrees, test full workflow
3. **Edge Case Testing**: Paths with spaces, special characters, symlinks
4. **Performance Testing**: Shell startup time impact measurement
