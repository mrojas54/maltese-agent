import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

export interface McpOptions {
  binary: string;          // path to falcon-mcp binary
  root: string;            // sandbox root (the worktree path)
  readOnly?: boolean;
  enableExec?: boolean;
  /** Extra env vars to pass through StdioClientTransport's curated allowlist.
   *  Useful for CARGO_TARGET_DIR (share build cache across worktrees) or
   *  RUST_LOG (debug falcon-mcp internals). */
  env?: Record<string, string>;
}

export class FalconMcpClient {
  private client: Client | null = null;
  private transport: StdioClientTransport | null = null;

  constructor(private opts: McpOptions) {}

  async connect(): Promise<void> {
    const args = ["--stdio", "--root", this.opts.root];
    if (this.opts.readOnly) args.push("--read-only");
    if (this.opts.enableExec) args.push("--enable-exec");

    // Implicit pass-through: if CARGO_TARGET_DIR is set in our env, forward it
    // to falcon-mcp. This lets cargo invocations in fresh worktrees reuse
    // build artifacts from the parent worktree's target/, dropping triage
    // and verify wall-clock from minutes to seconds.
    const env = { ...this.opts.env };
    if (process.env.CARGO_TARGET_DIR && env.CARGO_TARGET_DIR === undefined) {
      env.CARGO_TARGET_DIR = process.env.CARGO_TARGET_DIR;
    }

    this.transport = new StdioClientTransport({
      command: this.opts.binary,
      args,
      // StdioClientTransport's default env is a curated allowlist (PATH, HOME,
      // etc.) — NOT the full process.env. Merge the allowlist with any extra
      // vars the caller wants to pass through (CARGO_TARGET_DIR, RUST_LOG,
      // custom config paths, etc.).
      ...(Object.keys(env).length > 0 && {
        env: {
          ...Object.fromEntries(
            ["PATH", "HOME", "USER", "SHELL", "TMPDIR", "LANG", "LC_ALL"]
              .map((k) => [k, process.env[k] ?? ""])
              .filter(([, v]) => v !== ""),
          ),
          ...env,
        },
      }),
    });
    this.client = new Client({ name: "falcon-detective", version: "0.1.0" }, { capabilities: {} });
    await this.client.connect(this.transport);
  }

  async call<T = unknown>(toolName: string, args: Record<string, unknown>): Promise<T> {
    if (!this.client) throw new Error("not connected; call connect() first");
    // The MCP SDK's default per-request timeout is 60s, which trips on cold
    // cargo compiles in fresh worktrees. cargo_check/test/clippy each can
    // exceed 5 minutes when compiling tokio + axum + hyper + reqwest + serde
    // from scratch. 15 minutes leaves real margin without masking hangs.
    const result = await this.client.callTool(
      { name: toolName, arguments: args },
      undefined,
      { timeout: 900_000 },
    );
    if (result.isError) {
      throw new Error(`tool ${toolName} returned error: ${JSON.stringify(result.content)}`);
    }
    return (result.structuredContent ?? result.content) as T;
  }

  async close(): Promise<void> {
    await this.client?.close();
    this.transport = null;
    this.client = null;
  }
}
