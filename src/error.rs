use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentreeError {
    #[error("Git error: {0}")]
    Git(String),

    #[error("Config parse error: {0}")]
    ConfigParse(#[from] toml::de::Error),

    #[error("Failed to load config from '{path}': {error}")]
    ConfigLoad { path: String, error: String },

    #[error("Configuration error: {0}")]
    ConfigError(String),

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

    #[error(
        "No worktree found for branch '{branch}'.\nUse `agentree list` to see available worktrees."
    )]
    WorktreeNotFound { branch: String },

    #[error("Worktree path escapes base directory: {path}\nThis is a security risk. Check your configuration.")]
    WorktreePathTraversal { path: String },

    #[error("Branch '{branch}' does not exist.\nUse `git branch -a` to see available branches or `agentree create {branch}` to create a new worktree.")]
    BranchNotFound { branch: String },

    #[error("Branch '{branch}' not found. Did you mean: {suggestions}?\nUse `git branch -a` to see available branches.")]
    BranchNotFoundWithSuggestions { branch: String, suggestions: String },

    #[error("Workspace path '{}' is not accessible: {reason}\n{hint}", path.display())]
    PathNotAccessible {
        path: std::path::PathBuf,
        reason: String,
        hint: String,
    },

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

    #[error("Update failed: {0}")]
    UpdateError(String),

    #[error(
        "Permission denied: {0}\nHint: Try running with sudo if binary is in system directory"
    )]
    PermissionDenied(String),

    #[error("Failed to check for updates: {0}")]
    UpdateCheckFailed(String),

    #[error("Daemon error: {0}")]
    DaemonError(String),

    #[error("Docker daemon is not running. Please start Docker Desktop and try again.\nCheck status with: docker info")]
    DockerNotRunning,

    #[error("Docker version {current} does not support sandboxes.\nMinimum required: Engine {minimum_engine} / Desktop {minimum_desktop}\nUpdate Docker Desktop: https://www.docker.com/products/docker-desktop/")]
    DockerSandboxNotSupported {
        current: String,
        minimum_engine: String,
        minimum_desktop: String,
    },

    #[error("Docker Sandboxes are not supported on Linux.\nDocker Sandboxes use microVMs which require macOS or Windows.\nConsider using the 'claude-vm' backend instead for VM isolation on Linux.")]
    DockerSandboxLinuxNotSupported,

    #[error("Docker sandbox '{name}' not found.\nThe sandbox may have been removed outside of agentree.\nTry running: agentree remove {branch} && agentree create {branch}")]
    SandboxNotFound { name: String, branch: String },

    #[error("tmux is not installed. Install it with: brew install tmux (macOS) or apt install tmux (Linux)")]
    TmuxNotFound,

    #[error(
        "Daemon is not running and could not be started.\nTry running `agentree daemon` manually."
    )]
    DaemonNotRunning,

    #[error("Daemon failed to start. Check logs: {log_path}")]
    DaemonStartFailed { log_path: String },

    #[error("tmux error: {0}")]
    TmuxError(String),
}

pub type Result<T> = std::result::Result<T, AgentreeError>;
