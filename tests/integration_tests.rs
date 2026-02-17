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

    // List worktrees with table format
    let output = test_repo.agentree(&["list", "--format", "table"]);
    assert!(output.status.success(), "list command should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("feature-test"),
        "Should show feature-test branch"
    );
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
    assert!(
        output.status.success(),
        "create should succeed for existing branch"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("existing-branch"),
        "Should mention the branch"
    );
    assert!(
        stdout.contains("Created"),
        "Should indicate worktree was created"
    );
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
    assert!(
        !stdout.contains("temp-branch"),
        "temp-branch should be removed"
    );
}

#[test]
fn test_invalid_branch_name() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Try to create worktree with invalid branch name starting with dash
    // Note: clap will treat "-invalid" as a flag, so we need to use "--" to pass it as a value
    let output = test_repo.agentree(&["create", "--", "-invalid"]);
    assert!(
        !output.status.success(),
        "Should fail for branch starting with dash"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("dash"),
        "Error should mention dash issue: {}",
        stderr
    );

    // Try reserved name
    let output = test_repo.agentree(&["create", "HEAD"]);
    assert!(
        !output.status.success(),
        "Should fail for reserved name HEAD"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("reserved"),
        "Error should mention reserved ref"
    );
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
        "Should show no worktrees message. Stdout: '{}', Stderr: '{}'",
        stdout,
        stderr
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
    assert!(
        stdout.contains("feature-with-base"),
        "Should mention the branch"
    );

    // Verify the worktree exists
    let worktrees_dir = test_repo.worktrees_dir();
    let repo_name = test_repo.path().file_name().unwrap().to_str().unwrap();
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
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    // Should be an array
    assert!(json.is_array(), "JSON output should be an array");
    let array = json.as_array().unwrap();
    assert!(!array.is_empty(), "Should have at least one worktree");

    // Check first entry has expected fields
    let first = &array[0];
    assert!(first.get("branch").is_some(), "Should have branch field");
    assert!(first.get("path").is_some(), "Should have path field");
    assert!(
        first.get("modified").is_some(),
        "Should have modified field"
    );

    // Verify it contains our branch
    let branches: Vec<String> = array
        .iter()
        .filter_map(|e| e.get("branch").and_then(|b| b.as_str()).map(String::from))
        .collect();
    assert!(
        branches.contains(&"json-test".to_string()),
        "Should contain json-test branch"
    );
}

#[test]
fn test_remove_nonexistent() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Try to remove a branch that has no worktree
    let output = test_repo.agentree(&["remove", "nonexistent-branch"]);
    assert!(
        !output.status.success(),
        "Should fail for nonexistent worktree"
    );

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
    assert!(
        output.status.success(),
        "Second create should succeed (idempotent)"
    );
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
    assert!(
        stdout.contains("Cleanup complete"),
        "Should show completion message"
    );
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
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

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
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    let array = json.as_array().expect("Should be array");
    assert!(!array.is_empty(), "Should have at least one worktree");

    // Find our worktree
    let worktree = array
        .iter()
        .find(|w| w.get("branch").and_then(|b| b.as_str()) == Some("json-backend-test"))
        .expect("Should find json-backend-test");

    // Verify fields exist
    assert!(
        worktree.get("backend").is_some(),
        "Should have backend field"
    );
    assert!(
        worktree.get("created").is_some(),
        "Should have created field"
    );

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

    // List worktrees with table format
    let output = test_repo.agentree(&["list", "--format", "table"]);
    assert!(output.status.success(), "list should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify table has CREATED column
    assert!(
        stdout.contains("CREATED"),
        "Table header should include CREATED column"
    );
}

#[test]
fn test_list_default_format() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "default-format-test"]);
    assert!(output.status.success(), "create should succeed");

    // List worktrees without format flag (should use default two-lines format)
    let output = test_repo.agentree(&["list"]);
    assert!(output.status.success(), "list should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify two-lines format: header line + branch line + arrow path line
    assert!(
        stdout.contains("BRANCH"),
        "Should have BRANCH column header"
    );
    assert!(
        stdout.contains("default-format-test"),
        "Should show branch name"
    );
    assert!(stdout.contains("→"), "Should show arrow for path");
}

#[test]
fn test_list_two_lines_format() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "two-lines-test"]);
    assert!(output.status.success(), "create should succeed");

    // List with explicit two-lines format
    let output = test_repo.agentree(&["list", "--format", "two-lines"]);
    assert!(output.status.success(), "list should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify format has headers
    assert!(stdout.contains("BRANCH"), "Should have BRANCH header");
    assert!(stdout.contains("BACKEND"), "Should have BACKEND header");
    assert!(stdout.contains("MODIFIED"), "Should have MODIFIED header");

    // Verify branch is shown
    assert!(stdout.contains("two-lines-test"), "Should show branch name");

    // Verify arrow for absolute path
    assert!(stdout.contains("→"), "Should show → for path line");
}

#[test]
fn test_list_card_format() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "card-test"]);
    assert!(output.status.success(), "create should succeed");

    // List with card format
    let output = test_repo.agentree(&["list", "--format", "card"]);
    assert!(output.status.success(), "list should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify card format has box drawing and labels
    assert!(stdout.contains("┌─"), "Should have card box top");
    assert!(stdout.contains("│"), "Should have card box sides");
    assert!(stdout.contains("└─"), "Should have card box bottom");
    assert!(stdout.contains("Path:"), "Should have Path label");
    assert!(stdout.contains("Backend:"), "Should have Backend label");
    assert!(stdout.contains("Created:"), "Should have Created label");
    assert!(stdout.contains("Modified:"), "Should have Modified label");
    assert!(stdout.contains("card-test"), "Should show branch name");
}

#[test]
fn test_list_json_format() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "json-test"]);
    assert!(output.status.success(), "create should succeed");

    // List with json format
    let output = test_repo.agentree(&["list", "--format", "json"]);
    assert!(output.status.success(), "list should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify valid JSON
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    // Should be an array
    assert!(parsed.is_array(), "JSON should be an array");

    let array = parsed.as_array().unwrap();
    assert!(!array.is_empty(), "Should have at least one worktree");

    // Check first entry has expected fields
    let first = &array[0];
    assert!(first.get("branch").is_some(), "Should have branch field");
    assert!(first.get("path").is_some(), "Should have path field");
    assert!(first.get("backend").is_some(), "Should have backend field");
    assert!(first.get("created").is_some(), "Should have created field");
    assert!(
        first.get("modified").is_some(),
        "Should have modified field"
    );

    // Verify branch name
    let branch = first.get("branch").unwrap().as_str().unwrap();
    assert_eq!(branch, "json-test", "Branch should be json-test");
}

#[test]
fn test_list_json_flag_deprecated() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "json-legacy-test"]);
    assert!(output.status.success(), "create should succeed");

    // List with deprecated --json flag
    let output = test_repo.agentree(&["list", "--json"]);
    assert!(output.status.success(), "list with --json should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should still produce valid JSON
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");
    assert!(parsed.is_array(), "JSON should be an array");

    // Should show deprecation warning
    assert!(
        stderr.contains("deprecated"),
        "Should show deprecation warning"
    );
    assert!(
        stderr.contains("--format=json"),
        "Should suggest --format=json"
    );
}

#[test]
fn test_list_json_and_format_conflict() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Try using both --json and --format (should fail)
    let output = test_repo.agentree(&["list", "--json", "--format", "table"]);
    assert!(
        !output.status.success(),
        "Should fail with conflicting flags"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("conflict") || stderr.contains("cannot be used with"),
        "Should show conflict error"
    );
}

#[test]
fn test_list_empty_with_formats() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // List with no worktrees - test each format handles empty state

    // Two-lines format
    let output = test_repo.agentree(&["list", "--format", "two-lines"]);
    assert!(output.status.success(), "list should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No worktrees found"),
        "Two-lines should show empty message"
    );

    // Table format
    let output = test_repo.agentree(&["list", "--format", "table"]);
    assert!(output.status.success(), "list should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No worktrees found"),
        "Table should show empty message"
    );

    // Card format
    let output = test_repo.agentree(&["list", "--format", "card"]);
    assert!(output.status.success(), "list should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No worktrees found"),
        "Card should show empty message"
    );

    // JSON format
    let output = test_repo.agentree(&["list", "--format", "json"]);
    assert!(output.status.success(), "list should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "[]", "JSON should show empty array");
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
fn test_exec_with_base() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a base branch and add a commit to it
    test_repo.create_branch("base-branch");
    test_repo.git(&["checkout", "base-branch"]);
    test_repo.commit("Base branch commit");
    test_repo.git(&["checkout", "main"]);

    // Run exec with --base flag
    let output = test_repo.agentree(&[
        "exec",
        "new-from-base",
        "--base",
        "base-branch",
        "--",
        "pwd",
    ]);
    assert!(
        output.status.success(),
        "exec with --base should succeed: {}",
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
fn test_exec_with_base_shorthand() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a base branch
    test_repo.create_branch("base-branch");
    test_repo.git(&["checkout", "base-branch"]);
    test_repo.commit("Base branch commit");
    test_repo.git(&["checkout", "main"]);

    // Run exec with -b shorthand
    let output = test_repo.agentree(&[
        "exec",
        "new-from-base-short",
        "-b",
        "base-branch",
        "--",
        "pwd",
    ]);
    assert!(
        output.status.success(),
        "exec with -b should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify worktree was created
    let output = test_repo.agentree(&["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("new-from-base-short"),
        "new-from-base-short worktree should exist"
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
    assert!(!output.status.success(), "exec should fail without command");

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
    assert!(stdout.contains("--base"), "Help should mention --base flag");
}

#[test]
fn test_exec_help_shows_command_separator() {
    let test_repo = TestRepo::new();
    test_repo.init_git();

    // Run exec --help
    let output = test_repo.agentree(&["exec", "--help"]);
    assert!(output.status.success(), "exec --help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("COMMAND"), "Help should mention command");
    assert!(stdout.contains("--"), "Help should show -- separator");
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
    assert!(stdout.contains("--base"), "Help should mention --base flag");
}

#[test]
fn test_editor_help_shows_options() {
    let test_repo = TestRepo::new();
    test_repo.init_git();

    // Run editor --help
    let output = test_repo.agentree(&["editor", "--help"]);
    assert!(output.status.success(), "editor --help should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Branch name"),
        "Help should mention branch name"
    );
    assert!(
        stdout.contains("START_REF"),
        "Help should mention START_REF"
    );
    assert!(
        stdout.contains("--editor"),
        "Help should mention --editor flag"
    );
    assert!(
        stdout.contains("ARGS"),
        "Help should mention additional args"
    );
}

// NOTE: test_completion_includes_editor_command was removed because the
// completion simplification (commit 720a19c) removed positional branch completion entirely.
// The test was checking for implementation details (case statements) that no longer exist.
// Completions now focus on flag values (--base, --agent, --backend) which are tested elsewhere.
