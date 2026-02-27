use crate::daemon::protocol::{AgentStatus, WorkspaceInfo};
use crate::error::{AgentreeError, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct DaemonState {
    pub workspaces: Arc<Mutex<HashMap<String, WorkspaceInfo>>>,
    pub repo_root: PathBuf,
}

impl DaemonState {
    pub fn new(repo_root: PathBuf) -> Self {
        DaemonState {
            workspaces: Arc::new(Mutex::new(HashMap::new())),
            repo_root,
        }
    }

    /// Rebuild workspace map from git worktree list
    pub fn refresh_all(&self) -> Result<()> {
        let worktrees = list_worktrees_for_repo(&self.repo_root);

        let mut map = self
            .workspaces
            .lock()
            .map_err(|e| AgentreeError::Git(format!("State lock poisoned: {}", e)))?;
        map.clear();

        for (branch, path) in worktrees {
            let info = build_workspace_info(&branch, &path);
            map.insert(branch, info);
        }

        Ok(())
    }

    /// Refresh a single workspace (called by file watcher on change)
    pub fn update_workspace(&self, branch: &str) {
        let path = {
            let map = match self.workspaces.lock() {
                Ok(m) => m,
                Err(_) => return,
            };
            map.get(branch).map(|w| PathBuf::from(&w.path))
        };

        if let Some(path) = path {
            let info = build_workspace_info(branch, &path);
            if let Ok(mut map) = self.workspaces.lock() {
                map.insert(branch.to_string(), info);
            }
        } else {
            // New workspace detected — re-scan all
            let _ = self.refresh_all();
        }
    }

    /// Remove `.agentree/attention.md` from the workspace
    pub fn clear_attention(&self, branch: &str) -> Result<()> {
        let path = {
            let map = self
                .workspaces
                .lock()
                .map_err(|e| AgentreeError::Git(format!("State lock poisoned: {}", e)))?;
            map.get(branch).map(|w| PathBuf::from(&w.path))
        };

        if let Some(path) = path {
            let attention_file = path.join(".agentree").join("attention.md");
            if attention_file.exists() {
                std::fs::remove_file(&attention_file).map_err(AgentreeError::Io)?;
            }
            // Update cached info
            let mut info = build_workspace_info(branch, &path);
            info.attention = None;
            if let Ok(mut map) = self.workspaces.lock() {
                map.insert(branch.to_string(), info);
            }
        }

        Ok(())
    }

    /// Return a sorted snapshot of all workspace states
    pub fn snapshot(&self) -> Vec<WorkspaceInfo> {
        let map = match self.workspaces.lock() {
            Ok(m) => m,
            Err(_) => return vec![],
        };
        let mut workspaces: Vec<WorkspaceInfo> = map.values().cloned().collect();
        workspaces.sort_by(|a, b| a.branch.cmp(&b.branch));
        workspaces
    }

    /// Given a file path that changed, find which branch owns it
    pub fn find_branch_for_path(&self, changed_path: &Path) -> Option<String> {
        let map = self.workspaces.lock().ok()?;
        for (branch, info) in map.iter() {
            let workspace_path = PathBuf::from(&info.path);
            if changed_path.starts_with(&workspace_path) {
                return Some(branch.clone());
            }
        }
        None
    }

    /// Return (branch, .agentree/ path) for all known workspaces
    pub fn get_all_agentree_paths(&self) -> Vec<(String, PathBuf)> {
        let map = match self.workspaces.lock() {
            Ok(m) => m,
            Err(_) => return vec![],
        };
        map.iter()
            .map(|(branch, info)| {
                (
                    branch.clone(),
                    PathBuf::from(&info.path).join(".agentree"),
                )
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Git helpers (use -C flag so no current-dir dependency)
// ---------------------------------------------------------------------------

fn list_worktrees_for_repo(repo_root: &Path) -> Vec<(String, PathBuf)> {
    let root_str = repo_root.to_string_lossy();
    let output = std::process::Command::new("git")
        .args(["-C", &root_str, "worktree", "list", "--porcelain"])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            parse_worktree_porcelain(&text)
        }
        _ => vec![],
    }
}

fn parse_worktree_porcelain(output: &str) -> Vec<(String, PathBuf)> {
    let mut result = Vec::new();
    let mut skip_first = true;

    for block in output.split("\n\n") {
        if block.trim().is_empty() {
            continue;
        }

        if skip_first {
            // First block is always the main repo — skip it
            skip_first = false;
            continue;
        }

        let mut path: Option<PathBuf> = None;
        let mut branch: Option<String> = None;

        for line in block.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                path = Some(PathBuf::from(p));
            } else if let Some(b) = line.strip_prefix("branch ") {
                let name = b.strip_prefix("refs/heads/").unwrap_or(b);
                branch = Some(name.to_string());
            }
        }

        if let (Some(p), Some(b)) = (path, branch) {
            result.push((b, p));
        }
    }

    result
}

fn build_workspace_info(branch: &str, path: &Path) -> WorkspaceInfo {
    WorkspaceInfo {
        branch: branch.to_string(),
        path: path.to_string_lossy().to_string(),
        agent_status: read_agent_status(path),
        attention: read_attention(path),
        commits_ahead: get_commits_ahead(path),
        files_changed: get_files_changed(path),
        last_activity: get_last_activity(path),
    }
}

fn read_agent_status(path: &Path) -> Option<AgentStatus> {
    let status_file = path.join(".agentree").join("status.json");
    let content = std::fs::read_to_string(status_file).ok()?;
    serde_json::from_str(&content).ok()
}

fn read_attention(path: &Path) -> Option<String> {
    let attention_file = path.join(".agentree").join("attention.md");
    let content = std::fs::read_to_string(attention_file).ok()?;
    if content.trim().is_empty() {
        None
    } else {
        Some(content)
    }
}

fn get_commits_ahead(path: &Path) -> u32 {
    let path_str = path.to_string_lossy();
    // @{u} refers to the upstream tracking branch
    let output = std::process::Command::new("git")
        .args(["-C", &path_str, "rev-list", "--count", "HEAD", "^@{u}"])
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .trim()
            .parse()
            .unwrap_or(0),
        _ => 0,
    }
}

fn get_files_changed(path: &Path) -> u32 {
    let path_str = path.to_string_lossy();
    let output = std::process::Command::new("git")
        .args(["-C", &path_str, "diff", "--shortstat"])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            let text = text.trim();
            if text.is_empty() {
                return 0;
            }
            // Format: "N file(s) changed, ..."
            text.split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0)
        }
        _ => 0,
    }
}

fn get_last_activity(path: &Path) -> Option<String> {
    let path_str = path.to_string_lossy();
    let output = std::process::Command::new("git")
        .args(["-C", &path_str, "log", "-1", "--format=%cI"])
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}
