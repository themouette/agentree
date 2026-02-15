mod helpers;

use helpers::*;
use std::process::Command;

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

// ===== Phase 4 Integration Tests =====

#[test]
fn test_create_saves_metadata() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "metadata-test"]);
    assert!(output.status.success(), "create should succeed");

    // Verify metadata via list --json (simpler than filesystem checks)
    let output = test_repo.agentree(&["list", "--json"]);
    assert!(output.status.success(), "list --json should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .expect("Output should be valid JSON");

    let worktrees = json.as_array().expect("Should be array");
    let metadata_worktree = worktrees
        .iter()
        .find(|w| w.get("branch").and_then(|b| b.as_str()) == Some("metadata-test"))
        .expect("Should find metadata-test worktree");

    // Verify metadata fields exist
    assert!(
        metadata_worktree.get("backend").is_some(),
        "Should have backend field"
    );
    assert!(
        metadata_worktree.get("created").is_some(),
        "Should have created field"
    );

    // Backend should be "local" (default when no config)
    let backend = metadata_worktree
        .get("backend")
        .and_then(|b| b.as_str())
        .expect("backend should be a string");
    assert_eq!(backend, "local", "Default backend should be 'local'");
}

#[test]
fn test_list_shows_backend_column() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "backend-test"]);
    assert!(output.status.success(), "create should succeed");

    // List worktrees in table format
    let output = test_repo.agentree(&["list"]);
    assert!(output.status.success(), "list should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify table has BACKEND column
    assert!(
        stdout.contains("BACKEND"),
        "Table header should include BACKEND column"
    );

    // Verify worktree row shows backend (should be "local")
    let lines: Vec<&str> = stdout.lines().collect();
    let backend_row = lines.iter().find(|line| line.contains("backend-test"));
    assert!(backend_row.is_some(), "Should find backend-test in list");

    let row = backend_row.unwrap();
    assert!(
        row.contains("local"),
        "Should show 'local' backend in row: {}",
        row
    );
}

#[test]
fn test_list_json_includes_backend() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "json-backend-test"]);
    assert!(output.status.success(), "create should succeed");

    // List with JSON output
    let output = test_repo.agentree(&["list", "--json"]);
    assert!(output.status.success(), "list --json should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .expect("Output should be valid JSON");

    let array = json.as_array().expect("Should be array");
    assert!(array.len() >= 1, "Should have at least one worktree");

    // Find our worktree
    let worktree = array
        .iter()
        .find(|w| w.get("branch").and_then(|b| b.as_str()) == Some("json-backend-test"))
        .expect("Should find json-backend-test");

    // Verify fields exist
    assert!(worktree.get("backend").is_some(), "Should have backend field");
    assert!(worktree.get("created").is_some(), "Should have created field");

    // Verify backend value
    let backend = worktree.get("backend").and_then(|b| b.as_str());
    assert_eq!(backend, Some("local"), "Backend should be 'local'");
}

#[test]
fn test_cd_prints_path() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "cd-test"]);
    assert!(output.status.success(), "create should succeed");

    // Run cd command
    let output = test_repo.agentree(&["cd", "cd-test"]);
    assert!(output.status.success(), "cd should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify output format
    assert!(
        stdout.starts_with("cd '"),
        "Output should start with 'cd '': {}",
        stdout
    );

    // Verify path contains worktree directory
    let worktrees_dir = test_repo.worktrees_dir();
    let worktree_path = worktrees_dir
        .join(test_repo.path().file_name().unwrap())
        .join("cd-test");
    let expected_in_output = worktree_path.to_string_lossy();

    assert!(
        stdout.contains(&*expected_in_output),
        "Output should contain worktree path. Expected substring: {}, Got: {}",
        expected_in_output,
        stdout
    );
}

#[test]
fn test_cd_nonexistent_branch() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Try cd to nonexistent branch
    let output = test_repo.agentree(&["cd", "nonexistent-branch"]);
    assert!(
        !output.status.success(),
        "cd should fail for nonexistent branch"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No worktree found") || stderr.contains("nonexistent-branch"),
        "Error should mention worktree not found or branch name: {}",
        stderr
    );
}

#[test]
fn test_list_shows_created_column() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "created-test"]);
    assert!(output.status.success(), "create should succeed");

    // List worktrees
    let output = test_repo.agentree(&["list"]);
    assert!(output.status.success(), "list should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify table has CREATED column
    assert!(
        stdout.contains("CREATED"),
        "Table header should include CREATED column"
    );
}

#[test]
fn test_remove_merged_cleanup() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree for merge-test branch
    let output = test_repo.agentree(&["create", "merge-test"]);
    assert!(
        output.status.success(),
        "create should succeed: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Switch to worktree and create a commit
    let worktrees_dir = test_repo.worktrees_dir();
    let repo_name = test_repo.path().file_name().unwrap();
    let worktree_path = worktrees_dir.join(repo_name).join("merge-test");

    std::fs::write(worktree_path.join("merge-file.txt"), "merge content")
        .expect("Failed to create file in worktree");

    Command::new("git")
        .args(["add", "."])
        .current_dir(&worktree_path)
        .output()
        .expect("git add should work");

    Command::new("git")
        .args(["commit", "-m", "Add merge file"])
        .current_dir(&worktree_path)
        .output()
        .expect("git commit should work");

    // Merge the branch into main
    test_repo.git(&["merge", "--no-ff", "merge-test", "-m", "Merge merge-test"]);

    // Verify the worktree exists before remove
    let output = test_repo.agentree(&["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("merge-test"),
        "merge-test should exist before remove"
    );

    // Run remove --merged
    let output = test_repo.agentree(&["remove", "--merged", "main"]);
    assert!(
        output.status.success(),
        "remove --merged should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify the worktree is removed
    let output = test_repo.agentree(&["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("merge-test"),
        "merge-test should be removed after --merged cleanup"
    );
}

#[test]
fn test_exec_autocreates_and_runs_command() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Run exec which should auto-create workspace and run command
    let output = test_repo.agentree(&["exec", "feat-exec", "--", "echo", "workspace-test"]);
    assert!(
        output.status.success(),
        "exec should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify output contains expected text
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("workspace-test"),
        "Output should contain 'workspace-test'"
    );

    // Verify worktree was created
    let output = test_repo.agentree(&["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("feat-exec"),
        "feat-exec worktree should exist after exec"
    );
}

#[test]
fn test_exec_with_start_ref() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a base branch and add a commit to it
    test_repo.create_branch("base-branch");
    test_repo.git(&["checkout", "base-branch"]);
    test_repo.commit("Base branch commit");
    test_repo.git(&["checkout", "main"]);

    // Run exec with start_ref
    let output = test_repo.agentree(&["exec", "new-from-base", "base-branch", "--", "pwd"]);
    assert!(
        output.status.success(),
        "exec with start_ref should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify worktree was created
    let output = test_repo.agentree(&["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("new-from-base"),
        "new-from-base worktree should exist"
    );
}

#[test]
fn test_exec_existing_workspace() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create workspace first
    let output = test_repo.agentree(&["create", "existing-ws"]);
    assert!(output.status.success(), "create should succeed");

    // Run exec on existing workspace
    let output = test_repo.agentree(&["exec", "existing-ws", "--", "echo", "reuse-test"]);
    assert!(
        output.status.success(),
        "exec on existing workspace should succeed"
    );

    // Verify output
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("reuse-test"),
        "Output should contain 'reuse-test'"
    );

    // Should NOT say "Created" (should silently reuse)
    assert!(
        !stdout.contains("Created"),
        "Should not show 'Created' message for existing workspace"
    );
}

#[test]
fn test_exec_requires_command() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Try exec without command (no -- and no command)
    let output = test_repo.agentree(&["exec", "some-branch"]);
    assert!(
        !output.status.success(),
        "exec should fail without command"
    );

    // Clap should error about missing required argument
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("required") || stderr.contains("COMMAND"),
        "Error should mention missing command argument: {}",
        stderr
    );
}

#[test]
fn test_shell_help_shows_args() {
    let test_repo = TestRepo::new();
    test_repo.init_git();

    // Run shell --help
    let output = test_repo.agentree(&["shell", "--help"]);
    assert!(output.status.success(), "shell --help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Branch name"),
        "Help should mention branch name"
    );
    assert!(
        stdout.contains("START_REF"),
        "Help should mention START_REF"
    );
}

#[test]
fn test_exec_help_shows_command_separator() {
    let test_repo = TestRepo::new();
    test_repo.init_git();

    // Run exec --help
    let output = test_repo.agentree(&["exec", "--help"]);
    assert!(output.status.success(), "exec --help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("COMMAND"),
        "Help should mention command"
    );
    assert!(
        stdout.contains("--"),
        "Help should show -- separator"
    );
}

#[test]
fn test_agent_help_shows_flags() {
    let test_repo = TestRepo::new();
    test_repo.init_git();

    // Run agent --help
    let output = test_repo.agentree(&["agent", "--help"]);
    assert!(output.status.success(), "agent --help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Branch name"),
        "Help should mention branch name"
    );
    assert!(
        stdout.contains("START_REF"),
        "Help should mention START_REF"
    );
}
