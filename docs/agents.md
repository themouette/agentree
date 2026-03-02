# Agent Abstraction

Agentree uses an `Agent` trait to encapsulate per-agent setup and teardown hooks. This mirrors the `Backend` trait design.

## How it works

Before starting an agent process, agentree calls `Agent::prepare(workspace_path)`, which returns a token. After the agent exits, `Agent::cleanup(workspace_path, &token)` is called using the same token to undo any changes made during prepare.

## Built-in agents

### claude

Injects an `<!-- agentree:start -->` block into `CLAUDE.md` explaining the status protocol and attention convention. Also merges `allowedTools` entries for `.agentree/**` into `.claude/settings.json`. Both changes are reverted on cleanup.

### generic (default for unknown agent names)

No-op: prepare does nothing and returns immediately, cleanup does nothing. Used automatically for agents not explicitly implemented (opencode, custom tools, etc.).

## Configuration

Agents are configured in `.agentree.toml`:

```toml
[agent]
default = "claude"

[agent.claude]
bin = "claude"
default_args = []

[agent.opencode]
bin = "opencode"
default_args = ["--quiet"]
```

## Adding a new agent

1. Create `src/agent/my_agent.rs` implementing the `Agent` trait:
   ```rust
   pub struct MyAgent;
   impl Agent for MyAgent {
       type PrepareToken = ();
       fn prepare(&self, workspace_path: &Path) -> Result<()> { Ok(()) }
       fn cleanup(&self, _workspace_path: &Path, _token: &()) {}
       fn name(&self) -> &str { "my-agent" }
   }
   ```
2. Add the variant to `AgentType` enum in `src/agent/mod.rs`
3. Add the variant to `AgentToken` enum
4. Add a match arm in `AgentType::from_name`
5. Delegate in the `Agent for AgentType` impl
