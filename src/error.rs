use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentreeError {
    #[error("Git error: {0}")]
    Git(String),

    #[error("Config parse error: {0}")]
    ConfigParse(#[from] toml::de::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

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
}

pub type Result<T> = std::result::Result<T, AgentreeError>;
