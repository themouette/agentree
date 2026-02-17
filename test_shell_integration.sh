#!/usr/bin/env bash
# Test shell integration installation and functionality

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓${NC} $1"; }
fail() { echo -e "${RED}✗${NC} $1"; return 1; }
info() { echo -e "${YELLOW}ℹ${NC} $1"; }

FAILED=0

run_test() {
    if "$@"; then
        return 0
    else
        FAILED=$((FAILED + 1))
        return 1
    fi
}

# Build agentree
info "Building agentree..."
cargo build --quiet || { fail "Build failed"; exit 1; }

AGENTREE="./target/debug/agentree"
TEST_DIR=$(mktemp -d)
trap "rm -rf $TEST_DIR" EXIT

info "Running shell integration tests..."
echo ""

# Test 1: Output syntax
info "Test 1: Validating shell-init output"

run_test bash -c "$AGENTREE shell-init --shell bash | bash -n 2>/dev/null" && \
    pass "Bash output syntax valid" || fail "Bash syntax invalid"

run_test bash -c "$AGENTREE shell-init --shell zsh | bash -n 2>/dev/null" && \
    pass "Zsh output syntax valid" || fail "Zsh syntax invalid"

FISH_OUT=$($AGENTREE shell-init --shell fish)
if echo "$FISH_OUT" | grep -q "function agentree"; then
    pass "Fish output contains function definition"
else
    run_test fail "Fish missing function definition"
fi

echo ""

# Test 2: RC file modification
info "Test 2: Testing RC file modification"

MOCK_RC="$TEST_DIR/.bashrc"
touch "$MOCK_RC"

# Source color variables and functions
# Extract to temp file to avoid broken pipe on macOS
FUNCTIONS_FILE="$TEST_DIR/functions.sh"
sed -n '/^# Colors/,/^$/p; /^log_/,/^}/p; /^detect_shell_rc()/,/^}/p; /^add_shell_init()/,/^}/p' install.sh > "$FUNCTIONS_FILE"
source "$FUNCTIONS_FILE" || {
    run_test fail "Failed to source install script functions"
    echo "Skipping remaining tests"
    exit 1
}

# Add shell integration
add_shell_init "$MOCK_RC" >/dev/null 2>&1

if grep -q 'eval "$(agentree shell-init' "$MOCK_RC"; then
    pass "RC file contains initialization line"
else
    run_test fail "RC file missing initialization line"
fi

# Test idempotency
add_shell_init "$MOCK_RC" >/dev/null 2>&1
COUNT=$(grep "agentree shell-init" "$MOCK_RC" | wc -l | tr -d ' ')
if [ "$COUNT" = "1" ]; then
    pass "Idempotent: no duplicate lines"
else
    run_test fail "Not idempotent: found $COUNT occurrences"
fi

echo ""

# Test 3: Function loading
info "Test 3: Testing function loading"

TEST_SCRIPT="$TEST_DIR/test_load.sh"
cat > "$TEST_SCRIPT" << 'EOF'
eval "$(./target/debug/agentree shell-init --shell bash)"
declare -f agentree >/dev/null 2>&1
EOF

chmod +x "$TEST_SCRIPT"

if bash "$TEST_SCRIPT"; then
    pass "Wrapper function loads correctly"
else
    run_test fail "Wrapper function failed to load"
fi

echo ""

# Test 4: Auto-detection
info "Test 4: Testing auto-detection"

if $AGENTREE shell-init 2>/dev/null | grep -q "agentree()"; then
    pass "Auto-detection produces output"
else
    run_test fail "Auto-detection failed"
fi

echo ""

# Summary
if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}✗ $FAILED test(s) failed${NC}"
    exit 1
fi
