use crate::error::{AgentreeError, Result};
use std::path::Path;

use super::Agent;

const AGENTREE_START: &str = "<!-- agentree:start -->";
const AGENTREE_END: &str = "<!-- agentree:end -->";

/// Token returned by `ClaudeAgent::prepare`.
///
/// Tracks whether `.claude/` was created by `prepare` so that
/// `cleanup` can remove it if it is now empty.
pub struct ClaudeToken {
    pub(crate) claude_dir_created: bool,
}

/// Agent implementation for Claude.
///
/// `prepare` injects an `<!-- agentree:start -->` block into `CLAUDE.md`
/// and merges `allowedTools` entries into `.claude/settings.json`.
/// `cleanup` reverts those changes.
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
        inject_agentree_block(&workspace_path.join("CLAUDE.md"))?;

        // Merge our allowedTools entries into settings.json
        let settings_path = claude_dir.join("settings.json");
        ensure_agentree_allowed_tools(&settings_path, workspace_path)?;

        Ok(ClaudeToken { claude_dir_created })
    }

    fn cleanup(&self, workspace_path: &Path, token: &Self::PrepareToken) {
        // 1. Remove agentree block from CLAUDE.md (delete file if now empty)
        remove_agentree_block(&workspace_path.join("CLAUDE.md"));

        let claude_dir = workspace_path.join(".claude");

        // 2. Remove our entries from settings.json (delete file if now empty)
        remove_agentree_allowed_tools(&claude_dir.join("settings.json"), workspace_path);

        // 3. Remove .claude/ if we created it and it is now empty
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

/// Inject the agentree CLAUDE.md block into `path`.
///
/// The content is wrapped in XML markers so it can be cleanly extracted later:
/// ```text
/// <!-- agentree:start -->
/// <template content>
/// <!-- agentree:end -->
/// ```
///
/// Idempotent: if `<!-- agentree:start -->` is already present, does nothing.
/// Appends to an existing file; creates the file if it does not exist.
fn inject_agentree_block(path: &Path) -> Result<()> {
    let template = include_str!("../../templates/CLAUDE.md");
    let block = format!("{}\n{}{}\n", AGENTREE_START, template, AGENTREE_END);

    if path.exists() {
        let content = std::fs::read_to_string(path).map_err(AgentreeError::Io)?;
        if content.contains(AGENTREE_START) {
            return Ok(()); // already injected
        }
        let separator = if content.ends_with('\n') {
            "\n"
        } else {
            "\n\n"
        };
        std::fs::write(path, format!("{}{}{}", content, separator, block))
            .map_err(AgentreeError::Io)?;
    } else {
        std::fs::write(path, &block).map_err(AgentreeError::Io)?;
    }
    Ok(())
}

/// Remove the `<!-- agentree:start -->…<!-- agentree:end -->` block from `path`.
///
/// Non-fatal: all errors are silently ignored.
/// Deletes the file if it becomes empty (only whitespace) after removal.
fn remove_agentree_block(path: &Path) {
    if !path.exists() {
        return;
    }
    let content = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return,
    };

    let start_pos = match content.find(AGENTREE_START) {
        Some(p) => p,
        None => return,
    };
    let end_pos = match content.find(AGENTREE_END) {
        Some(p) => p,
        None => return,
    };

    // Byte offset just past `<!-- agentree:end -->`, skipping one trailing newline
    let end_byte = end_pos + AGENTREE_END.len();
    let end_byte = if content.as_bytes().get(end_byte) == Some(&b'\n') {
        end_byte + 1
    } else {
        end_byte
    };

    let before = &content[..start_pos];
    let after = &content[end_byte..];

    let remaining = match (before.trim().is_empty(), after.trim().is_empty()) {
        (true, true) => String::new(),
        (true, false) => after.trim_start_matches('\n').to_string(),
        (false, true) => format!("{}\n", before.trim_end_matches('\n')),
        (false, false) => format!(
            "{}\n{}",
            before.trim_end_matches('\n'),
            after.trim_start_matches('\n')
        ),
    };

    if remaining.trim().is_empty() {
        let _ = std::fs::remove_file(path);
    } else {
        let final_content = if remaining.ends_with('\n') {
            remaining
        } else {
            remaining + "\n"
        };
        let _ = std::fs::write(path, final_content);
    }
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
    use super::*;

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
}
