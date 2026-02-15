mod helpers;

use helpers::*;

#[test]
fn test_create_and_list_worktree() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "feature-test"]);
    assert!(output.status.success(), "create command should succeed");

    // List worktrees
    let output = test_repo.agentree(&["list"]);
    assert!(output.status.success(), "list command should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("feature-test"), "Should show feature-test branch");
    // Table output should have columns
    assert!(stdout.contains("BRANCH"), "Should have BRANCH column");
    assert!(stdout.contains("PATH"), "Should have PATH column");
    assert!(stdout.contains("MODIFIED"), "Should have MODIFIED column");
}

#[test]
fn test_create_existing_branch() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a branch
    test_repo.create_branch("existing-branch");

    // Create worktree for existing branch
    let output = test_repo.agentree(&["create", "existing-branch"]);
    assert!(output.status.success(), "create should succeed for existing branch");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("existing-branch"), "Should mention the branch");
    assert!(stdout.contains("Created"), "Should indicate worktree was created");
}

#[test]
fn test_remove_worktree() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "temp-branch"]);
    assert!(output.status.success(), "create should succeed");

    // Remove the worktree
    let output = test_repo.agentree(&["remove", "temp-branch"]);
    assert!(output.status.success(), "remove should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Removed"), "Should show removal message");

    // Verify it's gone
    let output = test_repo.agentree(&["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("temp-branch"), "temp-branch should be removed");
}

#[test]
fn test_invalid_branch_name() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Try to create worktree with invalid branch name starting with dash
    // Note: clap will treat "-invalid" as a flag, so we need to use "--" to pass it as a value
    let output = test_repo.agentree(&["create", "--", "-invalid"]);
    assert!(!output.status.success(), "Should fail for branch starting with dash");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("dash"), "Error should mention dash issue: {}", stderr);

    // Try reserved name
    let output = test_repo.agentree(&["create", "HEAD"]);
    assert!(!output.status.success(), "Should fail for reserved name HEAD");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("reserved"), "Error should mention reserved ref");
}

#[test]
fn test_list_empty_repo() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // List should work even with no additional worktrees
    let output = test_repo.agentree(&["list"]);
    assert!(output.status.success(), "list should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should show "No worktrees found" since we skip the main repo
    assert!(
        stdout.contains("No worktrees found"),
        "Should show no worktrees message. Stdout: '{}', Stderr: '{}'", stdout, stderr
    );
}

#[test]
fn test_create_with_base() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create worktree with explicit base
    let output = test_repo.agentree(&["create", "feature-with-base", "--base", "main"]);
    assert!(
        output.status.success(),
        "create with --base should succeed: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("feature-with-base"), "Should mention the branch");

    // Verify the worktree exists
    let worktrees_dir = test_repo.worktrees_dir();
    let repo_name = test_repo
        .path()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let expected_path = worktrees_dir.join(repo_name).join("feature-with-base");
    assert!(expected_path.exists(), "Worktree directory should exist");
}

#[test]
fn test_list_json_output() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "json-test"]);
    assert!(output.status.success(), "create should succeed");

    // List with JSON output
    let output = test_repo.agentree(&["list", "--json"]);
    assert!(output.status.success(), "list --json should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse JSON
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .expect("Output should be valid JSON");

    // Should be an array
    assert!(json.is_array(), "JSON output should be an array");
    let array = json.as_array().unwrap();
    assert!(array.len() >= 1, "Should have at least one worktree");

    // Check first entry has expected fields
    let first = &array[0];
    assert!(first.get("branch").is_some(), "Should have branch field");
    assert!(first.get("path").is_some(), "Should have path field");
    assert!(first.get("modified").is_some(), "Should have modified field");

    // Verify it contains our branch
    let branches: Vec<String> = array
        .iter()
        .filter_map(|e| e.get("branch").and_then(|b| b.as_str()).map(String::from))
        .collect();
    assert!(branches.contains(&"json-test".to_string()), "Should contain json-test branch");
}

#[test]
fn test_remove_nonexistent() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Try to remove a branch that has no worktree
    let output = test_repo.agentree(&["remove", "nonexistent-branch"]);
    assert!(!output.status.success(), "Should fail for nonexistent worktree");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("nonexistent-branch") || stderr.contains("No worktree found"),
        "Error should mention the branch or worktree not found"
    );
}

#[test]
fn test_create_idempotent() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "idempotent-test"]);
    assert!(output.status.success(), "First create should succeed");
    let stdout1 = String::from_utf8_lossy(&output.stdout);
    assert!(stdout1.contains("Created"), "First call should create");

    // Create the same worktree again (idempotent)
    let output = test_repo.agentree(&["create", "idempotent-test"]);
    assert!(output.status.success(), "Second create should succeed (idempotent)");
    let stdout2 = String::from_utf8_lossy(&output.stdout);
    assert!(stdout2.contains("Resuming"), "Second call should resume");
}

#[test]
fn test_clean_no_orphans() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Run clean on a repo with no orphans
    let output = test_repo.agentree(&["clean"]);
    assert!(output.status.success(), "clean should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Cleanup complete"), "Should show completion message");
}
