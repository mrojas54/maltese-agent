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

## Tools

19 tools across 5 categories. All paths resolve through the `--root` sandbox jail; symlinks that escape are rejected. `--read-only` disables every write tool (fs writes, patches, git mutations).

### Filesystem

| Tool | Description |
| --- | --- |
| `fs_read` | Read a UTF-8 file from inside the sandbox. |
| `fs_write` | Write a file inside the sandbox; creates parent dirs. Rejected in `--read-only`. |
| `fs_apply_patch` | Apply a unified diff to a file. Rejected in `--read-only`. |
| `fs_list` | List directory entries with an optional glob filter on entry names. |
| `fs_search` | ripgrep regex search; returns up to `max` matches with file/line/column. |
| `fs_search_ast` | ast-grep structural search; defaults to `lang=rust`. |

### Cargo

All cargo tools use `--message-format=json` and run against a `crate_path` inside the sandbox.

| Tool | Description |
| --- | --- |
| `cargo_check` | Returns parsed errors and warnings. |
| `cargo_test` | Returns passed/failed test names plus total wall-clock duration. |
| `cargo_clippy` | Returns `clippy::*` lints; `fix=true` applies machine-applicable fixes. |
| `cargo_fmt` | `check=true` reports diffs without changes; otherwise applies formatting in place. |

### Git

Scoped to the workshop repo. Commits are authored as `falcon-detective@local`.

| Tool | Description |
| --- | --- |
| `git_worktree_add` | Create worktree under `<root>/.runs/<name>` based on `base` (default `HEAD`). |
| `git_worktree_remove` | Remove a worktree at the given path with `--force`. |
| `git_add` | Stage one or more sandbox-relative paths. |
| `git_commit` | Commit the staged index; returns the new 40-char SHA. |
| `git_diff` | Unified diff for an optional `rev` (default: unstaged worktree diff). |
| `git_log` | Recent commits as parsed records (sha, author, ISO date, subject); optional path filter. |
| `git_blame` | Blame a single 1-based line; returns sha, author, author-time, and line text. |

### Prompt safety

| Tool | Description |
| --- | --- |
| `prompt_lint` | Scan a prompt template for hidden directives, role overrides, trigger conditionals, and anomalous few-shot examples. Pure function — no sandbox or filesystem access. |

### Exec

| Tool | Description |
| --- | --- |
| `exec_run` | Run an allowlisted external binary. Disabled by default; requires `--enable-exec`. Allowlist: `cargo`, `rustc`, `rustfmt`, `ripgrep`/`rg`, `git`, `ast-grep`/`sg`. |

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
