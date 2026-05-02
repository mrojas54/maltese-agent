# POC Findings — Falcon-MCP Tasks 1–3

**Date:** 2026-05-01
**Scope:** Proof-of-concept run of the first three tasks of [`2026-05-01-falcon-mcp-implementation.md`](plans/2026-05-01-falcon-mcp-implementation.md) using the Superpowers subagent-driven-development workflow.
**Branch:** `feature/falcon-mcp` (in `.worktrees/falcon-mcp/`).
**Status:** All three tasks shipped, 9 tests passing, build clean. Loop discipline validated. Plan deltas captured below.

## What was executed

| Commit | Task | What |
|---|---|---|
| `c17c170` | 1 | Crate scaffold: Cargo.toml, main.rs CLI, lib.rs stub, README |
| `e930321` | 2 | Sandbox module (initial): path-jail + binary allowlist + 4 tests |
| `626a46c` | 2 | Sandbox follow-up: replaced `unwrap()` panic with proper error, added 3 security tests (symlink-escape, non-existent-path success/rejection) |
| `107565b` | 3 | rmcp ServerHandler scaffold + first tool (`fs_read`) end-to-end via stdio |
| `61ab340` | 3 | Task 3 follow-up: removed redundant `bytes.clone()`; replaced `e.to_string()` with `format!("{e:#}")` so anyhow chains reach callers |

**Tests:** 9 passing (4 sandbox + 3 sandbox security + 2 fs_read integration). `cargo build` and `cargo clippy --all-targets -- -D warnings` clean. Live `tools/list` over stdio confirms `fs_read` is registered with `inputSchema` and `outputSchema`.

## Subagent-driven-development loop validation

| Stage | Dispatches | What was caught |
|---|---|---|
| Implementer | 4 (3 initial + 1 fix) | Surfaced rmcp 0.1 → 1.6 version drift on first run |
| Spec reviewer | 3 | Verified scope: no extra work, no missing requirements |
| Quality reviewer | 4 (3 initial + 1 re-review) | Caught 4 real issues that implementer + spec reviewer missed |

**Issues the quality reviewer caught (would have shipped without it):**

1. **`unwrap()` on `joined.file_name()`** in `sandbox.rs::resolve` — would panic the server on `resolve("foo/..")` (a likely model output). DoS class.
2. **Missing symlink-escape test** — the central security property of the sandbox had no direct assertion.
3. **Redundant `bytes.clone()`** in `fs_read` — doubled peak memory per call. Would have replicated to all 5 fs tools.
4. **`e.to_string()` loses anyhow chain** — every error reaching the agent would have been a one-line summary (`"resolving path"`) instead of the full chain (`"resolving path: <details>: escapes sandbox root"`). Would have replicated to ~15 tools.

The two-stage review (spec compliance, then code quality) earned its keep on every task.

## Token usage

| Task | Total tokens (incl. retries) | Notes |
|---|---|---|
| Task 1 | ~96K | Clean pass, no fix loop |
| Task 2 | ~155K | One quality-review fix loop |
| Task 3 | ~290K (cumulative) — net ~140K for Task 3 alone | Fix applied inline (manual) rather than re-dispatch |
| **Average per task** | **~130K** | Range: 96K (clean) → 155K (with fix loop) |

Projection: 18 tasks for falcon-mcp × ~130K = **~2.3M tokens for full Plan 1 execution**. Plans 2 + 3 are smaller; full implementation across all three plans likely **3–5M tokens**.

## Plan-vs-reality deltas (apply before resuming)

The plans were drafted assuming `rmcp = "0.1"`. Reality is `rmcp 1.6.x`. The API differences are systematic — once mapped, they generalize across all 14 tool tasks in Plan 1.

### Cargo.toml deltas

```diff
- rmcp = { version = "0.1", features = ["server", "transport-io", "transport-streamable-http-server", "macros"] }
+ rmcp = { version = "1", features = ["server", "transport-io", "transport-streamable-http-server", "macros", "client", "transport-child-process"] }
- # schemars = "0.8"  (plan reference)
+ schemars = "1"
```

The `client` and `transport-child-process` features are required by the integration tests (which spawn the binary as a child and drive it via the MCP client transport). `schemars` must match rmcp's transitive version (1.x); mixing 0.8 and 1.x produces orphan-impl errors on `JsonSchema`.

### Tool registration pattern (replaces plan's Task 3+ code samples)

Canonical reference: [`modelcontextprotocol/rust-sdk/examples/servers/src/structured_output.rs`](https://github.com/modelcontextprotocol/rust-sdk/blob/main/examples/servers/src/structured_output.rs).

```rust
use rmcp::{
    Json, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router,
};

#[derive(Clone)]
pub struct FalconMcp {
    sandbox: Arc<Sandbox>,
    tool_router: ToolRouter<Self>,
}

#[tool_router(router = tool_router)]
impl FalconMcp {
    pub fn new(sandbox: Sandbox) -> Self {
        Self {
            sandbox: Arc::new(sandbox),
            tool_router: Self::tool_router(),
        }
    }

    /// Doc comments flow into the MCP `description` field.
    #[tool(name = "fs_read", description = "Read a file from within the sandbox root.")]
    pub async fn fs_read(
        &self,
        params: Parameters<fs_basic::FsReadArgs>,
    ) -> Result<Json<fs_basic::FsReadResult>, String> {
        fs_basic::fs_read(self.sandbox.clone(), params.0).await
            .map(Json)
            .map_err(|e| format!("{e:#}"))   // ← preserves anyhow chain
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for FalconMcp {}     // ← empty body; macro does the work
```

**Key rules for every later tool:**

- Input arg type: `#[derive(Debug, Deserialize, JsonSchema)]`
- Output type: `#[derive(Debug, Serialize, JsonSchema)]`
- Method signature: `async fn name(&self, params: Parameters<ArgsType>) -> Result<Json<ResultType>, String>`
- Error mapping: **`map_err(|e| format!("{e:#}"))`** — never `.to_string()`
- Doc comments on the method become the MCP tool description; doc comments on input fields become field descriptions in the input schema.

### Test pattern (replaces plan's `tests/*_test.rs` boilerplate)

```rust
use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,            // ← PLURAL (singular form is deprecated alias)
    transport::{ConfigureCommandExt, TokioChildProcess},
};

async fn spawn_server(root: &std::path::Path)
    -> rmcp::service::RunningService<rmcp::RoleClient, ()>
{
    let cmd = Command::new(env!("CARGO_BIN_EXE_falcon-mcp"))
        .configure(|c| { c.arg("--stdio").arg("--root").arg(root).kill_on_drop(true); });
    ()
        .serve(TokioChildProcess::new(cmd).unwrap())
        .await
        .expect("connect to falcon-mcp")
}

#[tokio::test]
async fn fs_read_returns_file_content() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "the falcon").unwrap();
    let client = spawn_server(dir.path()).await;
    let result = client.call_tool(CallToolRequestParams {
        name: "fs_read".into(),
        arguments: Some(json!({"path": "hello.txt"}).as_object().unwrap().clone()),
    }).await.expect("call fs_read");
    let structured = result.structured_content.expect("structured result");
    assert_eq!(structured["content"], "the falcon");
    assert_eq!(structured["bytes"], 10);
    client.cancel().await.unwrap();
}
```

Use `kill_on_drop(true)` to prevent zombie children on test panic. Use `client.cancel().await.unwrap()` to cleanly close. The escape-rejection test should accept either `Result::Err` OR `Ok(r) with r.is_error == true` — both are valid rmcp error surfaces.

### Stdio hygiene (Task 3 onwards)

Anything that writes to stdout in the binary path corrupts MCP frames. Specifically:

- **Remove** any `println!` from `main.rs` (Task 1 left a placeholder; Task 3 removed it).
- **Tracing must write to stderr**: `tracing_subscriber::fmt().with_writer(std::io::stderr).init()`.
- The integration test pattern above already pipes stdout/stderr correctly via `TokioChildProcess`.

## Plan 2 (falcon-agent) and Plan 3 (falcon-detective)

**Plan 2** uses axum + reqwest + serde — no rmcp. The Gemini REST call format and `@google/genai` SDK shape may have drifted since the plan was drafted; verify against current Google docs at execution time. The deterministic-fake-LLM pattern is independent of any external API.

**Plan 3** uses `@modelcontextprotocol/sdk` (TypeScript MCP client) and `@google/genai` (TS SDK). Both may have version drift similar to rmcp's. Verify against current `@modelcontextprotocol/sdk` README before dispatching Task 4 (MCP client wrapper) of Plan 3.

`@barnum/barnum` is also a moving target — see spec §9 question 2. Robert may have changed combinator names or signatures since the design was drafted. Verify before dispatching Plan 3 Tasks 6+ (the handlers).

## Open questions still standing (from spec §9)

None of these were settled during the POC:

1. **Bill — MCP SDK choice.** We used `rmcp 1.6` because that's what's published. If the "Google Rust MCP Server" Bill is supporting is a separate codebase, this entire plan retargets.
2. **Robert — Barnum API stability.** Plan 3 references `@barnum/barnum` combinators (`pipe`, `forEach`, `loop`, `branch`, `tryCatch`, `withTimeout`). Need confirmation these are stable.
3. **Workshop time budget / audience prerequisites / environment** — pacing, not architecture.
4. **Rate-limiting at workshop time** — affects cassette mode default.
5. **Repo visibility / @malt3se_lover handling** — affects how the planted commit is authored.

## Recommendations for resuming

**Before dispatching Plan 1 Task 4:**
1. Apply the Cargo.toml deltas above (already done in `feature/falcon-mcp`).
2. Reference this doc + `examples/servers/src/structured_output.rs` in every Task 4–14 implementer brief.
3. Use the `format!("{e:#}")` error mapping in every new tool.
4. Use `CallToolRequestParams` (plural) in every new test.

**Before dispatching Plan 3 Task 4 (MCP client wrapper):**
1. Verify `@modelcontextprotocol/sdk` API for `Client`, `StdioClientTransport`, `callTool`. The plan's snippet may need adaptation similar to the rmcp drift.

**Cost reality check:** at ~130K tokens/task, full execution is **3–5M tokens**. Consider whether to run this in one campaign or spread across sessions.

## Files & artifacts

- POC commits: `feature/falcon-mcp` (5 commits, `51306bd..61ab340`)
- This doc: `docs/superpowers/poc-findings-2026-05-01.md`
- Plans (with header pointers added): `docs/superpowers/plans/2026-05-01-falcon-{mcp,agent,detective}-implementation.md`
- Spec: `docs/superpowers/specs/2026-04-30-maltese-circus-design.md`
