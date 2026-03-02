use crate::error::{AgentreeError, Result};
use std::path::Path;

use super::{inject_agentree_block, remove_agentree_block, Agent, AGENTREE_HOOK_MARKER};

/// Token returned by `ClaudeAgent::prepare`.
///
/// Tracks whether `.claude/` was created by `prepare` so that
/// `cleanup` can remove it if it is now empty.
pub struct ClaudeToken {
    pub(crate) claude_dir_created: bool,
}

/// Agent implementation for Claude.
///
/// `prepare` injects an `<!-- agentree:start -->` block into `CLAUDE.md`,
/// merges `allowedTools` entries into `.claude/settings.json`, and injects
/// four Claude Code hook configurations (`PreToolUse`, `PostToolUse`,
/// `UserPromptSubmit`, `Stop`) that automate the `.agentree/attention.md`
/// lifecycle.  Any stale `attention.md` from the prior session is also cleared.
/// `cleanup` reverts all of those changes.
pub struct ClaudeAgent;

impl ClaudeAgent {
    pub fn new() -> Self {
        ClaudeAgent
    }
}

impl Default for ClaudeAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl Agent for ClaudeAgent {
    type PrepareToken = ClaudeToken;

    fn prepare(&self, workspace_path: &Path) -> Result<Self::PrepareToken> {
        let claude_dir = workspace_path.join(".claude");
        let claude_dir_created = !claude_dir.exists();
        if claude_dir_created {
            std::fs::create_dir_all(&claude_dir).map_err(AgentreeError::Io)?;
        }

        // Inject agentree block into CLAUDE.md (idempotent)
        inject_agentree_block(
            &workspace_path.join("CLAUDE.md"),
            include_str!("../../templates/CLAUDE.md"),
        )?;

        // Merge our allowedTools entries into settings.json
        let settings_path = claude_dir.join("settings.json");
        ensure_agentree_allowed_tools(&settings_path, workspace_path)?;

        // Inject hook entries into settings.json (PreToolUse, PostToolUse,
        // UserPromptSubmit, Stop) — automates attention.md lifecycle
        ensure_agentree_hooks(&settings_path, workspace_path)?;

        // Clear stale attention.md from prior session (Stop hook may have set it;
        // new session starts fresh so the dashboard shows no stale request)
        let attention_path = workspace_path.join(".agentree").join("attention.md");
        if attention_path.exists() {
            let _ = std::fs::remove_file(&attention_path);
        }

        Ok(ClaudeToken { claude_dir_created })
    }

    fn cleanup(&self, workspace_path: &Path, token: &Self::PrepareToken) {
        // 1. Remove agentree block from CLAUDE.md (delete file if now empty)
        remove_agentree_block(&workspace_path.join("CLAUDE.md"));

        let claude_dir = workspace_path.join(".claude");

        // 2. Remove our entries from settings.json (delete file if now empty)
        remove_agentree_allowed_tools(&claude_dir.join("settings.json"), workspace_path);

        // 3. Remove agentree hook entries from settings.json
        remove_agentree_hooks(&claude_dir.join("settings.json"));

        // 4. Remove .claude/ if we created it and it is now empty
        if token.claude_dir_created {
            if let Ok(mut entries) = std::fs::read_dir(&claude_dir) {
                if entries.next().is_none() {
                    let _ = std::fs::remove_dir(&claude_dir);
                }
            }
        }
    }

    fn name(&self) -> &str {
        "claude"
    }
}

// ─── Hook command builders ────────────────────────────────────────────────────

/// Build the shell command for the `PreToolUse` hook.
///
/// The command:
/// 1. Saves stdin (tool JSON from Claude Code) to `$INPUT`
/// 2. Extracts the tool name via `jq` (falls back to `"tool"` if jq is absent)
/// 3. Extracts a brief input summary (command / path / file_path), truncated to 200 chars
/// 4. Writes `attention.md`: first line `"Waiting for approval: <TOOL>"`, optional second
///    line with the input summary
/// 5. Always exits 0 — PreToolUse hooks must never block legitimate tool calls
///
/// The absolute `.agentree/` path is baked in so the hook works regardless of
/// the working directory Claude Code uses when executing it.
fn build_pretooluse_command(abs_agentree: &str) -> String {
    // `abs_agentree` is wrapped in double-quotes in the shell command.
    // This is safe for paths with spaces, but would break for paths containing
    // a literal `"` character.  Such paths are valid on Linux/macOS but
    // vanishingly rare in practice; we accept the trade-off rather than
    // adding a heavyweight escaping step.
    format!(
        "INPUT=$(cat); \
TOOL=$(printf '%s' \"$INPUT\" | jq -r '.tool_name // \"tool\"' 2>/dev/null || echo \"tool\"); \
CMD=$(printf '%s' \"$INPUT\" | jq -r '.tool_input.command // .tool_input.path // .tool_input.file_path // \"\"' 2>/dev/null | head -c 200); \
mkdir -p \"{attn_dir}\"; \
{{ printf 'Waiting for approval: %s\\n' \"$TOOL\"; [ -n \"$CMD\" ] && printf '%s\\n' \"$CMD\"; }} > \"{attn_dir}/attention.md\"; \
exit 0 {marker}",
        attn_dir = abs_agentree,
        marker = AGENTREE_HOOK_MARKER
    )
}

/// Build the shell command for the `Stop` hook.
///
/// Writes `status.json`: `{"phase":"done"}` to signal the dashboard that the
/// agent session has ended. Does not write `attention.md` — the session end
/// is a status update, not an attention request.
fn build_stop_command(abs_agentree: &str) -> String {
    // Same double-quote assumption as `build_pretooluse_command`.
    format!(
        "mkdir -p \"{attn_dir}\"; \
printf '{{\"phase\":\"done\"}}\\n' > \"{attn_dir}/status.json\" {marker}",
        attn_dir = abs_agentree,
        marker = AGENTREE_HOOK_MARKER
    )
}

// ─── Hook injection / removal ────────────────────────────────────────────────

/// Returns `true` if any hook entry in `group` contains [`AGENTREE_HOOK_MARKER`].
///
/// `group` is a single element of the per-event hook-group array in
/// `settings.json`, e.g.:
/// ```json
/// { "matcher": "", "hooks": [{ "type": "command", "command": "..." }] }
/// ```
fn group_has_agentree_marker(group: &serde_json::Value) -> bool {
    group
        .get("hooks")
        .and_then(|h| h.as_array())
        .map(|arr| {
            arr.iter().any(|h| {
                h.get("command")
                    .and_then(|c| c.as_str())
                    .map(|s| s.contains(AGENTREE_HOOK_MARKER))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Ensure `.claude/settings.json` contains agentree-owned hook groups for the
/// four lifecycle events used to automate `attention.md` management.
///
/// Claude Code hook format in `settings.json`:
/// ```json
/// {
///   "hooks": {
///     "PreToolUse":      [{ "matcher": "", "hooks": [{ "type": "command", "command": "..." }] }],
///     "PostToolUse":     [...],
///     "UserPromptSubmit":[...],
///     "Stop":            [...]
///   }
/// }
/// ```
///
/// Idempotent: existing agentree groups (identified by [`AGENTREE_HOOK_MARKER`])
/// are not duplicated.  Pre-existing user hooks are left untouched.
fn ensure_agentree_hooks(settings_path: &Path, worktree_path: &Path) -> Result<()> {
    let abs = worktree_path
        .canonicalize()
        .unwrap_or_else(|_| worktree_path.to_path_buf());
    let abs_agentree = abs.join(".agentree").to_string_lossy().into_owned();

    // Build command strings for each event
    let rm_cmd = format!(
        "rm -f \"{}/attention.md\" {}",
        abs_agentree, AGENTREE_HOOK_MARKER
    );
    let hooks_to_inject: &[(&str, String)] = &[
        ("PreToolUse", build_pretooluse_command(&abs_agentree)),
        ("PostToolUse", rm_cmd.clone()),
        ("PostToolUseFailure", rm_cmd.clone()),
        ("UserPromptSubmit", rm_cmd.clone()),
        ("Stop", build_stop_command(&abs_agentree)),
    ];

    // Parse existing file or start from an empty object
    let mut value: serde_json::Value = if settings_path.exists() {
        let raw = std::fs::read_to_string(settings_path).map_err(AgentreeError::Io)?;
        serde_json::from_str(&raw).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // Get or create `value["hooks"]` as a JSON object
    {
        let obj = value.as_object_mut().ok_or_else(|| {
            AgentreeError::ConfigError("settings.json root is not an object".into())
        })?;

        let hooks_obj = obj
            .entry("hooks")
            .or_insert(serde_json::json!({}))
            .as_object_mut()
            .ok_or_else(|| {
                AgentreeError::ConfigError("settings.json hooks is not an object".into())
            })?;

        for (event, command) in hooks_to_inject {
            let groups = hooks_obj
                .entry(*event)
                .or_insert(serde_json::json!([]))
                .as_array_mut()
                .ok_or_else(|| {
                    AgentreeError::ConfigError(format!("{event} hooks is not an array"))
                })?;

            // Idempotency: skip if an agentree group is already present
            let already_present = groups.iter().any(group_has_agentree_marker);
            if !already_present {
                groups.push(serde_json::json!({
                    "matcher": "",
                    "hooks": [{"type": "command", "command": command}]
                }));
            }
        }
    }

    let updated = serde_json::to_string_pretty(&value)?;
    std::fs::write(settings_path, updated + "\n").map_err(AgentreeError::Io)?;
    Ok(())
}

/// Remove agentree-owned hook groups from `.claude/settings.json`.
///
/// Only groups whose `command` contains [`AGENTREE_HOOK_MARKER`] are removed.
/// User-defined hooks are never touched.
///
/// Non-fatal: all errors are silently ignored.
/// Cleans up empty arrays, an empty `"hooks"` object, and the entire file if it
/// would become `{}` after removal (matching the behaviour of
/// `remove_agentree_allowed_tools`).
fn remove_agentree_hooks(settings_path: &Path) {
    if !settings_path.exists() {
        return;
    }
    let raw = match std::fs::read_to_string(settings_path) {
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

    if let Some(hooks_val) = obj.get_mut("hooks") {
        if let Some(hooks_obj) = hooks_val.as_object_mut() {
            for event in &[
                "PreToolUse",
                "PostToolUse",
                "PostToolUseFailure",
                "UserPromptSubmit",
                "Stop",
            ] {
                if let Some(groups) = hooks_obj.get_mut(*event).and_then(|v| v.as_array_mut()) {
                    groups.retain(|g| !group_has_agentree_marker(g));
                }
            }
            // Remove event keys whose arrays are now empty
            hooks_obj.retain(|_, v| v.as_array().map(|a| !a.is_empty()).unwrap_or(true));
        }
        // Remove "hooks" key if the object is now empty
        if hooks_val.as_object().map(|o| o.is_empty()).unwrap_or(false) {
            obj.remove("hooks");
        }
    }

    if obj.is_empty() {
        let _ = std::fs::remove_file(settings_path);
    } else if let Ok(updated) = serde_json::to_string_pretty(&value) {
        let _ = std::fs::write(settings_path, updated + "\n");
    }
}

// ─── allowedTools helpers ────────────────────────────────────────────────────

/// Ensure `.claude/settings.json` contains `allowedTools` entries for `.agentree/**`.
///
/// Both relative (`Write(.agentree/**)`) and absolute path variants are added so
/// that Claude Code grants permission regardless of which form it checks.
///
/// If the file does not exist it is created. If it already exists, any missing
/// entries are appended to the `allowedTools` array so pre-existing project
/// settings are preserved.
fn ensure_agentree_allowed_tools(settings_path: &Path, worktree_path: &Path) -> Result<()> {
    let abs = worktree_path
        .canonicalize()
        .unwrap_or_else(|_| worktree_path.to_path_buf());
    let abs_agentree = abs.join(".agentree").to_string_lossy().into_owned();

    let required: Vec<String> = vec![
        "Write(.agentree/**)".into(),
        "Edit(.agentree/**)".into(),
        format!("Write({}/**)", abs_agentree),
        format!("Edit({}/**)", abs_agentree),
    ];

    // Parse existing file or start from an empty object
    let mut value: serde_json::Value = if settings_path.exists() {
        let raw = std::fs::read_to_string(settings_path).map_err(AgentreeError::Io)?;
        serde_json::from_str(&raw).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let allowed = value
        .as_object_mut()
        .ok_or_else(|| AgentreeError::ConfigError("settings.json root is not an object".into()))?
        .entry("allowedTools")
        .or_insert(serde_json::json!([]))
        .as_array_mut()
        .ok_or_else(|| AgentreeError::ConfigError("allowedTools is not an array".into()))?;

    for entry in &required {
        if !allowed
            .iter()
            .any(|v: &serde_json::Value| v.as_str() == Some(entry.as_str()))
        {
            allowed.push(serde_json::json!(entry));
        }
    }

    let updated = serde_json::to_string_pretty(&value)?;
    std::fs::write(settings_path, updated + "\n").map_err(AgentreeError::Io)?;
    Ok(())
}

/// Remove agentree's `allowedTools` entries from `.claude/settings.json`.
///
/// Non-fatal: all errors are silently ignored.
/// Deletes the file if it would become `{}` after removal.
fn remove_agentree_allowed_tools(settings_path: &Path, worktree_path: &Path) {
    if !settings_path.exists() {
        return;
    }
    let abs = worktree_path
        .canonicalize()
        .unwrap_or_else(|_| worktree_path.to_path_buf());
    let abs_agentree = abs.join(".agentree").to_string_lossy().into_owned();

    let to_remove: Vec<String> = vec![
        "Write(.agentree/**)".into(),
        "Edit(.agentree/**)".into(),
        format!("Write({}/**)", abs_agentree),
        format!("Edit({}/**)", abs_agentree),
    ];

    let raw = match std::fs::read_to_string(settings_path) {
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

    if let Some(arr) = obj.get_mut("allowedTools").and_then(|v| v.as_array_mut()) {
        arr.retain(|v| v.as_str().is_none_or(|s| !to_remove.iter().any(|r| r == s)));
        if arr.is_empty() {
            obj.remove("allowedTools");
        }
    }

    if obj.is_empty() {
        let _ = std::fs::remove_file(settings_path);
    } else if let Ok(updated) = serde_json::to_string_pretty(&value) {
        let _ = std::fs::write(settings_path, updated + "\n");
    }
}

#[cfg(test)]
mod tests {
    use super::super::{AGENTREE_END, AGENTREE_START};
    use super::*;

    // ─── Existing tests (migrated from Phase 8) ───────────────────────────────

    #[test]
    fn test_setup_creates_files_in_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = ClaudeAgent::new();

        let token = agent.prepare(path).unwrap();

        // CLAUDE.md created with markers and template content
        let claude_md = std::fs::read_to_string(path.join("CLAUDE.md")).unwrap();
        assert!(claude_md.contains(AGENTREE_START));
        assert!(claude_md.contains(AGENTREE_END));
        assert!(claude_md.contains("Agentree Status Protocol"));

        // settings.json created with allowedTools entries
        let settings_raw =
            std::fs::read_to_string(path.join(".claude").join("settings.json")).unwrap();
        assert!(settings_raw.contains("Write(.agentree/**)"));
        assert!(settings_raw.contains("Edit(.agentree/**)"));

        // .claude/ was freshly created
        assert!(token.claude_dir_created);
    }

    #[test]
    fn test_cleanup_removes_files_when_no_other_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = ClaudeAgent::new();

        let token = agent.prepare(path).unwrap();
        agent.cleanup(path, &token);

        // CLAUDE.md gone (only contained the agentree block)
        assert!(!path.join("CLAUDE.md").exists());
        // .claude/ gone (we created it and it is now empty)
        assert!(!path.join(".claude").exists());
    }

    #[test]
    fn test_cleanup_preserves_existing_claude_md_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = ClaudeAgent::new();

        // Pre-existing project CLAUDE.md
        let existing = "# My Project\n\nSome project documentation.\n";
        std::fs::write(path.join("CLAUDE.md"), existing).unwrap();

        let token = agent.prepare(path).unwrap();

        // After setup both existing content and agentree block are present
        let content = std::fs::read_to_string(path.join("CLAUDE.md")).unwrap();
        assert!(content.contains("# My Project"));
        assert!(content.contains(AGENTREE_START));

        agent.cleanup(path, &token);

        // After cleanup only the original content remains
        let content = std::fs::read_to_string(path.join("CLAUDE.md")).unwrap();
        assert!(content.contains("# My Project"));
        assert!(!content.contains(AGENTREE_START));
        assert!(!content.contains(AGENTREE_END));
    }

    #[test]
    fn test_cleanup_preserves_extra_settings_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = ClaudeAgent::new();

        // Pre-create .claude/ with settings.json containing an unrelated key
        let claude_dir = path.join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{"someOtherKey": "value"}"#,
        )
        .unwrap();

        let token = agent.prepare(path).unwrap();
        // .claude/ was not created by us
        assert!(!token.claude_dir_created);

        agent.cleanup(path, &token);

        // settings.json still exists with the other key, but allowedTools removed
        let raw = std::fs::read_to_string(claude_dir.join("settings.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["someOtherKey"], "value");
        assert!(value.get("allowedTools").is_none());
    }

    #[test]
    fn test_setup_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = ClaudeAgent::new();

        agent.prepare(path).unwrap();
        agent.prepare(path).unwrap(); // second call must not duplicate

        // Marker appears exactly once
        let content = std::fs::read_to_string(path.join("CLAUDE.md")).unwrap();
        assert_eq!(content.matches(AGENTREE_START).count(), 1);

        // Each allowedTools entry appears exactly once
        let raw = std::fs::read_to_string(path.join(".claude").join("settings.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let tools = value["allowedTools"].as_array().unwrap();
        let write_count = tools
            .iter()
            .filter(|v| v.as_str() == Some("Write(.agentree/**)"))
            .count();
        assert_eq!(write_count, 1);
    }

    // ─── New Phase 9 tests: hook injection ───────────────────────────────────

    #[test]
    fn test_prepare_injects_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = ClaudeAgent::new();

        agent.prepare(path).unwrap();

        let raw = std::fs::read_to_string(path.join(".claude").join("settings.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();

        // "hooks" key must be a JSON object
        let hooks = value.get("hooks").expect("hooks key missing");
        assert!(hooks.is_object(), "hooks should be an object");

        // Each of the five events must have at least one group with the marker
        for event in &[
            "PreToolUse",
            "PostToolUse",
            "PostToolUseFailure",
            "UserPromptSubmit",
            "Stop",
        ] {
            let groups = hooks[*event]
                .as_array()
                .unwrap_or_else(|| panic!("{event} array missing"));
            let has_agentree = groups.iter().any(group_has_agentree_marker);
            assert!(has_agentree, "no agentree hook found for {event}");
        }

        // Stop hook must update status.json with phase done
        let stop_groups = hooks["Stop"].as_array().unwrap();
        let stop_cmd = stop_groups
            .iter()
            .flat_map(|g| g.get("hooks").and_then(|h| h.as_array()))
            .flatten()
            .find_map(|h| h.get("command").and_then(|c| c.as_str()))
            .expect("Stop hook command not found");
        assert!(
            stop_cmd.contains("status.json"),
            "Stop hook command should write status.json"
        );
        assert!(
            !stop_cmd.contains("attention.md"),
            "Stop hook command should not write attention.md"
        );
    }

    #[test]
    fn test_prepare_hooks_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = ClaudeAgent::new();

        agent.prepare(path).unwrap();
        agent.prepare(path).unwrap(); // second call must not duplicate

        let raw = std::fs::read_to_string(path.join(".claude").join("settings.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let hooks = value.get("hooks").expect("hooks key missing");

        for event in &[
            "PreToolUse",
            "PostToolUse",
            "PostToolUseFailure",
            "UserPromptSubmit",
            "Stop",
        ] {
            let groups = hooks[*event].as_array().unwrap();
            let agentree_count = groups
                .iter()
                .filter(|g| group_has_agentree_marker(g))
                .count();
            assert_eq!(
                agentree_count, 1,
                "expected exactly 1 agentree hook group for {event}, found {agentree_count}"
            );
        }
    }

    #[test]
    fn test_cleanup_removes_hooks_preserves_user_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = ClaudeAgent::new();

        // Pre-create .claude/ with a user-defined PreToolUse hook
        let claude_dir = path.join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let user_settings = r#"{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "hooks": [{ "type": "command", "command": "user-script.sh" }] }
    ]
  }
}"#;
        std::fs::write(claude_dir.join("settings.json"), user_settings).unwrap();

        let token = agent.prepare(path).unwrap();
        agent.cleanup(path, &token);

        // settings.json must still exist (user hook preserved)
        let raw = std::fs::read_to_string(claude_dir.join("settings.json"))
            .expect("settings.json should still exist after cleanup");
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();

        // User group must still be present
        let pre_groups = value["hooks"]["PreToolUse"].as_array().unwrap();
        let user_present = pre_groups.iter().any(|g| {
            g.get("hooks")
                .and_then(|h| h.as_array())
                .map(|arr| arr.iter().any(|h| h["command"] == "user-script.sh"))
                .unwrap_or(false)
        });
        assert!(user_present, "user hook should be preserved after cleanup");

        // No agentree hook should remain in any event
        for event in &[
            "PreToolUse",
            "PostToolUse",
            "PostToolUseFailure",
            "UserPromptSubmit",
            "Stop",
        ] {
            if let Some(groups) = value["hooks"].get(*event).and_then(|v| v.as_array()) {
                let agentree_present = groups.iter().any(group_has_agentree_marker);
                assert!(
                    !agentree_present,
                    "agentree hook should be removed from {event} after cleanup"
                );
            }
        }
    }

    #[test]
    fn test_cleanup_removes_hooks_key_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = ClaudeAgent::new();

        let token = agent.prepare(path).unwrap();
        agent.cleanup(path, &token);

        // settings.json may not exist at all (everything was created by us and removed)
        // OR if it still exists, it must not contain a "hooks" key
        let settings_path = path.join(".claude").join("settings.json");
        if settings_path.exists() {
            let raw = std::fs::read_to_string(&settings_path).unwrap();
            let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert!(
                value.get("hooks").is_none(),
                "hooks key should be absent after cleanup of an empty settings.json"
            );
        }
        // file not existing is also acceptable (we created .claude/ and settings.json)
    }

    #[test]
    fn test_prepare_clears_stale_attention() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = ClaudeAgent::new();

        // Create a stale attention.md from a prior session
        let agentree_dir = path.join(".agentree");
        std::fs::create_dir_all(&agentree_dir).unwrap();
        std::fs::write(agentree_dir.join("attention.md"), "stale content").unwrap();

        agent.prepare(path).unwrap();

        assert!(
            !agentree_dir.join("attention.md").exists(),
            "prepare() should remove stale attention.md from prior session"
        );
    }

    #[test]
    fn test_hook_commands_use_absolute_paths() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let agent = ClaudeAgent::new();

        agent.prepare(path).unwrap();

        let raw = std::fs::read_to_string(path.join(".claude").join("settings.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();

        // Get the PreToolUse command
        let pre_groups = value["hooks"]["PreToolUse"].as_array().unwrap();
        let pre_cmd = pre_groups
            .iter()
            .find(|g| group_has_agentree_marker(g))
            .and_then(|g| g.get("hooks").and_then(|h| h.as_array()))
            .and_then(|arr| arr.first())
            .and_then(|h| h.get("command").and_then(|c| c.as_str()))
            .expect("PreToolUse agentree command not found");

        // The command must contain the absolute path of the temp dir
        let abs_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let abs_str = abs_path.to_string_lossy();
        assert!(
            pre_cmd.contains(abs_str.as_ref()),
            "PreToolUse hook command should contain absolute path '{}', got: {}",
            abs_str,
            pre_cmd
        );

        // Must not use bare ".agentree" relative path
        assert!(
            !pre_cmd.contains(" .agentree/") && !pre_cmd.contains("\"./agentree"),
            "PreToolUse hook command should not use relative .agentree path"
        );
    }
}
