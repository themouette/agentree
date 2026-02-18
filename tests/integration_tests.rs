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
fn test_create_fails_if_exists() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "idempotent-test"]);
    assert!(output.status.success(), "First create should succeed");
    let stdout1 = String::from_utf8_lossy(&output.stdout);
    assert!(stdout1.contains("Created"), "First call should create");

    // Create the same worktree again — must fail with a helpful hint
    let output = test_repo.agentree(&["create", "idempotent-test"]);
    assert!(
        !output.status.success(),
        "Second create should fail: worktree already exists"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("already exists") || stderr.contains("agentree agent"),
        "Error should mention the existing worktree or suggest 'agentree agent'"
    );
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

    // Verify card format shows the branch name and backend label
    assert!(
        stdout.contains("backend-test"),
        "Should show branch name in card"
    );
    assert!(
        stdout.contains("Backend:"),
        "Card should include Backend label"
    );

    // Verify the backend value is shown as "local"
    let lines: Vec<&str> = stdout.lines().collect();
    let backend_line = lines.iter().find(|line| line.contains("Backend:"));
    assert!(backend_line.is_some(), "Should find Backend: line in card");

    let line = backend_line.unwrap();
    assert!(
        line.contains("local"),
        "Should show 'local' backend: {}",
        line
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
fn test_cd_autocreates_worktree() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // cd to a branch that does not exist yet — should auto-create the worktree
    let output = test_repo.agentree(&["cd", "new-branch"]);
    assert!(
        output.status.success(),
        "cd should succeed by creating the worktree: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("cd '"),
        "Output should be a cd command: {}",
        stdout
    );
    assert!(
        stdout.contains("new-branch"),
        "Output should reference the branch name: {}",
        stdout
    );
}

#[test]
fn test_cd_autocreates_with_base() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // cd with an explicit base branch
    let output = test_repo.agentree(&["cd", "based-branch", "-b", "main"]);
    assert!(
        output.status.success(),
        "cd with base should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("based-branch"),
        "Output should reference the new branch: {}",
        stdout
    );
}

#[test]
fn test_cd_warns_when_branch_checked_out_in_main_repo() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree on a separate branch so we have somewhere to "come from".
    let wt_output = test_repo.agentree(&["create", "other-branch"]);
    assert!(wt_output.status.success(), "create worktree should succeed");

    // The main repo remains on 'main'. Running `agentree cd main` from within
    // the 'other-branch' worktree must warn that 'main' lives in the main repo,
    // not in a dedicated worktree.
    let worktrees_dir = test_repo.worktrees_dir();
    let repo_name = test_repo.path().file_name().unwrap();
    let wt_path = worktrees_dir.join(repo_name).join("other-branch");

    let output = test_repo.agentree_from(&wt_path, &["cd", "main"]);
    assert!(
        output.status.success(),
        "cd should succeed even when branch is in main repo: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // stdout must still be a valid cd command pointing at the main repo
    assert!(
        stdout.starts_with("cd '"),
        "stdout should be a cd command: {stdout}"
    );

    // stderr must warn that the branch lives in the main repo
    assert!(
        stderr.contains("checked out in the main repository"),
        "stderr should warn about main repo: {stderr}"
    );
    assert!(
        stderr.contains("Tip:"),
        "stderr should include a tip: {stderr}"
    );
}

#[test]
fn test_cd_warns_already_in_main_repo_on_that_branch() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // The test repo starts on 'main'. Running `agentree cd main` while the
    // CWD is already the main repo should emit the "already here" variant.
    let output = test_repo.agentree(&["cd", "main"]);
    assert!(
        output.status.success(),
        "cd should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("already here"),
        "stderr should say 'already here': {stderr}"
    );
    assert!(
        stderr.contains("Tip:"),
        "stderr should include a tip: {stderr}"
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

    // List worktrees without format flag (should use default card format)
    let output = test_repo.agentree(&["list"]);
    assert!(output.status.success(), "list should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify card format: box-drawing characters and branch name in header
    assert!(
        stdout.contains("default-format-test"),
        "Should show branch name"
    );
    assert!(stdout.contains("┌─"), "Should show card top border");
    assert!(stdout.contains("└─"), "Should show card bottom border");
    assert!(stdout.contains("Path:"), "Should show Path label");
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
fn test_list_merged_filters_worktrees() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create two worktrees
    test_repo.agentree(&["create", "merged-branch"]);
    test_repo.agentree(&["create", "unmerged-branch"]);

    let repo_name = test_repo.path().file_name().unwrap();

    // Give unmerged-branch its own commit so it is genuinely not merged
    let unmerged_path = test_repo
        .worktrees_dir()
        .join(repo_name)
        .join("unmerged-branch");
    std::fs::write(unmerged_path.join("unmerged.txt"), "content").expect("Failed to create file");
    Command::new("git")
        .args(["add", "."])
        .current_dir(&unmerged_path)
        .output()
        .expect("git add should work");
    Command::new("git")
        .args(["commit", "-m", "Unmerged work"])
        .current_dir(&unmerged_path)
        .output()
        .expect("git commit should work");

    // Commit something in merged-branch and merge it into main
    let merged_path = test_repo
        .worktrees_dir()
        .join(repo_name)
        .join("merged-branch");
    std::fs::write(merged_path.join("merged.txt"), "content").expect("Failed to create file");
    Command::new("git")
        .args(["add", "."])
        .current_dir(&merged_path)
        .output()
        .expect("git add should work");
    Command::new("git")
        .args(["commit", "-m", "Add merged file"])
        .current_dir(&merged_path)
        .output()
        .expect("git commit should work");
    test_repo.git(&[
        "merge",
        "--no-ff",
        "merged-branch",
        "-m",
        "Merge merged-branch",
    ]);

    // list --merged main should show only merged-branch
    let output = test_repo.agentree(&["list", "--merged", "main", "--no-dirty-check"]);
    assert!(
        output.status.success(),
        "list --merged should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("merged-branch"),
        "Should show merged-branch: {}",
        stdout
    );
    assert!(
        !stdout.contains("unmerged-branch"),
        "Should not show unmerged-branch: {}",
        stdout
    );
}

#[test]
fn test_list_merged_empty_message() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree with its own commit so it is genuinely not merged
    test_repo.agentree(&["create", "not-merged"]);
    let repo_name = test_repo.path().file_name().unwrap();
    let worktree_path = test_repo.worktrees_dir().join(repo_name).join("not-merged");
    std::fs::write(worktree_path.join("work.txt"), "content").expect("Failed to create file");
    Command::new("git")
        .args(["add", "."])
        .current_dir(&worktree_path)
        .output()
        .expect("git add should work");
    Command::new("git")
        .args(["commit", "-m", "Unmerged work"])
        .current_dir(&worktree_path)
        .output()
        .expect("git commit should work");

    // list --merged main should show the specific empty message
    let output = test_repo.agentree(&["list", "--merged", "main", "--no-dirty-check"]);
    assert!(output.status.success(), "list --merged should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No merged worktrees found for 'main'"),
        "Should show targeted empty message: {}",
        stdout
    );
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
fn test_remove_merged_default_current_branch() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree for feature branch
    let output = test_repo.agentree(&["create", "feature-auto"]);
    assert!(
        output.status.success(),
        "create should succeed: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Switch to worktree and create a commit
    let worktrees_dir = test_repo.worktrees_dir();
    let repo_name = test_repo.path().file_name().unwrap();
    let worktree_path = worktrees_dir.join(repo_name).join("feature-auto");

    std::fs::write(worktree_path.join("feature.txt"), "feature content")
        .expect("Failed to create file in worktree");

    Command::new("git")
        .args(["add", "."])
        .current_dir(&worktree_path)
        .output()
        .expect("git add should work");

    Command::new("git")
        .args(["commit", "-m", "Add feature file"])
        .current_dir(&worktree_path)
        .output()
        .expect("git commit should work");

    // Merge the branch into main (in main repo)
    test_repo.git(&["merge", "--no-ff", "feature-auto", "-m", "Merge feature"]);

    // Verify current branch is main
    let current_branch = Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .current_dir(test_repo.path())
        .output()
        .expect("git symbolic-ref should work");
    let branch = String::from_utf8_lossy(&current_branch.stdout)
        .trim()
        .to_string();
    assert_eq!(branch, "main", "Should be on main branch");

    // Run remove --merged without argument (should use current branch = main)
    let output = test_repo.agentree(&["remove", "--merged"]);
    assert!(
        output.status.success(),
        "remove --merged should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify the worktree is removed
    let output = test_repo.agentree(&["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("feature-auto"),
        "feature-auto should be removed after --merged cleanup"
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

// ===== Docker Sandbox Backend Tests =====

/// Helper to check if Docker is available and running
fn is_docker_available() -> bool {
    // Check if docker binary exists
    if which::which("docker").is_err() {
        return false;
    }

    // Check if Docker daemon is running
    let output = Command::new("docker").arg("info").output();

    match output {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// Helper to check if we're on a Linux system
fn is_linux() -> bool {
    cfg!(target_os = "linux")
}

#[test]
fn test_docker_sandbox_backend_validation_linux() {
    if !is_linux() {
        // Skip on non-Linux platforms
        return;
    }

    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Try to create with docker-sandbox backend on Linux
    let output = test_repo.agentree(&["create", "docker-test", "--backend", "docker-sandbox"]);

    // Should fail on Linux
    assert!(
        !output.status.success(),
        "docker-sandbox should not be supported on Linux"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not supported on Linux") || stderr.contains("microVM"),
        "Error should mention Linux not being supported: {}",
        stderr
    );
}

#[test]
fn test_docker_sandbox_backend_validation_not_installed() {
    if is_linux() {
        // Skip on Linux - it will fail with different error
        return;
    }

    if is_docker_available() {
        // Skip if Docker is actually installed
        return;
    }

    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Try to create with docker-sandbox backend when Docker not installed
    let output = test_repo.agentree(&["create", "docker-test", "--backend", "docker-sandbox"]);

    // Should fail with binary not found
    assert!(
        !output.status.success(),
        "docker-sandbox should fail when Docker not installed"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("docker") || stderr.contains("Docker Desktop"),
        "Error should mention Docker installation: {}",
        stderr
    );
}

#[test]
fn test_docker_sandbox_config() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a config file with docker-sandbox settings
    let config_content = r#"
[backend]
default = "docker-sandbox"

[docker-sandbox]
binary = "docker"
persistent = true
"#;

    let config_path = test_repo.path().join(".agentree.toml");
    std::fs::write(&config_path, config_content).expect("Failed to write config");

    // Create workspace - will fail if Docker not available, but that's okay
    // We're testing that the config is parsed correctly
    let output = test_repo.agentree(&["create", "docker-config-test"]);

    // If it failed, check it's for Docker reasons, not config parsing
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Should NOT be a config parse error
        assert!(
            !stderr.contains("parse") && !stderr.contains("TOML"),
            "Should not be a config parse error: {}",
            stderr
        );
        // Should be Docker-related error (unless on Linux)
        if !is_linux() {
            assert!(
                stderr.contains("Docker") || stderr.contains("docker"),
                "Error should be Docker-related: {}",
                stderr
            );
        }
    }
}

#[test]
fn test_docker_sandbox_backend_cli_flag() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Test that --backend docker-sandbox is accepted (validation will fail if Docker not available)
    let output = test_repo.agentree(&["create", "docker-cli-test", "--backend", "docker-sandbox"]);

    // If it failed, check it's not because the backend name wasn't recognized
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("Unknown backend") && !stderr.contains("invalid backend"),
            "Backend name should be recognized even if Docker not available: {}",
            stderr
        );
    }
}

#[test]
fn test_docker_sandbox_backend_case_insensitive() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Test that dockersandbox (no hyphen) is accepted
    let output = test_repo.agentree(&["create", "docker-nohyphen", "--backend", "dockersandbox"]);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Should not be an "unknown backend" error
        assert!(
            !stderr.contains("Unknown backend"),
            "dockersandbox should be recognized as valid backend name: {}",
            stderr
        );
    }

    // Test uppercase
    let output = test_repo.agentree(&["create", "docker-upper", "--backend", "DOCKER-SANDBOX"]);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("Unknown backend"),
            "DOCKER-SANDBOX should be recognized: {}",
            stderr
        );
    }
}

#[test]
fn test_remove_dirty_worktree_fails_without_force() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "dirty-branch"]);
    assert!(output.status.success(), "create should succeed");

    // Get the worktree path
    let repo_name = test_repo.path().file_name().unwrap();
    let worktree_path = test_repo
        .worktrees_dir()
        .join(repo_name)
        .join("dirty-branch");

    // Make uncommitted changes
    std::fs::write(worktree_path.join("dirty.txt"), "uncommitted change")
        .expect("Failed to create dirty file");

    // Try to remove without force - should fail
    let output = test_repo.agentree(&["remove", "dirty-branch"]);
    assert!(
        !output.status.success(),
        "remove should fail with uncommitted changes"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("uncommitted changes") || stderr.contains("modified files"),
        "Error should mention uncommitted changes: {}",
        stderr
    );
    assert!(
        stderr.contains("agentree remove -f"),
        "Error should suggest -f flag: {}",
        stderr
    );
}

#[test]
fn test_remove_dirty_worktree_with_force() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "dirty-force-branch"]);
    assert!(output.status.success(), "create should succeed");

    // Get the worktree path
    let repo_name = test_repo.path().file_name().unwrap();
    let worktree_path = test_repo
        .worktrees_dir()
        .join(repo_name)
        .join("dirty-force-branch");

    // Make uncommitted changes
    std::fs::write(worktree_path.join("dirty.txt"), "uncommitted change")
        .expect("Failed to create dirty file");

    // Remove with -f should succeed
    let output = test_repo.agentree(&["remove", "-f", "dirty-force-branch"]);
    assert!(
        output.status.success(),
        "remove with -f should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Removed"), "Should show removal message");
}

#[test]
fn test_remove_locked_worktree_fails_without_force() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "locked-branch"]);
    assert!(output.status.success(), "create should succeed");

    // Lock the worktree using git
    let repo_name = test_repo.path().file_name().unwrap();
    let worktree_path = test_repo
        .worktrees_dir()
        .join(repo_name)
        .join("locked-branch");
    test_repo.git(&[
        "worktree",
        "lock",
        worktree_path.to_str().unwrap(),
        "--reason",
        "test lock",
    ]);

    // Try to remove without force - should fail
    let output = test_repo.agentree(&["remove", "locked-branch"]);
    assert!(
        !output.status.success(),
        "remove should fail for locked worktree"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("locked"),
        "Error should mention locked status: {}",
        stderr
    );
    assert!(
        stderr.contains("agentree remove -ff") || stderr.contains("agentree remove --unlock"),
        "Error should suggest -ff or --unlock: {}",
        stderr
    );
}

#[test]
fn test_remove_locked_worktree_with_double_force() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "locked-ff-branch"]);
    assert!(output.status.success(), "create should succeed");

    // Lock the worktree
    let repo_name = test_repo.path().file_name().unwrap();
    let worktree_path = test_repo
        .worktrees_dir()
        .join(repo_name)
        .join("locked-ff-branch");
    test_repo.git(&[
        "worktree",
        "lock",
        worktree_path.to_str().unwrap(),
        "--reason",
        "test lock",
    ]);

    // Remove with -ff should succeed
    let output = test_repo.agentree(&["remove", "-ff", "locked-ff-branch"]);
    assert!(
        output.status.success(),
        "remove with -ff should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Removed"), "Should show removal message");
}

#[test]
fn test_remove_locked_worktree_with_unlock() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "locked-unlock-branch"]);
    assert!(output.status.success(), "create should succeed");

    // Lock the worktree
    let repo_name = test_repo.path().file_name().unwrap();
    let worktree_path = test_repo
        .worktrees_dir()
        .join(repo_name)
        .join("locked-unlock-branch");
    test_repo.git(&[
        "worktree",
        "lock",
        worktree_path.to_str().unwrap(),
        "--reason",
        "test lock",
    ]);

    // Remove with --unlock should succeed
    let output = test_repo.agentree(&["remove", "--unlock", "locked-unlock-branch"]);
    assert!(
        output.status.success(),
        "remove with --unlock should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Removed"), "Should show removal message");

    // Check that unlock message was printed
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unlocked worktree"),
        "Should show unlock message: {}",
        stderr
    );
}

#[test]
fn test_remove_unlock_already_unlocked() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree (not locked)
    let output = test_repo.agentree(&["create", "unlocked-branch"]);
    assert!(output.status.success(), "create should succeed");

    // Remove with --unlock on an already unlocked worktree should work
    let output = test_repo.agentree(&["remove", "--unlock", "unlocked-branch"]);
    assert!(
        output.status.success(),
        "remove with --unlock should succeed even when not locked: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Removed"), "Should show removal message");
}

#[test]
fn test_error_message_includes_path() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "path-test-branch"]);
    assert!(output.status.success(), "create should succeed");

    // Lock it
    let repo_name = test_repo.path().file_name().unwrap();
    let worktree_path = test_repo
        .worktrees_dir()
        .join(repo_name)
        .join("path-test-branch");
    test_repo.git(&[
        "worktree",
        "lock",
        worktree_path.to_str().unwrap(),
        "--reason",
        "test",
    ]);

    // Try to remove without force
    let output = test_repo.agentree(&["remove", "path-test-branch"]);
    assert!(!output.status.success(), "remove should fail");

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Error message should include the actual path (checking for branch name in path)
    assert!(
        stderr.contains("path-test-branch"),
        "Error should include worktree path with branch name: {}",
        stderr
    );
    assert!(
        stderr.contains("Location:"),
        "Error should have Location label: {}",
        stderr
    );
}

// ===== Doctor Command Tests =====

#[test]
fn test_doctor_clean_repo() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    test_repo.agentree(&["create", "test-branch"]);

    // Run doctor on clean repo
    let output = test_repo.agentree(&["doctor"]);
    assert!(
        output.status.success(),
        "doctor should succeed on clean repo"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No issues found") || stdout.contains("healthy"),
        "Should indicate no issues: {}",
        stdout
    );
}

#[test]
fn test_doctor_finds_orphaned_directory() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "orphan-test"]);
    assert!(output.status.success(), "create should succeed");

    // Get the worktree path
    let repo_name = test_repo.path().file_name().unwrap();
    let worktree_path = test_repo
        .worktrees_dir()
        .join(repo_name)
        .join("orphan-test");

    // Remove git metadata directly to create orphaned directory
    // (prune only works if directory is already gone)
    let metadata_path = test_repo.path().join(".git/worktrees/orphan-test");
    std::fs::remove_dir_all(&metadata_path).expect("Failed to remove git metadata");

    // Verify directory still exists
    assert!(
        worktree_path.exists(),
        "Orphaned directory should still exist"
    );

    // Run doctor - should detect orphaned directory
    let output = test_repo.agentree(&["doctor"]);

    // Doctor should exit with error when issues found
    assert!(
        !output.status.success(),
        "doctor should exit with error when issues found"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Orphaned Directory") || stderr.contains("orphaned_directory"),
        "Should detect orphaned directory: {}",
        stderr
    );
    assert!(
        stderr.contains("orphan-test") || stderr.contains(&worktree_path.display().to_string()),
        "Should mention the orphaned path: {}",
        stderr
    );
}

#[test]
fn test_doctor_finds_broken_metadata() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree
    let output = test_repo.agentree(&["create", "broken-test"]);
    assert!(output.status.success(), "create should succeed");

    // Get the worktree path
    let repo_name = test_repo.path().file_name().unwrap();
    let worktree_path = test_repo
        .worktrees_dir()
        .join(repo_name)
        .join("broken-test");

    // Delete the directory but keep git metadata
    std::fs::remove_dir_all(&worktree_path).expect("Failed to remove worktree directory");

    // Run doctor - should detect broken metadata
    let output = test_repo.agentree(&["doctor"]);

    // Should exit with error
    assert!(
        !output.status.success(),
        "doctor should exit with error when issues found"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Broken Metadata") || stderr.contains("broken_metadata"),
        "Should detect broken metadata: {}",
        stderr
    );
    assert!(
        stderr.contains("broken-test") || stderr.contains(&worktree_path.display().to_string()),
        "Should mention the broken worktree: {}",
        stderr
    );
}

#[test]
fn test_doctor_json_output() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Run doctor with JSON format on clean repo
    let output = test_repo.agentree(&["doctor", "--format", "json"]);
    assert!(output.status.success(), "doctor should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse JSON
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    // Verify structure
    assert!(json.get("scan_time").is_some(), "Should have scan_time");
    assert!(json.get("issues").is_some(), "Should have issues array");
    assert!(json.get("summary").is_some(), "Should have summary");

    // Verify summary structure
    let summary = json.get("summary").unwrap();
    assert!(summary.get("total").is_some(), "Summary should have total");
    assert!(
        summary.get("errors").is_some(),
        "Summary should have errors"
    );
    assert!(
        summary.get("warnings").is_some(),
        "Summary should have warnings"
    );

    // For clean repo, should have 0 issues
    let total = summary.get("total").unwrap().as_u64().unwrap();
    assert_eq!(total, 0, "Clean repo should have 0 issues");
}

#[test]
fn test_doctor_json_with_issues() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create orphaned directory
    let output = test_repo.agentree(&["create", "json-orphan"]);
    assert!(output.status.success());

    // Remove git metadata directly to create orphaned directory
    let metadata_path = test_repo.path().join(".git/worktrees/json-orphan");
    std::fs::remove_dir_all(&metadata_path).expect("Failed to remove git metadata");

    // Run doctor with JSON format
    let output = test_repo.agentree(&["doctor", "--format", "json"]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    // Verify issues array is not empty
    let issues = json.get("issues").unwrap().as_array().unwrap();
    assert!(
        !issues.is_empty(),
        "Should have at least one issue detected"
    );

    // Verify issue structure
    let first_issue = &issues[0];
    assert!(first_issue.get("type").is_some(), "Issue should have type");
    assert!(first_issue.get("path").is_some(), "Issue should have path");
    assert!(
        first_issue.get("description").is_some(),
        "Issue should have description"
    );
    assert!(first_issue.get("fix").is_some(), "Issue should have fix");
}

// ─── --dirty filter ──────────────────────────────────────────────────────────

#[test]
fn test_list_dirty_filter_shows_only_dirty_worktrees() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    test_repo.agentree(&["create", "clean-branch"]);
    test_repo.agentree(&["create", "dirty-filter-branch"]);

    // Make uncommitted changes in dirty-filter-branch
    let repo_name = test_repo.path().file_name().unwrap();
    let dirty_path = test_repo
        .worktrees_dir()
        .join(repo_name)
        .join("dirty-filter-branch");
    std::fs::write(dirty_path.join("work.txt"), "uncommitted").unwrap();

    let output = test_repo.agentree(&["list", "--dirty"]);
    assert!(
        output.status.success(),
        "list --dirty should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("dirty-filter-branch"),
        "Should show dirty-filter-branch: {}",
        stdout
    );
    assert!(
        !stdout.contains("clean-branch"),
        "Should not show clean-branch: {}",
        stdout
    );
}

#[test]
fn test_list_dirty_conflicts_with_no_dirty_check() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    let output = test_repo.agentree(&["list", "--dirty", "--no-dirty-check"]);
    assert!(
        !output.status.success(),
        "Combining --dirty and --no-dirty-check should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--no-dirty-check") || stderr.contains("--dirty"),
        "Should mention conflicting flags: {}",
        stderr
    );
}

#[test]
fn test_remove_dirty_filter_removes_dirty_worktrees() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    test_repo.agentree(&["create", "will-keep"]);
    test_repo.agentree(&["create", "will-remove-dirty"]);

    // Make uncommitted changes only in will-remove-dirty
    let repo_name = test_repo.path().file_name().unwrap();
    let dirty_path = test_repo
        .worktrees_dir()
        .join(repo_name)
        .join("will-remove-dirty");
    std::fs::write(dirty_path.join("work.txt"), "uncommitted").unwrap();

    // remove --dirty should remove dirty worktrees (auto force)
    let output = test_repo.agentree(&["remove", "--dirty"]);
    assert!(
        output.status.success(),
        "remove --dirty should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let list = test_repo.agentree(&["list", "--no-dirty-check"]);
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(
        stdout.contains("will-keep"),
        "will-keep should still be present: {}",
        stdout
    );
    assert!(
        !stdout.contains("will-remove-dirty"),
        "will-remove-dirty should have been removed: {}",
        stdout
    );
}

// ─── --stale filter ───────────────────────────────────────────────────────────

#[test]
fn test_list_stale_zero_days_shows_all() {
    // --stale 0 means "not modified in the last 0 days" = everything qualifies
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    test_repo.agentree(&["create", "some-branch"]);

    let output = test_repo.agentree(&["list", "--stale", "0", "--no-dirty-check"]);
    assert!(output.status.success(), "list --stale 0 should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("some-branch"),
        "Should show some-branch with --stale 0: {}",
        stdout
    );
}

#[test]
fn test_list_stale_large_threshold_shows_nothing() {
    // --stale 99999 means "not modified in 99999 days" = nothing matches a freshly created worktree
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    test_repo.agentree(&["create", "fresh-branch"]);

    let output = test_repo.agentree(&["list", "--stale", "99999", "--no-dirty-check"]);
    assert!(output.status.success(), "list --stale 99999 should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("fresh-branch"),
        "fresh-branch should not appear with very large threshold: {}",
        stdout
    );
}

#[test]
fn test_list_stale_default_value() {
    // --stale without a value should default to 30 days (no error, just runs)
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    test_repo.agentree(&["create", "any-branch"]);

    let output = test_repo.agentree(&["list", "--stale", "--no-dirty-check"]);
    assert!(
        output.status.success(),
        "list --stale (no value) should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ─── --branch pattern filter ─────────────────────────────────────────────────

#[test]
fn test_list_branch_pattern_filter() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    test_repo.agentree(&["create", "feature-one"]);
    test_repo.agentree(&["create", "feature-two"]);
    test_repo.agentree(&["create", "bugfix-one"]);

    let output = test_repo.agentree(&["list", "--branch", "feature-*", "--no-dirty-check"]);
    assert!(
        output.status.success(),
        "list --branch should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("feature-one"), "Should show feature-one");
    assert!(stdout.contains("feature-two"), "Should show feature-two");
    assert!(
        !stdout.contains("bugfix-one"),
        "Should not show bugfix-one: {}",
        stdout
    );
}

#[test]
fn test_remove_branch_pattern_filter() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    test_repo.agentree(&["create", "wip-alpha"]);
    test_repo.agentree(&["create", "wip-beta"]);
    test_repo.agentree(&["create", "release-v1"]);

    let output = test_repo.agentree(&["remove", "--branch", "wip-*"]);
    assert!(
        output.status.success(),
        "remove --branch should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let list = test_repo.agentree(&["list", "--no-dirty-check"]);
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(!stdout.contains("wip-alpha"), "wip-alpha should be removed");
    assert!(!stdout.contains("wip-beta"), "wip-beta should be removed");
    assert!(
        stdout.contains("release-v1"),
        "release-v1 should still be present: {}",
        stdout
    );
}

// ─── --clean / --not-locked inverse filters ───────────────────────────────────

#[test]
fn test_list_clean_filter_shows_only_clean_worktrees() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    test_repo.agentree(&["create", "clean-one"]);
    test_repo.agentree(&["create", "dirty-one"]);

    let repo_name = test_repo.path().file_name().unwrap();
    let dirty_path = test_repo.worktrees_dir().join(repo_name).join("dirty-one");
    std::fs::write(dirty_path.join("change.txt"), "uncommitted").unwrap();

    let output = test_repo.agentree(&["list", "--clean"]);
    assert!(output.status.success(), "list --clean should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("clean-one"), "Should show clean-one");
    assert!(
        !stdout.contains("dirty-one"),
        "Should not show dirty-one: {}",
        stdout
    );
}

#[test]
fn test_list_not_locked_filter_shows_only_unlocked_worktrees() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    test_repo.agentree(&["create", "free-one"]);
    test_repo.agentree(&["create", "locked-one"]);

    let repo_name = test_repo.path().file_name().unwrap();
    let locked_path = test_repo.worktrees_dir().join(repo_name).join("locked-one");
    test_repo.git(&["worktree", "lock", locked_path.to_str().unwrap()]);

    let output = test_repo.agentree(&["list", "--not-locked", "--no-dirty-check"]);
    assert!(output.status.success(), "list --not-locked should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("free-one"), "Should show free-one");
    assert!(
        !stdout.contains("locked-one"),
        "Should not show locked-one: {}",
        stdout
    );
}

#[test]
fn test_remove_clean_filter_removes_clean_worktrees() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    test_repo.agentree(&["create", "keep-dirty"]);
    test_repo.agentree(&["create", "remove-clean"]);

    // Make keep-dirty actually dirty
    let repo_name = test_repo.path().file_name().unwrap();
    let dirty_path = test_repo.worktrees_dir().join(repo_name).join("keep-dirty");
    std::fs::write(dirty_path.join("work.txt"), "uncommitted").unwrap();

    let output = test_repo.agentree(&["remove", "--clean"]);
    assert!(
        output.status.success(),
        "remove --clean should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let list = test_repo.agentree(&["list", "--no-dirty-check"]);
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(
        stdout.contains("keep-dirty"),
        "keep-dirty should still be present: {}",
        stdout
    );
    assert!(
        !stdout.contains("remove-clean"),
        "remove-clean should have been removed: {}",
        stdout
    );
}

// ─── --locked filter ─────────────────────────────────────────────────────────

#[test]
fn test_list_locked_shows_only_locked_worktrees() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    test_repo.agentree(&["create", "free-branch"]);
    test_repo.agentree(&["create", "locked-branch"]);

    let repo_name = test_repo.path().file_name().unwrap();
    let locked_path = test_repo
        .worktrees_dir()
        .join(repo_name)
        .join("locked-branch");
    test_repo.git(&["worktree", "lock", locked_path.to_str().unwrap()]);

    let output = test_repo.agentree(&["list", "--locked", "--no-dirty-check"]);
    assert!(
        output.status.success(),
        "list --locked should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("locked-branch"),
        "Should show locked-branch: {}",
        stdout
    );
    assert!(
        !stdout.contains("free-branch"),
        "Should not show free-branch: {}",
        stdout
    );
}

#[test]
fn test_remove_locked_filter_removes_locked_worktrees() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    test_repo.agentree(&["create", "keep-branch"]);
    test_repo.agentree(&["create", "to-remove-locked"]);

    let repo_name = test_repo.path().file_name().unwrap();
    let locked_path = test_repo
        .worktrees_dir()
        .join(repo_name)
        .join("to-remove-locked");
    test_repo.git(&["worktree", "lock", locked_path.to_str().unwrap()]);

    // remove --locked should remove locked worktrees (auto-unlocks)
    let output = test_repo.agentree(&["remove", "--locked"]);
    assert!(
        output.status.success(),
        "remove --locked should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let list = test_repo.agentree(&["list", "--no-dirty-check"]);
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(
        stdout.contains("keep-branch"),
        "keep-branch should still be present: {}",
        stdout
    );
    assert!(
        !stdout.contains("to-remove-locked"),
        "to-remove-locked should have been removed: {}",
        stdout
    );
}

// ─── --not-merged filter ────────────────────────────────────────────────────

#[test]
fn test_list_not_merged_filters_worktrees() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Use clearly distinct names (no substring overlap)
    test_repo.agentree(&["create", "done-work"]);
    test_repo.agentree(&["create", "active-work"]);

    let repo_name = test_repo.path().file_name().unwrap();

    // Give active-work a unique commit so it is genuinely not merged
    let active_path = test_repo
        .worktrees_dir()
        .join(repo_name)
        .join("active-work");
    std::fs::write(active_path.join("active.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&active_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Active work"])
        .current_dir(&active_path)
        .output()
        .unwrap();

    // Commit something in done-work and merge it into main
    let done_path = test_repo.worktrees_dir().join(repo_name).join("done-work");
    std::fs::write(done_path.join("done.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&done_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Done work"])
        .current_dir(&done_path)
        .output()
        .unwrap();
    test_repo.git(&["merge", "--no-ff", "done-work", "-m", "Merge done-work"]);

    // list --not-merged main should show only active-work
    let output = test_repo.agentree(&["list", "--not-merged", "main", "--no-dirty-check"]);
    assert!(
        output.status.success(),
        "list --not-merged should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("active-work"),
        "Should show active-work: {}",
        stdout
    );
    assert!(
        !stdout.contains("done-work"),
        "Should not show done-work: {}",
        stdout
    );
}

#[test]
fn test_list_not_merged_empty_message() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a branch and merge it — so there are NO unmerged worktrees
    test_repo.agentree(&["create", "will-be-merged"]);
    let repo_name = test_repo.path().file_name().unwrap();
    let wt_path = test_repo
        .worktrees_dir()
        .join(repo_name)
        .join("will-be-merged");
    std::fs::write(wt_path.join("f.txt"), "x").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&wt_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "feat"])
        .current_dir(&wt_path)
        .output()
        .unwrap();
    test_repo.git(&["merge", "--no-ff", "will-be-merged", "-m", "Merge"]);

    let output = test_repo.agentree(&["list", "--not-merged", "main", "--no-dirty-check"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No unmerged worktrees found for 'main'"),
        "Should show targeted empty message: {}",
        stdout
    );
}

#[test]
fn test_remove_not_merged_removes_only_unmerged() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    test_repo.agentree(&["create", "merged-x"]);
    test_repo.agentree(&["create", "unmerged-x"]);

    let repo_name = test_repo.path().file_name().unwrap();

    // Give unmerged-x a unique commit
    let unmerged_path = test_repo.worktrees_dir().join(repo_name).join("unmerged-x");
    std::fs::write(unmerged_path.join("u.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&unmerged_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Unmerged"])
        .current_dir(&unmerged_path)
        .output()
        .unwrap();

    // Merge merged-x into main
    let merged_path = test_repo.worktrees_dir().join(repo_name).join("merged-x");
    std::fs::write(merged_path.join("m.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&merged_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Merged"])
        .current_dir(&merged_path)
        .output()
        .unwrap();
    test_repo.git(&["merge", "--no-ff", "merged-x", "-m", "Merge"]);

    // remove --not-merged main should remove only unmerged-x
    let output = test_repo.agentree(&["remove", "--not-merged", "main"]);
    assert!(
        output.status.success(),
        "remove --not-merged should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // merged-x should still exist
    let list = test_repo.agentree(&["list", "--no-dirty-check"]);
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(
        stdout.contains("merged-x"),
        "merged-x should still be present: {}",
        stdout
    );
    assert!(
        !stdout.contains("unmerged-x"),
        "unmerged-x should have been removed: {}",
        stdout
    );
}

#[test]
fn test_remove_not_locked_filter_removes_unlocked_worktrees() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    test_repo.agentree(&["create", "unlocked-wt"]);
    test_repo.agentree(&["create", "locked-wt"]);

    // Lock one worktree
    let repo_name = test_repo.path().file_name().unwrap();
    let locked_path = test_repo.worktrees_dir().join(repo_name).join("locked-wt");
    test_repo.git(&["worktree", "lock", locked_path.to_str().unwrap()]);

    // remove --not-locked should remove only the unlocked worktree
    let output = test_repo.agentree(&["remove", "--not-locked"]);
    assert!(
        output.status.success(),
        "remove --not-locked should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let list = test_repo.agentree(&["list", "--no-dirty-check"]);
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(
        !stdout.contains("unlocked-wt"),
        "unlocked-wt should have been removed: {}",
        stdout
    );
    assert!(
        stdout.contains("locked-wt"),
        "locked-wt should still be present: {}",
        stdout
    );
}

#[test]
fn test_list_merged_empty_message_uses_current_branch() {
    let test_repo = TestRepo::new();
    test_repo.init_git();
    test_repo.commit("Initial commit");

    // Create a worktree with its own commit so it is genuinely not merged
    test_repo.agentree(&["create", "unmerged-for-sentinel"]);
    let repo_name = test_repo.path().file_name().unwrap();
    let wt_path = test_repo
        .worktrees_dir()
        .join(repo_name)
        .join("unmerged-for-sentinel");
    std::fs::write(wt_path.join("work.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&wt_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Unmerged work"])
        .current_dir(&wt_path)
        .output()
        .unwrap();

    // Get the current branch name (should be "main")
    let branch_output = Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .current_dir(test_repo.path())
        .output()
        .expect("git symbolic-ref should work");
    let current_branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();

    // Run list --merged without a value (sentinel HEAD should be resolved to current branch)
    let output = test_repo.agentree(&["list", "--merged", "--no-dirty-check"]);
    assert!(output.status.success(), "list --merged should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // The empty message must NOT contain "HEAD" and MUST contain the branch name
    assert!(
        !stdout.contains("'HEAD'"),
        "Empty message should not show HEAD sentinel: {}",
        stdout
    );
    assert!(
        stdout.contains(&current_branch),
        "Empty message should show current branch '{}': {}",
        current_branch,
        stdout
    );
}
