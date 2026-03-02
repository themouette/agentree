use crate::error::{AgentreeError, Result};
use std::path::Path;

use super::{inject_agentree_block, remove_agentree_block, Agent, AGENTREE_HOOK_MARKER};

/// Token returned by `OpencodeAgent::prepare`.
///
/// Tracks whether `opencode.json` was created by `prepare` (did not exist before)
/// so that `cleanup` can remove it if it is now empty after our hooks are removed.
pub struct OpencodeToken {
    pub(crate) opencode_json_created: bool,
}

/// Agent implementation for OpenCode.
///
/// `prepare` injects an `<!-- agentree:start -->` block into `AGENTS.md`
/// (OpenCode's workspace context file), injects a `session_completed` hook into
/// `opencode.json` at the workspace root that writes `{"phase":"done"}` to
/// `.agentree/status.json` when the session ends, and writes `"OpenCode running"`
/// to `.agentree/attention.md` as a guaranteed dashboard signal.
/// `cleanup` reverts all of those changes.
pub struct OpencodeAgent;

impl OpencodeAgent {
    pub fn new() -> Self {
        OpencodeAgent
    }
}

impl Default for OpencodeAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl Agent for OpencodeAgent {
    type PrepareToken = OpencodeToken;

    fn prepare(&self, workspace_path: &Path) -> Result<Self::PrepareToken> {
        let config_path = workspace_path.join("opencode.json");
        let opencode_json_created = !config_path.exists();

        // 1. Inject agentree block into AGENTS.md (idempotent)
        inject_agentree_block(
            &workspace_path.join("AGENTS.md"),
            include_str!("../../templates/AGENTS.md"),
        )?;

        // 2. Inject session_completed hook into opencode.json
        ensure_agentree_hooks(&config_path, workspace_path)?;

        // 3. Write guaranteed attention signal so the dashboard shows the workspace as active.
        //    OpenCode has no PreToolUse/UserPromptSubmit equivalent in config hooks,
        //    so we use a coarse-grained "running" signal here and rely on cleanup()
        //    to clear it when the session ends.
        let agentree_dir = workspace_path.join(".agentree");
        std::fs::create_dir_all(&agentree_dir).map_err(AgentreeError::Io)?;
        std::fs::write(agentree_dir.join("attention.md"), "OpenCode running\n")
            .map_err(AgentreeError::Io)?;

        Ok(OpencodeToken {
            opencode_json_created,
        })
    }

    fn cleanup(&self, workspace_path: &Path, token: &Self::PrepareToken) {
        // 1. Remove agentree block from AGENTS.md (delete file if now empty)
        remove_agentree_block(&workspace_path.join("AGENTS.md"));

        // 2. Remove agentree hooks from opencode.json
        let config_path = workspace_path.join("opencode.json");
        remove_agentree_hooks(&config_path);

        let agentree_dir = workspace_path.join(".agentree");

        // 3. Write status.json with phase:done so the dashboard knows the session ended.
        //    Best-effort: create the directory if needed, then write.
        let _ = std::fs::create_dir_all(&agentree_dir);
        let _ = std::fs::write(agentree_dir.join("status.json"), "{\"phase\":\"done\"}\n");

        // 4. Clear attention.md (best-effort)
        let _ = std::fs::remove_file(agentree_dir.join("attention.md"));

        // 5. If we created opencode.json and it is now {} (empty object), delete it
        if token.opencode_json_created && config_path.exists() {
            if let Ok(raw) = std::fs::read_to_string(&config_path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                    if v.as_object().map(|o| o.is_empty()).unwrap_or(false) {
                        let _ = std::fs::remove_file(&config_path);
                    }
                }
            }
        }
    }

    fn name(&self) -> &str {
        "opencode"
    }
}

// ─── Hook injection / removal ────────────────────────────────────────────────

/// Returns `true` if any entry in `arr` (the `session_completed` array) contains
/// [`AGENTREE_HOOK_MARKER`] in its `command` array.
///
/// OpenCode experimental hook format uses `"command": ["sh", "-c", "..."]` (array),
/// unlike Claude Code which uses a plain string. We must check `command.as_array()`.
fn array_has_agentree_marker(arr: &[serde_json::Value]) -> bool {
    arr.iter().any(|entry| {
        entry
            .get("command")
            .and_then(|c| c.as_array())
            .map(|cmd| {
                cmd.iter().any(|part| {
                    part.as_str()
                        .map(|s| s.contains(AGENTREE_HOOK_MARKER))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    })
}

/// Ensure `opencode.json` contains an agentree-owned `session_completed` hook.
///
/// OpenCode experimental hook format (project root `opencode.json`):
/// ```json
/// {
///   "experimental": {
///     "hook": {
///       "session_completed": [
///         {
///           "command": ["sh", "-c", "...shell command... # agentree-hook"],
///           "environment": {}
///         }
///       ]
///     }
///   }
/// }
/// ```
///
/// Idempotent: existing agentree entries (identified by [`AGENTREE_HOOK_MARKER`])
/// are not duplicated. Pre-existing user hooks are left untouched.
fn ensure_agentree_hooks(config_path: &Path, worktree_path: &Path) -> Result<()> {
    let abs = worktree_path
        .canonicalize()
        .unwrap_or_else(|_| worktree_path.to_path_buf());
    let abs_agentree = abs.join(".agentree").to_string_lossy().into_owned();

    let stop_cmd = format!(
        "mkdir -p \"{dir}\" && printf '{{\"phase\":\"done\"}}\\n' > \"{dir}/status.json\" {marker}",
        dir = abs_agentree,
        marker = AGENTREE_HOOK_MARKER
    );

    // Parse existing file or start from an empty object
    let mut value: serde_json::Value = if config_path.exists() {
        let raw = std::fs::read_to_string(config_path).map_err(AgentreeError::Io)?;
        serde_json::from_str(&raw).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // Navigate: value["experimental"]["hook"]["session_completed"] -> array
    let obj = value.as_object_mut().ok_or_else(|| {
        AgentreeError::ConfigError("opencode.json root is not an object".into())
    })?;

    let experimental = obj
        .entry("experimental")
        .or_insert(serde_json::json!({}));
    let hook = experimental
        .as_object_mut()
        .ok_or_else(|| AgentreeError::ConfigError("experimental is not an object".into()))?
        .entry("hook")
        .or_insert(serde_json::json!({}));
    let session_completed = hook
        .as_object_mut()
        .ok_or_else(|| AgentreeError::ConfigError("hook is not an object".into()))?
        .entry("session_completed")
        .or_insert(serde_json::json!([]))
        .as_array_mut()
        .ok_or_else(|| {
            AgentreeError::ConfigError("session_completed is not an array".into())
        })?;

    // Idempotency: skip if agentree entry already present
    if !array_has_agentree_marker(session_completed) {
        session_completed.push(serde_json::json!({
            "command": ["sh", "-c", stop_cmd],
            "environment": {}
        }));
    }

    let updated = serde_json::to_string_pretty(&value)?;
    std::fs::write(config_path, updated + "\n").map_err(AgentreeError::Io)?;
    Ok(())
}

/// Remove agentree-owned entries from the `session_completed` hook array in `opencode.json`.
///
/// Only entries whose `command` array contains [`AGENTREE_HOOK_MARKER`] are removed.
/// User-defined hooks are never touched.
///
/// Non-fatal: all errors are silently ignored.
/// Cleans up empty arrays and empty intermediate objects. Deletes the file if it
/// would become `{}` after removal.
fn remove_agentree_hooks(config_path: &Path) {
    if !config_path.exists() {
        return;
    }
    let raw = match std::fs::read_to_string(config_path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut value: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return,
    };

    let obj = match value.as_object_mut() {
        Some(o) => o,
        None => return,
    };

    // Navigate and clean up session_completed
    let experimental_empty;
    {
        let experimental = match obj.get_mut("experimental").and_then(|v| v.as_object_mut()) {
            Some(e) => e,
            None => return,
        };

        let hook_empty;
        {
            let hook = match experimental
                .get_mut("hook")
                .and_then(|v| v.as_object_mut())
            {
                Some(h) => h,
                None => return,
            };

            if let Some(sc) = hook
                .get_mut("session_completed")
                .and_then(|v| v.as_array_mut())
            {
                sc.retain(|entry| {
                    !entry
                        .get("command")
                        .and_then(|c| c.as_array())
                        .map(|arr| {
                            arr.iter().any(|p| {
                                p.as_str()
                                    .map(|s| s.contains(AGENTREE_HOOK_MARKER))
                                    .unwrap_or(false)
                            })
                        })
                        .unwrap_or(false)
                });

                if sc.is_empty() {
                    hook.remove("session_completed");
                }
            }

            hook_empty = hook.is_empty();
        }

        if hook_empty {
            experimental.remove("hook");
        }

        experimental_empty = experimental.is_empty();
    }

    if experimental_empty {
        obj.remove("experimental");
    }

    if obj.is_empty() {
        let _ = std::fs::remove_file(config_path);
    } else if let Ok(updated) = serde_json::to_string_pretty(&value) {
        let _ = std::fs::write(config_path, updated + "\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{AGENTREE_END, AGENTREE_START};

    #[test]
    fn test_prepare_creates_files_in_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = OpencodeAgent::new();

        let _token = agent.prepare(path).unwrap();

        // AGENTS.md created with markers and template content
        let agents_md = std::fs::read_to_string(path.join("AGENTS.md")).unwrap();
        assert!(agents_md.contains(AGENTREE_START));
        assert!(agents_md.contains(AGENTREE_END));
        assert!(agents_md.contains("Agentree Status Protocol"));

        // opencode.json created at workspace root (NOT in .opencode/)
        assert!(path.join("opencode.json").exists());
        assert!(!path.join(".opencode").join("opencode.json").exists());

        // .agentree/attention.md written with "OpenCode running"
        let attention = std::fs::read_to_string(path.join(".agentree").join("attention.md")).unwrap();
        assert!(attention.contains("OpenCode running"));
    }

    #[test]
    fn test_prepare_injects_hook_into_opencode_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = OpencodeAgent::new();

        agent.prepare(path).unwrap();

        let raw = std::fs::read_to_string(path.join("opencode.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();

        // experimental.hook.session_completed must exist as an array
        let sc = value["experimental"]["hook"]["session_completed"]
            .as_array()
            .expect("session_completed should be an array");

        // At least one entry must have the agentree marker in its command array
        let has_agentree = array_has_agentree_marker(sc);
        assert!(has_agentree, "no agentree entry found in session_completed");

        // The command entry containing the marker must write status.json
        let agentree_entry = sc
            .iter()
            .find(|e| {
                e.get("command")
                    .and_then(|c| c.as_array())
                    .map(|arr| {
                        arr.iter()
                            .any(|p| p.as_str().map(|s| s.contains(AGENTREE_HOOK_MARKER)).unwrap_or(false))
                    })
                    .unwrap_or(false)
            })
            .expect("agentree entry not found");

        // command must be an array ["sh", "-c", "..."]
        let cmd_arr = agentree_entry["command"]
            .as_array()
            .expect("command should be an array");
        assert_eq!(cmd_arr[0], "sh");
        assert_eq!(cmd_arr[1], "-c");

        // The shell command string must mention status.json
        let shell_cmd = cmd_arr[2].as_str().unwrap();
        assert!(shell_cmd.contains("status.json"), "hook command should write status.json");
    }

    #[test]
    fn test_prepare_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = OpencodeAgent::new();

        agent.prepare(path).unwrap();
        agent.prepare(path).unwrap(); // second call must not duplicate

        // AGENTS.md marker appears exactly once
        let content = std::fs::read_to_string(path.join("AGENTS.md")).unwrap();
        assert_eq!(content.matches(AGENTREE_START).count(), 1);

        // session_completed has exactly one agentree entry
        let raw = std::fs::read_to_string(path.join("opencode.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let sc = value["experimental"]["hook"]["session_completed"]
            .as_array()
            .expect("session_completed should be an array");
        let agentree_count = sc
            .iter()
            .filter(|e| {
                e.get("command")
                    .and_then(|c| c.as_array())
                    .map(|arr| {
                        arr.iter()
                            .any(|p| p.as_str().map(|s| s.contains(AGENTREE_HOOK_MARKER)).unwrap_or(false))
                    })
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(agentree_count, 1, "expected exactly one agentree entry in session_completed, found {agentree_count}");
    }

    #[test]
    fn test_cleanup_removes_files_when_no_other_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = OpencodeAgent::new();

        let token = agent.prepare(path).unwrap();
        agent.cleanup(path, &token);

        // AGENTS.md gone (only contained the agentree block)
        assert!(!path.join("AGENTS.md").exists());
        // opencode.json gone (we created it and it is now {})
        assert!(!path.join("opencode.json").exists());
        // attention.md gone
        assert!(!path.join(".agentree").join("attention.md").exists());
    }

    #[test]
    fn test_cleanup_writes_status_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = OpencodeAgent::new();

        let token = agent.prepare(path).unwrap();
        agent.cleanup(path, &token);

        // .agentree/status.json must exist with {"phase":"done"}
        let raw = std::fs::read_to_string(path.join(".agentree").join("status.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["phase"], "done");
    }

    #[test]
    fn test_cleanup_preserves_existing_agents_md_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = OpencodeAgent::new();

        // Pre-existing project AGENTS.md
        let existing = "# My Project\n\nSome project documentation.\n";
        std::fs::write(path.join("AGENTS.md"), existing).unwrap();

        let token = agent.prepare(path).unwrap();

        // After prepare, both existing content and agentree block are present
        let content = std::fs::read_to_string(path.join("AGENTS.md")).unwrap();
        assert!(content.contains("# My Project"));
        assert!(content.contains(AGENTREE_START));

        agent.cleanup(path, &token);

        // After cleanup, only the original content remains
        let content = std::fs::read_to_string(path.join("AGENTS.md")).unwrap();
        assert!(content.contains("# My Project"));
        assert!(!content.contains(AGENTREE_START));
        assert!(!content.contains(AGENTREE_END));
    }

    #[test]
    fn test_cleanup_preserves_existing_opencode_json_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = OpencodeAgent::new();

        // Pre-existing opencode.json with a user key
        let existing = r#"{"myProjectKey": "someValue"}"#;
        std::fs::write(path.join("opencode.json"), existing).unwrap();

        let token = agent.prepare(path).unwrap();
        // opencode.json was NOT created by us
        assert!(!token.opencode_json_created);

        agent.cleanup(path, &token);

        // opencode.json still exists (user key preserved)
        let raw = std::fs::read_to_string(path.join("opencode.json"))
            .expect("opencode.json should still exist after cleanup");
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["myProjectKey"], "someValue");
        // agentree experimental section should be gone
        assert!(value.get("experimental").is_none());
    }

    #[test]
    fn test_cleanup_preserves_user_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = OpencodeAgent::new();

        // Pre-existing opencode.json with a user-defined session_completed hook
        let existing = serde_json::json!({
            "experimental": {
                "hook": {
                    "session_completed": [
                        {
                            "command": ["notify-send", "OpenCode done"],
                            "environment": {}
                        }
                    ]
                }
            }
        });
        std::fs::write(path.join("opencode.json"), serde_json::to_string(&existing).unwrap()).unwrap();

        let token = agent.prepare(path).unwrap();
        agent.cleanup(path, &token);

        // opencode.json still exists (user hook preserved)
        let raw = std::fs::read_to_string(path.join("opencode.json"))
            .expect("opencode.json should still exist after cleanup");
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();

        let sc = value["experimental"]["hook"]["session_completed"]
            .as_array()
            .expect("session_completed should still exist");

        // User hook preserved
        let user_present = sc.iter().any(|e| {
            e.get("command")
                .and_then(|c| c.as_array())
                .map(|arr| arr.iter().any(|p| p.as_str() == Some("notify-send")))
                .unwrap_or(false)
        });
        assert!(user_present, "user session_completed hook should be preserved");

        // Agentree hook removed
        let agentree_present = array_has_agentree_marker(sc);
        assert!(!agentree_present, "agentree hook should be removed after cleanup");
    }

    #[test]
    fn test_hook_uses_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = OpencodeAgent::new();

        agent.prepare(path).unwrap();

        let raw = std::fs::read_to_string(path.join("opencode.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let sc = value["experimental"]["hook"]["session_completed"]
            .as_array()
            .unwrap();

        // Find the agentree entry
        let agentree_entry = sc
            .iter()
            .find(|e| {
                e.get("command")
                    .and_then(|c| c.as_array())
                    .map(|arr| {
                        arr.iter()
                            .any(|p| p.as_str().map(|s| s.contains(AGENTREE_HOOK_MARKER)).unwrap_or(false))
                    })
                    .unwrap_or(false)
            })
            .expect("agentree entry not found");

        let shell_cmd = agentree_entry["command"][2].as_str().unwrap();

        // Must contain the absolute path of the temp dir
        let abs_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let abs_str = abs_path.to_string_lossy();
        assert!(
            shell_cmd.contains(abs_str.as_ref()),
            "hook command should contain absolute path '{}', got: {}",
            abs_str,
            shell_cmd
        );

        // Must not use bare ".agentree" relative path
        assert!(
            !shell_cmd.contains(" .agentree/") && !shell_cmd.contains("\"./agentree"),
            "hook command should not use relative .agentree path"
        );
    }
}
