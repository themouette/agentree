use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentreeError {
    #[error("Git error: {0}")]
    Git(String),

    #[error("Config parse error: {0}")]
    ConfigParse(#[from] toml::de::Error),

    #[error("Failed to load config from '{path}': {error}")]
    ConfigLoad { path: String, error: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Git worktree is locked: {reason}\nTo unlock, run: git worktree unlock {path}")]
    WorktreeLocked { reason: String, path: String },

    #[error("Git version {version} is too old. Worktrees require Git 2.5+.\nDownload the latest version: https://git-scm.com/downloads")]
    GitVersionTooOld { version: String },

    #[error("Repository uses submodules. Git worktree support for submodules is experimental.\nSee: https://git-scm.com/docs/git-worktree#_bugs")]
    SubmodulesDetected,

    #[error("Git worktree error: {0}")]
    Worktree(String),

    #[error("No worktree found for branch '{branch}'.\nUse `agentree list` to see available worktrees.")]
    WorktreeNotFound { branch: String },

    #[error("Worktree path escapes base directory: {path}\nThis is a security risk. Check your configuration.")]
    WorktreePathTraversal { path: String },

    #[error("Branch '{branch}' does not exist.\nUse `git branch -a` to see available branches or `agentree create {branch}` to create a new worktree.")]
    BranchNotFound { branch: String },

    #[error("Backend '{backend}' not found. Install it using:\n  {install_instructions}")]
    BackendBinaryNotFound {
        backend: String,
        binary: String,
        install_instructions: String,
    },

    #[error("Backend '{backend}' version {current} is too old.\nMinimum required: {minimum}\nUpdate using:\n  {update_instructions}")]
    BackendVersionTooOld {
        backend: String,
        current: String,
        minimum: String,
        update_instructions: String,
    },

    #[error("Backend '{backend}' execution failed: {error}")]
    BackendExecution { backend: String, error: String },

    #[error("Backend '{backend}' exited with code {exit_code:?}")]
    BackendFailed {
        backend: String,
        exit_code: Option<i32>,
    },

    #[error("Unknown backend '{name}'. Available backends: {available:?}")]
    BackendNotFound {
        name: String,
        available: Vec<String>,
    },

    #[error("Failed to parse version '{version}': {error}")]
    VersionParse { version: String, error: String },
}

pub type Result<T> = std::result::Result<T, AgentreeError>;
