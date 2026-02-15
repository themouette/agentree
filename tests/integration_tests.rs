mod helpers;

use helpers::*;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_create_and_list_worktree() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "feature-test", "--base", "main"]);
    assert!(output.status.success(), "create command should succeed");

    // List worktrees
    let output = test_repo.agentree(&["list"]);
    assert!(output.status.success(), "list command should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("feature-test"), "Should show feature-test branch");
    assert!(stdout.contains("main"), "Should show main branch");
}

#[test]
fn test_create_existing_branch() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a branch
    test_repo.git(&["branch", "existing-branch"]);

    // Create worktree for existing branch
    let output = test_repo.agentree(&["create", "existing-branch"]);
    assert!(output.status.success(), "create should succeed for existing branch");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("existing-branch"), "Should mention the branch");
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

    // Try to create worktree with invalid branch name
    let output = test_repo.agentree(&["create", "-invalid"]);
    assert!(!output.status.success(), "Should fail for branch starting with dash");

    let output = test_repo.agentree(&["create", "HEAD"]);
    assert!(!output.status.success(), "Should fail for reserved name HEAD");
}

#[test]
fn test_list_empty_repo() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // List should work even with just main worktree
    let output = test_repo.agentree(&["list"]);
    assert!(output.status.success(), "list should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("main"), "Should show main branch");
}
