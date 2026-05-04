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

## Connecting from Gemini CLI

Copy `.gemini/settings.example.json` to `~/.gemini/settings.json` (or the project-local equivalent) and update the absolute path to the falcon-mcp binary plus the sandbox root. The Gemini CLI will see falcon-mcp's tools alongside any other configured MCP servers.

If you ever deploy falcon-mcp to a remote host (Cloud Run or similar), swap the command/args block for `httpUrl`:

```json
{
  "mcpServers": {
    "falcon-mcp": {
      "httpUrl": "https://your-host.example.com/mcp",
      "trust": false
    }
  }
}
```
