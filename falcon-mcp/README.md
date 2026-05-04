# falcon-mcp

Sandboxed MCP server exposing dev tools (fs, cargo, git, prompt-safety lint) for the falcon-detective coding agent.

## Quick start

```bash
# stdio (for Barnum subprocess use)
cd falcon-mcp && cargo run -- --stdio --root ./worktree

# HTTP (for Cloud Run / Gemini CLI)
cd falcon-mcp && cargo run -- --http 8080 --root /workspace
```

See `docs/superpowers/specs/2026-04-30-maltese-agent-design.md` §6 for the design.
