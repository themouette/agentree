use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

/// Test repository helper
pub struct TestRepo {
    temp_dir: TempDir,
    repo_path: PathBuf,
}

impl TestRepo {
    /// Create a new test repository in a temporary directory
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo_path = temp_dir.path().to_path_buf();

        Self { temp_dir, repo_path }
    }

    /// Initialize git repository
    pub fn init_git(&self) {
        self.git(&["init"]);
        self.git(&["config", "user.email", "test@example.com"]);
        self.git(&["config", "user.name", "Test User"]);
    }

    /// Create a commit
    pub fn commit(&self, message: &str) {
        // Create a test file if repo is empty
        let test_file = self.repo_path.join("test.txt");
        if !test_file.exists() {
            fs::write(&test_file, "test content").expect("Failed to write test file");
        } else {
            // Modify the file
            let content = fs::read_to_string(&test_file).unwrap_or_default();
            fs::write(&test_file, format!("{}\nmodified", content))
                .expect("Failed to modify test file");
        }

        self.git(&["add", "."]);
        self.git(&["commit", "-m", message]);
    }

    /// Create a branch
    pub fn create_branch(&self, name: &str) {
        self.git(&["branch", name]);
    }

    /// Run a git command in the test repository
    pub fn git(&self, args: &[&str]) -> Output {
        Command::new("git")
            .args(args)
            .current_dir(&self.repo_path)
            .output()
            .expect("Failed to run git command")
    }

    /// Run agentree command in the test repository
    pub fn agentree(&self, args: &[&str]) -> Output {
        let binary_path = self.get_agentree_binary();

        Command::new(&binary_path)
            .args(args)
            .current_dir(&self.repo_path)
            .output()
            .expect("Failed to run agentree command")
    }

    /// Get path to the agentree binary
    fn get_agentree_binary(&self) -> PathBuf {
        // Look for binary in target/debug or target/release
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let debug_binary = Path::new(manifest_dir).join("target/debug/agentree");
        let release_binary = Path::new(manifest_dir).join("target/release/agentree");

        if release_binary.exists() {
            release_binary
        } else if debug_binary.exists() {
            debug_binary
        } else {
            panic!("agentree binary not found. Run 'cargo build' first.");
        }
    }

    /// Get the repository path
    pub fn path(&self) -> &Path {
        &self.repo_path
    }
}
