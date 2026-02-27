use serde::{Deserialize, Serialize};

/// Status written by an agent into `{worktree}/.agentree/status.json`
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct AgentStatus {
    pub phase: String,
    pub current_task: Option<String>,
    /// RFC3339 timestamp of last agent activity
    pub last_activity: Option<String>,
}

/// Request sent from dashboard client to daemon over Unix socket (newline-delimited JSON)
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "cmd")]
pub enum Request {
    #[serde(rename = "list")]
    List,
    #[serde(rename = "clear_attention")]
    ClearAttention { branch: String },
}

/// Response sent from daemon to dashboard client (newline-delimited JSON)
#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    Workspaces(Vec<WorkspaceInfo>),
    Ok,
    Err(String),
}

/// Snapshot of a single workspace's state
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkspaceInfo {
    pub branch: String,
    pub path: String,
    /// Parsed from `.agentree/status.json`, None if file missing or unreadable
    pub agent_status: Option<AgentStatus>,
    /// Contents of `.agentree/attention.md`, None if file missing or empty
    pub attention: Option<String>,
    /// Commits ahead of upstream (0 if no upstream or git query fails)
    pub commits_ahead: u32,
    /// Files with uncommitted changes (0 if clean or query fails)
    pub files_changed: u32,
    /// RFC3339 timestamp of last git commit in the workspace
    pub last_activity: Option<String>,
}
