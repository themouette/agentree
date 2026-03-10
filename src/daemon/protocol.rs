use serde::{Deserialize, Serialize};

/// Status written by an agent into `{worktree}/.agentree/status.json`
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(default)]
pub struct AgentStatus {
    pub phase: Option<String>,
    pub current_task: Option<String>,
    // last_activity intentionally absent — daemon derives from git log timestamp
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
    /// Resolved agent binary to use for this workspace (from config, or None)
    pub agent_bin: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_list_round_trip() {
        let req = Request::List;
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"cmd\":\"list\""));
        let back: Request = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, Request::List));
    }

    #[test]
    fn request_clear_attention_round_trip() {
        let req = Request::ClearAttention {
            branch: "feature/auth".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("clear_attention"));
        assert!(json.contains("feature/auth"));
        let back: Request = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, Request::ClearAttention { branch } if branch == "feature/auth"));
    }

    #[test]
    fn response_ok_round_trip() {
        let resp = Response::Ok;
        let json = serde_json::to_string(&resp).unwrap();
        let back: Response = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, Response::Ok));
    }

    #[test]
    fn response_err_round_trip() {
        let resp = Response::Err("something went wrong".to_string());
        let json = serde_json::to_string(&resp).unwrap();
        let back: Response = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, Response::Err(msg) if msg == "something went wrong"));
    }

    #[test]
    fn response_workspaces_round_trip() {
        let info = WorkspaceInfo {
            branch: "main".to_string(),
            path: "/repo".to_string(),
            agent_status: None,
            attention: None,
            commits_ahead: 3,
            files_changed: 1,
            last_activity: Some("2024-01-01T00:00:00Z".to_string()),
            agent_bin: None,
        };
        let resp = Response::Workspaces(vec![info]);
        let json = serde_json::to_string(&resp).unwrap();
        let back: Response = serde_json::from_str(&json).unwrap();
        match back {
            Response::Workspaces(ws) => {
                assert_eq!(ws.len(), 1);
                assert_eq!(ws[0].branch, "main");
                assert_eq!(ws[0].commits_ahead, 3);
            }
            _ => panic!("expected Workspaces response"),
        }
    }
}
